//! The floor rule: hands turn the arm forward before they reach the floor,
//! the hover exhaust shrinks toward its nozzle, and nothing else may go
//! below the floor either.

use super::actor::Build;
use super::math::{m_height, q_from_axis_angle, q_mul, M4, V3};
use crate::robot::{Robot, Skin, UNIT};

/// How close to the floor a hand may come, in robot units.
pub const HAND_CLEARANCE: f64 = 2.5;

/// How far below the floor, in model units, a vertex may round to before
/// the root is raised. Parts standing on the floor reach it through a chain
/// of bone offsets whose sum rounds to a few 1e-17 either side of zero; the
/// root does not move for that.
const ROUNDING: f64 = 1e-12;

/// The precomputed boxes the floor rule measures with.
#[derive(Clone, Debug)]
pub(super) struct Floor {
    /// For each bone: the rigid parts it carries.
    rigid: Vec<Vec<usize>>,
    /// For each bone: the skinned parts that weight it.
    skinned: Vec<Vec<usize>>,
    /// For each bone: the bounds, relative to its joint at rest, of every
    /// vertex it carries or weights; `None` for a bone that moves nothing.
    boxes: Vec<Option<[V3; 2]>>,
    /// For each arm, the height its hand keeps above the floor.
    arms: Vec<f64>,
}

/// The lowest height of a box relative to a bone's joint, carried by the
/// bone's matrix: a bound below every vertex in the box.
fn box_low(m: &M4, [lo, hi]: [V3; 2]) -> f64 {
    let pick = |a: f64, k: usize| (a * lo[k]).min(a * hi[k]);
    pick(m[1], 0) + pick(m[5], 1) + pick(m[9], 2) + m[13]
}

impl Floor {
    pub fn new(robot: &Robot) -> Self {
        let n = robot.bones.len();
        let mut rigid = vec![Vec::new(); n];
        let mut skinned = vec![Vec::new(); n];
        let mut boxes: Vec<Option<[V3; 2]>> = vec![None; n];
        let mut grow = |bone: usize, p: V3| {
            let j = robot.bones[bone].joint;
            let l = [p[0] - j[0], p[1] - j[1], p[2] - j[2]];
            let b = boxes[bone].get_or_insert([l, l]);
            for k in 0..3 {
                b[0][k] = b[0][k].min(l[k]);
                b[1][k] = b[1][k].max(l[k]);
            }
        };
        for (i, part) in robot.parts.iter().enumerate() {
            match &part.skin {
                Skin::Rigid { bone } => {
                    rigid[*bone].push(i);
                    for p in &part.mesh.positions {
                        grow(*bone, *p);
                    }
                }
                Skin::Skinned { joints, weights } => {
                    for (v, (js, ws)) in joints.iter().zip(weights).enumerate() {
                        for k in 0..4 {
                            if ws[k] > 0.0 {
                                let bone = js[k] as usize;
                                if !skinned[bone].contains(&i) {
                                    skinned[bone].push(i);
                                }
                                grow(bone, part.mesh.positions[v]);
                            }
                        }
                    }
                }
            }
        }
        let rest: Vec<M4> = robot.bones.iter().map(|b| b.rest).collect();
        let arms = robot
            .motion
            .arms
            .iter()
            .map(|a| {
                let low = hand_low(&boxes, &rest, a.wrist, a.grip);
                (HAND_CLEARANCE * UNIT).min(low).max(1e-4) - 1e-4
            })
            .collect();
        Self {
            rigid,
            skinned,
            boxes,
            arms,
        }
    }

    /// Applies the floor rule to a built pose with the given thrust.
    pub fn constrain(&self, b: &mut Build, thrust: f64) {
        self.hands(b);
        self.exhaust(b, thrust);
        self.lift(b);
    }

    /// A hand that would come closer to the floor than it is allowed turns
    /// its whole arm forward (toward +z) about the shoulder by the smallest
    /// angle that clears it, found by bisection; the hand is measured by the
    /// corners of its rest bounds as the wrist and grip carry them.
    fn hands(&self, b: &mut Build) {
        let robot: &Robot = b.robot;
        for (a, &floor) in robot.motion.arms.iter().zip(&self.arms) {
            let q0 = b.slots[a.upper].q;
            let low = |b: &mut Build, phi: f64| {
                b.slots[a.upper].q = q_mul(q_from_axis_angle([1.0, 0.0, 0.0], -phi), q0);
                b.update_from(a.upper);
                hand_low(&self.boxes, &b.world, a.wrist, a.grip)
            };
            if hand_low(&self.boxes, &b.world, a.wrist, a.grip) >= floor {
                continue;
            }
            let (mut lo, mut hi) = (0.0, std::f64::consts::PI * 0.6);
            if low(b, hi) < floor {
                continue;
            }
            for _ in 0..12 {
                let mid = (lo + hi) / 2.0;
                if low(b, mid) >= floor {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            low(b, hi);
        }
    }

    /// Scales the exhaust along its bone's y by `min(thrust, s)`, where `s`
    /// is the largest scale that keeps every exhaust vertex at or above the
    /// floor.
    fn exhaust(&self, b: &mut Build, thrust: f64) {
        let Some(bone) = b.robot.motion.exhaust else {
            return;
        };
        let m = b.world[bone];
        let j = b.robot.bones[bone].joint;
        let mut k = thrust.max(0.0);
        for &i in &self.rigid[bone] {
            for p in &b.robot.parts[i].mesh.positions {
                let l = [p[0] - j[0], p[1] - j[1], p[2] - j[2]];
                // The vertex's height is fixed + k · along.
                let fixed = m[1] * l[0] + m[9] * l[2] + m[13];
                let along = m[5] * l[1];
                if along < 0.0 {
                    k = k.min((fixed / -along).max(0.0));
                }
            }
        }
        b.slots[bone].s[1] = k;
        b.update_from(bone);
    }

    /// Raises the root by however far the lowest vertex has gone below the
    /// floor, beyond rounding. Each bone's box gives a bound below its
    /// vertices; only bones whose bound is below the floor have their
    /// vertices measured.
    fn lift(&self, b: &mut Build) {
        let suspects: Vec<usize> = (0..self.boxes.len())
            .filter(|&j| self.boxes[j].is_some_and(|bx| box_low(&b.world[j], bx) < -ROUNDING))
            .collect();
        if suspects.is_empty() {
            return;
        }
        let robot = b.robot;
        let mut lowest: f64 = 0.0;
        let mut seen = Vec::new();
        for &j in &suspects {
            let joint = robot.bones[j].joint;
            for &i in &self.rigid[j] {
                for p in &robot.parts[i].mesh.positions {
                    let l = [p[0] - joint[0], p[1] - joint[1], p[2] - joint[2]];
                    lowest = lowest.min(m_height(&b.world[j], l));
                }
            }
            for &i in &self.skinned[j] {
                if seen.contains(&i) {
                    continue;
                }
                seen.push(i);
                let part = &robot.parts[i];
                let Skin::Skinned { joints, weights } = &part.skin else {
                    continue;
                };
                for (v, p) in part.mesh.positions.iter().enumerate() {
                    let mut y = 0.0;
                    for k in 0..4 {
                        if weights[v][k] == 0.0 {
                            continue;
                        }
                        let bone = joints[v][k] as usize;
                        let joint = robot.bones[bone].joint;
                        let l = [p[0] - joint[0], p[1] - joint[1], p[2] - joint[2]];
                        y += weights[v][k] * m_height(&b.world[bone], l);
                    }
                    lowest = lowest.min(y);
                }
            }
        }
        if lowest < -ROUNDING {
            b.slots[0].t[1] -= lowest;
            b.update_from(0);
        }
    }
}

/// The lowest corner of a hand's bounds, as its wrist and grip bones carry
/// them.
fn hand_low(boxes: &[Option<[V3; 2]>], world: &[M4], wrist: usize, grip: usize) -> f64 {
    [wrist, grip]
        .iter()
        .filter_map(|&j| boxes[j].map(|bx| box_low(&world[j], bx)))
        .fold(f64::INFINITY, f64::min)
}
