//! Procedural palette recipes instead of lookup tables.
//!
//! Each recipe maps a seat's base hue to a full color set (body, outline,
//! belly, part accent, blush). The palette region selects the recipe and hue.

/// Resolved colors for one creature.
pub struct Colors {
    pub body: String,
    pub outline: String,
    pub belly: String,
    pub part: String,
    pub blush: String,
    /// LINE ink: mouth strokes, brows, closed-eye arcs. Flips light on dark
    /// bodies so line-work keeps contrast.
    pub ink: String,
    /// Pupil fill — always dark. Pupils are not line-work: a light "pupil"
    /// on a dark body reads as a blank ghost orb.
    pub pupil: String,
    /// Light sclera backing drawn behind pupils on dark bodies only; None
    /// on light bodies where the body itself is the backdrop.
    pub sclera: Option<String>,
    pub name: &'static str,
}

fn hsl(h: f64, s: f64, l: f64) -> String {
    format!("hsl({:.0} {:.0}% {:.0}%)", h.rem_euclid(360.0), s, l)
}

/// Voronoi mottle fills (light, dark) keyed to the recipe: same-hue shade
/// steps of the body color, translucent so the body tone stays dominant.
/// A fixed black/white overlay read as cracked glass — mottle must
/// belong to the creature's own palette.
pub fn mottle(recipe: usize, h: f64) -> (String, String) {
    let a = |hh: f64, s: f64, l: f64, alpha: f64| {
        format!("hsl({:.0} {s:.0}% {l:.0}% / {alpha})", hh.rem_euclid(360.0))
    };
    match recipe {
        0 => (a(h, 70.0, 90.0, 0.55), a(h + 10.0, 55.0, 68.0, 0.35)),
        1 => (a(h, 66.0, 90.0, 0.55), a(h + 12.0, 52.0, 70.0, 0.35)),
        2 => (a(h, 82.0, 76.0, 0.5), a(h + 14.0, 70.0, 52.0, 0.35)),
        3 => (a(h, 42.0, 38.0, 0.5), a(h - 8.0, 40.0, 18.0, 0.45)),
        4 => (a(h, 28.0, 72.0, 0.5), a(h - 12.0, 26.0, 50.0, 0.35)),
        _ => (a(h, 30.0, 94.0, 0.55), a(h + 8.0, 26.0, 76.0, 0.35)),
    }
}

/// Belly-patch fill: OPAQUE by design — the translucent bellies let the
/// voronoi mottle bleed through, and an opaque belly is immune to whatever
/// pattern lives on the hide. `variant` is genome-driven:
/// 0 = classic near-white, 1 = lighter same-hue tone, 2 = warm cream,
/// 3 = soft accent tint. The dark recipe keeps its luminous register.
pub fn belly_fill(recipe: usize, h: f64, variant: u8) -> String {
    // Every variant stays anchored to the creature's own hue. A fixed warm
    // hue read as a foreign patch on cool bodies — a near-white cream blob
    // on a green creature looks like a hole punched in it — so
    // "warm" is now a bounded shift of the body hue rather than a constant.
    let warm = h + warm_shift(h);
    if recipe == 3 {
        return match variant % 4 {
            0 => hsl(h + 6.0, 45.0, 48.0),
            1 => hsl(h, 40.0, 38.0),
            2 => hsl(warm, 30.0, 55.0),
            _ => hsl(h + 24.0, 42.0, 50.0),
        };
    }
    let (s, l): (f64, f64) = match recipe {
        0 => (66.0, 82.0),
        1 => (62.0, 83.0),
        2 => (78.0, 64.0),
        4 => (24.0, 62.0),
        _ => (28.0, 88.0),
    };
    // Lightness is a LIFT of the body tone, never an absolute near-white.
    // A fixed 93% lightness is a gentle highlight on a pale body and a stark
    // hole on a mid-tone one (seed 5033: white patch on mid-green read as a
    // puncture). Relative lift keeps the belly reading as the creature's own
    // underside at every body value.
    // Saturation stays near the body's too. Lightness alone was not enough:
    // a strongly desaturated patch reads as grey-white however modest its
    // lift, which is the other half of why a belly can look like a hole
    // rather than an underside.
    match variant % 4 {
        0 => hsl(h, s * 0.66, (l + 13.0).min(94.0)),
        1 => hsl(h, s * 0.82, (l + 9.0).min(94.0)),
        2 => hsl(warm, s * 0.6, (l + 11.0).min(94.0)),
        _ => hsl(h + 28.0, s * 0.62, (l + 8.0).min(92.0)),
    }
}

/// Bounded hue nudge toward warm (45 deg), along the shorter arc and capped
/// at 30 deg. Warmth becomes a family resemblance instead of a jump to a
/// hue the creature does not otherwise wear.
fn warm_shift(h: f64) -> f64 {
    let delta = (45.0 - h.rem_euclid(360.0) + 540.0).rem_euclid(360.0) - 180.0;
    delta.clamp(-30.0, 30.0)
}

/// Tone variant of the body color: same hue, one shade step lighter
/// (`dir` > 0) or darker (`dir` < 0). Extremity layer-color experiment B:
/// variation without leaving the creature's own color.
pub fn tone(recipe: usize, h: f64, dir: i32) -> String {
    let (s, l) = match recipe {
        0 => (66.0, 82.0),
        1 => (62.0, 83.0),
        2 => (78.0, 64.0),
        3 => (38.0, 26.0),
        4 => (24.0, 62.0),
        _ => (28.0, 88.0),
    };
    let dl: f64 = if dir > 0 { 9.0 } else { -11.0 };
    hsl(
        h,
        (s - dl.abs() * 0.3).max(12.0),
        (l + dl).clamp(10.0, 94.0),
    )
}

/// recipe index x base hue -> colors.
pub fn resolve(recipe: usize, h: f64) -> Colors {
    match recipe {
        // Pastel analogous: soft body, +28deg part accent, gentle outline.
        0 => Colors {
            name: "pastel analogous",
            body: hsl(h, 66.0, 82.0),
            outline: hsl(h, 42.0, 38.0),
            belly: "rgba(255,255,255,0.45)".into(),
            part: hsl(h + 28.0, 55.0, 62.0),
            blush: hsl(h + 8.0, 75.0, 72.0),
            ink: "#241f26".into(),
            pupil: "#241f26".into(),
            sclera: None,
        },
        // Pastel + complementary accent: parts pop against the body.
        1 => Colors {
            name: "pastel complement",
            body: hsl(h, 62.0, 83.0),
            outline: hsl(h, 40.0, 36.0),
            belly: "rgba(255,255,255,0.45)".into(),
            part: hsl(h + 180.0, 52.0, 58.0),
            blush: hsl(h + 170.0, 60.0, 74.0),
            ink: "#241f26".into(),
            pupil: "#241f26".into(),
            sclera: None,
        },
        // Toy-saturated: candy chroma, white belly, darker same-hue outline.
        2 => Colors {
            name: "toy saturated",
            body: hsl(h, 78.0, 64.0),
            outline: hsl(h, 62.0, 30.0),
            belly: "rgba(255,255,255,0.6)".into(),
            part: hsl(h + 40.0, 80.0, 55.0),
            blush: hsl(h - 12.0, 85.0, 70.0),
            ink: "#241f26".into(),
            pupil: "#241f26".into(),
            sclera: None,
        },
        // Dark gravitas band: deep body, luminous belly, ink flips light.
        3 => Colors {
            name: "dark gravitas",
            body: hsl(h, 38.0, 26.0),
            outline: hsl(h, 30.0, 14.0),
            belly: format!("hsl({:.0} 45% 42% / 0.55)", (h + 6.0).rem_euclid(360.0)),
            part: hsl(h + 24.0, 45.0, 44.0),
            blush: format!("hsl({:.0} 60% 55% / 0.4)", (h + 14.0).rem_euclid(360.0)),
            ink: "#f4eee6".into(),
            pupil: hsl(h, 32.0, 12.0),
            sclera: Some("#f6f1e9".into()),
        },
        // Muted earthy: desaturated ugly-cute swamp band.
        4 => Colors {
            name: "muted earthy",
            body: hsl(h, 24.0, 62.0),
            outline: hsl(h, 26.0, 30.0),
            belly: "rgba(250,246,235,0.5)".into(),
            part: hsl(h - 26.0, 30.0, 48.0),
            blush: hsl(h + 20.0, 35.0, 58.0),
            ink: "#2b2622".into(),
            pupil: "#2b2622".into(),
            sclera: None,
        },
        // Cream body + colorful parts: identity moves to the accents.
        _ => Colors {
            name: "cream + accents",
            body: hsl(h, 28.0, 88.0),
            outline: hsl(h, 24.0, 42.0),
            belly: "rgba(255,255,255,0.55)".into(),
            part: hsl(h + 140.0, 62.0, 55.0),
            blush: hsl(h + 150.0, 70.0, 74.0),
            ink: "#241f26".into(),
            pupil: "#241f26".into(),
            sclera: None,
        },
    }
}
