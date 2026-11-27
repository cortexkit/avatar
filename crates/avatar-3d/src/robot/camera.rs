//! The stage camera: a three-quarter view from slightly right of and above
//! the robot, framing it whole through everything it does: at rest, and in
//! every reaction and status loop.

use std::f64::consts::PI;

use avatar_small_robots::Mood;
use avatar_svg::wild::math;

use super::{Robot, Skin};
use crate::motion::math::m_apply;
use crate::motion::{Actor, Pose, LOOP_BLEND, REACTIONS, STATUS_LOOPS};

/// Degrees the stage view is turned toward +x, the viewer's right.
pub const AZIMUTH: f64 = 11.0;
/// Degrees the stage view looks down from.
pub const ELEVATION: f64 = 12.0;
/// The vertical field of view in degrees: a long lens, so the front reads
/// as the 2D drawing.
pub const FOV_Y: f64 = 20.0;
/// The frame's height in pixels; the frame is square.
pub const FRAME_PX: u32 = 288;
/// How far from the frame's centre, as a share of the half frame, the
/// bounds the robot reaches ([`REACH_SAMPLES_PER_SECOND`]) may reach.
pub const FILL: f64 = 0.9;
/// How often the poses that bound the stage are sampled: every reaction
/// played from rest in the neutral mood, from its start to its end, and
/// every status loop started in the neutral mood, from its start through
/// its fade-in and one whole period, all with the idle layer off.
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

/// The stage camera for a robot: it looks at the centre of the box that
/// holds every vertex at rest and in every sampled pose ([`reach`]) from
/// [`AZIMUTH`] and [`ELEVATION`], backed off until every corner of the box
/// lies within [`FILL`] of the square frame. The rest of the robot must be
/// built, since the poses come from its motion.
pub(super) fn stage(robot: &Robot) -> Camera {
    let (lo, hi) = reach(robot);
    frame(lo, hi)
}

/// The box holding every vertex of the robot at rest and in each pose
/// sampled [`REACH_SAMPLES_PER_SECOND`] times a second through every
/// reaction and status loop.
fn reach(robot: &Robot) -> ([f64; 3], [f64; 3]) {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let mut hold = |p: [f64; 3]| {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    };
    for p in robot.parts.iter().flat_map(|p| p.mesh.positions.iter()) {
        hold(*p);
    }
    let mut sample = |actor: &Actor<'_>, end: f64| {
        let steps = (end * REACH_SAMPLES_PER_SECOND).floor() as u32;
        for k in 0..=steps {
            posed(
                robot,
                &actor.evaluate(f64::from(k) / REACH_SAMPLES_PER_SECOND),
                &mut hold,
            );
        }
    };
    for reaction in REACTIONS {
        let mut actor = Actor::new(robot, Mood::Neutral).without_idle();
        actor.play(0.0, reaction);
        sample(&actor, robot.motion.reaction(reaction).duration);
    }
    for status in STATUS_LOOPS {
        let mut actor = Actor::new(robot, Mood::Neutral).without_idle();
        actor.set_loop(0.0, Some(status));
        sample(
            &actor,
            LOOP_BLEND + robot.motion.status_loop(status).duration,
        );
    }
    (lo, hi)
}

/// Calls `hold` with every vertex of the robot in a pose. Every rest
/// matrix is the translation to its joint, so a bone carries a rest point
/// `v` to `M · (v − joint)`.
fn posed(robot: &Robot, pose: &Pose, hold: &mut impl FnMut([f64; 3])) {
    let carry = |bone: usize, v: [f64; 3]| {
        let j = robot.bones[bone].joint;
        m_apply(&pose.bones[bone], [v[0] - j[0], v[1] - j[1], v[2] - j[2]])
    };
    for part in &robot.parts {
        match &part.skin {
            Skin::Rigid { bone } => {
                for v in &part.mesh.positions {
                    hold(carry(*bone, *v));
                }
            }
            Skin::Skinned { joints, weights } => {
                for (i, v) in part.mesh.positions.iter().enumerate() {
                    let mut p = [0.0; 3];
                    for (j, w) in joints[i].iter().zip(&weights[i]) {
                        if *w != 0.0 {
                            let q = carry(*j as usize, *v);
                            for k in 0..3 {
                                p[k] += w * q[k];
                            }
                        }
                    }
                    hold(p);
                }
            }
        }
    }
}

/// A camera with no position yet, for a robot whose stage is framed once
/// the rest of it is built.
pub(super) fn unset() -> Camera {
    Camera {
        eye: [0.0; 3],
        target: [0.0; 3],
        up: [0.0, 1.0, 0.0],
        fov_y_degrees: FOV_Y,
        aspect: 1.0,
        height_px: FRAME_PX,
    }
}

/// The stage view of a box: it looks at the box's centre in x and y, at
/// z = 0, from [`AZIMUTH`] and [`ELEVATION`], backed off until every
/// corner lies within [`FILL`] of the square frame.
fn frame(lo: [f64; 3], hi: [f64; 3]) -> Camera {
    let target = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.0];
    let (az, el) = (AZIMUTH * PI / 180.0, ELEVATION * PI / 180.0);
    let direction = [
        math::sin(az) * math::cos(el),
        math::sin(el),
        math::cos(az) * math::cos(el),
    ];
    let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    let mut distance = 0.5 * extent / tan(0.5 * FOV_Y);
    let mut camera = Camera {
        eye: [0.0; 3],
        target,
        up: [0.0, 1.0, 0.0],
        fov_y_degrees: FOV_Y,
        aspect: 1.0,
        height_px: FRAME_PX,
    };
    for _ in 0..16 {
        camera.eye = [0, 1, 2].map(|k| target[k] + direction[k] * distance);
        let mut over: f64 = 0.0;
        for i in 0..8 {
            let corner = [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] });
            let q = camera.project(corner);
            over = over.max(q[0].abs()).max(q[1].abs());
        }
        if over <= FILL {
            break;
        }
        distance *= over / FILL + 0.01;
    }
    camera
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
