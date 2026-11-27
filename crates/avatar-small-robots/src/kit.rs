//! Arithmetic, the gene reader and the colours every part of a robot uses.
//!
//! Generation uses only `+ - * /`, `sqrt`, comparisons, `floor` and Rust's
//! `round`, which IEEE 754 makes exact on every platform; sine and cosine
//! come from `avatar_svg::wild::math`, the polynomial the reference copies.

pub(crate) use avatar_svg::wild::math::{cos, sin};
use avatar_svg::Rgb;

pub(crate) const PI: f64 = std::f64::consts::PI;
/// One degree in radians, computed the way the reference computes it.
pub(crate) const DEG: f64 = PI / 180.0;

/// A point in frame units: x to the right, y down.
pub(crate) type P2 = [f64; 2];

/// Clamps without the panic of `f64::clamp` when `lo > hi`: the reference
/// clamps a few ranges that can come out empty, and then the lower bound wins.
pub(crate) fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

pub(crate) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub(crate) fn hyp(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

pub(crate) fn mix(p: P2, q: P2, t: f64) -> P2 {
    [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
}

/// The smaller of two numbers, as JavaScript's `Math.min` gives it for
/// finite input.
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

pub(crate) fn max(a: f64, b: f64) -> f64 {
    if b > a {
        b
    } else {
        a
    }
}

pub(crate) fn min3(a: f64, b: f64, c: f64) -> f64 {
    min(min(a, b), c)
}

/// JavaScript's `Math.round`: the nearest integer, halves toward positive
/// infinity. `x - floor(x)` is exact, so the test on it is too.
pub(crate) fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// Reads genes from the stored genome. Every gene the reference reads goes
/// through one of these, with the same byte, bit offset and width.
#[derive(Clone, Copy)]
pub(crate) struct Reader<'a> {
    bytes: &'a [u8; crate::GENOME_LEN],
}

/// The fixed scramble the reference applies to a choice gene of `n` bits.
const SCRAMBLE: [u32; 7] = [1, 1, 3, 3, 5, 11, 19];

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8; crate::GENOME_LEN]) -> Self {
        Self { bytes }
    }

    /// `n` bits of the byte at `byte`, starting `shift` bits above its lowest bit.
    pub(crate) fn raw(&self, byte: usize, shift: u32, n: u32) -> u32 {
        (u32::from(self.bytes[byte]) >> shift) & ((1 << n) - 1)
    }

    /// The bits as a plain number from 0 to 1.
    pub(crate) fn unit(&self, byte: usize, shift: u32, n: u32) -> f64 {
        f64::from(self.raw(byte, shift, n)) / f64::from((1_u32 << n) - 1)
    }

    pub(crate) fn lin(&self, byte: usize, shift: u32, n: u32, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit(byte, shift, n)
    }

    pub(crate) fn flag(&self, byte: usize, shift: u32) -> bool {
        self.raw(byte, shift, 1) == 1
    }

    /// An index chosen in proportion to whole-number weights that add up to
    /// a power of two, after the reference's one-to-one scramble of the bits.
    pub(crate) fn pick(&self, byte: usize, shift: u32, weights: &[u32], offset: u32) -> usize {
        let total: u32 = weights.iter().sum();
        let n = total.trailing_zeros();
        assert_eq!(
            1 << n,
            total,
            "choice weights must add up to a power of two"
        );
        let v = self.raw(byte, shift, n);
        let mut t = (v * SCRAMBLE[n as usize] + offset) & (total - 1);
        for (i, w) in weights.iter().enumerate() {
            if t < *w {
                return i;
            }
            t -= w;
        }
        weights.len() - 1
    }
}

/// An sRGB colour from its `0xRRGGBB` hex value.
pub(crate) const fn hex(v: u32) -> Rgb {
    Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

fn round_half_even(x: f64) -> f64 {
    let f = x.floor();
    let d = x - f;
    if d > 0.5 || (d == 0.5 && f % 2.0 != 0.0) {
        f + 1.0
    } else {
        f
    }
}

/// An `hsl()` colour as creature.wild resolves it: each component rounded
/// to a whole number with ties to even, then converted with `f64::round`.
pub(crate) fn hsl(h: f64, s: f64, l: f64) -> Rgb {
    let hue = round_half_even(((h % 360.0) + 360.0) % 360.0) % 360.0;
    let s = round_half_even(clamp(s, 0.0, 100.0)) / 100.0;
    let l = round_half_even(clamp(l, 0.0, 100.0)) / 100.0;
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
    let rgb = match sector.floor() as i64 {
        0 => [chroma, second, 0.0],
        1 => [second, chroma, 0.0],
        2 => [0.0, chroma, second],
        3 => [0.0, second, chroma],
        4 => [second, 0.0, chroma],
        _ => [chroma, 0.0, second],
    };
    let off = l - chroma / 2.0;
    let channel = |c: f64| clamp(((c + off) * 255.0).round(), 0.0, 255.0) as u8;
    Rgb {
        r: channel(rgb[0]),
        g: channel(rgb[1]),
        b: channel(rgb[2]),
    }
}

/// A colour part of the way from `a` to `b`, each channel rounded as
/// JavaScript's `Math.round` rounds it.
pub(crate) fn blend(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let channel = |p: u8, q: u8| {
        let p = f64::from(p);
        clamp(js_round(p + (f64::from(q) - p) * t), 0.0, 255.0) as u8
    };
    Rgb {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
    }
}

/// One material: its colour, a shade for the side turned from the light, a
/// tint for the side turned to it, and the line round a piece of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[deny(missing_docs)]
pub struct Material {
    /// Unshaded sRGB paint colour.
    pub base: Rgb,
    /// sRGB shade used on the unlit side.
    pub dark: Rgb,
    /// sRGB tint used on the lit side.
    pub lite: Rgb,
    /// sRGB outline colour.
    pub line: Rgb,
}

const fn material(base: u32, dark: u32, lite: u32, line: u32) -> Material {
    Material {
        base: hex(base),
        dark: hex(dark),
        lite: hex(lite),
        line: hex(line),
    }
}

/// The paint of a magnet hand and of its tips.
pub(crate) const MAGNET: Material = material(0xe0483c, 0xb3342b, 0xf2796e, 0x5a1a16);
pub(crate) const MAGNET_TIP: Material = material(0xf4f2ee, 0xd8d4cc, 0xffffff, 0x5a1a16);

/// The names of the metals, in the order the metal gene picks them.
pub const METAL_NAMES: [&str; 5] = ["steel", "gunmetal", "brass", "copper", "enamel"];
/// Hue, saturation and lightness of each metal's base, shade, tint and line.
const METALS: [[[f64; 3]; 4]; 5] = [
    [
        [214.0, 14.0, 70.0],
        [214.0, 14.0, 54.0],
        [214.0, 22.0, 87.0],
        [216.0, 18.0, 28.0],
    ],
    [
        [222.0, 10.0, 44.0],
        [222.0, 12.0, 32.0],
        [222.0, 10.0, 60.0],
        [222.0, 14.0, 16.0],
    ],
    [
        [44.0, 52.0, 62.0],
        [40.0, 50.0, 47.0],
        [48.0, 70.0, 79.0],
        [36.0, 50.0, 25.0],
    ],
    [
        [20.0, 52.0, 59.0],
        [16.0, 50.0, 43.0],
        [24.0, 66.0, 75.0],
        [14.0, 48.0, 23.0],
    ],
    [
        [40.0, 22.0, 91.0],
        [36.0, 14.0, 77.0],
        [0.0, 0.0, 100.0],
        [30.0, 10.0, 40.0],
    ],
];
/// The hues a lamp, a lit screen or a status light can be; -1 is white.
const GLOW_HUES: [f64; 8] = [186.0, 152.0, 88.0, 44.0, 328.0, 208.0, 22.0, -1.0];

/// What a status light shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[deny(missing_docs)]
pub enum Status {
    /// The robot's dim idle glow.
    Idle,
    /// The robot's full glow.
    Bright,
    /// Success (green).
    Ok,
    /// Attention (amber).
    Warn,
    /// Error (red).
    Err,
    /// Cold or inactive (blue).
    Cold,
    /// Unlit.
    Off,
}

#[deny(missing_docs)]
impl Status {
    /// The statuses in declaration order, used to index colour tables.
    pub const ALL: [Self; 7] = [
        Self::Idle,
        Self::Bright,
        Self::Ok,
        Self::Warn,
        Self::Err,
        Self::Cold,
        Self::Off,
    ];
}

/// Every colour a robot is painted with.
#[derive(Clone, Debug)]
pub(crate) struct Palette {
    /// Which of creature.wild's recipes the body colour comes from: 0
    /// pastel, 2 vivid, 3 dark, 4 muted.
    pub(crate) recipe: u8,
    pub(crate) dark: bool,
    pub(crate) body: Rgb,
    pub(crate) paint: Material,
    pub(crate) rubber: Material,
    pub(crate) ink: Rgb,
    pub(crate) white: Rgb,
    pub(crate) shadow: Rgb,
    pub(crate) drop: Rgb,
    pub(crate) drop_edge: Rgb,
    pub(crate) mark_ink: Rgb,
    pub(crate) anger: Rgb,
    pub(crate) steam: Rgb,
    pub(crate) steam_edge: Rgb,
    pub(crate) metal_kind: usize,
    pub(crate) metal: Material,
    pub(crate) accent_kind: u32,
    pub(crate) accent: Material,
    pub(crate) glow: Rgb,
    pub(crate) glow_lite: Rgb,
    pub(crate) glow_dim: Rgb,
    pub(crate) glass: Rgb,
    pub(crate) blush: Rgb,
}

impl Palette {
    pub(crate) fn status(&self, s: Status) -> Rgb {
        match s {
            Status::Idle => blend(self.glow_dim, self.glow, 0.8),
            Status::Bright => self.glow,
            Status::Ok => hex(0x63d67a),
            Status::Warn => hex(0xffb53a),
            Status::Err => hex(0xf04d3a),
            Status::Cold => hex(0x72b4f2),
            Status::Off => hex(0x4b4753),
        }
    }

    pub(crate) fn bulb(&self, s: Status, lit: bool) -> Rgb {
        let color = self.status(s);
        if lit {
            color
        } else {
            blend(hex(0xf1f3f5), color, 0.22)
        }
    }

    pub(crate) fn beacon(&self, s: Status, rays: bool) -> Rgb {
        let color = self.status(s);
        if rays {
            color
        } else {
            blend(self.glass, color, 0.7)
        }
    }
}

/// Byte 1104 holds the metal, the second paint and the colour of the lights;
/// 1105 the tint of the glass.
const PAINT: usize = 1104;

pub(crate) fn build_palette(r: &Reader) -> Palette {
    let rsel = r.raw(160, 0, 3);
    let recipe: u8 = if rsel <= 2 {
        0
    } else if rsel <= 4 {
        2
    } else if rsel <= 6 {
        4
    } else {
        3
    };
    let h = (f64::from(r.raw(161, 0, 8)) / 255.0 + f64::from(r.raw(162, 0, 8)) / 65535.0) * 360.0;
    let dark = recipe == 3;
    // Body saturation and lightness, outline saturation and lightness, and
    // the turn of the hue, saturation and lightness of the part colour.
    let (body_s, body_l, outline, part) = match recipe {
        0 => (66.0, 82.0, [42.0, 38.0], [28.0, 55.0, 62.0]),
        2 => (78.0, 64.0, [62.0, 30.0], [40.0, 80.0, 55.0]),
        3 => (38.0, 26.0, [30.0, 14.0], [24.0, 45.0, 44.0]),
        _ => (24.0, 62.0, [26.0, 30.0], [-26.0, 30.0, 48.0]),
    };
    let body = hsl(h, body_s, body_l);
    let paint = Material {
        base: body,
        dark: hsl(h, body_s - 3.3, clamp(body_l - 11.0, 10.0, 94.0)),
        lite: hsl(h, body_s - 2.7, clamp(body_l + 9.0, 10.0, 94.0)),
        line: hsl(h, outline[0], outline[1]),
    };
    let picked = r.pick(PAINT, 0, &[3, 2, 1, 1, 1], 3);
    // Gunmetal on dark paint would leave limbs and hull one dark mass.
    let metal_kind = if dark && picked == 1 { 0 } else { picked };
    let m = METALS[metal_kind];
    let metal = Material {
        base: hsl(m[0][0], m[0][1], m[0][2]),
        dark: hsl(m[1][0], m[1][1], m[1][2]),
        lite: hsl(m[2][0], m[2][1], m[2][2]),
        line: hsl(m[3][0], m[3][1], m[3][2]),
    };
    let accent_kind = r.raw(PAINT, 3, 2);
    let c = match accent_kind {
        0 => [h + part[0], part[1], part[2]],
        1 => [
            h + 180.0,
            if dark { 48.0 } else { 62.0 },
            if dark { 52.0 } else { 60.0 },
        ],
        2 => [44.0, 40.0, 90.0],
        _ => [47.0, 92.0, 58.0],
    };
    let accent = Material {
        base: hsl(c[0], c[1], c[2]),
        dark: hsl(c[0], c[1], max(8.0, c[2] - 13.0)),
        lite: hsl(c[0], c[1] * 0.9, min(97.0, c[2] + 9.0)),
        line: hsl(c[0], min(60.0, c[1] * 0.7), max(12.0, c[2] * 0.42)),
    };
    let glow_kind = r.raw(PAINT, 5, 3) as usize;
    let g = GLOW_HUES[glow_kind];
    let white = g < 0.0;
    let glow = if white {
        hex(0xf3f7fb)
    } else {
        hsl(g, 96.0, 66.0)
    };
    let glow_lite = if white {
        hex(0xffffff)
    } else {
        hsl(g, 100.0, 87.0)
    };
    let glow_dim = if white {
        hex(0x7f8894)
    } else {
        hsl(g, 46.0, 36.0)
    };
    // The glass of a lens or a dark screen: near black, tinted by the paint
    // or by the lights, and kept darker on dark paint.
    let v = r.raw(PAINT + 1, 0, 2);
    let glass = match v {
        0 => hex(if dark { 0x111217 } else { 0x16171d }),
        1 => hsl(h, 42.0, if dark { 12.0 } else { 17.0 }),
        _ => hsl(
            if white { 215.0 } else { g },
            if v == 3 { 52.0 } else { 46.0 },
            if v == 3 {
                if dark {
                    16.0
                } else {
                    23.0
                }
            } else if dark {
                11.0
            } else {
                15.0
            },
        ),
    };
    let blush = if dark {
        hsl(h + 14.0, 60.0, 55.0)
    } else if recipe == 0 {
        hsl(h + 8.0, 75.0, 72.0)
    } else if recipe == 2 {
        hsl(h - 12.0, 85.0, 70.0)
    } else {
        hsl(h + 20.0, 35.0, 58.0)
    };
    Palette {
        recipe,
        dark,
        body,
        paint,
        rubber: material(0x3a3640, 0x28252d, 0x55505c, 0x17151b),
        ink: hex(if dark { 0xf4eee6 } else { 0x241f26 }),
        white: hex(0xfbfaf6),
        shadow: hex(0x3a2f47),
        drop: hex(0xa9d9f4),
        drop_edge: hex(0x5ba3d2),
        mark_ink: hex(if dark { 0x8d8494 } else { 0x6f6877 }),
        anger: hex(0xe0553f),
        steam: hex(0xfdfcf8),
        steam_edge: hex(0xb9b3bd),
        metal_kind,
        metal,
        accent_kind,
        accent,
        glow,
        glow_lite,
        glow_dim,
        glass,
        blush,
    }
}
