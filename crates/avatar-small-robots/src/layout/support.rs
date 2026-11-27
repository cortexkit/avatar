//! Running gear dimensions and anchor positions.

use crate::arms::joint2;
use crate::gear::{ankle_of, multi_parts, to_g, under, Shape};
use crate::kit::lerp;
use crate::robot::Robot;
use crate::Mood;

use super::{Frame, GearShape};

/// Running gear with the dimensions and rest mounts used by the drawing.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct GearLayout {
    /// Decoded assembly kind.
    pub kind: &'static str,
    /// Decoded feet when the assembly carries separately picked feet.
    pub foot_kind: Option<&'static str>,
    /// Dimensionless size multiplier for picked feet before per-leg adjustments.
    pub foot_scale: f64,
    /// Neutral half-width of the footprint in robot units.
    pub footprint: f64,
    /// Reference half-width of the hull the gear is mounted to, in robot units.
    pub host_half_width: f64,
    /// Whether the gear hovers in neutral.
    pub hover: bool,
    /// Height above the floor of the landed hull base, in robot units.
    pub land: f64,
    /// Fitted construction dimensions and member counts of the running gear.
    pub shape: GearShape,
    /// Placement of the support members and their feet at rest.
    pub supports: Vec<SupportLayout>,
}

/// One support member of a running gear assembly.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct SupportLayout {
    /// Drawing class (a shared class can cover several assembly members).
    pub class: &'static str,
    /// Coordinate frame for its placement.
    pub frame: Frame,
    /// Hip of a leg, centre of a wheel, centre of a belt, or mount of an emitter.
    pub base: [f64; 2],
    /// Knee of a leg or control point of a hose, if present.
    pub middle: Option<[f64; 2]>,
    /// Ankle or end of the member, when distinct from its base.
    pub end: Option<[f64; 2]>,
    /// Foot fitted at the end of the member, if it has one.
    pub foot: Option<FootLayout>,
}

/// One foot belonging to a running gear member, in the ground frame.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct FootLayout {
    /// Decoded foot kind, or the fixed pad, round tip or sucker of a support.
    pub kind: &'static str,
    /// Centre of the sole in pre-fit robot units. The floor is y = 0.
    pub sole: [f64; 2],
    /// Dimensionless size multiplier.
    pub scale: f64,
}

impl GearLayout {
    pub(super) fn new(c: &Robot) -> Self {
        let g = &c.gear;
        let pose = c.pose(Mood::Neutral);
        let mut supports = Vec::new();
        let picked = |ankle: [f64; 2], ah, scale| FootLayout {
            kind: g.foot_kind,
            sole: [ankle[0], ankle[1] + ah],
            scale,
        };
        let mut support = |class, frame, base, middle, end, foot| {
            supports.push(SupportLayout {
                class,
                frame,
                base,
                middle,
                end,
                foot,
            });
        };
        let hip = |x| to_g(pose, under(c, x));
        let sides = [(-1.0, "rb-leg rb-leg-l"), (1.0, "rb-leg rb-leg-r")];
        let wheel_sides = [(-1.0, "rb-wheel rb-wheel-l"), (1.0, "rb-wheel rb-wheel-r")];
        let centre = "rb-leg rb-leg-c";
        let ground = Frame::Ground;
        match &g.shape {
            Shape::Legs { ah, hx, fx, l, .. } | Shape::Tripod { ah, hx, fx, l, .. } => {
                let tripod = g.kind == "tripod";
                for (side, class) in sides {
                    let base = hip(side * hx);
                    let a = ankle_of(base, side * fx, *ah, l * 0.995);
                    let (middle, end) = if tripod || g.kind == "biped" {
                        let (e, t) = joint2(base, a, l / 2.0, l / 2.0, |e| side * e[0]);
                        (Some(e), t)
                    } else {
                        (None, a)
                    };
                    let foot = picked(a, *ah, if tripod { g.fs * 0.9 } else { g.fs });
                    support(class, ground, base, middle, Some(end), Some(foot));
                }
                if tripod {
                    let base = to_g(pose, [0.0, -1.0]);
                    let a = ankle_of(base, base[0] * 0.5, *ah, g.rise - ah + 6.0);
                    support(
                        centre,
                        ground,
                        base,
                        None,
                        Some(a),
                        Some(picked(a, *ah, g.fs)),
                    );
                }
            }
            Shape::Multi { legs, tip, .. } => {
                let classes = [
                    ["rb-leg rb-leg-l1", "rb-leg rb-leg-r1"],
                    ["rb-leg rb-leg-l2", "rb-leg rb-leg-r2"],
                    ["rb-leg rb-leg-l3", "rb-leg rb-leg-r3"],
                ];
                for (i, leg) in legs.iter().enumerate() {
                    for (side, class) in [-1.0, 1.0].into_iter().zip(classes[i]) {
                        let (base, _, knee, t) = multi_parts(pose, side, leg);
                        let (kind, offset) = if *tip {
                            ("pad", 3.0)
                        } else {
                            ("ball", 0.5 + 3.3 * c.scale)
                        };
                        let foot = FootLayout {
                            kind,
                            sole: [t[0], t[1] + offset],
                            scale: c.scale,
                        };
                        support(class, ground, base, Some(knee), Some(t), Some(foot));
                    }
                }
            }
            Shape::Pogo { ah, l, .. } => {
                let base = to_g(pose, [0.0, -1.0]);
                let a = ankle_of(base, base[0] * 0.4, *ah, *l);
                support(
                    centre,
                    ground,
                    base,
                    None,
                    Some(a),
                    Some(picked(a, *ah, g.fs * 1.15)),
                );
            }
            Shape::Hoses { legs, .. } => {
                let classes = [
                    ["rb-leg rb-leg-l1", "rb-leg rb-leg-r1"],
                    ["rb-leg rb-leg-l2", "rb-leg rb-leg-r2"],
                ];
                for ([hx, fx], classes) in legs.iter().zip(classes) {
                    for (side, class) in [-1.0, 1.0].into_iter().zip(classes) {
                        let base = hip(side * hx);
                        let end = [side * fx, -7.0 * c.scale];
                        let control = [base[0] + side * 2.0, end[1] - 1.0];
                        let foot = FootLayout {
                            kind: "sucker",
                            sole: [end[0], 0.0],
                            scale: c.scale,
                        };
                        support(class, ground, base, Some(control), Some(end), Some(foot));
                    }
                }
            }
            Shape::Treads { th, tx, .. } => {
                for (side, class) in [(-1.0, "rb-tread rb-tread-l"), (1.0, "rb-tread rb-tread-r")] {
                    support(class, ground, [side * tx, -th * 0.5], None, None, None);
                }
            }
            Shape::Track { th, tw, wheels, .. } => {
                support("rb-track", ground, [0.0, -th / 2.0], None, None, None);
                for i in 0..*wheels {
                    let x = if *wheels == 1 {
                        0.0
                    } else {
                        lerp(
                            -tw + th / 2.0,
                            tw - th / 2.0,
                            f64::from(i) / f64::from(wheels - 1),
                        )
                    };
                    support("rb-wheel", ground, [x, -th / 2.0], None, None, None);
                }
            }
            Shape::Wheels { r, wx, .. } | Shape::Discs { r, wx, .. } => {
                for (side, class) in wheel_sides {
                    support(class, ground, [side * wx, -*r], None, None, None);
                }
            }
            Shape::Mono { r, .. } => {
                let class = if g.kind == "ball" {
                    "rb-ball"
                } else {
                    "rb-wheel rb-wheel-c"
                };
                support(class, ground, [0.0, -*r], None, None, None);
            }
            Shape::Hover { top, nh, .. } => {
                support(
                    "rb-hover",
                    Frame::UpperBody,
                    [0.0, under(c, *top)[1]],
                    None,
                    Some([0.0, *nh]),
                    None,
                );
            }
        }
        Self {
            kind: g.kind,
            foot_kind: (!g.foot_kind.is_empty()).then_some(g.foot_kind),
            foot_scale: g.fs,
            footprint: g.foot,
            host_half_width: g.wl,
            hover: g.hover,
            land: g.land,
            shape: g.shape.clone(),
            supports,
        }
    }
}
