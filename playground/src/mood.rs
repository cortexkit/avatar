//! Mood morphs for grid 4: the 7-mood wire contract expressed on the new
//! face vocabulary.
//!
//! Approach: moods are LAYERED morphs, not per-archetype redraws — a lid
//! overlay in body color (coverage + angle), on-demand brows (neutral faces
//! have none; mood draws them), a mouth override, an eye scale, and small
//! iconographic adjuncts where they earn their keep (sleepy Zz, working
//! sweat bead). If this layering expresses all 7 on every kept eye archetype,
//! the vocabulary freezes; wherever it fails, the archetype changes, not the
//! contract.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Neutral,
    Working,
    Happy,
    Waiting,
    Blocked,
    Sad,
    Sleepy,
}

pub const ALL: [Mood; 7] = [
    Mood::Neutral,
    Mood::Working,
    Mood::Happy,
    Mood::Waiting,
    Mood::Blocked,
    Mood::Sad,
    Mood::Sleepy,
];

pub fn name(m: Mood) -> &'static str {
    match m {
        Mood::Neutral => "neutral",
        Mood::Working => "working",
        Mood::Happy => "happy",
        Mood::Waiting => "waiting",
        Mood::Blocked => "blocked",
        Mood::Sad => "sad",
        Mood::Sleepy => "sleepy",
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Brows {
    None,
    /// Flat, slightly lowered: concentration.
    Focus,
    /// Raised high arcs: expectation.
    Raised,
    /// Angled inward-down: anger/refusal.
    Knit,
    /// Angled inward-UP: worry/grief.
    Worry,
}

#[derive(Clone, Copy, Debug)]
pub enum MouthMood {
    /// Keep the pack's own mouth.
    Keep,
    /// Wider, more curved smile (or open-happy stays open-happy).
    Grin,
    /// Small o.
    O,
    /// Pressed flat line.
    Pressed,
    /// Downturned arc.
    Down,
}

#[derive(Clone, Copy, Debug)]
pub enum Adjunct {
    None,
    /// Two Z glyphs floating up-right.
    Zz,
    /// One sweat bead on the temple.
    Sweat,
}

pub struct MoodSpec {
    /// Upper-lid coverage of the eye, 0..1.
    pub lid: f64,
    /// Lid rotation in degrees; positive = inner edge LOWER (anger),
    /// negative = inner edge HIGHER (sorrow). Mirrored per side.
    pub lid_angle: f64,
    /// Eye scale multiplier.
    pub eye_scale: f64,
    /// Lower-lid raise 0..1: genuine-smile squint from below.
    pub squint: f64,
    pub brows: Brows,
    pub mouth: MouthMood,
    pub adjunct: Adjunct,
}

pub fn spec(m: Mood) -> MoodSpec {
    match m {
        Mood::Neutral => MoodSpec {
            lid: 0.0, lid_angle: 0.0, eye_scale: 1.0, squint: 0.0,
            brows: Brows::None, mouth: MouthMood::Keep, adjunct: Adjunct::None,
        },
        Mood::Working => MoodSpec {
            lid: 0.22, lid_angle: 4.0, eye_scale: 0.96, squint: 0.1,
            brows: Brows::Focus, mouth: MouthMood::Pressed, adjunct: Adjunct::Sweat,
        },
        Mood::Happy => MoodSpec {
            lid: 0.0, lid_angle: 0.0, eye_scale: 1.04, squint: 0.45,
            brows: Brows::None, mouth: MouthMood::Grin, adjunct: Adjunct::None,
        },
        Mood::Waiting => MoodSpec {
            lid: 0.0, lid_angle: 0.0, eye_scale: 1.16, squint: 0.0,
            brows: Brows::Raised, mouth: MouthMood::O, adjunct: Adjunct::None,
        },
        Mood::Blocked => MoodSpec {
            lid: 0.38, lid_angle: 14.0, eye_scale: 1.0, squint: 0.0,
            brows: Brows::Knit, mouth: MouthMood::Pressed, adjunct: Adjunct::None,
        },
        Mood::Sad => MoodSpec {
            lid: 0.3, lid_angle: -12.0, eye_scale: 0.97, squint: 0.0,
            brows: Brows::Worry, mouth: MouthMood::Down, adjunct: Adjunct::None,
        },
        Mood::Sleepy => MoodSpec {
            lid: 0.72, lid_angle: 0.0, eye_scale: 1.0, squint: 0.0,
            brows: Brows::None, mouth: MouthMood::O, adjunct: Adjunct::Zz,
        },
    }
}

/// Lid overlay for one eye: a body-colored shape covering the top `lid`
/// fraction, rotated by `angle` (sign pre-mirrored by the caller), with a
/// definition stroke along its lower edge.
pub fn lid_overlay(
    x: f64,
    y: f64,
    r: f64,
    lid: f64,
    angle_deg: f64,
    body_fill: &str,
    outline: &str,
) -> String {
    if lid <= 0.01 {
        return String::new();
    }
    // Cover from above: rect bottom edge sits at the coverage line.
    let cover = r * 1.35;
    let bottom = y - r + 2.0 * r * lid;
    format!(
        "<g transform='rotate({angle_deg:.1} {x:.1} {y:.1})'><rect x='{:.1}' y='{:.1}' width='{:.1}' height='{:.1}' fill='{body_fill}'/><line x1='{:.1}' y1='{bottom:.1}' x2='{:.1}' y2='{bottom:.1}' stroke='{outline}' stroke-width='2.2' stroke-linecap='round'/></g>",
        x - cover, bottom - 2.6 * r, cover * 2.0, 2.6 * r,
        x - cover * 0.82, x + cover * 0.82,
    )
}

/// Lower-lid squint overlay: body-colored swell from below (genuine smile).
pub fn squint_overlay(x: f64, y: f64, r: f64, squint: f64, body_fill: &str, outline: &str) -> String {
    if squint <= 0.01 {
        return String::new();
    }
    let top = y + r - 2.0 * r * squint;
    format!(
        "<path d='M{:.1} {:.1} Q{x:.1} {:.1} {:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z' fill='{body_fill}'/><path d='M{:.1} {:.1} Q{x:.1} {:.1} {:.1} {:.1}' stroke='{outline}' stroke-width='2.2' fill='none' stroke-linecap='round'/>",
        x - r * 1.2, y + r * 1.3,
        top - r * 0.4,
        x + r * 1.2, y + r * 1.3,
        x + r * 1.2, y + r * 1.5, x - r * 1.2, y + r * 1.5,
        x - r * 1.05, y + r * 1.05,
        top,
        x + r * 1.05, y + r * 1.05,
    )
}

/// Brow strokes; `side` is -1 left, +1 right.
pub fn brow(kind: Brows, x: f64, y: f64, r: f64, side: f64, ink: &str) -> String {
    let by = y - r * 1.7;
    let w = r * 0.95;
    let (inner_dy, outer_dy, lift) = match kind {
        Brows::None => return String::new(),
        Brows::Focus => (0.1 * r, 0.0, 0.0),
        Brows::Raised => (-0.15 * r, -0.05 * r, -r * 0.5),
        Brows::Knit => (0.55 * r, -0.15 * r, 0.1 * r),
        Brows::Worry => (-0.55 * r, 0.15 * r, 0.0),
    };
    // Inner end faces the face center: left eye (side=-1) inner is +x,
    // right eye (side=+1) inner is -x.
    let (x_inner, x_outer) = (x - side * w, x + side * w);
    format!(
        "<line x1='{:.1}' y1='{:.1}' x2='{:.1}' y2='{:.1}' stroke='{ink}' stroke-width='3.2' stroke-linecap='round'/>",
        x_inner, by + inner_dy + lift, x_outer, by + outer_dy + lift,
    )
}

/// Iconographic adjuncts, drawn near the crown-right.
pub fn adjunct(kind: Adjunct, cx: f64, top_y: f64, scale: f64, ink: &str) -> String {
    match kind {
        Adjunct::None => String::new(),
        // Pushed wide of the crown so tall top-parts (tuft, antennae) never
        // collide with the glyphs (grid-4 polish note).
        Adjunct::Zz => format!(
            "<text x='{:.1}' y='{:.1}' font-family='monospace' font-weight='bold' font-size='{:.0}' fill='{ink}' opacity='0.75'>z</text><text x='{:.1}' y='{:.1}' font-family='monospace' font-weight='bold' font-size='{:.0}' fill='{ink}' opacity='0.55'>Z</text>",
            cx + 68.0 * scale, top_y + 16.0 * scale, 22.0 * scale,
            cx + 86.0 * scale, top_y - 2.0 * scale, 28.0 * scale,
        ),
        Adjunct::Sweat => format!(
            "<path d='M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} Z' fill='#7fb8e8' opacity='0.85'/>",
            cx + 52.0 * scale, top_y + 18.0 * scale,
            cx + 60.0 * scale, top_y + 34.0 * scale,
            cx + 52.0 * scale, top_y + 40.0 * scale,
            cx + 44.0 * scale, top_y + 34.0 * scale,
            cx + 52.0 * scale, top_y + 18.0 * scale,
        ),
    }
}
