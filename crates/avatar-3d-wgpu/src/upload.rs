//! Texture upload: the conversion that makes `Rgba8UnormSrgb` sampling
//! return linear colour premultiplied by alpha, and the mip chain.
//!
//! A robot's texture pixels ([`avatar_3d::robot::Texture`]) are sRGB colour
//! premultiplied by alpha in byte values, as the rasterizer returns them. An sRGB texture decodes each
//! stored byte as sRGB, so storing those bytes as they are would decode
//! `a · s` as if it were a colour and darken every antialiased edge. The
//! conversion recovers the straight sRGB colour `s = c / a`, decodes it to
//! linear light, multiplies by `a` and stores that encoded back to sRGB
//! (0 where `a = 0`). Alpha is stored as it is. Sampling then yields linear
//! premultiplied colour, which blends as `t + under · (1 − t.a)`.

use std::sync::OnceLock;

use crate::colour::{decode_table, encode_byte, srgb_to_linear};

/// The converted byte of a premultiplied channel `c` under alpha `a`.
pub fn convert_channel(c: u8, a: u8) -> u8 {
    static TABLE: OnceLock<Vec<u8>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = vec![0_u8; 256 * 256];
        for a in 1..256_usize {
            for c in 0..256_usize {
                let straight = (c as f64 / a as f64).min(1.0);
                let premultiplied = srgb_to_linear(straight) * (a as f64 / 255.0);
                t[a * 256 + c] = encode_byte(premultiplied);
            }
        }
        t
    });
    table[usize::from(a) * 256 + usize::from(c)]
}

/// Converts RGBA8 pixels of sRGB premultiplied by alpha, as the rasterizer
/// returns them, into the bytes an `Rgba8UnormSrgb` texture stores.
pub fn convert(pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len());
    for p in pixels.as_chunks::<4>().0 {
        let a = p[3];
        out.extend_from_slice(&[
            convert_channel(p[0], a),
            convert_channel(p[1], a),
            convert_channel(p[2], a),
            a,
        ]);
    }
    out
}

/// The mip chain of a converted square texture, level 0 first: each level
/// averages 2 × 2 texels of the one above in linear premultiplied terms.
pub fn mip_chain(level0: Vec<u8>, size: u32) -> Vec<Vec<u8>> {
    let decode = decode_table();
    let mut levels = vec![level0];
    let mut size = size as usize;
    while size > 1 {
        let above = levels.last().expect("level 0 is present");
        let half = size / 2;
        let mut next = Vec::with_capacity(half * half * 4);
        for y in 0..half {
            for x in 0..half {
                let texel = |dx: usize, dy: usize| ((2 * y + dy) * size + 2 * x + dx) * 4;
                let corners = [texel(0, 0), texel(1, 0), texel(0, 1), texel(1, 1)];
                for k in 0..3 {
                    let sum: f64 = corners
                        .iter()
                        .map(|i| decode[usize::from(above[i + k])])
                        .sum();
                    next.push(encode_byte(sum / 4.0));
                }
                let alpha: u32 = corners.iter().map(|i| u32::from(above[i + 3])).sum();
                next.push(((alpha + 2) / 4) as u8);
            }
        }
        levels.push(next);
        size = half;
    }
    levels
}

/// The number of mip levels of a square texture of this edge.
pub fn mip_levels(size: u32) -> u32 {
    u32::BITS - size.leading_zeros()
}
