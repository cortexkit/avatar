//! The stage camera: a three-quarter view from slightly right of and above
//! the robot, framing it whole through everything it does.
//!
//! A robot works out the box it reaches ([`super::Robot::reach`]) and its
//! camera when first asked, and [`frame_box`] frames that box from any yaw
//! and pitch, on any frame shape and at any zoom; from [`AZIMUTH`] and
//! [`ELEVATION`] on a square frame, unzoomed, it gives the robot's own
//! camera ([`super::Robot::camera`]).

use std::collections::HashSet;
use std::f64::consts::PI;

use avatar_small_robots::MOODS;
use avatar_svg::wild::math;

use super::{Robot, Skin};
use crate::motion::math::{m_apply, M4};
use crate::motion::{Actor, Pose, LOOP_BLEND, MOOD_BLEND, REACTIONS, STATUS_LOOPS};

/// Degrees the stage view is turned toward +x, the viewer's right.
pub const AZIMUTH: f64 = 11.0;
/// Degrees the stage view looks down from.
pub const ELEVATION: f64 = 12.0;
/// The vertical field of view in degrees: a long lens, so the front reads
/// as the 2D drawing.
pub const FOV_Y: f64 = 20.0;
/// The frame's height in pixels; the frame is square.
pub const FRAME_PX: u32 = 288;
/// How far from the frame's centre, as a share of the half frame, the box
/// the robot reaches ([`super::Robot::reach`]) may reach. The rest of the
/// frame is margin for what that box does not sample: the idle layer,
/// poses between samples, and one reaction cutting another short.
pub const FILL: f64 = 0.9;
/// How often the poses that bound the stage are sampled
/// ([`super::Robot::reach`]). Thirty a second keeps the sampling within
/// a robot's build budget; the robot tests check, 120 times a second, that
/// no posed vertex of the 33 robots of the plaza leaves the frame through
/// every mood, reaction and status loop with the idle layer on, which is
/// what the margin outside [`FILL`] is for.
pub const REACH_SAMPLES_PER_SECOND: f64 = 30.0;

/// A perspective camera as plain data, in model units.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Camera {
    /// Where the camera is.
    pub eye: [f64; 3],
    /// The point it looks at.
    pub target: [f64; 3],
    /// The up direction, +y.
    pub up: [f64; 3],
    /// The vertical field of view in degrees.
    pub fov_y_degrees: f64,
    /// Width over height of the frame.
    pub aspect: f64,
    /// The frame's height in pixels.
    pub height_px: u32,
}

impl Camera {
    /// A point's normalised device x and y (−1 to 1 across the frame) and
    /// its distance in front of the camera along the view direction.
    pub fn project(&self, p: [f64; 3]) -> [f64; 3] {
        let f = unit(sub(self.target, self.eye));
        let r = unit(cross(f, self.up));
        let u = cross(r, f);
        let v = sub(p, self.eye);
        let depth = dot(v, f);
        let k = tan(0.5 * self.fov_y_degrees);
        [
            dot(v, r) / (depth * k * self.aspect),
            dot(v, u) / (depth * k),
            depth,
        ]
    }
}

/// An axis-aligned box in model space: the points that lie between `lo`
/// and `hi` on every axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// The least x, y and z.
    pub lo: [f64; 3],
    /// The greatest x, y and z.
    pub hi: [f64; 3],
}

impl Bounds {
    /// No points yet: every axis runs from +∞ down to −∞.
    fn empty() -> Self {
        Self {
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
        }
    }

    /// Grows the box to hold `p`.
    fn hold(&mut self, p: [f64; 3]) {
        for k in 0..3 {
            self.lo[k] = self.lo[k].min(p[k]);
            self.hi[k] = self.hi[k].max(p[k]);
        }
    }
}

/// The box holding every vertex of the robot at rest and in each pose
/// sampled [`REACH_SAMPLES_PER_SECOND`] times a second through every
/// motion that starts from rest in a mood, with the idle layer off: in
/// each mood, its posture, every reaction from its start to its end, every
/// status loop through its fade-in and one whole period, every reaction
/// played over every status loop once the loop has faded in, and the blend
/// to each other mood.
fn reach(robot: &Robot) -> Bounds {
    let points = Points::new(robot);
    let mut bounds = Bounds::empty();
    for p in robot.parts.iter().flat_map(|p| p.mesh.positions.iter()) {
        bounds.hold(*p);
    }
    let mut sample = |actor: &Actor<'_>, end: f64| {
        let steps = (end * REACH_SAMPLES_PER_SECOND).floor() as u32;
        for k in 0..=steps {
            let pose = actor.evaluate(f64::from(k) / REACH_SAMPLES_PER_SECOND);
            points.hold(robot, &pose, &mut bounds);
        }
    };
    let motion = &robot.motion;
    for mood in MOODS {
        let still = || Actor::new(robot, mood).without_idle();
        sample(&still(), 0.0);
        for reaction in REACTIONS {
            let mut actor = still();
            actor.play(0.0, reaction);
            sample(&actor, motion.reaction(reaction).duration);
        }
        for status in STATUS_LOOPS {
            let mut actor = still();
            actor.set_loop(0.0, Some(status));
            sample(&actor, LOOP_BLEND + motion.status_loop(status).duration);
            for reaction in REACTIONS {
                let mut actor = still();
                actor.set_loop(0.0, Some(status));
                actor.play(LOOP_BLEND, reaction);
                sample(&actor, LOOP_BLEND + motion.reaction(reaction).duration);
            }
        }
        for other in MOODS.into_iter().filter(|m| *m != mood) {
            let mut actor = still();
            actor.set_mood(0.0, other);
            sample(&actor, MOOD_BLEND);
        }
    }
    bounds
}

/// A robot's rest vertices, ready to pose many times: the vertices of the
/// rigid parts gathered by bone, each relative to its bone's joint and
/// without repeats, and each skinned vertex once. Meshes repeat a position
/// wherever they split a vertex at a crease or seam; dropping the repeats
/// leaves the box the same.
struct Points {
    rigid: Vec<Group>,
    skinned: Vec<Blend>,
}

/// The rigid vertices one bone carries.
struct Group {
    bone: usize,
    /// Each vertex minus the bone's joint.
    offsets: Vec<[f64; 3]>,
    /// The box round `offsets`.
    extent: Extent,
}

/// A skinned vertex's four weights and its rest position.
type Weighted = ([f64; 4], [f64; 3]);

/// The skinned vertices that share one set of joint indices.
struct Blend {
    /// Each vertex with its weights.
    vertices: Vec<Weighted>,
    joints: [u32; 4],
    /// For each bone the vertices give a weight to, the box round the
    /// vertices minus that bone's joint.
    bones: Vec<(usize, Extent)>,
    /// Whether every weight is at least 0 and each vertex's weights add up
    /// to 1, so that each posed vertex lies among the points its bones
    /// carry it to and the boxes in `bones` bound it.
    convex: bool,
}

/// The centre and half extent of a box of points relative to a joint.
struct Extent {
    centre: [f64; 3],
    half: [f64; 3],
}

impl Extent {
    fn of<'a>(points: impl IntoIterator<Item = &'a [f64; 3]>) -> Self {
        let mut b = Bounds::empty();
        for p in points {
            b.hold(*p);
        }
        Self {
            centre: [0, 1, 2].map(|k| 0.5 * (b.lo[k] + b.hi[k])),
            half: [0, 1, 2].map(|k| 0.5 * (b.hi[k] - b.lo[k])),
        }
    }

    /// Whether every point of the box, carried by `m`, lies strictly inside
    /// `bounds`. The carried box is widened by far more than the rounding
    /// of either this calculation or of carrying a point on its own, so a
    /// box found inside holds no point that could grow `bounds`.
    fn inside(&self, m: &M4, bounds: &Bounds) -> bool {
        let c = m_apply(m, self.centre);
        (0..3).all(|k| {
            let extent = m[k].abs() * self.half[0]
                + m[4 + k].abs() * self.half[1]
                + m[8 + k].abs() * self.half[2];
            let slack = 1e-9 * (1.0 + c[k].abs() + extent);
            c[k] - extent - slack > bounds.lo[k] && c[k] + extent + slack < bounds.hi[k]
        })
    }
}

impl Points {
    fn new(robot: &Robot) -> Self {
        let mut rigid: Vec<(usize, Vec<[f64; 3]>)> = Vec::new();
        let mut skinned: Vec<([u32; 4], Vec<Weighted>)> = Vec::new();
        let mut seen = HashSet::new();
        let mut kept = HashSet::new();
        for part in &robot.parts {
            match &part.skin {
                Skin::Rigid { bone } => {
                    let j = robot.bones[*bone].joint;
                    let at = rigid.iter().position(|g| g.0 == *bone).unwrap_or_else(|| {
                        rigid.push((*bone, Vec::new()));
                        rigid.len() - 1
                    });
                    for v in &part.mesh.positions {
                        let offset = [v[0] - j[0], v[1] - j[1], v[2] - j[2]];
                        if seen.insert((*bone, offset.map(f64::to_bits))) {
                            rigid[at].1.push(offset);
                        }
                    }
                }
                Skin::Skinned { joints, weights } => {
                    for (i, v) in part.mesh.positions.iter().enumerate() {
                        let (js, ws) = (joints[i], weights[i]);
                        if !kept.insert((js, ws.map(f64::to_bits), v.map(f64::to_bits))) {
                            continue;
                        }
                        let at = skinned.iter().position(|b| b.0 == js).unwrap_or_else(|| {
                            skinned.push((js, Vec::new()));
                            skinned.len() - 1
                        });
                        skinned[at].1.push((ws, *v));
                    }
                }
            }
        }
        let rigid = rigid
            .into_iter()
            .map(|(bone, offsets)| Group {
                bone,
                extent: Extent::of(&offsets),
                offsets,
            })
            .collect();
        let skinned = skinned
            .into_iter()
            .map(|(joints, vertices)| {
                let mut bones = Vec::new();
                for slot in 0..4 {
                    let bone = joints[slot] as usize;
                    if vertices.iter().all(|(w, _)| w[slot] == 0.0)
                        || bones.iter().any(|b: &(usize, Extent)| b.0 == bone)
                    {
                        continue;
                    }
                    let j = robot.bones[bone].joint;
                    let offsets: Vec<[f64; 3]> = vertices
                        .iter()
                        .map(|(_, v)| [v[0] - j[0], v[1] - j[1], v[2] - j[2]])
                        .collect();
                    bones.push((bone, Extent::of(&offsets)));
                }
                let convex = vertices.iter().all(|(w, _)| {
                    w.iter().all(|x| *x >= 0.0) && (w[0] + w[1] + w[2] + w[3] - 1.0).abs() < 1e-12
                });
                Blend {
                    vertices,
                    joints,
                    bones,
                    convex,
                }
            })
            .collect();
        Self { rigid, skinned }
    }

    /// Grows `bounds` to hold every vertex in a pose. Every rest matrix is
    /// the translation to its joint, so a bone carries a rest point `v` to
    /// `M · (v − joint)`.
    ///
    /// Vertices are carried one by one only where the box round them,
    /// carried by their bones, might reach past `bounds`; elsewhere none of
    /// them can grow it, so the result is the same as carrying every
    /// vertex.
    fn hold(&self, robot: &Robot, pose: &Pose, bounds: &mut Bounds) {
        for g in &self.rigid {
            let m = &pose.bones[g.bone];
            if !g.extent.inside(m, bounds) {
                for v in &g.offsets {
                    bounds.hold(m_apply(m, *v));
                }
            }
        }
        let carry = |bone: usize, v: [f64; 3]| {
            let j = robot.bones[bone].joint;
            m_apply(&pose.bones[bone], [v[0] - j[0], v[1] - j[1], v[2] - j[2]])
        };
        for b in &self.skinned {
            if b.convex
                && b.bones
                    .iter()
                    .all(|(bone, r)| r.inside(&pose.bones[*bone], bounds))
            {
                continue;
            }
            for (weights, v) in &b.vertices {
                let mut p = [0.0; 3];
                for (j, w) in b.joints.iter().zip(weights) {
                    if *w != 0.0 {
                        let q = carry(*j as usize, *v);
                        for k in 0..3 {
                            p[k] += w * q[k];
                        }
                    }
                }
                bounds.hold(p);
            }
        }
    }
}

/// A robot's reach box and its stage camera: [`frame_box`] of that box
/// from [`AZIMUTH`] and [`ELEVATION`] on a square frame, unzoomed.
pub(super) fn stage(robot: &Robot) -> (Bounds, Camera) {
    let reach = reach(robot);
    let camera = frame_box(reach.lo, reach.hi, AZIMUTH, ELEVATION, 1.0, 1.0);
    (reach, camera)
}

/// The camera that frames the box from `lo` to `hi`. It looks at the box's
/// centre in x and y, at z = 0, from `yaw_deg` degrees round toward +x from
/// the front (+z) and `pitch_deg` degrees above the floor's plane, with the
/// vertical field of view [`FOV_Y`] on a frame `aspect` times as wide as it
/// is high. It starts where the box's larger extent in x and y spans the
/// field of view and backs off, multiplying the distance by
/// `over / FILL + 0.01`, until every corner of the box projects within
/// [`FILL`] of the frame's centre on both axes; `over` is the farthest any
/// corner projects, as a share of the half frame. While a corner lies
/// behind the eye, or too near it to project, it first doubles the
/// distance, at least to twice the farthest that corner lies toward the
/// eye from the target. Last, it multiplies the distance by `zoom`. The
/// camera's up is +y and its frame height [`FRAME_PX`].
///
/// A robot's [`camera`](super::Robot::camera) is
/// `frame_box(reach.lo, reach.hi, AZIMUTH, ELEVATION, 1.0, 1.0)` of its
/// [`reach`](super::Robot::reach), bit for bit.
///
/// # Panics
///
/// Panics outside its domain, which is:
///
/// - `lo` and `hi` finite, with `lo ≤ hi` on every axis;
/// - `yaw_deg` and `pitch_deg` finite, with the pitch's cosine, as the
///   portable [`math::cos`] computes it, not zero: the view is never
///   straight up or down, along the camera's up (at −90° or 270°, for
///   example);
/// - `aspect` and `zoom` finite and greater than 0;
/// - a framing distance, zoom included, that an f64 holds: finite, and
///   not so small that the eye cannot be told apart from the target.
pub fn frame_box(
    lo: [f64; 3],
    hi: [f64; 3],
    yaw_deg: f64,
    pitch_deg: f64,
    aspect: f64,
    zoom: f64,
) -> Camera {
    assert!(
        lo.iter().chain(&hi).all(|v| v.is_finite()) && (0..3).all(|k| lo[k] <= hi[k]),
        "frame_box: the box must be finite with lo <= hi on every axis, not {lo:?} to {hi:?}"
    );
    assert!(
        yaw_deg.is_finite() && pitch_deg.is_finite(),
        "frame_box: the yaw and pitch must be finite, not {yaw_deg} and {pitch_deg}"
    );
    assert!(
        aspect.is_finite() && aspect > 0.0,
        "frame_box: the aspect must be finite and greater than 0, not {aspect}"
    );
    assert!(
        zoom.is_finite() && zoom > 0.0,
        "frame_box: the zoom must be finite and greater than 0, not {zoom}"
    );
    // Whole turns are taken off in degrees first, which is exact, so the
    // portable sine only ever sees angles within one turn.
    let (az, el) = (
        (yaw_deg % 360.0) * PI / 180.0,
        (pitch_deg % 360.0) * PI / 180.0,
    );
    let level = math::cos(el);
    assert!(
        level != 0.0,
        "frame_box: a pitch of {pitch_deg} degrees looks straight along the camera's up"
    );
    let direction = [math::sin(az) * level, math::sin(el), math::cos(az) * level];
    let target = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.0];
    let corners: [[f64; 3]; 8] =
        std::array::from_fn(|i| [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }));
    let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    let mut distance = 0.5 * extent / tan(0.5 * FOV_Y);
    let mut camera = Camera {
        eye: [0.0; 3],
        target,
        up: [0.0, 1.0, 0.0],
        fov_y_degrees: FOV_Y,
        aspect,
        height_px: FRAME_PX,
    };
    // Every step that does not fit multiplies the distance by more than
    // 1.01, or at least doubles it, so the loop ends: the box fits once
    // the distance is large enough, and an overflow panics first.
    loop {
        assert!(
            distance.is_finite(),
            "frame_box: no finite distance frames {lo:?} to {hi:?} on a frame of aspect {aspect}"
        );
        camera.eye = place(target, direction, distance);
        match overshoot(&camera, &corners) {
            Some(over) if over <= FILL => break,
            Some(over) => distance *= over / FILL + 0.01,
            None => {
                let ahead = corners
                    .iter()
                    .map(|c| dot(sub(*c, target), direction))
                    .fold(0.0, f64::max);
                distance = (2.0 * distance).max(2.0 * ahead).max(f64::MIN_POSITIVE);
            }
        }
    }
    let zoomed = distance * zoom;
    assert!(
        zoomed.is_finite(),
        "frame_box: zoom {zoom} takes the framing distance {distance} past the largest f64"
    );
    camera.eye = place(target, direction, zoomed);
    assert!(
        camera.project(target).iter().all(|v| v.is_finite()),
        "frame_box: zoom {zoom} brings the eye too close to the target to tell them apart"
    );
    camera
}

/// The eye `distance` from `target` along the unit `direction`.
fn place(target: [f64; 3], direction: [f64; 3], distance: f64) -> [f64; 3] {
    [0, 1, 2].map(|k| target[k] + direction[k] * distance)
}

/// The farthest any corner projects from the frame's centre on either
/// axis, as a share of the half frame, or `None` when a corner lies behind
/// the eye or does not project to a finite point.
fn overshoot(camera: &Camera, corners: &[[f64; 3]; 8]) -> Option<f64> {
    let mut over: f64 = 0.0;
    for corner in corners {
        let q = camera.project(*corner);
        if !(q.iter().all(|v| v.is_finite()) && q[2] > 0.0) {
            return None;
        }
        over = over.max(q[0].abs()).max(q[1].abs());
    }
    Some(over)
}

/// The tangent of an angle in degrees, within the portable tangent's
/// domain for the half fields of view used here.
fn tan(degrees: f64) -> f64 {
    let a = degrees * PI / 180.0;
    math::sin(a) / math::cos(a)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}
