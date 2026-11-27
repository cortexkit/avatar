//! Colour on the CPU: the sRGB transfer functions and the dark tone the
//! shading model blends toward.
//!
//! Everything here uses only `+ − × ÷`, `sqrt` and comparisons, so no
//! platform maths library is involved: roots are found by Newton's method.

use avatar_small_robots::Rgb;

/// A colour channel's sRGB value, 0 to 1, as linear light.
pub fn srgb_to_linear(s: f64) -> f64 {
    let s = s.clamp(0.0, 1.0);
    if s <= 0.04045 {
        s / 12.92
    } else {
        // x^2.4 = x² · (x^(1/5))².
        let x = (s + 0.055) / 1.055;
        let r = root(x, 5);
        x * x * r * r
    }
}

/// Linear light, 0 to 1, as a colour channel's sRGB value.
pub fn linear_to_srgb(l: f64) -> f64 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        12.92 * l
    } else {
        // l^(1/2.4) = (l^(1/12))⁵, and l^(1/12) = √√∛l.
        let r = root(l, 3).sqrt().sqrt();
        1.055 * r * r * r * r * r - 0.055
    }
}

/// The `n`th root of `x` in `(0, 1]`, by Newton's method from above, where
/// it falls monotonically to the root.
fn root(x: f64, n: u32) -> f64 {
    let mut y: f64 = 1.0;
    for _ in 0..200 {
        let mut p = 1.0;
        for _ in 0..n - 1 {
            p *= y;
        }
        let next = ((f64::from(n) - 1.0) * y + x / p) / f64::from(n);
        if next >= y {
            break;
        }
        y = next;
    }
    y
}

/// The linear value of each 8-bit sRGB byte.
pub fn decode_table() -> &'static [f64; 256] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[f64; 256]> = OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| srgb_to_linear(i as f64 / 255.0)))
}

/// Linear light as the nearest 8-bit sRGB byte, rounding in sRGB terms as
/// an `Rgba8UnormSrgb` write does.
pub fn encode_byte(l: f64) -> u8 {
    use std::sync::OnceLock;
    // Byte b covers linear values from `edges[b − 1]` up to `edges[b]`:
    // each edge is the linear value of an sRGB half step.
    static EDGES: OnceLock<[f64; 255]> = OnceLock::new();
    let edges =
        EDGES.get_or_init(|| std::array::from_fn(|b| srgb_to_linear((b as f64 + 0.5) / 255.0)));
    edges.partition_point(|e| *e <= l) as u8
}

/// An sRGB colour's channels as linear light.
pub fn linear(c: Rgb) -> [f64; 3] {
    let t = decode_table();
    [
        t[usize::from(c.r)],
        t[usize::from(c.g)],
        t[usize::from(c.b)],
    ]
}

/// The dark tone of a colour, as a per-channel ratio of its linear value,
/// for a finish that moves the shadow side `amount` of the way toward it.
///
/// The dark tone keeps the colour's own hue, with saturation `0.95 s + 10`
/// (at most 85, in percent) and lightness `0.42 l` (8 to 38 percent), so
/// shade reads as the same paint in less light rather than as grey. The
/// blend is cooled slightly by taking 92% of red and green.
pub fn dark_ratio(c: Rgb, amount: f64) -> [f64; 3] {
    let base = linear(c);
    let dark = linear(own_dark(c));
    let cool = [0.92, 0.92, 1.0];
    [0, 1, 2].map(|k| {
        let d = (base[k] + (dark[k] - base[k]) * amount) * cool[k];
        d / base[k].max(1e-4)
    })
}

/// The dark tone of a colour's own hue, rounded to bytes.
fn own_dark(c: Rgb) -> Rgb {
    let [h, s, l] = hsl([c.r, c.g, c.b].map(|v| f64::from(v) / 255.0));
    let rgb = from_hsl(
        h,
        (s * 0.95 + 10.0).clamp(0.0, 85.0),
        (l * 0.42).clamp(8.0, 38.0),
    );
    let [r, g, b] = rgb.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Rgb { r, g, b }
}

/// Hue in degrees and saturation and lightness in percent.
fn hsl([r, g, b]: [f64; 3]) -> [f64; 3] {
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    let (mut h, mut s) = (0.0, 0.0);
    if mx > mn {
        let d = mx - mn;
        s = if l > 0.5 {
            d / (2.0 - mx - mn)
        } else {
            d / (mx + mn)
        };
        h = if mx == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if mx == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h *= 60.0;
    }
    [h, s * 100.0, l * 100.0]
}

/// Channels 0 to 1 from hue in degrees and saturation and lightness in
/// percent.
fn from_hsl(h: f64, s: f64, l: f64) -> [f64; 3] {
    let h = h.rem_euclid(360.0);
    let (s, l) = (s.clamp(0.0, 100.0) / 100.0, l.clamp(0.0, 100.0) / 100.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let t = if hp < 1.0 {
        [c, x, 0.0]
    } else if hp < 2.0 {
        [x, c, 0.0]
    } else if hp < 3.0 {
        [0.0, c, x]
    } else if hp < 4.0 {
        [0.0, x, c]
    } else if hp < 5.0 {
        [x, 0.0, c]
    } else {
        [c, 0.0, x]
    };
    t.map(|v| v + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_functions_match_known_values_and_invert() {
        // Values of the sRGB standard's formula.
        assert!((srgb_to_linear(0.5) - 0.214_041_140_5).abs() < 1e-9);
        assert!((srgb_to_linear(0.04) - 0.04 / 12.92).abs() < 1e-15);
        assert!((linear_to_srgb(0.214_041_140_5) - 0.5).abs() < 1e-9);
        assert_eq!(srgb_to_linear(1.0), 1.0);
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-12);
        for b in 0..=255_u8 {
            assert_eq!(encode_byte(decode_table()[usize::from(b)]), b);
        }
    }

    #[test]
    fn the_dark_tone_keeps_the_hue() {
        // A saturated blue darkens to a darker blue, not to grey.
        let ratio = dark_ratio(
            Rgb {
                r: 0x30,
                g: 0x60,
                b: 0xd0,
            },
            1.0,
        );
        assert!(ratio.iter().all(|r| *r < 1.0 && *r > 0.0));
        let dark = own_dark(Rgb {
            r: 0x30,
            g: 0x60,
            b: 0xd0,
        });
        assert!(dark.b > dark.g && dark.g > dark.r);
    }
}
