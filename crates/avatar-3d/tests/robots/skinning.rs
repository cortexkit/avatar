//! Posing from the exported data alone: rigid and skinned points, anchors,
//! and the stage camera, which holds the robot through every reaction and
//! status loop.

use avatar_3d::motion::{Actor, LOOP_BLEND, REACTIONS, STATUS_LOOPS};
use avatar_3d::robot::{build_with, BuildOptions, Robot, Skin, FILL, REACH_SAMPLES_PER_SECOND};
use avatar_small_robots::Mood;

use crate::cases;

type M4 = [f64; 16];
type V3 = [f64; 3];

/// Column-major product `a · b`.
fn mul(a: &M4, b: &M4) -> M4 {
    let mut out = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    out
}

fn apply(m: &M4, p: V3) -> V3 {
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

fn translation(t: V3) -> M4 {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, t[0], t[1], t[2], 1.0,
    ]
}

/// The inverse of a rest matrix, which must be a pure translation: every
/// bone's rest rotation is the identity.
fn inverse_rest(m: &M4) -> M4 {
    assert_eq!(
        &m[..12],
        &translation([0.0; 3])[..12],
        "a rest matrix with a rotation"
    );
    translation([-m[12], -m[13], -m[14]])
}

/// Whether bone `j` is `ancestor` or below it.
fn under(robot: &Robot, mut j: usize, ancestor: usize) -> bool {
    loop {
        if j == ancestor {
            return true;
        }
        match robot.bones[j].parent {
            Some(p) => j = p,
            None => return false,
        }
    }
}

/// Model-space joint matrices for a pose that turns `body` a quarter turn
/// about +z round its joint, carrying every bone below it.
fn quarter_turn(robot: &Robot) -> (Vec<M4>, V3) {
    let body = robot.bone("body").unwrap();
    let b = robot.bones[body].joint;
    let turn = [
        0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let about = mul(
        &translation(b),
        &mul(&turn, &translation([-b[0], -b[1], -b[2]])),
    );
    let joints = (0..robot.bones.len())
        .map(|j| {
            let rest = robot.bones[j].rest;
            if under(robot, j, body) {
                mul(&about, &rest)
            } else {
                rest
            }
        })
        .collect();
    (joints, b)
}

/// The quarter turn about the body's joint `b`, worked by hand:
/// (x, y) → (b.x − (y − b.y), b.y + (x − b.x)).
fn turned(b: V3, p: V3) -> V3 {
    [b[0] - (p[1] - b[1]), b[1] + (p[0] - b[0]), p[2]]
}

fn close(a: V3, b: V3) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-12)
}

#[test]
fn rigid_and_skinned_points_and_anchors_follow_a_pose() {
    // wild-port-0077 trails a cable skinned between the body and the floor.
    let (_, genome) = cases::corpus()
        .into_iter()
        .find(|(id, _)| id == "wild-port-0077")
        .unwrap();
    let robot = build_with(&genome, BuildOptions::untextured());
    let (joints, b) = quarter_turn(&robot);
    let skin_matrix = |j: usize| mul(&joints[j], &inverse_rest(&robot.bones[j].rest));

    // A rigid point: the head's first vertex turns with the body.
    let head = robot.parts.iter().find(|p| p.name == "head").unwrap();
    let Skin::Rigid { bone } = head.skin else {
        panic!("the head is rigid")
    };
    let v = head.mesh.positions[0];
    assert!(close(apply(&skin_matrix(bone), v), turned(b, v)));

    // A skinned point: a cable vertex halfway along blends the turned body
    // and the unmoved root by its weights.
    let cable = robot.parts.iter().find(|p| p.name == "cable").unwrap();
    let Skin::Skinned {
        joints: js,
        weights: ws,
    } = &cable.skin
    else {
        panic!("the cable is skinned")
    };
    let i = (0..ws.len())
        .find(|&i| ws[i][0] > 0.3 && ws[i][0] < 0.7)
        .expect("a cable vertex between its bones");
    let v = cable.mesh.positions[i];
    let posed = (0..4).fold([0.0; 3], |acc, k| {
        let p = apply(&skin_matrix(js[i][k] as usize), v);
        [0, 1, 2].map(|c| acc[c] + ws[i][k] * p[c])
    });
    assert_eq!(robot.bones[js[i][0] as usize].name, "cable");
    assert_eq!(robot.bones[js[i][1] as usize].name, "root");
    let t = turned(b, v);
    let want = [0, 1, 2].map(|c| ws[i][0] * t[c] + ws[i][1] * v[c]);
    assert!(close(posed, want), "{posed:?} != {want:?}");

    // Anchors: headTop turns with the head; ground stays at the contact point.
    let top = robot.anchor("headTop").unwrap();
    let bone = top.bone.unwrap();
    let rest = robot.bones[bone].joint;
    let at = [0, 1, 2].map(|k| rest[k] + top.offset[k]);
    assert!(close(apply(&skin_matrix(bone), at), turned(b, at)));
    let ground = robot.anchor("ground").unwrap();
    assert_eq!((ground.bone, ground.offset), (None, [0.0; 3]));
}

#[test]
fn the_stage_camera_is_the_three_quarter_view() {
    let (_, genome) = cases::corpus().into_iter().next().unwrap();
    let robot = build_with(&genome, BuildOptions::untextured());
    let c = robot.camera;
    assert_eq!(
        (c.up, c.fov_y_degrees, c.aspect, c.height_px),
        ([0.0, 1.0, 0.0], 20.0, 1.0, 288)
    );
    // Pinned for wild-port-0005.
    assert!(close(c.eye, PINNED_EYE), "eye {:?}", c.eye);
    assert!(close(c.target, PINNED_TARGET), "target {:?}", c.target);
    // It looks from 11 degrees toward +x and 12 degrees above:
    // (sin 11° cos 12°, sin 12°, cos 11° cos 12°).
    let (s11, c11, s12, c12) = (
        0.190_808_995_376_544_8,
        0.981_627_183_447_664,
        0.207_911_690_817_759_3,
        0.978_147_600_733_805_7,
    );
    let d = [
        c.eye[0] - c.target[0],
        c.eye[1] - c.target[1],
        c.eye[2] - c.target[2],
    ];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    assert!(close(d.map(|v| v / l), [s11 * c12, s12, c11 * c12]));
}

/// The stage camera of the first corpus genome, wild-port-0005 in
/// `vectors/small-robots/corpus.tsv`.
const PINNED_EYE: V3 = [1.7941724449515872, 3.2345367603992283, 9.204325157627393];
const PINNED_TARGET: V3 = [0.005032875407895765, 1.2414789671881963, 0.0];

/// Every vertex of a robot in a pose, posed from the exported data alone.
fn posed_vertices(robot: &Robot, joints: &[M4]) -> Vec<V3> {
    let skin = |j: usize| mul(&joints[j], &inverse_rest(&robot.bones[j].rest));
    let skins: Vec<M4> = (0..robot.bones.len()).map(skin).collect();
    let mut out = Vec::new();
    for part in &robot.parts {
        for (i, v) in part.mesh.positions.iter().enumerate() {
            out.push(match &part.skin {
                Skin::Rigid { bone } => apply(&skins[*bone], *v),
                Skin::Skinned { joints, weights } => (0..4).fold([0.0; 3], |acc, k| {
                    let p = apply(&skins[joints[i][k] as usize], *v);
                    [0, 1, 2].map(|c| acc[c] + weights[i][k] * p[c])
                }),
                _ => panic!("a part is rigid or skinned"),
            });
        }
    }
    out
}

/// Every pose the stage must hold: each reaction played from rest in
/// neutral to its end, and each status loop through its fade-in and one
/// period, sampled `REACH_SAMPLES_PER_SECOND` times a second with idle off.
fn stage_poses(robot: &Robot) -> Vec<Vec<M4>> {
    let mut poses = Vec::new();
    let mut sample = |actor: &Actor, end: f64| {
        let steps = (end * REACH_SAMPLES_PER_SECOND).floor() as u32;
        for k in 0..=steps {
            poses.push(
                actor
                    .evaluate(f64::from(k) / REACH_SAMPLES_PER_SECOND)
                    .bones,
            );
        }
    };
    for r in REACTIONS {
        let mut actor = Actor::new(robot, Mood::Neutral).without_idle();
        actor.play(0.0, r);
        sample(&actor, robot.motion.reaction(r).duration);
    }
    for l in STATUS_LOOPS {
        let mut actor = Actor::new(robot, Mood::Neutral).without_idle();
        actor.set_loop(0.0, Some(l));
        sample(&actor, LOOP_BLEND + robot.motion.status_loop(l).duration);
    }
    poses
}

#[test]
fn the_stage_holds_every_reaction_and_status_loop() {
    for (label, genome) in cases::corpus() {
        let robot = build_with(&genome, BuildOptions::untextured());
        let c = robot.camera;
        // The target is the centre, in x and y, of the box holding the rest
        // pose and every stage pose.
        let rest: Vec<M4> = robot.bones.iter().map(|b| b.rest).collect();
        let poses = stage_poses(&robot);
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for joints in std::iter::once(&rest).chain(&poses) {
            for p in posed_vertices(&robot, joints) {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        assert!(
            close(
                c.target,
                [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.0]
            ),
            "{label}: target {:?}",
            c.target
        );
        // No vertex leaves the frame, at rest or in any stage pose.
        for (n, joints) in std::iter::once(&rest).chain(&poses).enumerate() {
            for p in posed_vertices(&robot, joints) {
                let q = c.project(p);
                assert!(
                    q[0].abs() <= FILL && q[1].abs() <= FILL && q[2] > 0.0,
                    "{label}: sample {n} puts {p:?} at {q:?}, outside the frame"
                );
            }
        }
    }
}
