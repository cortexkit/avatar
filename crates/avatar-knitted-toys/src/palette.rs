//! Colours: the yarns and the felt and thread a toy is made of.
//!
//! Colours are HSL, as creature.wild's are, rounded the way wild's Rust
//! generator writes them: hue, saturation and lightness to whole numbers with
//! ties to even, then 8-bit RGB with halves away from zero. The main yarn is
//! therefore the very colour wild's drawing gives the same genome's body.

use avatar_svg::Rgb;

use crate::decode::Traits;
use crate::geom::{clamp, max, min, round_half_even};

/// Rust's `f64::round`: halves away from zero.
fn round_half_away(v: f64) -> f64 {
    if v < 0.0 {
        -(-v + 0.5).floor()
    } else {
        (v + 0.5).floor()
    }
}

pub fn hsl_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let hue = round_half_even(((h % 360.0) + 360.0) % 360.0) % 360.0;
    let sat = round_half_even(clamp(s, 0.0, 100.0)) / 100.0;
    let light = round_half_even(clamp(l, 0.0, 100.0)) / 100.0;
    let chroma = (1.0 - (2.0 * light - 1.0).abs()) * sat;
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
    let off = light - chroma / 2.0;
    let channel = |c: f64| clamp(round_half_away((c + off) * 255.0), 0.0, 255.0) as u8;
    Rgb {
        r: channel(rgb[0]),
        g: channel(rgb[1]),
        b: channel(rgb[2]),
    }
}

pub fn hsl(h: f64, s: f64, l: f64) -> Rgb {
    hsl_rgb(h, s, l)
}

/// A colour from its `#rrggbb` hex.
pub const fn rgb(hex: u32) -> Rgb {
    Rgb {
        r: (hex >> 16) as u8,
        g: (hex >> 8) as u8,
        b: hex as u8,
    }
}

/// How far apart two hues are round the colour wheel, 0 to 180.
pub fn hue_gap(a: f64, b: f64) -> f64 {
    ((((a - b) % 360.0) + 540.0) % 360.0 - 180.0).abs()
}

/// A yarn: its colour and the tones a stitched surface shows. `gap` is the
/// dark between stitches, `mid` and `lit` a stitch's shaded and lit legs,
/// `deep` the tone of rims and seams. `y` is the colour's brightness, 0 to 1.
#[derive(Clone, Debug)]
pub struct Yarn {
    /// Which yarn this is. Two yarns of the same colour are still different
    /// yarns: a run of rows changes yarn wherever the yarn changes.
    pub id: u32,
    pub h: f64,
    pub s: f64,
    pub l: f64,
    pub hex: Rgb,
    pub y: f64,
    pub mid: Rgb,
    pub lit: Rgb,
    pub gap: Rgb,
    pub deep: Rgb,
}

impl PartialEq for Yarn {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Makes yarns, each with its own identity.
struct Yarns(u32);

impl Yarns {
    fn yarn(&mut self, h: f64, s: f64, l: f64) -> Yarn {
        self.0 += 1;
        Yarn {
            id: self.0,
            ..yarn(h, s, l)
        }
    }
}

fn yarn(h: f64, s: f64, l: f64) -> Yarn {
    let s = clamp(s, 0.0, 100.0);
    let l = clamp(l, 8.0, 96.0);
    let hex = hsl_rgb(h, s, l);
    // A tone at another lightness with the same colourfulness (times k):
    // HSL saturation is colourfulness over the room the lightness leaves.
    let room = |v: f64| 1.0 - (2.0 * v / 100.0 - 1.0).abs();
    let tone = |l2: f64, k: f64| {
        let l2 = clamp(l2, 4.0, 97.0);
        hsl(h, min(100.0, s * room(l) * k / max(0.06, room(l2))), l2)
    };
    Yarn {
        id: 0,
        h,
        s,
        l,
        hex,
        y: (0.2126 * f64::from(hex.r) + 0.7152 * f64::from(hex.g) + 0.0722 * f64::from(hex.b))
            / 255.0,
        mid: tone(l - 4.0, 1.0),
        lit: tone(l + 7.0, 0.9),
        gap: tone(l - min(20.0, 0.55 * l + 2.0), 1.02),
        deep: tone(l * 0.5, 0.95),
    }
}

/// The felt colours of the face and the marks.
#[derive(Clone, Debug)]
pub struct Felt {
    pub tongue: Rgb,
    pub mouth: Rgb,
    pub tear: Rgb,
    pub tear_edge: Rgb,
    pub anger: Rgb,
    pub anger_edge: Rgb,
    pub white: Rgb,
    pub white_edge: Rgb,
    pub pad: Rgb,
    pub leaf: Rgb,
    pub leaf_edge: Rgb,
    pub sparkle: Rgb,
    pub sparkle_edge: Rgb,
}

#[derive(Clone, Debug)]
pub struct Palette {
    pub main: Yarn,
    pub accent: Yarn,
    pub second: Option<Yarn>,
    pub blush: Rgb,
    pub blush_deep: Rgb,
    pub blush_edge: Rgb,
    pub thread: Rgb,
    pub thread_dark: Rgb,
    pub felt: Felt,
    pub beak: Yarn,
    pub stalk: Yarn,
    pub dark: bool,
    pub light: Yarn,
    pub tint: Yarn,
    pub crest: Yarn,
    horn_cream: Yarn,
    horn_gold: Yarn,
    horn_grey: Yarn,
}

impl Palette {
    /// The horn yarn the toy's top part is knitted in.
    pub fn horn(&self, name: &str) -> &Yarn {
        match name {
            "cream" => &self.horn_cream,
            "gold" => &self.horn_gold,
            "accent" => &self.accent,
            _ => &self.horn_grey,
        }
    }

    pub fn eye_iris(name: &str) -> Rgb {
        match name {
            "amber" => hsl(38.0, 85.0, 54.0),
            "brown" => hsl(25.0, 48.0, 38.0),
            "green" => hsl(130.0, 42.0, 46.0),
            "blue" => hsl(210.0, 62.0, 54.0),
            _ => hsl(265.0, 44.0, 60.0),
        }
    }
}

/// Saturation and lightness of wild's four colour recipes.
fn recipe(tone: &str) -> (f64, f64) {
    match tone {
        "pastel" => (66.0, 82.0),
        "vivid" => (78.0, 64.0),
        "muted" => (24.0, 62.0),
        _ => (38.0, 26.0),
    }
}

/// The vivid recipe keeps wild's hue and lightness but is dyed less
/// strongly: at wild's strength a vivid toy reads as plastic, not wool.
const VIVID_YARN_SATURATION: f64 = 56.0;

pub fn palette(t: &Traits) -> Palette {
    let mut ys = Yarns(0);
    let h = t.hue_wild;
    let dark = t.tone == "dark";
    let (rs, l) = recipe(t.tone);
    let s = if t.tone == "vivid" {
        VIVID_YARN_SATURATION
    } else {
        rs
    };
    let main = ys.yarn(h, s, l);
    // The accent's hue comes from wild's belly variant: the main hue, the
    // main hue lighter, warmed towards orange, or turned 28 degrees.
    let fam = t.accent_family as usize;
    let warm = h + clamp(
        ((45.0 - (((h % 360.0) + 360.0) % 360.0) + 540.0) % 360.0) - 180.0,
        -30.0,
        30.0,
    );
    let acc_hue = [h, h, warm, h + 28.0][fam];
    let fam1 = fam == 1;
    let accent = match t.colour.accent {
        "cream" => match fam {
            0 => ys.yarn(38.0, 42.0, 89.0),
            1 => ys.yarn(48.0, 30.0, 95.0),
            2 => ys.yarn(30.0, 55.0, 87.0),
            _ => ys.yarn(h, 45.0, 91.0),
        },
        "tint" => {
            if dark {
                ys.yarn(acc_hue, s * 0.8, l + 22.0 + if fam1 { 6.0 } else { 0.0 })
            } else {
                ys.yarn(
                    acc_hue,
                    s * 0.62,
                    min(94.0, l + 12.0 + if fam1 { 4.0 } else { 0.0 }),
                )
            }
        }
        "shade" => {
            if dark {
                ys.yarn(acc_hue, s, l + 14.0)
            } else {
                ys.yarn(
                    acc_hue,
                    min(100.0, s * 1.05),
                    l - 20.0 - if fam1 { 5.0 } else { 0.0 },
                )
            }
        }
        "contrast" => ys.yarn(
            h + [150.0, 180.0, 205.0, 125.0][fam],
            52.0,
            clamp(l, 58.0, 80.0),
        ),
        _ => ys.yarn(h, 9.0, 30.0 + 4.0 * fam as f64),
    };
    // A second yarn for stripes, kept apart from the main yarn in lightness
    // when the hues are close.
    let second = (t.stripes.kind != "none").then(|| {
        let mut second = match t.colour.second.unwrap_or("dark") {
            "cream" => ys.yarn(40.0, 40.0, 92.0),
            "accent" => accent.clone(),
            "complement" => ys.yarn(h + 180.0, 50.0, if l > 55.0 { l - 12.0 } else { l + 24.0 }),
            "analog" => ys.yarn(
                h + 50.0,
                min(90.0, s * 1.05 + 6.0),
                if l > 55.0 { l - 9.0 } else { l + 22.0 },
            ),
            _ => ys.yarn(h, s, if l > 40.0 { l - 28.0 } else { l + 26.0 }),
        };
        if (second.y - main.y).abs() < 0.1 && hue_gap(second.h, h) < 40.0 {
            second = ys.yarn(
                second.h,
                second.s,
                if main.l > 55.0 {
                    main.l - 24.0
                } else {
                    main.l + 24.0
                },
            );
        }
        second
    });
    let bh = [352.0, 12.0, 335.0, 24.0][t.colour.blush as usize];
    let light = if accent.y >= 0.72 {
        accent.clone()
    } else {
        ys.yarn(
            acc_hue,
            if dark { 40.0 } else { s * 0.6 },
            min(94.0, max(l + 12.0, 85.0)),
        )
    };
    let tint = if dark {
        ys.yarn(h, s * 0.8, l + 20.0)
    } else {
        ys.yarn(h, s * 0.7, min(94.0, l + 10.0))
    };
    let crest = if dark {
        ys.yarn(h, s, l + 13.0)
    } else {
        ys.yarn(h, min(100.0, s * 1.05), l - 14.0)
    };
    Palette {
        blush: hsl(bh, 80.0, 77.0),
        blush_deep: hsl(bh + 4.0, 62.0, 55.0),
        blush_edge: hsl(bh, 55.0, 63.0),
        thread: if dark {
            hsl(40.0, 45.0, 90.0)
        } else {
            rgb(0x33262a)
        },
        thread_dark: rgb(0x2b2023),
        felt: Felt {
            tongue: hsl(350.0, 76.0, 70.0),
            mouth: hsl(350.0, 36.0, 23.0),
            tear: hsl(205.0, 82.0, 80.0),
            tear_edge: hsl(208.0, 60.0, 55.0),
            anger: hsl(6.0, 74.0, 54.0),
            anger_edge: hsl(4.0, 66.0, 42.0),
            white: hsl(45.0, 40.0, 97.0),
            white_edge: hsl(35.0, 14.0, 78.0),
            pad: hsl(bh, 62.0, 79.0),
            leaf: hsl(105.0, 45.0, 58.0),
            leaf_edge: hsl(110.0, 42.0, 40.0),
            sparkle: hsl(46.0, 92.0, 66.0),
            sparkle_edge: hsl(38.0, 70.0, 50.0),
        },
        beak: if hue_gap(h, 45.0) < 30.0 {
            ys.yarn(20.0, 80.0, 58.0)
        } else {
            ys.yarn(40.0, 88.0, 62.0)
        },
        stalk: ys.yarn(105.0, 40.0, 44.0),
        horn_cream: ys.yarn(40.0, 45.0, 88.0),
        horn_gold: ys.yarn(42.0, 70.0, 66.0),
        horn_grey: ys.yarn(250.0, 6.0, 66.0),
        main,
        accent,
        second,
        dark,
        light,
        tint,
        crest,
    }
}
