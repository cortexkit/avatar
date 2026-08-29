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

use avatar_svg::{Primitive, StrokeCap, StrokeJoin};

use crate::render;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
            lid: 0.0,
            lid_angle: 0.0,
            eye_scale: 1.0,
            squint: 0.0,
            brows: Brows::None,
            mouth: MouthMood::Keep,
            adjunct: Adjunct::None,
        },
        Mood::Working => MoodSpec {
            lid: 0.22,
            lid_angle: 4.0,
            eye_scale: 0.96,
            squint: 0.1,
            brows: Brows::Focus,
            mouth: MouthMood::Pressed,
            adjunct: Adjunct::Sweat,
        },
        Mood::Happy => MoodSpec {
            lid: 0.0,
            lid_angle: 0.0,
            eye_scale: 1.04,
            squint: 0.45,
            brows: Brows::None,
            mouth: MouthMood::Grin,
            adjunct: Adjunct::None,
        },
        Mood::Waiting => MoodSpec {
            lid: 0.0,
            lid_angle: 0.0,
            eye_scale: 1.16,
            squint: 0.0,
            brows: Brows::Raised,
            mouth: MouthMood::O,
            adjunct: Adjunct::None,
        },
        Mood::Blocked => MoodSpec {
            lid: 0.38,
            lid_angle: 14.0,
            eye_scale: 1.0,
            squint: 0.0,
            brows: Brows::Knit,
            mouth: MouthMood::Pressed,
            adjunct: Adjunct::None,
        },
        Mood::Sad => MoodSpec {
            lid: 0.3,
            lid_angle: -12.0,
            eye_scale: 0.97,
            squint: 0.0,
            brows: Brows::Worry,
            mouth: MouthMood::Down,
            adjunct: Adjunct::None,
        },
        Mood::Sleepy => MoodSpec {
            lid: 0.72,
            lid_angle: 0.0,
            eye_scale: 1.0,
            squint: 0.0,
            brows: Brows::None,
            mouth: MouthMood::O,
            adjunct: Adjunct::Zz,
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
) -> Vec<Primitive> {
    if lid <= 0.01 {
        return Vec::new();
    }
    let cover = r * 1.35;
    let bottom = y - r + 2.0 * r * lid;
    let transform = render::transform(vec![render::rotate(angle_deg, x, y)]);
    let mut primitives = vec![
        render::filled(
            render::rect(x - cover, bottom - 2.6 * r, cover * 2.0, 2.6 * r),
            body_fill,
            1.0,
            None,
            "wild-mood-lid-cover",
        ),
        render::stroked(
            render::path(
                vec![
                    render::move_to(x - cover * 0.82, bottom),
                    render::line_to(x + cover * 0.82, bottom),
                ],
                false,
            ),
            render::stroke(outline, 2.2, StrokeCap::Round, StrokeJoin::Miter),
            "wild-mood-lid-line",
        ),
    ];
    for primitive in &mut primitives {
        primitive.transform = Some(transform.clone());
    }
    primitives
}

/// Lower-lid squint overlay: body-colored swell from below (genuine smile).
pub fn squint_overlay(
    x: f64,
    y: f64,
    r: f64,
    squint: f64,
    body_fill: &str,
    outline: &str,
) -> Vec<Primitive> {
    if squint <= 0.01 {
        return Vec::new();
    }
    let top = y + r - 2.0 * r * squint;
    let start = (x - r * 1.2, y + r * 1.3);
    let right = (x + r * 1.2, y + r * 1.3);
    let mut cover = vec![
        render::move_to(start.0, start.1),
        render::quadratic_to(start, (x, top - r * 0.4), right),
    ];
    cover.push(render::line_to(x + r * 1.2, y + r * 1.5));
    cover.push(render::line_to(x - r * 1.2, y + r * 1.5));
    let line_start = (x - r * 1.05, y + r * 1.05);
    let line_end = (x + r * 1.05, y + r * 1.05);
    vec![
        render::filled(
            render::path(cover, true),
            body_fill,
            1.0,
            None,
            "wild-mood-squint-cover",
        ),
        render::stroked(
            render::path(
                vec![
                    render::move_to(line_start.0, line_start.1),
                    render::quadratic_to(line_start, (x, top), line_end),
                ],
                false,
            ),
            render::stroke(outline, 2.2, StrokeCap::Round, StrokeJoin::Miter),
            "wild-mood-squint-line",
        ),
    ]
}

/// Brow strokes; `side` is -1 left, +1 right.
pub fn brow(kind: Brows, x: f64, y: f64, r: f64, side: f64, ink: &str) -> Vec<Primitive> {
    let by = y - r * 1.7;
    let w = r * 0.95;
    let (inner_dy, outer_dy, lift) = match kind {
        Brows::None => return Vec::new(),
        Brows::Focus => (0.1 * r, 0.0, 0.0),
        Brows::Raised => (-0.15 * r, -0.05 * r, -r * 0.5),
        Brows::Knit => (0.55 * r, -0.15 * r, 0.1 * r),
        Brows::Worry => (-0.55 * r, 0.15 * r, 0.0),
    };
    let (x_inner, x_outer) = (x - side * w, x + side * w);
    vec![render::stroked(
        render::path(
            vec![
                render::move_to(x_inner, by + inner_dy + lift),
                render::line_to(x_outer, by + outer_dy + lift),
            ],
            false,
        ),
        render::stroke(ink, 3.2, StrokeCap::Round, StrokeJoin::Miter),
        "wild-mood-brow",
    )]
}

/// Iconographic adjuncts, drawn near the crown-right.
pub fn adjunct(kind: Adjunct, cx: f64, top_y: f64, scale: f64, ink: &str) -> Vec<Primitive> {
    match kind {
        Adjunct::None => Vec::new(),
        // A stroked glyph keeps the sleepy mark in the primitive contract;
        // system-font text would make the creature differ across renderers.
        Adjunct::Zz => {
            let mut primitives = z_glyph(
                cx + 68.0 * scale,
                top_y + 16.0 * scale,
                22.0 * scale,
                ink,
                0.75,
            );
            primitives.extend(z_glyph(
                cx + 86.0 * scale,
                top_y - 2.0 * scale,
                28.0 * scale,
                ink,
                0.55,
            ));
            primitives
        }
        Adjunct::Sweat => {
            let start = (cx + 52.0 * scale, top_y + 18.0 * scale);
            let right = (cx + 52.0 * scale, top_y + 40.0 * scale);
            let left = (cx + 52.0 * scale, top_y + 18.0 * scale);
            vec![render::filled(
                render::path(
                    vec![
                        render::move_to(start.0, start.1),
                        render::quadratic_to(
                            start,
                            (cx + 60.0 * scale, top_y + 34.0 * scale),
                            right,
                        ),
                        render::quadratic_to(
                            right,
                            (cx + 44.0 * scale, top_y + 34.0 * scale),
                            left,
                        ),
                    ],
                    true,
                ),
                "#7fb8e8",
                0.85,
                None,
                "wild-mood-sweat",
            )]
        }
    }
}

fn z_glyph(x: f64, baseline: f64, size: f64, ink: &str, opacity: f64) -> Vec<Primitive> {
    let top = baseline - size;
    let width = size * 0.6;
    vec![render::stroked(
        render::path(
            vec![
                render::move_to(x, top),
                render::line_to(x + width, top),
                render::line_to(x, baseline),
                render::line_to(x + width, baseline),
            ],
            false,
        ),
        render::stroke(
            ink,
            (size * 0.13).max(1.2),
            StrokeCap::Butt,
            StrokeJoin::Miter,
        ),
        "wild-mood-z",
    )]
    .into_iter()
    .map(|mut primitive| {
        primitive.opacity = render::opacity(opacity);
        primitive
    })
    .collect()
}
