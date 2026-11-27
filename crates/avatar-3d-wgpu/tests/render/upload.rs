//! A CPU model of how a texture reaches the screen: the upload conversion,
//! the sRGB decode a sampler does, bilinear filtering and the premultiplied
//! blend over the surface under it.

use avatar_3d_wgpu::colour::{decode_table, encode_byte};
use avatar_3d_wgpu::upload::convert;
use avatar_raster::{rasterize_svg, Alpha};

/// What an `Rgba8UnormSrgb` sampler returns at texel coordinate `(x, y)`
/// (texel centres at half-integers): each texel's colour decoded from sRGB
/// to linear and its alpha read as it is, then weighted bilinearly.
fn sample(stored: &[u8], size: usize, x: f64, y: f64) -> [f64; 4] {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let texel = |ix: f64, iy: f64| {
        let ix = (ix.max(0.0) as usize).min(size - 1);
        let iy = (iy.max(0.0) as usize).min(size - 1);
        let p = &stored[(iy * size + ix) * 4..][..4];
        let d = decode_table();
        [
            d[usize::from(p[0])],
            d[usize::from(p[1])],
            d[usize::from(p[2])],
            f64::from(p[3]) / 255.0,
        ]
    };
    let corners = [
        (texel(x0, y0), (1.0 - tx) * (1.0 - ty)),
        (texel(x0 + 1.0, y0), tx * (1.0 - ty)),
        (texel(x0, y0 + 1.0), (1.0 - tx) * ty),
        (texel(x0 + 1.0, y0 + 1.0), tx * ty),
    ];
    [0, 1, 2, 3].map(|k| corners.iter().map(|(c, w)| c[k] * w).sum())
}

/// The texture over a white hull at points around one row, blended
/// premultiplied (`t + white · (1 − t.a)`) and written to an sRGB target:
/// each output channel's byte.
fn over_white(stored: &[u8], size: usize, row: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for x in 0..size * 4 {
        for dy in [0.25, 0.5, 0.75] {
            let t = sample(stored, size, x as f64 / 4.0, row as f64 + dy);
            for c in &t[..3] {
                out.push(encode_byte(c + (1.0 - t[3])));
            }
        }
    }
    out
}

#[test]
fn an_antialiased_white_shape_over_white_stays_white_after_upload() {
    // A white disc whose rim crosses many texels of one row at fractional
    // coverage, as the rasterizer draws face and marks layers.
    let size = 64;
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
<circle cx="32.3" cy="33.7" r="23.6" fill="#ffffff"/></svg>"##;
    let image = rasterize_svg(svg, size as u32, Alpha::Premultiplied).expect("the SVG draws");
    let row = 11;
    let partial = image.pixels[row * size * 4..(row + 1) * size * 4]
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] > 0 && p[3] < 255)
        .count();
    assert!(
        partial >= 6,
        "the row crosses the rim at {partial} partly covered texels"
    );

    let converted = convert(&image.pixels);
    let white = over_white(&converted, size, row);
    let darkest = *white.iter().min().expect("the row has samples");
    assert!(
        darkest >= 254,
        "converted, the darkest output is {darkest}, more than 1/255 from white"
    );

    // Uploaded as the rasterizer returns it, the premultiplied colour is
    // decoded as if it were a colour, and the rim darkens.
    let raw = over_white(&image.pixels, size, row);
    let darkest = *raw.iter().min().expect("the row has samples");
    assert!(
        darkest < 235,
        "unconverted, the rim only darkens to {darkest}"
    );
}

#[test]
fn conversion_keeps_alpha_and_clears_transparent_texels() {
    let pixels = [10, 20, 30, 0, 255, 255, 255, 255, 64, 32, 0, 128];
    let converted = convert(&pixels);
    assert_eq!(converted[..4], [0, 0, 0, 0]);
    assert_eq!(converted[4..8], [255, 255, 255, 255]);
    assert_eq!(converted[11], 128);
    // Premultiplied 64 at alpha 128 is straight sRGB 64 / 128 = 0.5, linear
    // 0.2140; times 128/255 that is 0.1074, which encodes to sRGB 0.3615,
    // byte 92. Likewise 32 becomes byte 44.
    assert_eq!(converted[8..10], [92, 44]);
}
