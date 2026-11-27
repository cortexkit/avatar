//! What one frame draws of a posed robot, worked out on the CPU: the skin
//! matrices, each material's colour and emission, which face textures
//! show, where the anchors are and the ground shadow.
//!
//! The renderer uploads these as they are, so every choice a frame makes
//! can be checked here without a GPU.

use avatar_3d::motion::math::{m_apply, m_mul, M4};
use avatar_3d::motion::Pose;
use avatar_3d::robot::{Emission, LightKind, Robot, TextureContent};
use avatar_small_robots::Rgb;

/// The blink amount from which the blinking face and stalk-eye textures
/// show instead of the open ones.
pub const BLINK_SHOWS: f64 = 0.5;

/// How much a status light shows its own colour, unlit, when the pose's
/// glow is 0. Glow raises it to 1, so the light never changes colour, only
/// how strongly it shines.
pub const LAMP_REST_EMISSION: f64 = 0.85;

/// The textures a frame shows in the face slots, as indices into
/// [`Robot::textures`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Faces {
    /// The head's face: the face set's mood, blinking or not.
    pub head: Option<usize>,
    /// The left (−x) and right stalk eyes, on a stalk robot.
    pub stalks: [Option<usize>; 2],
}

/// The face textures a pose shows: the pose's face set, and the blinking
/// pair when its blink is at least [`BLINK_SHOWS`], on the face and the
/// stalk eyes together.
pub fn faces(robot: &Robot, pose: &Pose) -> Faces {
    let blinking = pose.blink >= BLINK_SHOWS;
    let find = |want: TextureContent| {
        robot
            .textures
            .iter()
            .position(|t| t.binding.content == want)
    };
    Faces {
        head: find(TextureContent::Face {
            mood: pose.face,
            blinking,
        }),
        stalks: [0, 1].map(|eye| {
            find(TextureContent::StalkEye {
                eye,
                mood: pose.face,
                blinking,
            })
        }),
    }
}

/// The slot a part's face texture fills in [`Faces`]: 1 for the head's
/// face, 2 and 3 for the left and right stalk eyes, 0 when no face texture
/// lies on the part.
pub fn face_slot(robot: &Robot, part: usize) -> u32 {
    robot
        .textures
        .iter()
        .filter(|t| t.binding.part == part)
        .find_map(|t| match t.binding.content {
            TextureContent::Face { .. } => Some(1),
            TextureContent::StalkEye { eye, .. } => Some(2 + eye as u32),
            _ => None,
        })
        .unwrap_or(0)
}

/// The marks texture lying on a part, if any.
pub fn marks(robot: &Robot, part: usize) -> Option<usize> {
    robot
        .textures
        .iter()
        .position(|t| t.binding.part == part && t.binding.content == TextureContent::Marks)
}

/// Each bone's skin matrix for a pose, `M_j · R_j⁻¹`: it carries a vertex
/// from its rest position to its posed position in model space.
pub fn skin_matrices(robot: &Robot, pose: &Pose) -> Vec<M4> {
    robot
        .bones
        .iter()
        .zip(&pose.bones)
        .map(|(bone, m)| m_mul(m, &inverse(&bone.rest)))
        .collect()
}

/// The matrix that carries normals the way `m` carries points: the inverse
/// transpose of its upper 3 × 3, with no translation.
pub fn normal_matrix(m: &M4) -> M4 {
    let inv = inverse(m);
    let mut out = [0.0; 16];
    for c in 0..3 {
        for r in 0..3 {
            out[c * 4 + r] = inv[r * 4 + c];
        }
    }
    out[15] = 1.0;
    out
}

/// The inverse of an affine column-major matrix.
pub fn inverse(m: &M4) -> M4 {
    let a = |r: usize, c: usize| m[c * 4 + r];
    let cof =
        |r0: usize, r1: usize, c0: usize, c1: usize| a(r0, c0) * a(r1, c1) - a(r0, c1) * a(r1, c0);
    // The adjugate of the upper 3 × 3, row by row.
    let adj = [
        [cof(1, 2, 1, 2), -cof(0, 2, 1, 2), cof(0, 1, 1, 2)],
        [-cof(1, 2, 0, 2), cof(0, 2, 0, 2), -cof(0, 1, 0, 2)],
        [cof(1, 2, 0, 1), -cof(0, 2, 0, 1), cof(0, 1, 0, 1)],
    ];
    let det = a(0, 0) * adj[0][0] + a(0, 1) * adj[1][0] + a(0, 2) * adj[2][0];
    let mut out = [0.0; 16];
    for r in 0..3 {
        for c in 0..3 {
            out[c * 4 + r] = adj[r][c] / det;
        }
    }
    for r in 0..3 {
        out[12 + r] = -(0..3).map(|k| out[k * 4 + r] * m[12 + k]).sum::<f64>();
    }
    out[15] = 1.0;
    out
}

/// A material's sRGB colour in a pose: a status light's comes from the
/// status colour table, read with the pose's status (an LED with its own
/// slot's), `lit` and `rays`; every other material keeps its base colour.
pub fn colour(robot: &Robot, material: usize, pose: &Pose) -> Rgb {
    let m = &robot.materials[material];
    match m.emission {
        Emission::Status { light } => {
            let status = match robot.status.lights[light].kind {
                LightKind::Led { slot } => pose.leds[slot],
                _ => pose.status,
            };
            robot.status.colour(light, status, pose.lit, pose.rays)
        }
        _ => m.base,
    }
}

/// How much a material shows its own colour unlit in a pose: a status
/// light [`LAMP_REST_EMISSION`] raised toward 1 by the glow, a jet's flame
/// the thrust, anything else nothing.
pub fn emission(robot: &Robot, material: usize, pose: &Pose) -> f64 {
    match robot.materials[material].emission {
        Emission::Status { .. } => {
            LAMP_REST_EMISSION + (1.0 - LAMP_REST_EMISSION) * pose.glow.clamp(0.0, 1.0)
        }
        Emission::Thrust => pose.thrust.clamp(0.0, 1.0),
        _ => 0.0,
    }
}

/// An anchor's position in a pose, in model space: a bone's anchor follows
/// its bone, `M_b · R_b⁻¹ · (joint + offset)`, and `ground` stays at its
/// offset whatever the root does.
pub fn anchor(robot: &Robot, pose: &Pose, name: &str) -> Option<[f64; 3]> {
    let a = robot.anchor(name)?;
    let Some(b) = a.bone else {
        return Some(a.offset);
    };
    let bone = &robot.bones[b];
    let rest = [0, 1, 2].map(|k| bone.joint[k] + a.offset[k]);
    let skin = m_mul(&pose.bones[b], &inverse(&bone.rest));
    Some(m_apply(&skin, rest))
}

/// The ground shadow: a soft disc on the floor under a robot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// The disc's centre, on the floor.
    pub centre: [f64; 3],
    /// Its half extents along x and z.
    pub radii: [f64; 2],
    /// Its opacity at the centre.
    pub opacity: f64,
}

/// The ground shadow of a robot in a pose. At rest it is an ellipse under
/// the middle of the rest bounds, 0.42 of their width across and at least
/// 0.5 of their depth deep (and never under 0.6 of its own width), at
/// opacity 0.26, or 0.16 under hover gear, which floats above its shadow.
/// When the root rises to height `h` above its rest height, with `k = h /
/// (h + height / 2)`, the disc shrinks by `1 − k/2` and fades by
/// `1 − 0.8 k`.
pub fn shadow(robot: &Robot, pose: &Pose) -> Shadow {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in robot.parts.iter().flat_map(|p| p.mesh.positions.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let rx = 0.42 * (hi[0] - lo[0]);
    let rz = (0.5 * (hi[2] - lo[2])).max(0.6 * rx);
    let root = robot.bone("root").expect("every robot has a root bone");
    let h = (pose.bones[root][13] - robot.bones[root].joint[1]).max(0.0);
    let height = robot.motion.height.max(1e-9);
    let k = h / (h + 0.5 * height);
    let scale = 1.0 - 0.5 * k;
    let base = if robot.gear.hover { 0.16 } else { 0.26 };
    Shadow {
        centre: [
            0.5 * (lo[0] + hi[0]) + pose.bones[root][12] - robot.bones[root].joint[0],
            0.0,
            0.5 * (lo[2] + hi[2]) + pose.bones[root][14] - robot.bones[root].joint[2],
        ],
        radii: [rx * scale, rz * scale],
        opacity: base * (1.0 - 0.8 * k),
    }
}
