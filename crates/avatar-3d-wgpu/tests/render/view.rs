//! Camera matrices and the anchor-to-screen projection against values
//! worked out by hand.

use avatar_3d::motion::Actor;
use avatar_3d::robot::{build_with, BuildOptions, AZIMUTH, ELEVATION, FOV_Y, FRAME_PX};
use avatar_3d_wgpu::frame;
use avatar_3d_wgpu::view::{apply, mul, View};
use avatar_small_robots::{Mood, MOODS};

use crate::genomes;

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

fn assert_all_close(got: &[f64], want: &[f64], tolerance: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(close(*g, *w, tolerance), "{what}[{i}]: {g} against {w}");
    }
}

#[test]
fn a_camera_on_the_z_axis_gives_the_hand_matrices_and_pixels() {
    // At z = 10 looking at the origin with a 90 degree field of view on a
    // frame twice as wide as high: the view matrix only moves z by −10, the
    // focal length is 1/tan(45°) = 1, and x is halved by the aspect.
    let view = View {
        eye: [0.0, 0.0, 10.0],
        target: [0.0; 3],
        up: [0.0, 1.0, 0.0],
        fov_y_degrees: 90.0,
        aspect: 2.0,
    };
    assert_all_close(
        &view.view_matrix(),
        &[
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, -10.0, 1.0,
        ],
        1e-15,
        "view matrix",
    );
    // Near 1 and far 100: depth −far / (far − near) and −near·far / (far −
    // near).
    assert_all_close(
        &view.projection_matrix(1.0, 100.0),
        &[
            0.5,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -100.0 / 99.0,
            -1.0,
            0.0,
            0.0,
            -100.0 / 99.0,
            0.0,
        ],
        1e-12,
        "projection matrix",
    );
    // The point (1, 2, 0) is 10 in front: ndc (0.5 · 1 / 10, 2 / 10) =
    // (0.05, 0.2), at depth (−100/99 · −10 − 100/99) / 10 = 10/11; in a
    // 200 × 100 frame that is pixel ((1.05 / 2) · 200, (0.8 / 2) · 100).
    let clip = apply(
        &mul(&view.projection_matrix(1.0, 100.0), &view.view_matrix()),
        [1.0, 2.0, 0.0],
    );
    assert_all_close(
        &[clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]],
        &[0.05, 0.2, 10.0 / 11.0],
        1e-12,
        "ndc",
    );
    assert_all_close(
        &view.to_screen([1.0, 2.0, 0.0], 200, 100).expect("in front"),
        &[105.0, 40.0],
        1e-9,
        "pixel",
    );
    assert_eq!(view.to_screen([0.0, 0.0, 11.0], 200, 100), None);
}

#[test]
fn the_stage_camera_gives_the_hand_basis_and_focal_length() {
    let robot = build_with(&genomes::corpus()[0], BuildOptions::untextured());
    let camera = robot.camera();
    assert_eq!(
        (camera.fov_y_degrees, camera.aspect, camera.height_px),
        (FOV_Y, 1.0, FRAME_PX)
    );
    assert_eq!((AZIMUTH, ELEVATION, FOV_Y), (11.0, 12.0, 20.0));
    let view = View::from(&camera);
    // sin and cos of 11 and 12 degrees.
    let (s11, c11) = (0.190_808_995_376_544_8, 0.981_627_183_447_664);
    let (s12, c12) = (0.207_911_690_817_759_34, 0.978_147_600_733_805_7);
    let right = [c11, 0.0, -s11];
    let up = [-s11 * s12, c12, -c11 * s12];
    let back = [s11 * c12, s12, c11 * c12];
    let m = view.view_matrix();
    for (row, want) in [right, up, back].iter().enumerate() {
        assert_all_close(
            &[m[row], m[4 + row], m[8 + row]],
            want,
            1e-12,
            &format!("view matrix row {row}"),
        );
        let e = camera.eye;
        let translation = -(want[0] * e[0] + want[1] * e[1] + want[2] * e[2]);
        assert!(close(m[12 + row], translation, 1e-12), "translation {row}");
    }
    // The eye looks at the target along the stage direction.
    let d = view.distance();
    assert_all_close(
        &[0, 1, 2].map(|k| (camera.eye[k] - camera.target[k]) / d),
        &back,
        1e-12,
        "eye direction",
    );
    // 1 / tan(10°) on both axes of the square frame.
    let p = view.projection_matrix(0.1, 100.0);
    assert!(close(p[0], 5.671_281_819_617_709, 1e-12), "{}", p[0]);
    assert!(close(p[5], 5.671_281_819_617_709, 1e-12), "{}", p[5]);

    // Through the matrices, every rest vertex lands where the exported
    // camera's own projection puts it, and the target in the middle.
    let (near, far) = view.clip_range();
    let pv = mul(&view.projection_matrix(near, far), &view.view_matrix());
    let centre = apply(&pv, camera.target);
    assert!(close(centre[0] / centre[3], 0.0, 1e-12) && close(centre[1] / centre[3], 0.0, 1e-12));
    for part in &robot.parts {
        for p in &part.mesh.positions {
            let clip = apply(&pv, *p);
            let want = camera.project(*p);
            assert!(close(clip[0] / clip[3], want[0], 1e-12), "{:?}", p);
            assert!(close(clip[1] / clip[3], want[1], 1e-12), "{:?}", p);
            assert!(
                clip[2] / clip[3] > 0.0 && clip[2] / clip[3] < 1.0,
                "{:?} is clipped",
                p
            );
        }
    }
}

#[test]
fn anchors_follow_their_bones_to_the_screen() {
    let robot = build_with(&genomes::corpus()[0], BuildOptions::untextured());
    let head_top = robot
        .anchor("headTop")
        .expect("every robot has a headTop anchor");
    let bone = head_top.bone.expect("headTop follows the head");
    let joint = robot.bones[bone].joint;
    let o = head_top.offset;

    // In a mood whose posture turns the head, the anchor is the posed bone
    // matrix applied to the offset, since a rest matrix only moves to the
    // joint.
    let pose = MOODS
        .iter()
        .map(|mood| Actor::new(&robot, *mood).without_idle().evaluate(1.0))
        .find(|pose| pose.bones[bone][..12] != robot.bones[bone].rest[..12])
        .expect("some mood's posture turns the head");
    let m = pose.bones[bone];
    let want = [0, 1, 2].map(|r| m[r] * o[0] + m[4 + r] * o[1] + m[8 + r] * o[2] + m[12 + r]);
    let got = frame::anchor(&robot, &pose, "headTop").expect("the anchor exists");
    assert_all_close(&got, &want, 1e-12, "posed anchor");

    // At rest it is the joint plus the offset.
    let rest = Actor::new(&robot, Mood::Neutral)
        .without_idle()
        .evaluate(0.0);
    let at_rest = frame::anchor(&robot, &rest, "headTop").expect("the anchor exists");
    assert_all_close(
        &at_rest,
        &[0, 1, 2].map(|k| joint[k] + o[k]),
        1e-12,
        "rest anchor",
    );

    // `ground` stays where it is whatever the root does.
    let ground = robot
        .anchor("ground")
        .expect("every robot has a ground anchor");
    assert_eq!(frame::anchor(&robot, &pose, "ground"), Some(ground.offset));

    // On screen: the stage camera centres its target, and the anchor lands
    // where its projection says, above the frame's middle.
    let view = View::from(&robot.camera());
    let px = view
        .to_screen(at_rest, 288, 288)
        .expect("in front of the camera");
    let ndc = robot.camera().project(at_rest);
    assert_all_close(
        &px,
        &[(ndc[0] + 1.0) * 144.0, (1.0 - ndc[1]) * 144.0],
        1e-9,
        "anchor pixel",
    );
    assert!(px[1] < 144.0, "the head's top is above the frame's middle");
    assert!((0.0..288.0).contains(&px[0]) && (0.0..288.0).contains(&px[1]));
}
