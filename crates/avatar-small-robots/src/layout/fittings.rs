//! Fittings in the frames used by the 2D emitter.

use crate::arms::ArmPose;
use crate::body::chassis_rivet_points;
use crate::face::Tech;
use crate::kit::min;
use crate::marks::Spot;
use crate::mood::MoodSpec;
use crate::parts::{cable_geom, mast_geom, pod_eye, stalk_at, MastGeom};
use crate::robot::{Pose, Robot};
use crate::Mood;

use super::{layers, Material, Region, Status};

/// Coordinate frame for a fitted part. All frames use pre-fit robot units
/// with negative y up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Frame {
    /// Local to the head, with origin at the head base.
    Head,
    /// Local to the upper assembly, with origin at the lowest hull base.
    UpperBody,
    /// Ground coordinates, with the floor at y = 0.
    Ground,
}

/// Antenna assembly identified by the decoded top choice.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TopLayout {
    /// Top kind; twin, triple and ears are assemblies of rod masts.
    pub kind: &'static str,
    /// Head-local antenna members in emitter order.
    pub masts: Vec<MastLayout>,
    /// Head-local stalks, left then right, for a stalk face.
    pub stalks: Vec<StalkLayout>,
}

/// A fitted antenna member on the head.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MastLayout {
    /// Mast kind, possibly a member of a larger antenna assembly.
    pub kind: &'static str,
    /// Drawing class of the member.
    pub class: &'static str,
    /// Left (−1), centre (0), or right (1).
    pub side: i8,
    /// Placement in head-local robot units at rest.
    pub rest: MastPlacement,
    /// Separate tip choice, absent on masts with built-in tips.
    pub tip: Option<TipLayout>,
}

/// The tip on a mast, fitted at the mast's terminal point and direction.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct TipLayout {
    /// Decoded tip kind. `bare` is an explicit absence of tip geometry.
    pub kind: &'static str,
    /// Lamp radius in robot units for a ball tip, otherwise zero.
    pub ball_radius: f64,
}

/// Head-local base, bend and tip of a stem in pre-fit robot units.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct MastPlacement {
    /// Stem base.
    pub base: [f64; 2],
    /// Bend point (a control point for a hose).
    pub bend: [f64; 2],
    /// Stem endpoint.
    pub end: [f64; 2],
    /// Unit direction at the base.
    pub base_dir: [f64; 2],
    /// Unit direction at the endpoint.
    pub end_dir: [f64; 2],
    /// Fitted stem length in robot units.
    pub length: f64,
    /// Whether both halves point in the same direction.
    pub straight: bool,
    /// Whether the stem has a built-in kink.
    pub kinked: bool,
}

impl From<MastGeom> for MastPlacement {
    fn from(g: MastGeom) -> Self {
        Self {
            base: g.b,
            bend: g.cp,
            end: g.t,
            base_dir: g.u1,
            end_dir: g.u2,
            length: g.len,
            straight: g.straight,
            kinked: g.kinked,
        }
    }
}

/// A lens pod on a head-mounted stalk at rest.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct StalkLayout {
    /// Left (−1) or right (1).
    pub side: i8,
    /// Drawing class of the stalk.
    pub class: &'static str,
    /// Rest stem and eye centre, in head-local robot units.
    pub rest: StalkPlacement,
    /// Lens housing half-width and half-height in robot units.
    pub radii: [f64; 2],
    /// Lens housing corner radii in robot units.
    pub corner_radii: [f64; 2],
    /// All-mood eye texture region, in head-local robot units.
    pub region: Region,
}

/// A head-local stem and its attached eye in pre-fit robot units.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct StalkPlacement {
    /// Stalk placement.
    pub stem: MastPlacement,
    /// Attached lens centre.
    pub eye_centre: [f64; 2],
}

fn stalk(c: &Robot, side: f64, m: &MoodSpec) -> StalkPlacement {
    let (stem, eye_centre) = stalk_at(c, side, m);
    StalkPlacement {
        stem: stem.into(),
        eye_centre,
    }
}

impl TopLayout {
    pub(super) fn new(c: &Robot) -> Self {
        let m = c.mood(Mood::Neutral);
        let tip = (!c.tip.is_empty()).then_some(TipLayout {
            kind: c.tip,
            ball_radius: c.tip_r,
        });
        let mut masts = Vec::new();
        let mut stalks = Vec::new();
        for part in &c.parts {
            match part.kind {
                crate::robot::PartKind::Mast(i) => {
                    let stem = &c.masts[i];
                    masts.push(MastLayout {
                        class: part.class,
                        kind: stem.kind,
                        side: stem.side as i8,
                        rest: mast_geom(c, stem, m).into(),
                        tip,
                    });
                }
                crate::robot::PartKind::Stalk(side) => {
                    let eye = pod_eye(c, side);
                    stalks.push(StalkLayout {
                        side: side as i8,
                        class: part.class,
                        rest: stalk(c, side, m),
                        radii: [eye.rx, eye.ry],
                        corner_radii: [eye.qx, eye.qy],
                        region: layers::stalk_region(c, side)
                            .expect("a stalk has a nonempty eye layer"),
                    });
                }
                _ => {}
            }
        }
        Self {
            kind: c.top,
            masts,
            stalks,
        }
    }
}

/// Head-local side fittings, including surface-only vents.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SideLayout {
    /// Decoded side kind, including `none` and `vents`.
    pub kind: &'static str,
    /// Side hardware in head-local robot units.
    pub fittings: Vec<SideFitting>,
    /// Absolute x, y and size multiplier of a pair of painted vents.
    pub vents: Option<[f64; 3]>,
}

/// One mechanical fitting bolted to a side of the head.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct SideFitting {
    /// Drawing class of the fitting.
    pub class: &'static str,
    /// Decoded side kind.
    pub kind: &'static str,
    /// Left (−1) or right (1).
    pub side: i8,
    /// Mount in head-local robot units.
    pub mount: [f64; 2],
    /// Outward reach in robot units.
    pub reach: f64,
    /// Dimensionless size multiplier.
    pub scale: f64,
}

impl SideLayout {
    pub(super) fn new(c: &Robot) -> Self {
        let fittings = c
            .parts
            .iter()
            .filter_map(|p| {
                let crate::robot::PartKind::Ear(side) = p.kind else {
                    return None;
                };
                let s = c.sides.as_ref()?;
                Some(SideFitting {
                    class: p.class,
                    kind: s.kind,
                    side: side as i8,
                    mount: [side * s.x, s.y],
                    reach: s.reach,
                    scale: s.k,
                })
            })
            .collect();
        Self {
            kind: c.side,
            fittings,
            vents: c.side_vents,
        }
    }
}

/// Fitted arm construction and its hand in the upper-body frame.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct ArmLayout {
    /// Drawing class of the arm.
    pub class: &'static str,
    /// Arm construction kind.
    pub kind: &'static str,
    /// Tool mounted at the wrist.
    pub hand: &'static str,
    /// Two nominal segment lengths in robot units, shoulder to wrist.
    pub lengths: [f64; 2],
    /// Arm width in robot units.
    pub width: f64,
    /// Elbow size multiplier used by the arm construction.
    pub joint_scale: f64,
    /// Dimensionless tool size multiplier.
    pub hand_scale: f64,
    /// sRGB material used for the arm shell or hose.
    pub material: Material,
    /// Neutral position in upper-body robot units.
    pub rest: ArmPlacement,
}

/// Fitted joints in upper-body robot units.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct ArmPlacement {
    /// Shoulder joint.
    pub shoulder: [f64; 2],
    /// Elbow joint (the control point for a hose).
    pub elbow: [f64; 2],
    /// Wrist joint.
    pub wrist: [f64; 2],
    /// Unit direction in which the hand points.
    pub hand_dir: [f64; 2],
}

impl From<&ArmPose> for ArmPlacement {
    fn from(a: &ArmPose) -> Self {
        Self {
            shoulder: a.s,
            elbow: a.e,
            wrist: a.t,
            hand_dir: a.u,
        }
    }
}

pub(super) fn arms(c: &Robot) -> [Option<ArmLayout>; 2] {
    let mut arms = [None, None];
    for p in &c.parts {
        if let crate::robot::PartKind::Arm(side) = p.kind {
            let a = &c.arms;
            arms[usize::from(side > 0.0)] = Some(ArmLayout {
                class: p.class,
                kind: a.style,
                hand: if side < 0.0 { a.hand_l } else { a.hand },
                lengths: [a.l1, a.l2],
                width: a.w,
                joint_scale: a.joint,
                hand_scale: a.hs,
                material: a.mat,
                rest: c.pose(Mood::Neutral).arm(side).into(),
            });
        }
    }
    arms
}

/// The neck and the chassis-to-pelvis waist joint, in upper-body robot units.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct BodyJoints {
    /// Kind, centreline ends and half-width of the neck, absent on a pod.
    pub neck: Option<JointLayout>,
    /// Centreline ends and half-width of the waist plate, present on a stack.
    pub waist: Option<JointLayout>,
}

/// A connector in the upper-body frame at rest.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct JointLayout {
    /// Decoded neck kind or `waist` for the stack connector.
    pub kind: &'static str,
    /// Lower mount on the centreline.
    pub base: [f64; 2],
    /// Upper end on the centreline.
    pub end: [f64; 2],
    /// Fitted half-width; zero for a flush neck.
    pub half_width: f64,
}

impl BodyJoints {
    pub(super) fn new(c: &Robot) -> Self {
        let neck = c.neck.as_ref().map(|n| JointLayout {
            kind: n.kind,
            base: [0.0, c.body_top + 3.0],
            end: [0.0, c.head_y - 3.0],
            half_width: n.w,
        });
        let waist = c
            .pelvis
            .as_ref()
            .zip(c.body.as_ref())
            .map(|(p, b)| JointLayout {
                kind: "waist",
                base: [0.0, -p.h + 2.5],
                end: [0.0, -p.h - 7.5],
                half_width: min(p.hw_at(-p.h + 2.0), b.hw_at(-2.0)) * 0.56,
            });
        Self { neck, waist }
    }
}

/// Front fitting on the head of a pod or on the chassis of a taller robot.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ChestLayout {
    /// Decoded chest kind, including `none`.
    pub kind: &'static str,
    /// Frame of the fitting.
    pub frame: Frame,
    /// Centre in the fitting's frame, in robot units.
    pub centre: [f64; 2],
    /// Half-width and half-height in robot units.
    pub radii: [f64; 2],
    /// Number of repeated features (for example LED lamps).
    pub count: u32,
    /// Digits on a stencil, empty for other chest kinds.
    pub digits: Vec<u32>,
}

impl ChestLayout {
    pub(super) fn new(c: &Robot) -> Self {
        let f = &c.chest;
        let frame = if c.body.is_some() {
            Frame::UpperBody
        } else {
            Frame::Head
        };
        Self {
            kind: f.kind,
            frame,
            centre: [0.0, f.y],
            radii: [f.w, f.h],
            count: f.n,
            digits: f.text.clone(),
        }
    }
}

/// Fitted surface contributions; the marks calls draw these into textures.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DecorLayout {
    /// Decoded head paint scheme.
    pub scheme: &'static str,
    /// Half-width in head-local robot units of the crown stripe, if present.
    pub stripe_half_width: Option<f64>,
    /// Head-local y coordinate below the crown stripe and its clear space.
    pub face_top: f64,
    /// Head-local seam and rivet rows.
    pub rows: Vec<DecorRow>,
    /// Whether the head's rivet rows use six places instead of four.
    pub six_rivets: bool,
    /// Optional head-local decal.
    pub decal: Option<DecorSpot>,
    /// Optional head-local wear mark.
    pub wear: Option<DecorSpot>,
    /// Upper-body-local positions of the four chassis rivets, when present.
    pub chassis_rivets: Option<[[f64; 2]; 4]>,
}

/// Row of seams or rivets over the head hull.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct DecorRow {
    /// Row height in head-local robot units.
    pub y: f64,
    /// Whether the row includes a seam.
    pub seam: bool,
    /// Whether the row includes rivets.
    pub rivets: bool,
}

/// A decal or wear mark in head-local robot units.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DecorSpot {
    /// Decoded decal or wear kind.
    pub kind: &'static str,
    /// Fitted centre.
    pub centre: [f64; 2],
    /// Size of the mark, in robot units.
    pub size: f64,
    /// Digits for a numbered decal, empty otherwise.
    pub digits: Vec<u32>,
}

impl From<&Spot> for DecorSpot {
    fn from(p: &Spot) -> Self {
        Self {
            kind: p.kind,
            centre: [p.x, p.y],
            size: p.s,
            digits: p.text.clone(),
        }
    }
}

impl DecorLayout {
    pub(super) fn new(c: &Robot) -> Self {
        let d = &c.decor;
        Self {
            scheme: d.scheme,
            stripe_half_width: (d.scheme == "stripe").then_some(d.stripe_w),
            face_top: d.top,
            rows: d
                .rows
                .iter()
                .map(|r| DecorRow {
                    y: r.y,
                    seam: r.seam,
                    rivets: r.rivets,
                })
                .collect(),
            six_rivets: d.six,
            decal: d.decal.as_ref().map(Into::into),
            wear: d.wear.as_ref().map(Into::into),
            chassis_rivets: chassis_rivet_points(c),
        }
    }
}

/// Cable or exhaust pipe attached to the robot's back at rest.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum BackLayout {
    /// Flexible cable with a plug; positions are in the ground frame.
    #[non_exhaustive]
    Cable {
        /// Drawing class.
        class: &'static str,
        /// Fitted cable and plug position.
        rest: CablePlacement,
    },
    /// Exhaust pipe; positions are in the head frame.
    #[non_exhaustive]
    Pipe {
        /// Drawing class.
        class: &'static str,
        /// Pipe mount.
        base: [f64; 2],
        /// Location of the outlet.
        outlet: [f64; 2],
        /// Dimensionless pipe size multiplier.
        scale: f64,
    },
}

/// Cable curve and plug in ground-local robot units.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct CablePlacement {
    /// Cable mount, following the upper-body pose.
    pub base: [f64; 2],
    /// Quadratic control point.
    pub control: [f64; 2],
    /// End of the cable where it meets the plug.
    pub tip: [f64; 2],
    /// Unit direction of the plug.
    pub plug_dir: [f64; 2],
}

fn cable(c: &Robot, m: &MoodSpec, pose: &Pose) -> Option<CablePlacement> {
    c.cable.as_ref()?;
    let g = cable_geom(c, m, pose);
    Some(CablePlacement {
        base: g.a,
        control: g.ctl,
        tip: g.tip,
        plug_dir: g.u,
    })
}

pub(super) fn back(c: &Robot) -> Option<BackLayout> {
    c.parts.iter().find_map(|p| match p.kind {
        crate::robot::PartKind::Cable => Some(BackLayout::Cable {
            class: p.class,
            rest: cable(c, c.mood(Mood::Neutral), c.pose(Mood::Neutral))?,
        }),
        crate::robot::PartKind::Pipe => {
            let pipe = c.pipe.as_ref()?;
            Some(BackLayout::Pipe {
                class: p.class,
                base: pipe.b,
                outlet: pipe.end,
                scale: pipe.k,
            })
        }
        _ => None,
    })
}

/// The resolved placement and carried controls for one mood. Angles are
/// clockwise in the 2D drawing; lengths are in pre-fit robot units.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MoodLayout {
    /// Mood whose fields these are.
    pub mood: Mood,
    /// Height gained from rest, after the gear's lift/landing limits.
    pub lift: f64,
    /// Rotation of the upper assembly about the lowest hull, including pod tilt.
    pub lean: f64,
    /// Vertical head delta at the neck, zero on a pod.
    pub drop: f64,
    /// Rotation of the head relative to its neck, zero on a pod.
    pub tilt: f64,
    /// Whether hover gear lands in this pose.
    pub landed: bool,
    /// Arm joints in the upper-body frame, left then right.
    pub arms: [Option<ArmPlacement>; 2],
    /// Resolved head-local antenna stems, in [`TopLayout::masts`] order.
    pub masts: Vec<MastPlacement>,
    /// Resolved head-local stems and centres of the stalk eyes, left then right.
    pub stalks: Vec<StalkPlacement>,
    /// Ground-local cable and plug placement if the robot has a cable.
    pub cable: Option<CablePlacement>,
    /// Dimensionless grip amount.
    pub grip: f64,
    /// Dimensionless antenna perk or droop.
    pub ant: f64,
    /// Extra antenna rotation in clockwise degrees.
    pub ant_tilt: f64,
    /// Dimensionless tail swing.
    pub tail: f64,
    /// Status light key.
    pub status: Status,
    /// Whether the beacon is at the full status colour.
    pub rays: bool,
    /// Whether the bulb is at the full status colour.
    pub lit: bool,
    /// Status colour keys for the four chest LEDs in draw order.
    pub leds: [Status; 4],
    /// Dimensionless gauge needle setting.
    pub needle: f64,
    /// Dimensionless hover thrust.
    pub thrust: f64,
}

impl MoodLayout {
    pub(super) fn new(c: &Robot, mood: Mood) -> Self {
        let m = c.mood(mood);
        let p = c.pose(mood);
        let arms = [-1.0, 1.0].map(|side| (c.arms.style != "none").then(|| p.arm(side).into()));
        let masts = c
            .masts
            .iter()
            .map(|mast| mast_geom(c, mast, m).into())
            .collect();
        let stalks = if c.f.tech == Tech::Stalk {
            [-1.0, 1.0]
                .into_iter()
                .map(|side| stalk(c, side, m))
                .collect()
        } else {
            Vec::new()
        };
        Self {
            mood,
            lift: p.lift,
            lean: p.lean,
            drop: p.drop,
            tilt: p.tilt,
            landed: p.landed,
            arms,
            masts,
            stalks,
            cable: cable(c, m, p),
            grip: m.grip,
            ant: m.ant,
            ant_tilt: m.ant_tilt,
            tail: m.tail,
            status: m.status,
            rays: m.rays,
            lit: m.lit,
            leds: m.leds,
            needle: m.needle,
            thrust: m.thrust,
        }
    }
}
