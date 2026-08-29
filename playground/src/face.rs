//! Face vocabulary for grid 2: eye archetypes x mouth families, plus the
//! additive life layer (cheek blush, belly patch).
//!
//! Coordinates are the blob frame (origin at blob center) with the cell center
//! passed in; every function returns render primitives.

use avatar_svg::{Primitive, StrokeCap, StrokeJoin};

use crate::{
    body::{width_at, BodyParams},
    render,
};

#[derive(Clone, Copy, Debug)]
pub enum EyeKind {
    /// Grid-1 baseline: black dot + white shine.
    DotShine,
    /// Big lidded ovals: white sclera, iris dot, upper lid line.
    Lidded,
    /// Closed happy arcs.
    HappyArc,
    /// Gremlin: one big dot, one small dot (deliberate asymmetry).
    Uneven,
    /// Half-lidded flat-top eyes: dignified/unimpressed.
    HalfLid,
}

#[derive(Clone, Copy, Debug)]
pub enum MouthKind {
    /// Small downturned-then-up arc (grid-1 baseline smile).
    Arc,
    /// Cat mouth: two bumps (omega).
    CatW,
    /// Open rounded-rect happy mouth.
    OpenHappy,
    /// Crooked line with one tooth poking up (ugly-cute).
    Snaggle,
    /// Flat short line.
    Flat,
    /// Tiny w-fangs: two down-triangles under a short line.
    Fangs,
    /// Small round o (surprise/yawn) — mood vocabulary.
    OMouth,
    /// Downturned arc (sad) — mood vocabulary.
    DownArc,
}

pub struct FacePack {
    pub name: &'static str,
    pub eyes: EyeKind,
    pub mouth: MouthKind,
}

pub const PACKS: [FacePack; 6] = [
    FacePack {
        name: "baseline",
        eyes: EyeKind::DotShine,
        mouth: MouthKind::Arc,
    },
    FacePack {
        name: "kitten",
        eyes: EyeKind::Lidded,
        mouth: MouthKind::CatW,
    },
    FacePack {
        name: "joy",
        eyes: EyeKind::HappyArc,
        mouth: MouthKind::OpenHappy,
    },
    FacePack {
        name: "gremlin",
        eyes: EyeKind::Uneven,
        mouth: MouthKind::Snaggle,
    },
    FacePack {
        name: "deadpan",
        eyes: EyeKind::HalfLid,
        mouth: MouthKind::Flat,
    },
    FacePack {
        name: "imp",
        eyes: EyeKind::DotShine,
        mouth: MouthKind::Fangs,
    },
];

/// Eye geometry shared by all archetypes: large, wide-set, low.
pub struct EyeFrame {
    pub lx: f64,
    pub rx: f64,
    pub y: f64,
    pub r: f64,
}

/// Public frame accessor so mood overlays (lids, squint, brows) land exactly
/// on the archetype geometry without re-deriving it.
pub fn frame(bp: &BodyParams, cx: f64, cy: f64) -> EyeFrame {
    eye_frame(bp, cx, cy, 0.0)
}

fn eye_frame(bp: &BodyParams, cx: f64, cy: f64, tilt: f64) -> EyeFrame {
    let ey = -bp.height * 0.08;
    let w = width_at(bp, ey);
    let ex = w * 0.42;
    let r = (bp.height * 0.085).min(w * 0.3);
    EyeFrame {
        lx: cx - ex,
        rx: cx + ex,
        y: cy + ey + tilt,
        r,
    }
}

/// Draws an eye pair. `wonk` shrinks the RIGHT eye only (0 = a matched
/// pair): mismatched eyes are the strongest ugly-cute signal in the whole
/// vocabulary, and before this it existed only as the `Uneven` archetype's
/// hardcoded 1.25/0.62 pair — one creature in five could be asymmetric and
/// the rest could not. Shrinking is the safe direction: an eye that only
/// gets smaller cannot breach the clearance or separation the placement
/// search proved for the full radius.
/// The four colour roles an eye draws with, kept together because they are
/// chosen together and confusing them is the defect this grouping exists
/// to prevent: `accent` is the body's outline colour, used for lid and
/// lash strokes that should read as part of the silhouette; `ink` is
/// line-work and flips light on dark bodies; `pupil` stays dark
/// everywhere; and `sclera` is the light backing that gives a dark body
/// the contrast a light one provides for free.
#[derive(Clone, Copy)]
pub struct EyeInk<'a> {
    pub accent: &'a str,
    pub ink: &'a str,
    pub pupil: &'a str,
    pub sclera: Option<&'a str>,
}

pub fn eyes(
    kind: EyeKind,
    bp: &BodyParams,
    cx: f64,
    cy: f64,
    tilt: f64,
    colors: EyeInk<'_>,
    wonk: f64,
) -> Vec<Primitive> {
    let (accent, ink, pupil) = (colors.accent, colors.ink, colors.pupil);
    let f = eye_frame(bp, cx, cy, tilt);
    let kl = wonk_factor(kind, wonk, false);
    let kr = wonk_factor(kind, wonk, true);
    let mut primitives = Vec::new();
    match kind {
        EyeKind::DotShine => {
            for (x, dy, k) in [(f.lx, -tilt, kl), (f.rx, tilt, kr)] {
                let y = f.y + dy;
                let r = f.r * k;
                primitives.push(render::filled(
                    render::circle(x, y, r),
                    pupil,
                    1.0,
                    None,
                    "wild-eye-pupil",
                ));
                primitives.push(render::filled(
                    render::circle(x + r * 0.32, y - r * 0.35, r * 0.3),
                    "white",
                    0.9,
                    None,
                    "wild-eye-highlight",
                ));
            }
        }
        EyeKind::Lidded => {
            for (x, dy, k) in [(f.lx, -tilt, kl), (f.rx, tilt, kr)] {
                let y = f.y + dy;
                let r = f.r * k;
                let rw = r * 1.1;
                let rh = r * 1.25;
                primitives.push(render::filled(
                    render::ellipse(x, y, rw, rh),
                    "white",
                    1.0,
                    Some(render::stroke(
                        accent,
                        2.0,
                        StrokeCap::Butt,
                        StrokeJoin::Miter,
                    )),
                    "wild-eye-sclera",
                ));
                primitives.push(render::filled(
                    render::circle(x, y + rh * 0.08, r * 0.78),
                    pupil,
                    1.0,
                    None,
                    "wild-eye-pupil",
                ));
                primitives.push(render::filled(
                    render::circle(x + r * 0.28, y - r * 0.2, r * 0.22),
                    "white",
                    0.9,
                    None,
                    "wild-eye-highlight",
                ));
                let start = (x - rw, y - rh * 0.1);
                let middle = (x + rw, y - rh * 0.1);
                let lower_right = (x + rw, y - rh * 0.28);
                let lower_left = (x - rw, y - rh * 0.28);
                let mut segments = vec![render::move_to(start.0, start.1)];
                segments.extend(render::arc_to(start, rw, rh, 0.0, false, true, middle));
                segments.push(render::line_to(lower_right.0, lower_right.1));
                segments.extend(render::arc_to(
                    lower_right,
                    rw,
                    rh * 0.9,
                    0.0,
                    false,
                    false,
                    lower_left,
                ));
                primitives.push(render::filled(
                    render::path(segments, true),
                    accent,
                    0.5,
                    None,
                    "wild-eye-lid",
                ));
            }
        }
        EyeKind::HappyArc => {
            for (x, dy, k) in [(f.lx, -tilt, kl), (f.rx, tilt, kr)] {
                let y = f.y + dy;
                let r = f.r * k;
                let from = (x - r, y + r * 0.2);
                let to = (x + r, y + r * 0.2);
                primitives.push(render::stroked(
                    render::path(
                        vec![
                            render::move_to(from.0, from.1),
                            render::quadratic_to(from, (x, y - r * 0.9), to),
                        ],
                        false,
                    ),
                    render::stroke(ink, 3.4, StrokeCap::Round, StrokeJoin::Miter),
                    "wild-eye-arc",
                ));
            }
        }
        EyeKind::Uneven => {
            primitives.push(render::filled(
                render::circle(f.lx, f.y - tilt, f.r * 1.25),
                pupil,
                1.0,
                None,
                "wild-eye-pupil",
            ));
            primitives.push(render::filled(
                render::circle(f.lx + f.r * 0.4, f.y - tilt - f.r * 0.44, f.r * 0.36),
                "white",
                0.9,
                None,
                "wild-eye-highlight",
            ));
            primitives.push(render::filled(
                render::circle(f.rx, f.y + tilt + f.r * 0.15, f.r * 0.62),
                pupil,
                1.0,
                None,
                "wild-eye-pupil",
            ));
            primitives.push(render::filled(
                render::circle(f.rx + f.r * 0.2, f.y + tilt - f.r * 0.08, f.r * 0.18),
                "white",
                0.9,
                None,
                "wild-eye-highlight",
            ));
        }
        EyeKind::HalfLid => {
            for (x, dy, k) in [(f.lx, -tilt, kl), (f.rx, tilt, kr)] {
                let y = f.y + dy;
                let r = f.r * k;
                let from = (x - r * 0.92, y - r * 0.25);
                let to = (x + r * 0.92, y - r * 0.25);
                let mut segments = vec![render::move_to(from.0, from.1)];
                segments.extend(render::arc_to(from, r, r, 0.0, true, false, to));
                primitives.push(render::filled(
                    render::path(segments, true),
                    pupil,
                    1.0,
                    None,
                    "wild-eye-pupil",
                ));
                primitives.push(render::stroked(
                    render::path(
                        vec![
                            render::move_to(x - r * 1.05, y - r * 0.3),
                            render::line_to(x + r * 1.05, y - r * 0.3),
                        ],
                        false,
                    ),
                    render::stroke(pupil, 2.6, StrokeCap::Round, StrokeJoin::Miter),
                    "wild-eye-lid-line",
                ));
            }
        }
    }
    primitives
}

/// Per-side eye radius factor. `wonk` is SIGNED: positive shrinks the right
/// eye, negative the left. An unsigned shrink would put every wonky
/// creature's smaller eye on the same side, which across a fleet reads as a
/// renderer quirk rather than as individual character — the same reason
/// TOG's hardcoded left-big/right-small `Uneven` pair could never feel like
/// per-creature variation.
///
/// Exposed so mood overlays (lids, squints) and the sclera backing land on
/// the radius the archetype actually drew; an overlay sized for the full
/// radius floats free of a shrunk eye.
pub fn wonk_factor(kind: EyeKind, wonk: f64, right: bool) -> f64 {
    if matches!(kind, EyeKind::Uneven) {
        return 1.0;
    }
    let shrinks_right = wonk >= 0.0;
    if right != shrinks_right {
        return 1.0;
    }
    (1.0 - wonk.abs()).clamp(0.55, 1.0)
}

pub fn mouth(kind: MouthKind, bp: &BodyParams, cx: f64, cy: f64, accent: &str) -> Vec<Primitive> {
    let ey = -bp.height * 0.08;
    let r = (bp.height * 0.085).min(width_at(bp, ey) * 0.3);
    let my = cy + ey + bp.height * 0.15;
    match kind {
        MouthKind::Arc => {
            let from = (cx - r * 0.5, my);
            let to = (cx + r * 0.5, my);
            vec![render::stroked(
                render::path(
                    vec![
                        render::move_to(from.0, from.1),
                        render::quadratic_to(from, (cx, my + r * 0.45), to),
                    ],
                    false,
                ),
                render::stroke(accent, 2.4, StrokeCap::Round, StrokeJoin::Miter),
                "wild-mouth-arc",
            )]
        }
        MouthKind::CatW => {
            let start = (cx - r * 0.8, my);
            let center = (cx, my);
            let end = (cx + r * 0.8, my);
            vec![render::stroked(
                render::path(
                    vec![
                        render::move_to(start.0, start.1),
                        render::quadratic_to(start, (cx - r * 0.4, my + r * 0.55), center),
                        render::quadratic_to(center, (cx + r * 0.4, my + r * 0.55), end),
                    ],
                    false,
                ),
                render::stroke(accent, 2.4, StrokeCap::Round, StrokeJoin::Miter),
                "wild-mouth-cat",
            )]
        }
        MouthKind::OpenHappy => {
            let outer_start = (cx - r * 0.7, my - r * 0.1);
            let outer_end = (cx + r * 0.7, my - r * 0.1);
            let tongue_start = (cx - r * 0.35, my + r * 0.30);
            let tongue_end = (cx + r * 0.35, my + r * 0.30);
            vec![
                render::filled(
                    render::path(
                        vec![
                            render::move_to(outer_start.0, outer_start.1),
                            render::quadratic_to(outer_start, (cx, my + r * 1.1), outer_end),
                        ],
                        true,
                    ),
                    "#4a2f33",
                    1.0,
                    Some(render::stroke(
                        accent,
                        2.0,
                        StrokeCap::Butt,
                        StrokeJoin::Miter,
                    )),
                    "wild-mouth-open",
                ),
                render::filled(
                    render::path(
                        vec![
                            render::move_to(tongue_start.0, tongue_start.1),
                            render::quadratic_to(tongue_start, (cx, my + r * 0.62), tongue_end),
                        ],
                        true,
                    ),
                    "#e88a93",
                    1.0,
                    None,
                    "wild-mouth-tongue",
                ),
            ]
        }
        MouthKind::Snaggle => {
            let from = (cx - r * 0.75, my + r * 0.15);
            let to = (cx + r * 0.7, my - r * 0.1);
            vec![
                render::stroked(
                    render::path(
                        vec![
                            render::move_to(from.0, from.1),
                            render::quadratic_to(from, (cx, my + r * 0.5), to),
                        ],
                        false,
                    ),
                    render::stroke(accent, 2.4, StrokeCap::Round, StrokeJoin::Miter),
                    "wild-mouth-snaggle-line",
                ),
                render::filled(
                    render::polygon(&[
                        (cx + r * 0.13, my + r * 0.23),
                        (cx + r * 0.42, my + r * 0.10),
                        (cx + r * 0.33, my + r * 0.55),
                    ]),
                    "white",
                    1.0,
                    Some(render::stroke(
                        accent,
                        1.4,
                        StrokeCap::Butt,
                        StrokeJoin::Round,
                    )),
                    "wild-mouth-tooth",
                ),
            ]
        }
        MouthKind::Flat => vec![render::stroked(
            render::path(
                vec![
                    render::move_to(cx - r * 0.55, my),
                    render::line_to(cx + r * 0.55, my),
                ],
                false,
            ),
            render::stroke(accent, 2.6, StrokeCap::Round, StrokeJoin::Miter),
            "wild-mouth-flat",
        )],
        MouthKind::OMouth => vec![render::filled(
            render::ellipse(cx, my + r * 0.2, r * 0.3, r * 0.38),
            "#4a2f33",
            1.0,
            Some(render::stroke(
                accent,
                1.8,
                StrokeCap::Butt,
                StrokeJoin::Miter,
            )),
            "wild-mouth-o",
        )],
        MouthKind::DownArc => {
            let from = (cx - r * 0.55, my + r * 0.35);
            let to = (cx + r * 0.55, my + r * 0.35);
            vec![render::stroked(
                render::path(
                    vec![
                        render::move_to(from.0, from.1),
                        render::quadratic_to(from, (cx, my - r * 0.25), to),
                    ],
                    false,
                ),
                render::stroke(accent, 2.4, StrokeCap::Round, StrokeJoin::Miter),
                "wild-mouth-down",
            )]
        }
        MouthKind::Fangs => {
            let from = (cx - r * 0.6, my);
            let to = (cx + r * 0.6, my);
            vec![
                render::stroked(
                    render::path(
                        vec![
                            render::move_to(from.0, from.1),
                            render::quadratic_to(from, (cx, my + r * 0.5), to),
                        ],
                        false,
                    ),
                    render::stroke(accent, 2.4, StrokeCap::Round, StrokeJoin::Miter),
                    "wild-mouth-fangs-line",
                ),
                render::filled(
                    render::polygon(&[
                        (cx - r * 0.45, my + r * 0.12),
                        (cx - r * 0.15, my + r * 0.14),
                        (cx - r * 0.3, my + r * 0.62),
                    ]),
                    "white",
                    1.0,
                    None,
                    "wild-mouth-fang",
                ),
                render::filled(
                    render::polygon(&[
                        (cx + r * 0.15, my + r * 0.14),
                        (cx + r * 0.45, my + r * 0.12),
                        (cx + r * 0.3, my + r * 0.62),
                    ]),
                    "white",
                    1.0,
                    None,
                    "wild-mouth-fang",
                ),
            ]
        }
    }
}

/// Ink-parameterized wrappers for grid 3: palettes may flip the face ink
/// (dark bodies need light features), so the archetype functions gain color
/// arguments while the grid-2 call sites keep their defaults.
///
/// Pupils are NOT ink: ink is line-work (mouth strokes, brows, closed-eye
/// arcs) and flips light on dark bodies, but a light "pupil" reads as a
/// blank ghost orb. Pupils stay dark everywhere; on dark bodies a light
/// sclera disc drawn BEHIND each open-eye pupil provides the contrast the
/// light body gives for free. Closed/arc eyes are line-work and keep ink.
pub fn eyes_inked(
    kind: EyeKind,
    bp: &BodyParams,
    cx: f64,
    cy: f64,
    tilt: f64,
    colors: EyeInk<'_>,
    wonk: f64,
) -> Vec<Primitive> {
    let sclera = colors.sclera;
    let f = eye_frame(bp, cx, cy, tilt);
    let mut primitives = Vec::new();
    if let Some(sc) = sclera {
        let (kl, kr) = (
            wonk_factor(kind, wonk, false),
            wonk_factor(kind, wonk, true),
        );
        match kind {
            EyeKind::DotShine => {
                for (x, k) in [(f.lx, kl), (f.rx, kr)] {
                    primitives.push(render::filled(
                        render::circle(x, f.y, f.r * k * 1.22),
                        sc,
                        1.0,
                        None,
                        "wild-eye-sclera-backing",
                    ));
                }
            }
            EyeKind::HalfLid => {
                for (x, k) in [(f.lx, kl), (f.rx, kr)] {
                    primitives.push(render::filled(
                        render::circle(x, f.y, f.r * k * 1.18),
                        sc,
                        1.0,
                        None,
                        "wild-eye-sclera-backing",
                    ));
                }
            }
            EyeKind::Uneven => {
                primitives.push(render::filled(
                    render::circle(f.lx, f.y - tilt, f.r * 1.25 * 1.18),
                    sc,
                    1.0,
                    None,
                    "wild-eye-sclera-backing",
                ));
                primitives.push(render::filled(
                    render::circle(f.rx, f.y + tilt + f.r * 0.15, f.r * 0.62 * 1.25),
                    sc,
                    1.0,
                    None,
                    "wild-eye-sclera-backing",
                ));
            }
            EyeKind::Lidded | EyeKind::HappyArc => {}
        }
    }
    primitives.extend(eyes(kind, bp, cx, cy, tilt, colors, wonk));
    primitives
}

pub fn mouth_inked(
    kind: MouthKind,
    bp: &BodyParams,
    cx: f64,
    cy: f64,
    accent: &str,
    _ink: &str,
) -> Vec<Primitive> {
    mouth(kind, bp, cx, cy, accent)
}

pub fn blush_colored(bp: &BodyParams, cx: f64, cy: f64, color: &str) -> Vec<Primitive> {
    blush_with_color(bp, cx, cy, color, 0.8)
}

pub fn belly_colored(bp: &BodyParams, cx: f64, cy: f64, color: &str) -> Vec<Primitive> {
    let mut primitives = belly(bp, cx, cy);
    for primitive in &mut primitives {
        primitive.paint = avatar_svg::Paint::Solid {
            color: render::css_rgb(color),
        };
    }
    primitives
}

/// Cheek blush: soft rosy ellipses outer-low of each eye.
pub fn blush(bp: &BodyParams, cx: f64, cy: f64) -> Vec<Primitive> {
    blush_with_color(bp, cx, cy, "#f0808f", 0.35)
}

fn blush_with_color(
    bp: &BodyParams,
    cx: f64,
    cy: f64,
    color: &str,
    opacity: f64,
) -> Vec<Primitive> {
    let ey = -bp.height * 0.08;
    let w = width_at(bp, ey);
    let bx = w * 0.62;
    let by = cy + ey + bp.height * 0.09;
    let rw = w * 0.14;
    let rh = rw * 0.62;
    vec![
        render::filled(
            render::ellipse(cx - bx, by, rw, rh),
            color,
            opacity,
            None,
            "wild-blush",
        ),
        render::filled(
            render::ellipse(cx + bx, by, rw, rh),
            color,
            opacity,
            None,
            "wild-blush",
        ),
    ]
}

/// Belly patch: a lighter inner blob hugging the lower mass.
pub fn belly(bp: &BodyParams, cx: f64, cy: f64) -> Vec<Primitive> {
    let half_h = bp.height / 2.0;
    let top = -half_h * 0.05;
    let bot = half_h * 0.92;
    let steps = 24;
    let mut points = Vec::with_capacity((steps + 1) * 2);
    for i in 0..=steps {
        let y = top + (bot - top) * (i as f64 / steps as f64);
        let x = width_at(bp, y) * 0.58;
        points.push((cx + x, cy + y));
    }
    for i in (0..=steps).rev() {
        let y = top + (bot - top) * (i as f64 / steps as f64);
        let x = width_at(bp, y) * 0.58;
        points.push((cx - x, cy + y));
    }
    vec![render::filled(
        render::polygon(&points),
        "white",
        0.42,
        None,
        "wild-belly",
    )]
}
