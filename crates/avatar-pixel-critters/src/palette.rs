//! Colour: the body colour creature.wild gives the genome, the ramps built
//! round it, and the single inks of the face and the mood marks.

use avatar_svg::Rgb;

use crate::genes::Reader;
use crate::grid::clampf;

/// Shading levels within a ramp.
pub(crate) const GLINT: u8 = 0;
pub(crate) const LIGHT: u8 = 1;
pub(crate) const BASE: u8 = 2;
pub(crate) const SHADE: u8 = 3;
pub(crate) const DEEP: u8 = 4;

/// Ramps: what a cell is made of.
pub(crate) const BODY: u8 = 0;
pub(crate) const BELLY: u8 = 1;
pub(crate) const MARK: u8 = 2;
pub(crate) const HORN: u8 = 3;
pub(crate) const WING: u8 = 4;
pub(crate) const BEAK: u8 = 5;
pub(crate) const GLOW: u8 = 6;
pub(crate) const POINT: u8 = 7;
const RAMPS: usize = 8;

/// Colour-table indices of the single colours that are not shaded. Index 0
/// is empty, then five steps of each ramp, then these.
pub(crate) const OUTLINE_DARK: u8 = 41;
pub(crate) const OUTLINE_LIT: u8 = 42;
pub(crate) const INNER: u8 = 43;
pub(crate) const WHITE: u8 = 44;
pub(crate) const PUPIL: u8 = 45;
pub(crate) const IRIS: u8 = 46;
pub(crate) const IRIS_LIGHT: u8 = 47;
pub(crate) const SHINE: u8 = 48;
pub(crate) const INK: u8 = 49;
pub(crate) const MOUTH_IN: u8 = 50;
pub(crate) const TONGUE: u8 = 51;
pub(crate) const TEETH: u8 = 52;
pub(crate) const BLUSH: u8 = 53;
pub(crate) const NOSE: u8 = 54;
pub(crate) const SWEAT: u8 = 55;
pub(crate) const ZZ: u8 = 56;
pub(crate) const ANGER: u8 = 57;
pub(crate) const TEAR: u8 = 58;
const TABLE_LEN: usize = 59;

/// The colour-table index of a ramp step.
pub(crate) fn ramp_colour(ramp: u8, level: u8) -> u8 {
    1 + ramp * 5 + level
}

/// creature.wild's palette recipes, by its recipe number: saturation and
/// lightness of the body, then of its outline.
fn recipe(number: u8) -> (&'static str, [f64; 2], [f64; 2]) {
    match number {
        0 => ("pastel", [66.0, 82.0], [42.0, 38.0]),
        2 => ("vivid", [78.0, 64.0], [62.0, 30.0]),
        3 => ("dark", [38.0, 26.0], [30.0, 14.0]),
        _ => ("muted", [24.0, 62.0], [26.0, 30.0]),
    }
}

/// A creature's colours.
#[derive(Clone, Debug)]
pub struct Palette {
    /// creature.wild's palette recipe: 0 pastel, 2 vivid, 3 dark, 4 muted.
    pub recipe: u8,
    /// The body hue in degrees.
    pub hue: f64,
    /// Whether the body is dark, which changes how the face is drawn.
    pub dark: bool,
    /// The body colour: the base step of the body ramp, which is the
    /// creature.wild body colour itself.
    pub body: Rgb,
    /// Index 0 is empty; then each ramp's five steps; then the inks.
    pub(crate) table: [Option<Rgb>; TABLE_LEN],
}

impl Palette {
    pub(crate) fn colour(&self, index: u8) -> Option<Rgb> {
        self.table[index as usize]
    }
}

/// Rust formats hsl() components with `{:.0}`, which rounds ties to even;
/// creature.wild's body colour goes through that, so the ramps do too.
fn round_half_even(x: f64) -> f64 {
    let f = x.floor();
    let d = x - f;
    // Above a half rounds up; exactly a half rounds to the even neighbour.
    if d > 0.5 || (d == 0.5 && f % 2.0 != 0.0) {
        f + 1.0
    } else {
        f
    }
}

fn rnd(v: f64) -> f64 {
    crate::grid::rnd(v)
}

/// An HSL colour as 8-bit RGB, with each component rounded first as
/// creature.wild's formatting rounds it.
fn hsl(h: f64, s: f64, l: f64) -> Rgb {
    let hue = round_half_even(((h % 360.0) + 360.0) % 360.0) % 360.0;
    let sat = round_half_even(clampf(s, 0.0, 100.0)) / 100.0;
    let light = round_half_even(clampf(l, 0.0, 100.0)) / 100.0;
    let chroma = (1.0 - (2.0 * light - 1.0).abs()) * sat;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
    let (r, g, b) = match sector.floor() as i32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let off = light - chroma / 2.0;
    let channel = |c: f64| clampf(rnd((c + off) * 255.0), 0.0, 255.0) as u8;
    Rgb {
        r: channel(r),
        g: channel(g),
        b: channel(b),
    }
}

/// The signed way round the colour wheel from hue `a` to hue `b`, -180..180.
fn hue_to(a: f64, b: f64) -> f64 {
    ((((b - a) % 360.0) + 540.0) % 360.0) - 180.0
}

/// Hue `h` moved towards hue `target` by at most `amount` degrees.
fn hue_toward(h: f64, target: f64, amount: f64) -> f64 {
    h + clampf(hue_to(h, target), -amount, amount)
}

const fn hex(v: u32) -> Rgb {
    Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

/// Five steps of one colour: glint, light, base, shade, deep. `d` gives the
/// lightness steps for glint, light, shade and deep, and `turn` how far the
/// hue turns per step, towards yellow in the light and blue in the shade.
fn ramp_of(h: f64, s: f64, l: f64, d: [f64; 4], turn: f64) -> [Rgb; 5] {
    let warm = |k: f64| hue_toward(h, 70.0, turn * k);
    let cool = |k: f64| hue_toward(h, 250.0, turn * k);
    // In HSL one saturation gives a far stronger colour at middle lightness
    // than near white, so the shaded steps take the saturation that makes
    // them `k` times as colourful as the base, no more.
    let strength = |v: f64| 1.0 - (2.0 * v / 100.0 - 1.0).abs();
    let sat = |l2: f64, k: f64| {
        clampf(
            k * s * strength(l) / (0.05f64).max(strength(l2)),
            s.min(8.0),
            (96f64).min(s + 2.0),
        )
    };
    let l3 = (6f64).max(l - d[2]);
    let l4 = (4f64).max(l - d[3]);
    [
        hsl(warm(2.0), (8f64).max(s - 8.0), (97f64).min(l + d[0])),
        hsl(warm(1.0), (8f64).max(s - 3.0), (95f64).min(l + d[1])),
        hsl(h, s, l),
        hsl(cool(1.0), sat(l3, 1.3), l3),
        hsl(cool(2.0), sat(l4, 1.6), l4),
    ]
}

/// Builds the palette, reading the body colour exactly as creature.wild
/// reads it (bytes 160 to 162) and the rest from the colour genes.
pub(crate) fn build_palette(reader: &mut Reader) -> Palette {
    reader.at(160);
    // creature.wild takes byte 160 modulo 8, so only its three low bits count.
    let rsel = reader.bits(3);
    reader.at(161);
    let b161 = f64::from(reader.bits(8));
    let b162 = f64::from(reader.bits(8));
    let number: u8 = match rsel {
        0..=2 => 0,
        3..=4 => 2,
        5..=6 => 4,
        _ => 3,
    };
    let h = (b161 / 255.0 + b162 / 65535.0) * 360.0;
    let dark = number == 3;
    let (_, [s, l], [so, lo]) = recipe(number);

    reader.at(crate::genes::DNA_COLOUR);
    // How strongly the body is shaded, and how far the hue turns per step.
    let contrast = [0.82, 1.0, 1.16, 1.32][reader.bits(2) as usize];
    let turn = [3.0, 7.0, 11.0, 15.0][reader.bits(2) as usize];
    let d = if dark {
        [15.0, 8.0, 6.0, 10.0]
    } else {
        [13.0, 8.0, 11.0, 21.0]
    }
    .map(|v: f64| v * contrast);
    let mut ramps = [[Rgb { r: 0, g: 0, b: 0 }; 5]; RAMPS];
    ramps[BODY as usize] = ramp_of(h, s, l, d, turn);
    let body = ramps[BODY as usize][BASE as usize];
    let cool_h = hue_toward(h, 250.0, turn * 2.0);
    let outline_lit = hsl(h, so, lo);
    let outline_dark = hsl(
        cool_h,
        (90f64).min(so + 4.0),
        (4f64).max(lo - if dark { 6.0 } else { 10.0 }),
    );
    let inner = hsl(
        hue_toward(h, 250.0, turn),
        (s + so) / 2.0,
        if dark {
            lo - 2.0
        } else {
            lo + if number == 0 { 9.0 } else { 6.0 }
        },
    );

    // The underside and the muzzle: a lighter shade of the body, never white.
    let belly_kind = reader.bits(2) as usize;
    ramps[BELLY as usize] = if dark {
        let turn_to_45 = if hue_to(h, 45.0) > 0.0 { 14.0 } else { -14.0 };
        ramp_of(
            h + [6.0, 0.0, turn_to_45, 20.0][belly_kind],
            [42.0, 30.0, 36.0, 40.0][belly_kind],
            [38.0, 43.0, 40.0, 40.0][belly_kind],
            [10.0, 6.0, 7.0, 13.0],
            turn,
        )
    } else {
        ramp_of(
            [h, h, hue_toward(h, 45.0, 16.0), h + 18.0][belly_kind],
            s * [0.85, 0.6, 0.8, 0.8][belly_kind],
            (93f64).min(l + [8.0, 11.0, 8.0, 8.0][belly_kind]),
            [5.0, 3.0, 9.0, 17.0],
            turn,
        )
    };

    // Markings: a darker or a lighter step of the body, on its hue or a
    // little beside it. A spot or patch turned further from the body's hue
    // reads as something stuck on or carried (a sticker, a bag), not coat.
    let mark_kind = reader.bits(2) as usize;
    let mh = h + [0.0, 10.0, -10.0, 0.0][mark_kind];
    let darker_l = if dark {
        l + 13.0
    } else {
        l - if number == 0 { 20.0 } else { 15.0 }
    };
    ramps[MARK as usize] = if mark_kind == 3 {
        ramp_of(
            mh,
            (10f64).max(s - 6.0),
            if dark {
                l + 16.0
            } else {
                (94f64).min(l + 10.0)
            },
            [8.0, 5.0, 8.0, 15.0],
            turn,
        )
    } else {
        ramp_of(mh, (92f64).min(s + 5.0), darker_l, d, turn)
    };
    // Ears and a tail darker than the body take the marking's step on the
    // body's own hue: in a hue of their own, ears read as a ribbon or
    // earmuffs and a tail as a bag.
    ramps[POINT as usize] = if mark_kind == 1 || mark_kind == 2 {
        ramp_of(h, (92f64).min(s + 5.0), darker_l, d, turn)
    } else {
        ramps[MARK as usize]
    };

    // Horn, claw and tooth: bone, dark horn, or a step of the body.
    let horn_kind = reader.bits(2);
    ramps[HORN as usize] = match horn_kind {
        0 => ramp_of(
            44.0,
            38.0,
            if dark { 78.0 } else { 88.0 },
            [8.0, 5.0, 13.0, 26.0],
            6.0,
        ),
        1 => ramp_of(
            h,
            16.0,
            if dark { 46.0 } else { 30.0 },
            [22.0, 12.0, 9.0, 16.0],
            4.0,
        ),
        2 => ramp_of(
            h,
            (10f64).max(s - 12.0),
            if dark { l + 24.0 } else { (94f64).min(l + 9.0) },
            [6.0, 4.0, 10.0, 20.0],
            turn,
        ),
        _ => ramp_of(
            h,
            (92f64).min(s + 4.0),
            if dark { l + 12.0 } else { l - 24.0 },
            [16.0, 9.0, 8.0, 15.0],
            turn,
        ),
    };
    // Wing membranes and fins: one step into the shade of the body.
    ramps[WING as usize] = ramp_of(
        hue_toward(h, 250.0, turn),
        (94f64).min(s + 2.0),
        if dark { l + 9.0 } else { l - 10.0 },
        d,
        turn,
    );
    ramps[BEAK as usize] = ramp_of(38.0, 92.0, 60.0, [18.0, 10.0, 11.0, 22.0], 8.0);

    let white = if dark { hex(0xf6f1e9) } else { hex(0xffffff) };
    // On a dark body the pupil is near black: a shade of the body would read
    // as the body showing through the eye.
    let pupil = if dark {
        hsl(h, 30.0, 7.0)
    } else if number == 4 {
        hex(0x2b2622)
    } else {
        hex(0x241f26)
    };
    // Line work is dark on a light body and a pale tint on a dark one.
    let ink = if dark {
        hsl(h, 30.0, 70.0)
    } else if number == 4 {
        hex(0x2b2622)
    } else {
        hex(0x241f26)
    };
    // Sixteen iris hues from amber round to violet, moved off the body's own
    // hue so that the eye stands out.
    let mut ih = 34.0 + f64::from(reader.bits(4)) * 19.0;
    if hue_to(ih, h).abs() < 24.0 {
        ih = if ih + 120.0 > 325.0 {
            190.0
        } else {
            ih + 120.0
        };
    }
    let iris = hsl(ih, 72.0, if dark { 56.0 } else { 46.0 });
    let iris_light = hsl(ih + 8.0, 78.0, if dark { 70.0 } else { 64.0 });
    // What glows: the knob of an antenna, in the eye's colour.
    ramps[GLOW as usize] = ramp_of(ih, 90.0, 66.0, [16.0, 9.0, 12.0, 22.0], 6.0);
    let mouth_in = if dark { hex(0x8f3d4b) } else { hex(0x4a2a31) };
    let nose = if dark {
        hex(0xe7b3b3)
    } else {
        hsl(
            hue_toward(h, 350.0, 40.0),
            44.0,
            if number == 0 { 42.0 } else { 32.0 },
        )
    };
    let blush = if dark {
        hsl(h + 14.0, 60.0, 50.0)
    } else if number == 0 {
        hsl(h + 8.0, 78.0, 70.0)
    } else if number == 2 {
        hsl(h - 12.0, 88.0, 74.0)
    } else {
        hsl(h + 20.0, 46.0, 52.0)
    };

    let mut table = [None; TABLE_LEN];
    for (r, ramp) in ramps.iter().enumerate() {
        for (k, &colour) in ramp.iter().enumerate() {
            table[1 + r * 5 + k] = Some(colour);
        }
    }
    let inks = [
        outline_dark,
        outline_lit,
        inner,
        white,
        pupil,
        iris,
        iris_light,
        hex(0xffffff),
        ink,
        mouth_in,
        hex(0xee8a98),
        hex(0xfffaf0),
        blush,
        nose,
        hex(0x7fd0f5),
        if dark { hex(0x6f6a86) } else { hex(0x5d5878) },
        hex(0xe2473f),
        hex(0x6fc2f0),
    ];
    for (i, colour) in inks.into_iter().enumerate() {
        table[OUTLINE_DARK as usize + i] = Some(colour);
    }
    Palette {
        recipe: number,
        hue: h,
        dark,
        body,
        table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_rounds_components_half_to_even_before_converting() {
        // 0.5 rounds to 0 and 1.5 to 2: a pure red at 50% lightness.
        assert_eq!(hsl(0.5, 100.0, 50.0), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(hsl(120.0, 100.0, 25.0), Rgb { r: 0, g: 128, b: 0 });
    }

    #[test]
    fn hue_to_takes_the_short_way_round() {
        assert_eq!(hue_to(350.0, 10.0), 20.0);
        assert_eq!(hue_to(10.0, 350.0), -20.0);
    }
}
