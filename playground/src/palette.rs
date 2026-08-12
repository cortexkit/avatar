//! Palette recipes for grid 3: procedural harmony instead of lookup tables.
//!
//! Each recipe maps a seat's base hue to a full color set (body, outline,
//! belly, part accent, blush). Recipes are the future palette-band genome
//! field; the base hue is the per-seat variance.

/// Resolved colors for one creature.
pub struct Colors {
    pub body: String,
    pub outline: String,
    pub belly: String,
    pub part: String,
    pub blush: String,
    /// Eye/mouth ink (near-black on light bodies, warm-light on dark).
    pub ink: String,
    pub name: &'static str,
}

fn hsl(h: f64, s: f64, l: f64) -> String {
    format!("hsl({:.0} {:.0}% {:.0}%)", h.rem_euclid(360.0), s, l)
}

pub const RECIPES: usize = 6;

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
        },
    }
}
