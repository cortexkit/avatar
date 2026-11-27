//! Posing from the exported data alone: rigid and skinned points, anchors,
//! and the stage camera.

use avatar_3d::robot::{build_with, BuildOptions, Robot, Skin};

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
    assert!(close(
        c.eye,
        [1.3774064138486284, 2.405042890320356, 6.919629648139448]
    ));
    assert!(close(
        c.target,
        [0.03236666372476038, 0.9067014519010492, 0.0]
    ));
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
    // Its target is the centre of the rest bounds, and the bounds fill the
    // frame to within 0.9 of its half size.
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in robot.parts.iter().flat_map(|p| p.mesh.positions.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    assert!(close(
        c.target,
        [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.0]
    ));
    for i in 0..8 {
        let corner = [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] });
        let q = c.project(corner);
        assert!(q[0].abs() <= 0.9 && q[1].abs() <= 0.9 && q[2] > 0.0);
    }
}
