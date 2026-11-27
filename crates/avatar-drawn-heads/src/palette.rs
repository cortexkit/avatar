//! Colour: the wild body colour and everything a head takes from it.

use avatar_svg::Rgb;

use crate::genes::{dna, Genes};
use crate::geom::{clamp, js_round, lerp, max, min, step};
use crate::hair::HairStyle;

/// Rust formats `hsl()` components with `{:.0}`, which rounds ties to even.
fn round_half_even(x: f64) -> f64 {
    let f = x.floor();
    let d = x - f;
    // Above a half rounds up; a half rounds up only from an odd integer.
    if d > 0.5 || (d == 0.5 && f % 2.0 != 0.0) {
        f + 1.0
    } else {
        f
    }
}

/// An `hsl()` colour as 8-bit RGB, the way the wild generator resolves it.
pub fn hsl(h: f64, s: f64, l: f64) -> Rgb {
    let hue = round_half_even(((h % 360.0) + 360.0) % 360.0) % 360.0;
    let sat = round_half_even(clamp(s, 0.0, 100.0)) / 100.0;
    let light = round_half_even(clamp(l, 0.0, 100.0)) / 100.0;
    let chroma = (1.0 - (2.0 * light - 1.0).abs()) * sat;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
    let (r, g, b) = match sector.floor() as i64 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let off = light - chroma / 2.0;
    let channel = |c: f64| clamp(((c + off) * 255.0).round(), 0.0, 255.0) as u8;
    Rgb {
        r: channel(r),
        g: channel(g),
        b: channel(b),
    }
}

pub fn hsl3(c: [f64; 3]) -> Rgb {
    hsl(c[0], c[1], c[2])
}

/// A colour part of the way from one colour to another.
pub fn blend(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let mix = |p: u8, q: u8| {
        let p = f64::from(p);
        js_round(p + (f64::from(q) - p) * t) as u8
    };
    Rgb {
        r: mix(a.r, b.r),
        g: mix(a.g, b.g),
        b: mix(a.b, b.b),
    }
}

pub const fn rgb(hex: u32) -> Rgb {
    Rgb {
        r: (hex >> 16) as u8,
        g: (hex >> 8) as u8,
        b: hex as u8,
    }
}

/// The saturation and lightness of a recipe's body, of the skin of a head
/// whose hair carries the body colour, and of its ink.
struct Recipe {
    body: [f64; 2],
    skin: [f64; 2],
    ink: [f64; 2],
}

fn recipe_of(recipe: u8) -> Recipe {
    match recipe {
        // Pastel: the skin is always the body colour itself.
        0 => Recipe {
            body: [66.0, 82.0],
            skin: [66.0, 82.0],
            ink: [40.0, 21.0],
        },
        2 => Recipe {
            body: [78.0, 64.0],
            skin: [70.0, 85.0],
            ink: [44.0, 20.0],
        },
        3 => Recipe {
            body: [38.0, 26.0],
            skin: [34.0, 82.0],
            ink: [30.0, 16.0],
        },
        _ => Recipe {
            body: [24.0, 62.0],
            skin: [22.0, 85.0],
            ink: [18.0, 20.0],
        },
    }
}

/// The hair of a head whose skin carries the body colour turns the body's
/// hue this far, or takes a dark (`None` with index 6) or pale (index 7) colour.
const HAIR_TURNS: [f64; 6] = [0.0, 28.0, -32.0, 180.0, 150.0, -150.0];
const HAIR_TURN_NAMES: [&str; 6] = [
    "same hue",
    "next hue",
    "next hue",
    "opposite hue",
    "far hue",
    "far hue",
];
const HAIR_TONES: [[f64; 2]; 4] = [[46.0, 27.0], [62.0, 43.0], [74.0, 57.0], [42.0, 67.0]];
const TONE_NAMES: [&str; 4] = [", deep", "", ", bright", ", soft"];
const IRIS_HUES: [i32; 8] = [28, 95, 205, 265, 44, 172, -1, -2];

/// Every colour of one head.
#[derive(Clone, Debug)]
pub struct Palette {
    /// creature.wild's recipe: 0 pastel, 2 vivid, 3 dark, 4 muted.
    pub recipe: u8,
    pub hue: f64,
    /// The body colour, exactly as creature.wild draws it.
    pub body: Rgb,
    /// The hair style, after a dark head trades a sparse style for a fuller one.
    pub hair_style: HairStyle,
    /// Whether the skin is the body colour (otherwise the hair is).
    pub skin_is_body: bool,
    pub skin: Rgb,
    /// The skin's lightness, in percent.
    pub skin_l: f64,
    /// The neck, and a nose that is filled in.
    pub skin_dark: Rgb,
    pub ink: Rgb,
    pub white: Rgb,
    pub blush: Rgb,
    pub flush: Rgb,
    pub mouth_in: Rgb,
    pub tongue: Rgb,
    pub drop: Rgb,
    pub drop_edge: Rgb,
    pub mark_ink: Rgb,
    pub anger: Rgb,
    pub steam: Rgb,
    pub steam_edge: Rgb,
    /// The hair colour as hue, saturation and lightness.
    pub hair_hsl: [f64; 3],
    /// What the hair colour is called.
    pub hair_name: String,
    pub hair: Rgb,
    /// Strands drawn in the hair.
    pub hair_line: Rgb,
    /// The stubble of a clipped head.
    pub hair_shade: Rgb,
    /// Single hairs drawn on the skin.
    pub hair_pen: Rgb,
    pub beard_hsl: [f64; 3],
    pub beard: Rgb,
    pub beard_line: Rgb,
    pub beard_pen: Rgb,
    pub brow: Rgb,
    pub iris: Rgb,
    pub lip: Rgb,
}

/// The hair styles that leave too little hair to show a colour.
pub fn sparse(style: HairStyle) -> bool {
    matches!(
        style,
        HairStyle::Bald | HairStyle::Wisps | HairStyle::Horseshoe | HairStyle::Combover
    )
}

pub fn build_palette(g: &Genes) -> Palette {
    let rsel = g.raw(160, 0, 3);
    let recipe: u8 = if rsel <= 2 {
        0
    } else if rsel <= 4 {
        2
    } else if rsel <= 6 {
        4
    } else {
        3
    };
    let h = (f64::from(g.raw(161, 0, 8)) / 255.0 + f64::from(g.raw(162, 0, 8)) / 65535.0) * 360.0;
    let rc = recipe_of(recipe);
    let body = hsl(h, rc.body[0], rc.body[1]);
    // A dark body colour is no skin for an ink drawing, so a dark head with
    // a sparse style takes a fuller one and wears its colour as hair.
    let mut style = HairStyle::pick(g);
    if recipe == 3 {
        style = match style {
            HairStyle::Bald => HairStyle::Buzz,
            HairStyle::Wisps => HairStyle::Crop,
            HairStyle::Horseshoe => HairStyle::Slick,
            HairStyle::Combover => HairStyle::Tousle,
            other => other,
        };
    }
    let skin_is_body = recipe == 0 || sparse(style);
    let sk = if skin_is_body { rc.body } else { rc.skin };
    let skin = if skin_is_body {
        body
    } else {
        hsl(h, sk[0], sk[1])
    };
    let skin_dark = hsl(h, sk[0] - 3.3, clamp(sk[1] - 11.0, 10.0, 94.0));
    let ink = hsl(h, rc.ink[0], rc.ink[1]);

    let (hair_hsl, hair_name) = if !skin_is_body {
        ([h, rc.body[0], rc.body[1]], "body colour".to_owned())
    } else {
        let mut turn = g.raw(dna::COLOUR, 0, 3) as usize;
        // A sparse style leaves only small patches of hair, single hairs and
        // facial hair. In a hue far from the skin's, such a patch at the
        // temple reads as an earmuff and a beard as a bib, so those heads
        // take the hue nearer the body colour that the far one stands in for.
        if sparse(style) && (3..=5).contains(&turn) {
            turn -= 3;
        }
        let tone = g.raw(dna::COLOUR, 3, 2) as usize;
        match turn {
            6 => (
                [
                    [h, 16.0, 15.0],
                    [22.0, 36.0, 24.0],
                    [h + 180.0, 10.0, 34.0],
                    [222.0, 14.0, 46.0],
                ][tone],
                ["black", "espresso", "charcoal", "slate"][tone].to_owned(),
            ),
            7 => (
                [
                    [40.0, 24.0, 95.0],
                    [215.0, 14.0, 79.0],
                    [42.0, 40.0, 75.0],
                    [215.0, 8.0, 63.0],
                ][tone],
                ["white", "silver", "ash", "grey"][tone].to_owned(),
            ),
            _ => (
                [
                    h + HAIR_TURNS[turn],
                    HAIR_TONES[tone][0],
                    HAIR_TONES[tone][1],
                ],
                format!("{}{}", HAIR_TURN_NAMES[turn], TONE_NAMES[tone]),
            ),
        }
    };
    let c = hair_hsl;
    let hair = hsl3(c);
    let hair_line = if c[2] < 38.0 {
        hsl(c[0], c[1], c[2] + 22.0)
    } else {
        hsl(c[0], min(100.0, c[1] + 4.0), c[2] - 24.0)
    };
    let hair_shade = if c[2] < 38.0 {
        hsl(c[0], c[1], max(3.0, c[2] - 13.0))
    } else {
        hair_line
    };
    let hair_pen = if c[2] < 60.0 { hair } else { hair_line };

    // Facial hair is a darker shade of the hair, and a dark grey with a
    // trace of the hue where that shade would be a red or a pink.
    let shade = f64::from(g.raw(dna::BEARD + 2, 6, 2));
    let beard_hsl = if c[2] >= 62.0 && c[1] <= 42.0 {
        [c[0], c[1], c[2] - 5.0 * shade]
    } else {
        let away = ((((c[0] - 355.0) % 360.0) + 540.0) % 360.0 - 180.0).abs();
        let red = 1.0 - step(18.0, 38.0, away);
        [
            c[0],
            lerp(c[1] * 0.85, min(c[1] * 0.25, 13.0), red),
            lerp(
                min(c[2] - 6.0, 38.0) - 5.0 * shade,
                clamp(c[2] * 0.62, 18.0, 31.0) - 3.0 * shade,
                red,
            ),
        ]
    };
    let b = beard_hsl;
    let beard = hsl3(b);
    let beard_line = if b[2] < 38.0 {
        hsl(b[0], b[1], b[2] + 22.0)
    } else {
        hsl(b[0], b[1], b[2] - 24.0)
    };
    let beard_pen = if b[2] < 60.0 { beard } else { beard_line };
    let brow = if c[2] < 40.0 {
        blend(hair, ink, 0.3)
    } else {
        blend(hair, ink, 0.62)
    };
    let iris = match IRIS_HUES[g.raw(dna::COLOUR + 1, 0, 3) as usize] {
        -1 => rgb(0x6f7b86),
        -2 => hsl(h, 46.0, 43.0),
        hue => hsl(f64::from(hue), 46.0, 43.0),
    };
    let lip = match g.raw(dna::COLOUR + 1, 3, 2) {
        0 => blend(skin_dark, rgb(0xd9627c), 0.35),
        1 => hsl(352.0, 58.0, 70.0),
        2 => hsl(346.0, 52.0, 52.0),
        _ => blend(skin_dark, ink, 0.25),
    };
    Palette {
        recipe,
        hue: h,
        body,
        hair_style: style,
        skin_is_body,
        skin,
        skin_l: sk[1],
        skin_dark,
        ink,
        white: rgb(0xfdfcf8),
        blush: rgb(0xf0768a),
        flush: rgb(0xe8503f),
        mouth_in: blend(ink, rgb(0x8c2f42), 0.55),
        tongue: rgb(0xef8f98),
        drop: rgb(0xa9d9f4),
        drop_edge: rgb(0x5ba3d2),
        mark_ink: hsl(h, 10.0, 44.0),
        anger: rgb(0xe0553f),
        steam: rgb(0xfdfcf8),
        steam_edge: rgb(0xb9b3bd),
        hair_hsl,
        hair_name,
        hair,
        hair_line,
        hair_shade,
        hair_pen,
        beard_hsl,
        beard,
        beard_line,
        beard_pen,
        brow,
        iris,
        lip,
    }
}
