//! Colours: the body takes the colour creature.wild gives the genome, the
//! leaves a green of their own, and flowers and fruit new colours.

use avatar_svg::Rgb;

use crate::maths::{clamp, lerp, max, min};
use crate::reader::{dna, Reader};

/// Rust's `{:.0}` formatting rounds ties to even; the reference reproduces
/// it so its body colour equals the shipped creature.wild drawing's.
fn round_half_even(x: f64) -> f64 {
    let whole = x.floor();
    let fraction = x - whole;
    if fraction > 0.5 || (fraction == 0.5 && whole % 2.0 != 0.0) {
        whole + 1.0
    } else {
        whole
    }
}

/// The reference's `hsl(h, s, l)`, converted to 8-bit RGB.
pub(crate) fn hsl(h: f64, s: f64, l: f64) -> Rgb {
    let hue = round_half_even(((h % 360.0) + 360.0) % 360.0) % 360.0;
    let saturation = round_half_even(clamp(s, 0.0, 100.0)) / 100.0;
    let lightness = round_half_even(clamp(l, 0.0, 100.0)) / 100.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
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
    let offset = lightness - chroma / 2.0;
    // Rust's round takes halves away from zero, as the reference does here.
    let channel = |c: f64| clamp(((c + offset) * 255.0).round(), 0.0, 255.0) as u8;
    Rgb {
        r: channel(r),
        g: channel(g),
        b: channel(b),
    }
}

pub(crate) const fn rgb(hex: u32) -> Rgb {
    Rgb {
        r: (hex >> 16) as u8,
        g: (hex >> 8) as u8,
        b: hex as u8,
    }
}

pub(crate) const WHITE: Rgb = rgb(0xffffff);

/// How far apart two hues are round the colour wheel, 0 to 180.
fn hue_gap(a: f64, b: f64) -> f64 {
    ((((a - b) % 360.0) + 540.0) % 360.0 - 180.0).abs()
}

/// creature.wild's four palette recipes: saturation and lightness of the
/// body, then of its outline.
fn recipe_tones(recipe: u8) -> ([f64; 2], [f64; 2]) {
    match recipe {
        0 => ([66.0, 82.0], [42.0, 38.0]),
        2 => ([78.0, 64.0], [62.0, 30.0]),
        3 => ([38.0, 26.0], [30.0, 14.0]),
        _ => ([24.0, 62.0], [26.0, 30.0]),
    }
}

/// Families of foliage: name, then hue, saturation and lightness ranges.
pub const FOLIAGE: [&str; 9] = [
    "leaf green",
    "lime",
    "forest",
    "teal",
    "sage",
    "olive",
    "fern",
    "jade",
    "moss",
];
const FOLIAGE_TONES: [[f64; 6]; 9] = [
    [100.0, 132.0, 48.0, 62.0, 44.0, 54.0],
    [72.0, 94.0, 56.0, 70.0, 48.0, 58.0],
    [136.0, 160.0, 40.0, 54.0, 30.0, 38.0],
    [160.0, 186.0, 40.0, 56.0, 38.0, 48.0],
    [96.0, 150.0, 16.0, 26.0, 58.0, 68.0],
    [60.0, 78.0, 38.0, 52.0, 38.0, 46.0],
    [108.0, 138.0, 42.0, 56.0, 34.0, 42.0],
    [142.0, 168.0, 28.0, 42.0, 48.0, 58.0],
    [78.0, 100.0, 40.0, 54.0, 36.0, 44.0],
];
const FOLIAGE_WEIGHTS: [u32; 9] = [6, 3, 3, 3, 2, 2, 2, 2, 1];

/// Every colour a creature is drawn in.
#[derive(Clone, Debug)]
pub(crate) struct Palette {
    pub(crate) recipe: u8,
    pub(crate) hue: f64,
    pub(crate) dark: bool,
    pub(crate) body: Rgb,
    pub(crate) outline: Rgb,
    pub(crate) tone_up: Rgb,
    pub(crate) tone_down: Rgb,
    pub(crate) deep: Rgb,
    pub(crate) pale: Rgb,
    pub(crate) ink: Rgb,
    pub(crate) pupil: Rgb,
    pub(crate) white: Rgb,
    pub(crate) mouth: Rgb,
    pub(crate) mouth_inside: Rgb,
    pub(crate) tongue: Rgb,
    pub(crate) blush: Rgb,
    pub(crate) blush_alpha: f64,
    pub(crate) shadow: Rgb,
    pub(crate) foliage: usize,
    pub(crate) leaf: Rgb,
    pub(crate) leaf_dark: Rgb,
    pub(crate) leaf_light: Rgb,
    pub(crate) leaf_hue: f64,
    pub(crate) leaf_l: f64,
    pub(crate) petal: Rgb,
    pub(crate) petal_dark: Rgb,
    pub(crate) petal_light: Rgb,
    pub(crate) centre: Rgb,
    pub(crate) fruit: Rgb,
    pub(crate) fruit_dark: Rgb,
    pub(crate) thorn: Rgb,
}

pub(crate) fn build_palette(r: &Reader<'_>) -> Palette {
    let rsel = r.u8(160) % 8;
    let recipe = if rsel <= 2 {
        0
    } else if rsel <= 4 {
        2
    } else if rsel <= 6 {
        4
    } else {
        3
    };
    let h = (f64::from(r.u8(161)) / 255.0 + f64::from(r.u8(162)) / 65535.0) * 360.0;
    let dark = recipe == 3;
    let ([s, l], [so, lo]) = recipe_tones(recipe);
    let body = hsl(h, s, l);
    let outline = hsl(h, so, lo);
    let ink_hex = if dark {
        rgb(0xf4eee6)
    } else if recipe == 4 {
        rgb(0x2b2622)
    } else {
        rgb(0x241f26)
    };
    let pupil = if dark {
        hsl(h, 32.0, 12.0)
    } else if recipe == 4 {
        rgb(0x2b2622)
    } else {
        rgb(0x241f26)
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

    let o = dna::COLOUR;
    let family = r.pick(o, &FOLIAGE_WEIGHTS);
    let tones = FOLIAGE_TONES[family];
    let lh = lerp(tones[0], tones[1], r.unit(o + 1));
    let mut ls = lerp(tones[2], tones[3], r.unit(o + 2));
    let mut ll = lerp(tones[4], tones[5], r.unit(o + 3));
    // Leaves must stand out from the body they grow on.
    if dark {
        ll = max(ll + 8.0, 46.0);
    } else if hue_gap(lh, h) < 42.0 && (ll - l).abs() < 20.0 {
        ll = if l > 60.0 { l - 27.0 } else { l + 24.0 };
        ls = max(ls, 40.0);
    }

    Palette {
        recipe,
        hue: h,
        dark,
        body,
        outline,
        tone_up: hsl(h, max(12.0, s - 2.7), min(94.0, l + 9.0)),
        tone_down: hsl(h, max(12.0, s - 3.3), max(10.0, l - 11.0)),
        deep: if dark {
            hsl(h, 34.0, 15.0)
        } else {
            hsl(h, min(70.0, s * 0.85), max(18.0, l - 26.0))
        },
        pale: if dark {
            hsl(h, 30.0, 42.0)
        } else {
            hsl(h, s * 0.55, min(96.0, l + 12.0))
        },
        ink: ink_hex,
        pupil,
        white: if dark { rgb(0xf6f1e9) } else { WHITE },
        mouth: if dark { rgb(0xf4eee6) } else { outline },
        mouth_inside: rgb(0x4a2f33),
        tongue: rgb(0xe88a93),
        blush,
        blush_alpha: if dark { 0.32 } else { 0.8 },
        shadow: rgb(0x281e32),
        foliage: family,
        leaf: hsl(lh, ls, ll),
        leaf_dark: hsl(lh, ls * 0.9, max(12.0, ll - 21.0)),
        leaf_light: hsl(lh, ls * 0.75, min(90.0, ll + 20.0)),
        leaf_hue: lh,
        leaf_l: ll,
        // Set by `build_accents` when the plant bears what they colour.
        petal: body,
        petal_dark: body,
        petal_light: body,
        centre: body,
        fruit: body,
        fruit_dark: body,
        thorn: body,
    }
}

/// Colours for what the plant bears: flowers, fruit and thorns. Each is
/// read only if the plant bears it.
pub(crate) fn build_accents(
    r: &Reader<'_>,
    p: &mut Palette,
    flowers: bool,
    fruit: bool,
    thorns: bool,
) {
    let o = dna::COLOUR;
    let h = p.hue;
    let l = recipe_tones(p.recipe).0[1];
    if flowers {
        let scheme = r.pick(o + 4, &[3, 2, 2, 3, 3, 2, 2, 2, 2, 1]);
        let [mut ph, ps, pl] = [
            [h + 180.0, 72.0, 68.0],
            [h + 120.0, 74.0, 66.0],
            [h - 120.0, 74.0, 66.0],
            [48.0, 94.0, 60.0],
            [334.0, 82.0, 68.0],
            [h + 32.0, 82.0, 74.0],
            [12.0, 86.0, 60.0],
            [258.0, 70.0, 70.0],
            [200.0, 80.0, 66.0],
            [46.0, 70.0, 86.0],
        ][scheme];
        ph += r.lin(o + 5, -14.0, 14.0);
        // A flower stands out from the body and from the leaves round it.
        if hue_gap(ph, h) < 34.0 && (pl - l).abs() < 16.0 {
            ph += 150.0;
        }
        if hue_gap(ph, p.leaf_hue) < 30.0 && (pl - p.leaf_l).abs() < 14.0 {
            ph += 45.0;
        }
        p.petal = hsl(ph, ps, pl);
        p.petal_dark = hsl(ph, ps * 0.8, pl - 26.0);
        p.petal_light = hsl(ph, ps * 0.8, min(95.0, pl + 14.0));
        let yellowish = hue_gap(ph, 48.0) < 26.0;
        p.centre = [
            if yellowish {
                hsl(26.0, 50.0, 26.0)
            } else {
                hsl(44.0, 92.0, 58.0)
            },
            hsl(26.0, 50.0, 26.0),
            hsl(50.0, 70.0, 88.0),
            p.outline,
        ][r.pick(o + 6, &[4, 2, 2, 1])];
    }
    if fruit {
        const FRUIT: [[f64; 3]; 6] = [
            [356.0, 76.0, 52.0],
            [232.0, 42.0, 32.0],
            [28.0, 90.0, 56.0],
            [50.0, 90.0, 58.0],
            [288.0, 44.0, 44.0],
            [340.0, 60.0, 82.0],
        ];
        let mut fi = r.pick(o + 7, &[3, 2, 2, 1, 1, 1]);
        if hue_gap(FRUIT[fi][0], h) < 30.0 && (FRUIT[fi][2] - l).abs() < 16.0 {
            fi = (fi + 1) % FRUIT.len();
        }
        p.fruit = hsl(FRUIT[fi][0], FRUIT[fi][1], FRUIT[fi][2]);
        p.fruit_dark = hsl(
            FRUIT[fi][0],
            FRUIT[fi][1] * 0.8,
            max(10.0, FRUIT[fi][2] - 24.0),
        );
    }
    if thorns {
        p.thorn = [
            p.deep,
            p.tone_down,
            if p.dark { rgb(0xe6dcc4) } else { rgb(0xf3ead2) },
        ][r.pick(o + 8, &[3, 2, 2])];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_matches_css_conversion() {
        assert_eq!(hsl(0.0, 100.0, 50.0), rgb(0xff0000));
        assert_eq!(hsl(120.0, 100.0, 25.0), rgb(0x008000));
        assert_eq!(hsl(-120.0, 100.0, 50.0), rgb(0x0000ff));
    }
}
