//! Arms and hands.

use avatar_small_robots::layout::{ArmLayout, Frame};

use super::build::{Builder, NewPart, SkinSpec};
use super::fittings::{dir3, place, tan_deg};
use super::solids::{
    self, ball, block, ellipsoid, hose, merge, or_ball, quadratic, rod, rods, Affine, V3,
};
use crate::mesh;

const SIDES: [&str; 2] = ["L", "R"];

/// Half the width the 2D ink outline adds round an arm, in robot units. The
/// 3D arm has no outline, so every arm radius grows by this much to keep
/// the 2D arm's visual weight instead of reading as a wire.
pub(super) const ARM_INK: f64 = 2.0;

/// A point of an arm in robot space. The shoulders sit `shoulder_z` of the
/// body's depth in front of its middle, and every point below a shoulder
/// leans toward the viewer by the arm-tilt trait, so the front view keeps
/// the 2D arm.
pub(super) fn arm_point(b: &Builder, arm: &ArmLayout, p: [f64; 2]) -> V3 {
    let z0 = b.depth.shoulder_z * 2.0 * b.body_half_depth();
    let lean = tan_deg(b.depth.arm_tilt);
    b.at(
        Frame::UpperBody,
        p,
        z0 + (p[1] - arm.rest.shoulder[1]) * lean,
    )
}

/// The centre of a hand in robot space: nine of its size units along the
/// hand's direction from the wrist.
pub(super) fn hand_centre(b: &Builder, arm: &ArmLayout) -> V3 {
    let r = arm.rest;
    arm_point(
        b,
        arm,
        place(r.wrist, r.hand_dir, 0.0, 9.0 * arm.hand_scale),
    )
}

/// Both arms: an upper and a lower segment on their own bones, or one
/// skinned hose, each with the metal shoulder joint the 2D draws over the
/// shoulder and ending in its hand.
pub(super) fn arms(b: &mut Builder) {
    for side in 0..2 {
        let Some(arm) = b.layout.arms[side] else {
            continue;
        };
        let x = SIDES[side];
        let r = arm.rest;
        let body = b.find("body");
        let s = arm_point(b, &arm, r.shoulder);
        let e = arm_point(b, &arm, r.elbow);
        let t = arm_point(b, &arm, r.wrist);
        let upper = b.bone(format!("upperarm.{x}"), Some(body), s);
        let fore = b.bone(format!("forearm.{x}"), Some(upper), e);
        let wrist = b.bone(format!("wrist.{x}"), Some(fore), t);
        let grip = b.bone(format!("grip.{x}"), Some(wrist), t);
        let material = b.mats.named("arm", arm.material.base);
        let metal = b.mats.metal();
        let w = arm.width;
        let entry = ("arms", arm.kind);
        let put = |b: &mut Builder, role: &'static str, material: usize, bone: usize, mesh| {
            b.rigid(
                format!("arm.{x}.{role}"),
                entry,
                role,
                arm.class,
                Frame::UpperBody,
                r.shoulder,
                material,
                bone,
                mesh,
            );
        };
        match arm.kind {
            "hose" => {
                // The 2D hose's control point, from which its curve passes
                // through the elbow.
                let c = [0, 1, 2].map(|k| 2.0 * e[k] - 0.5 * (s[k] + t[k]));
                let points = quadratic(s, c, t, 12);
                let mesh = hose(&points, 0.5 * w + ARM_INK);
                let skin = along_arm(&mesh, &points, [upper, fore, wrist]);
                b.add(NewPart {
                    name: format!("arm.{x}.hose-arm"),
                    entry,
                    role: "hose-arm",
                    class: arm.class,
                    frame: Frame::UpperBody,
                    placement: r.shoulder,
                    material,
                    mesh,
                    skin,
                });
            }
            "stub" => put(b, "stub-arm", material, upper, rod(s, t, 0.5 * w + ARM_INK)),
            "telescope" => {
                put(
                    b,
                    "arm-upper",
                    material,
                    upper,
                    rod(s, e, 0.5 * w + ARM_INK),
                );
                put(b, "arm-lower", metal, fore, rod(e, t, 0.25 * w + ARM_INK));
            }
            kind => {
                let (lower, joint) = if kind == "shell" {
                    (0.45 * w, 0.46 * w * arm.joint_scale)
                } else {
                    (0.5 * w, 0.55 * w * arm.joint_scale)
                };
                put(
                    b,
                    "arm-upper",
                    material,
                    upper,
                    rod(s, e, 0.5 * w + ARM_INK),
                );
                put(b, "arm-lower", material, fore, rod(e, t, lower + ARM_INK));
                put(
                    b,
                    "elbow",
                    metal,
                    fore,
                    ball(e, joint.max(0.5 * w) + ARM_INK),
                );
            }
        }
        // The 2D draws a metal disc of radius (w/2 + 2.6) times the joint
        // scale over the shoulder, with its outline; a ball of that size
        // reads as the same disc from the front and as a joint from the side.
        let shoulder = (0.5 * w + 2.6) * arm.joint_scale + 0.5 * ARM_INK;
        put(b, "shoulder", metal, upper, ball(s, shoulder));
        hand(b, &arm, side, wrist, grip);
    }
}

/// Weights for a tube built ring by ring along `points`: each ring blends
/// the three bones by its place along the path, the first bone at the
/// start, the second at the middle and the third at the end. Ball ends
/// follow their ring.
fn along_arm(mesh: &mesh::Mesh, points: &[V3], bones: [usize; 3]) -> SkinSpec {
    let n = points.len();
    let rings = n * super::solids::ROD as usize;
    let mut joints = Vec::with_capacity(mesh.positions.len());
    let mut weights = Vec::with_capacity(mesh.positions.len());
    for i in 0..mesh.positions.len() {
        let t = if i < rings {
            (i / super::solids::ROD as usize) as f64 / (n - 1) as f64
        } else if i < rings + (mesh.positions.len() - rings) / 2 {
            0.0
        } else {
            1.0
        };
        let (j, w) = if t <= 0.5 {
            ([bones[0], bones[1]], 2.0 * t)
        } else {
            ([bones[1], bones[2]], 2.0 * t - 1.0)
        };
        joints.push([j[0] as u32, j[1] as u32, 0, 0]);
        weights.push([1.0 - w, w, 0.0, 0.0]);
    }
    SkinSpec::Skinned(joints, weights)
}

/// A hand at the wrist, pointing along the hand's direction. Jaws hang on
/// the grip bone; the rest on the wrist.
fn hand(b: &mut Builder, arm: &ArmLayout, side: usize, wrist: usize, grip: usize) {
    let r = arm.rest;
    let hs = arm.hand_scale;
    let u = r.hand_dir;
    let x = SIDES[side];
    let kind = arm.hand;
    let entry = ("hand", kind);
    let p = |b: &Builder, across: f64, along: f64| {
        arm_point(b, arm, place(r.wrist, u, across * hs, along * hs))
    };
    let w = (3.3 * hs).max(2.6);
    let metal = b.mats.metal();
    let put = |b: &mut Builder, role: &'static str, material: usize, bone: usize, mesh| {
        b.rigid(
            format!("hand.{x}.{role}"),
            entry,
            role,
            arm.class,
            Frame::UpperBody,
            r.wrist,
            material,
            bone,
            mesh,
        );
    };
    let chain = |b: &Builder, pts: &[[f64; 2]], radius: f64| {
        let pts: Vec<V3> = pts.iter().map(|q| p(b, q[0], q[1])).collect();
        rods(&pts, radius)
    };
    let along = dir3(u);
    match kind {
        "pincer" => {
            put(b, "hand", metal, wrist, ball(p(b, 0.0, 2.0), 0.9 * w));
            let jaws = merge(vec![
                chain(
                    b,
                    &[[-3.0, 3.0], [-10.0, 9.0], [-8.5, 18.0], [-6.5, 22.5]],
                    0.72 * w,
                ),
                chain(
                    b,
                    &[[3.0, 3.0], [8.0, 8.5], [6.5, 15.0], [5.0, 19.0]],
                    0.55 * w,
                ),
            ]);
            put(b, "jaw", metal, grip, jaws);
        }
        "claw" => {
            put(
                b,
                "hand",
                metal,
                wrist,
                chain(b, &[[0.0, 3.0], [0.0, 19.0]], 0.5 * w),
            );
            let jaws = merge(vec![
                chain(b, &[[-2.5, 2.5], [-8.5, 9.0], [-7.5, 17.5]], 0.5 * w),
                chain(b, &[[2.5, 2.5], [8.5, 9.0], [7.5, 17.5]], 0.5 * w),
            ]);
            put(b, "jaw", metal, grip, jaws);
        }
        "mitt" => {
            let material = if b.layout.palette.accent_kind == 3 {
                b.mats.rubber()
            } else {
                metal
            };
            let mesh = ellipsoid([0.0; 3], [8.0 * hs, 9.5 * hs, 4.0 * hs]);
            let mesh = solids::transform(mesh, &Affine::along(along, p(b, 0.0, 9.5)));
            put(b, "hand", material, wrist, mesh);
        }
        "ball" => put(b, "hand", metal, wrist, ball(p(b, 0.0, 8.5), 8.2 * hs)),
        "magnet" => {
            let red = b.mats.magnet();
            let body = chain(
                b,
                &[
                    [-6.5, 14.0],
                    [-6.5, 7.0],
                    [-4.0, 3.5],
                    [4.0, 3.5],
                    [6.5, 7.0],
                    [6.5, 14.0],
                ],
                0.75 * w,
            );
            put(b, "hand", red, wrist, body);
            let tip = b.mats.magnet_tip();
            let tips = merge(vec![
                chain(b, &[[-6.5, 14.0], [-6.5, 16.5]], 0.75 * w),
                chain(b, &[[6.5, 14.0], [6.5, 16.5]], 0.75 * w),
            ]);
            put(b, "hand-tip", tip, wrist, tips);
        }
        "sucker" => {
            put(
                b,
                "hand",
                metal,
                wrist,
                chain(b, &[[0.0, 0.0], [0.0, 5.0]], 0.55 * w),
            );
            let cup = or_ball(
                mesh::lathe(
                    &[
                        [0.0, 4.0 * hs],
                        [3.5 * hs, 4.0 * hs],
                        [10.5 * hs, 14.5 * hs],
                        [0.0, 14.5 * hs],
                    ],
                    16,
                ),
                [0.0, 9.0 * hs, 0.0],
                5.0 * hs,
            );
            let rubber = b.mats.rubber();
            put(
                b,
                "hand-cup",
                rubber,
                wrist,
                solids::transform(cup, &Affine::along(along, p(b, 0.0, 0.0))),
            );
        }
        "fork" => {
            let mesh = merge(vec![
                chain(
                    b,
                    &[[-6.0, 17.5], [-6.0, 5.5], [6.0, 5.5], [6.0, 17.5]],
                    0.5 * w,
                ),
                chain(b, &[[0.0, 0.0], [0.0, 5.5]], 0.55 * w),
            ]);
            put(b, "hand", metal, wrist, mesh);
        }
        _ => {
            let palm = block(
                Affine::along(along, p(b, 0.0, 1.8)),
                [4.6 * hs, 2.8 * hs, 3.0 * hs],
                0.6 * hs,
            );
            put(b, "hand", metal, wrist, palm);
            let jaws = merge(vec![
                chain(
                    b,
                    &[[-3.5, 4.0], [-9.0, 9.0], [-9.0, 14.5], [-4.5, 18.5]],
                    0.5 * w,
                ),
                chain(
                    b,
                    &[[3.5, 4.0], [9.0, 9.0], [9.0, 14.5], [4.5, 18.5]],
                    0.5 * w,
                ),
            ]);
            put(b, "jaw", metal, grip, jaws);
        }
    }
}
