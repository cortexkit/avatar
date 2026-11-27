//! The palette: wild's recipe and hue, nudged, with a second colour picked
//! by a colour scheme, an accent, bone and the shades the parts take.

use avatar_svg::Rgb;

use crate::genes::{Reader, Wild};
use crate::geom::{clamp, max, min};

/// Rust's `{:.0}` rounding (ties to even), which the wild palette's hsl()
/// text went through on its way to 8-bit colour.
fn round_half_even(x: f64) -> f64 {
    let f = x.floor();
    let d = x - f;
    if d > 0.5 {
        return f + 1.0;
    }
    if d < 0.5 {
        return f;
    }
    if f % 2.0 == 0.0 {
        f
    } else {
        f + 1.0
    }
}

/// An hsl() colour as 8-bit RGB, as the wild palette resolves it.
pub fn hsl(h: f64, s: f64, l: f64) -> Rgb {
    let hh = round_half_even(((h % 360.0) + 360.0) % 360.0);
    let ss = round_half_even(clamp(s, 0.0, 100.0)) / 100.0;
    let ll = round_half_even(clamp(l, 0.0, 100.0)) / 100.0;
    let chroma = (1.0 - (2.0 * ll - 1.0).abs()) * ss;
    let sector = (((hh % 360.0) + 360.0) % 360.0) / 60.0;
    let second = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
    let rgb = match sector.floor() as i64 {
        0 => [chroma, second, 0.0],
        1 => [second, chroma, 0.0],
        2 => [0.0, chroma, second],
        3 => [0.0, second, chroma],
        4 => [second, 0.0, chroma],
        _ => [chroma, 0.0, second],
    };
    let off = ll - chroma / 2.0;
    let c = |v: f64| clamp(((v + off) * 255.0).round(), 0.0, 255.0) as u8;
    Rgb {
        r: c(rgb[0]),
        g: c(rgb[1]),
        b: c(rgb[2]),
    }
}

pub const fn hex(v: u32) -> Rgb {
    Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

fn warm_shift(h: f64) -> f64 {
    let d = ((45.0 - ((h % 360.0) + 360.0) % 360.0 + 540.0) % 360.0) - 180.0;
    clamp(d, -30.0, 30.0)
}

/// Every colour a creature is drawn with, by role.
#[derive(Clone)]
pub struct Palette {
    pub recipe: u32,
    pub scheme: usize,
    pub dark: bool,
    pub body: Rgb,
    pub outline: Rgb,
    pub part: Rgb,
    pub part2: Rgb,
    pub tone_up: Rgb,
    pub tone_down: Rgb,
    pub light: Rgb,
    pub dark_c: Rgb,
    pub limb_mark: Rgb,
    pub lift: Rgb,
    pub lift_pale: Rgb,
    pub accent: Rgb,
    pub bone: Rgb,
    pub blush: Rgb,
    pub blush_alpha: f64,
    pub ink: Rgb,
    pub pupil: Rgb,
    pub sclera: Option<Rgb>,
    pub white: Rgb,
    pub iris: Rgb,
    pub iris_pale: Rgb,
    pub tone_rim: Rgb,
    pub shadow: Rgb,
    pub mouth: Rgb,
    pub mouth_inside: Rgb,
    pub tongue: Rgb,
    pub belly: Rgb,
    pub mottle: [(Rgb, f64); 2],
    pub mark: Rgb,
    pub mark_deep: Rgb,
    pub mark_pale: Rgb,
    /// What a torso or other mass below the head is filled with; the body
    /// colour unless the zones say otherwise.
    pub torso: Rgb,
}

impl Palette {
    /// The colour a role name stands for, or a literal `#rrggbb`.
    pub fn ink(&self, role: &str) -> Rgb {
        match role {
            "body" => self.body,
            "outline" => self.outline,
            "part" => self.part,
            "part2" => self.part2,
            "toneUp" => self.tone_up,
            "toneDown" => self.tone_down,
            "light" => self.light,
            "dark" => self.dark_c,
            "limbMark" => self.limb_mark,
            "lift" => self.lift,
            "liftPale" => self.lift_pale,
            "accent" => self.accent,
            "bone" => self.bone,
            "blush" => self.blush,
            "ink" => self.ink,
            "pupil" => self.pupil,
            "white" => self.white,
            "iris" => self.iris,
            "irisPale" => self.iris_pale,
            "toneRim" => self.tone_rim,
            "shadow" => self.shadow,
            "mouth" => self.mouth,
            "mouthInside" => self.mouth_inside,
            "tongue" => self.tongue,
            "belly" => self.belly,
            "mark" => self.mark,
            "markDeep" => self.mark_deep,
            "markPale" => self.mark_pale,
            "torso" => self.torso,
            "sclera" => self.sclera.unwrap_or(self.white),
            _ => {
                let h = role
                    .strip_prefix('#')
                    .unwrap_or_else(|| panic!("unknown palette role {role}"));
                hex(u32::from_str_radix(h, 16).expect("hex colour"))
            }
        }
    }
}

pub const SCHEMES: [&str; 8] = [
    "kin",
    "counter-kin",
    "complement",
    "split warm",
    "split cool",
    "triad",
    "counter-triad",
    "one hue",
];

pub fn build_palette(r: &Reader, w: &Wild) -> Palette {
    let h = w.hue;
    let recipe = w.recipe;
    let dark = recipe == 3;
    let (s0, l0) = match recipe {
        0 => (66.0, 82.0),
        2 => (78.0, 64.0),
        3 => (38.0, 26.0),
        _ => (24.0, 62.0),
    };
    // The body keeps wild's recipe and hue, nudged in lightness and saturation.
    let dl = if dark {
        r.lin(1824, -3.0, 8.0)
    } else {
        r.lin(1824, -9.0, 6.0)
    };
    let ds = r.lin(1825, -16.0, 14.0);
    let belly_shift = r.lin(1832, -14.0, 14.0);
    let mark_shift = r.lin(1833, -18.0, 18.0);
    let blush_shift = r.lin(1834, -10.0, 10.0);
    let accent_lift = r.lin(1835, -6.0, 6.0);
    let s = s0 + ds;
    let l = l0 + dl;
    // Where wild puts its second colour, by recipe: hue offset, saturation,
    // lightness.
    let (kin, sp, lp): (f64, f64, f64) = match recipe {
        0 => (28.0, 55.0, 62.0),
        2 => (40.0, 80.0, 55.0),
        3 => (24.0, 45.0, 44.0),
        _ => (-26.0, 30.0, 48.0),
    };
    let scheme = r.pick(1826, &[4.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
    let off = [kin, -kin * 1.3, 180.0, 150.0, 210.0, 120.0, -120.0, 0.0][scheme];
    let h2 = h + off;
    let accent_pick = r.mod_(1827, 6) as usize;
    let ha = [h + 180.0, h2 + 180.0, h + 90.0, h - 90.0, 32.0, 196.0][accent_pick];
    let bone_pick = r.mod_(1829, 3);
    let outline = if dark {
        hsl(h, 30.0, 14.0)
    } else if recipe == 0 {
        hsl(h, 42.0, 38.0)
    } else if recipe == 2 {
        hsl(h, 62.0, 30.0)
    } else {
        hsl(h, 26.0, 30.0)
    };
    let body = hsl(h, s, l);
    let warm = h + warm_shift(h);
    let v = w.belly_variant;
    let hb = h + belly_shift;
    let belly = if dark {
        [
            hsl(hb + 6.0, 45.0, 48.0),
            hsl(hb, 40.0, 38.0),
            hsl(warm + belly_shift, 30.0, 55.0),
            hsl(hb + 24.0, 42.0, 50.0),
        ][v as usize]
    } else {
        [
            hsl(hb, s0 * 0.66, min(94.0, l0 + 13.0)),
            hsl(hb, s0 * 0.82, min(94.0, l0 + 9.0)),
            hsl(warm + belly_shift, s0 * 0.6, min(94.0, l0 + 11.0)),
            hsl(hb + 28.0, s0 * 0.62, min(92.0, l0 + 8.0)),
        ][v as usize]
    };
    let mottle = if recipe == 0 {
        [
            (hsl(h, 70.0, 90.0), 0.55),
            (hsl(h + 10.0, 55.0, 68.0), 0.35),
        ]
    } else if recipe == 2 {
        [(hsl(h, 82.0, 76.0), 0.5), (hsl(h + 14.0, 70.0, 52.0), 0.35)]
    } else if dark {
        [(hsl(h, 42.0, 38.0), 0.5), (hsl(h - 8.0, 40.0, 18.0), 0.45)]
    } else {
        [(hsl(h, 28.0, 72.0), 0.5), (hsl(h - 12.0, 26.0, 50.0), 0.35)]
    };
    Palette {
        recipe,
        scheme,
        dark,
        body,
        outline,
        part: if scheme == 7 {
            hsl(h, sp, lp - 6.0)
        } else {
            hsl(h2, sp, lp)
        },
        part2: hsl(
            h2,
            sp * 0.85,
            min(90.0, lp + if dark { 16.0 } else { 18.0 }),
        ),
        tone_up: hsl(h, max(12.0, s0 - 2.7), min(94.0, l0 + 9.0)),
        tone_down: hsl(h, max(12.0, s0 - 3.3), max(10.0, l0 - 11.0)),
        light: if dark {
            hsl(h, 32.0, 54.0)
        } else {
            hsl(h, s * 0.55, min(95.0, l + 13.0))
        },
        dark_c: if dark {
            hsl(h, 34.0, 12.0)
        } else {
            hsl(h, min(70.0, s * 0.85), max(18.0, l - 30.0))
        },
        limb_mark: if dark {
            hsl(h, 34.0, 15.0)
        } else {
            hsl(h, s * 0.9, l - 16.0)
        },
        lift: hsl(h, s, l + 9.0),
        lift_pale: hsl(h, s * 0.85, l + 18.0),
        accent: if dark {
            hsl(ha, 82.0, 62.0 + accent_lift)
        } else if recipe == 4 {
            hsl(ha, 62.0, 54.0 + accent_lift)
        } else {
            hsl(ha, 84.0, 58.0 + accent_lift)
        },
        bone: match bone_pick {
            0 => {
                if dark {
                    hex(0xe9e2d0)
                } else {
                    hex(0xf6efdd)
                }
            }
            1 => {
                if dark {
                    hsl(h, 12.0, 70.0)
                } else {
                    hsl(h, 14.0, 84.0)
                }
            }
            _ => {
                if dark {
                    hsl(h, 16.0, 9.0)
                } else {
                    hsl(h, 24.0, 24.0)
                }
            }
        },
        blush: if dark {
            hsl(h + 14.0 + blush_shift, 60.0, 55.0)
        } else if recipe == 0 {
            hsl(h + 8.0 + blush_shift, 75.0, 72.0)
        } else if recipe == 2 {
            hsl(h - 12.0 + blush_shift, 85.0, 70.0)
        } else {
            hsl(h + 20.0 + blush_shift, 35.0, 58.0)
        },
        blush_alpha: if dark { 0.32 } else { 0.8 },
        ink: if dark {
            hex(0xf4eee6)
        } else if recipe == 4 {
            hex(0x2b2622)
        } else {
            hex(0x241f26)
        },
        pupil: if dark {
            hsl(h, 32.0, 12.0)
        } else if recipe == 4 {
            hex(0x2b2622)
        } else {
            hex(0x241f26)
        },
        sclera: dark.then_some(hex(0xf6f1e9)),
        white: if dark { hex(0xf6f1e9) } else { hex(0xffffff) },
        iris: hsl(
            ha,
            if dark { 75.0 } else { 70.0 },
            if dark { 58.0 } else { 50.0 },
        ),
        iris_pale: hsl(ha, 72.0, 74.0),
        tone_rim: if recipe == 0 {
            hsl(h, 38.0, 52.0)
        } else if recipe == 2 {
            hsl(h, 55.0, 40.0)
        } else {
            hsl(h, 24.0, 40.0)
        },
        shadow: hex(0x281e32),
        // A mouth drawn in wild's outline colour vanishes on the dark
        // recipe, so there the mouth takes the light ink the brows use.
        mouth: if dark { hex(0xf4eee6) } else { outline },
        mouth_inside: hex(0x4a2f33),
        tongue: hex(0xe88a93),
        belly,
        mottle,
        mark: if dark {
            hsl(h2 + mark_shift, 50.0, 46.0)
        } else {
            hsl(h2 + mark_shift, min(85.0, sp + 8.0), max(30.0, lp - 6.0))
        },
        mark_deep: if dark {
            hsl(h, 36.0, 13.0)
        } else {
            hsl(h, min(72.0, s * 0.9), max(20.0, l - 24.0))
        },
        mark_pale: if dark {
            hsl(h, 30.0, 44.0)
        } else {
            hsl(h, s * 0.5, min(96.0, l + 11.0))
        },
        torso: body,
    }
}
