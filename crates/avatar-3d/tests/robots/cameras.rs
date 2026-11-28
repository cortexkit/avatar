//! The stage camera: its pinned bits, the reach box it is fitted to, and
//! `frame_box`'s domain.

use std::f64::consts::PI;
use std::panic::{catch_unwind, AssertUnwindSafe};

use avatar_3d::robot::camera::{frame_box, Camera};
use avatar_3d::robot::{build_with, BuildOptions, AZIMUTH, ELEVATION, FILL};
use avatar_svg::wild::math::{cos, sin};

use crate::cases;

#[test]
fn every_pinned_robot_keeps_its_stage_camera_bit_for_bit() {
    let committed = cases::committed_cameras();
    let pinned = cases::cases(cases::committed_prefix());
    assert_eq!(
        committed.len(),
        pinned.len(),
        "the camera pins list every pinned robot"
    );
    for ((label, genome), want) in pinned.iter().zip(&committed) {
        let robot = build_with(genome, BuildOptions::untextured());
        let label = cases::camera_label(label);
        assert_eq!(
            &cases::camera_line(&label, &robot.camera()),
            want,
            "the stage camera differs from its pin"
        );
        let reach = robot.reach();
        let framed = frame_box(reach.lo, reach.hi, AZIMUTH, ELEVATION, 1.0, 1.0);
        assert_eq!(
            &cases::camera_line(&label, &framed),
            want,
            "frame_box of the reach box is not the stage camera"
        );
    }
}

/// A robot-sized box: the reach of a robot about a body wide.
const LO: [f64; 3] = [-0.6, 0.0, -0.4];
const HI: [f64; 3] = [0.7, 1.9, 0.5];

/// Every corner of the box from `lo` to `hi`.
fn corners(lo: [f64; 3], hi: [f64; 3]) -> [[f64; 3]; 8] {
    std::array::from_fn(|i| [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }))
}

/// The distance from the camera's eye to its target, scaled so that its
/// square neither overflows nor underflows.
fn distance(c: &Camera) -> f64 {
    let d = [0, 1, 2].map(|k| c.eye[k] - c.target[k]);
    let m = d.iter().fold(0.0, |m: f64, v| m.max(v.abs()));
    let s = d.map(|v| v / m);
    m * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt()
}

/// The panic message of `frame_box` for these arguments, or `None` when it
/// returns.
fn refusal(
    lo: [f64; 3],
    hi: [f64; 3],
    yaw: f64,
    pitch: f64,
    aspect: f64,
    zoom: f64,
) -> Option<String> {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        frame_box(lo, hi, yaw, pitch, aspect, zoom)
    }));
    outcome.err().map(|e| {
        e.downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default()
    })
}

/// Asserts that `frame_box` panics for these arguments with a message
/// containing `reason`.
#[track_caller]
fn assert_refused(
    case: &str,
    (lo, hi, yaw, pitch, aspect, zoom): ([f64; 3], [f64; 3], f64, f64, f64, f64),
    reason: &str,
) {
    match refusal(lo, hi, yaw, pitch, aspect, zoom) {
        Some(message) => assert!(
            message.contains(reason),
            "{case}: refused with {message:?}, not for {reason:?}"
        ),
        None => panic!("{case}: frame_box returned instead of panicking"),
    }
}

#[test]
fn frame_box_panics_outside_its_stated_domain() {
    let (nan, inf) = (f64::NAN, f64::INFINITY);
    let ok = (LO, HI, AZIMUTH, ELEVATION, 1.0, 1.0);
    let cases = [
        (
            "NaN lo",
            ([nan, 0.0, 0.0], HI, AZIMUTH, ELEVATION, 1.0, 1.0),
        ),
        (
            "infinite hi",
            (LO, [0.7, inf, 0.5], AZIMUTH, ELEVATION, 1.0, 1.0),
        ),
        (
            "lo above hi",
            ([0.0, 0.0, 0.6], HI, AZIMUTH, ELEVATION, 1.0, 1.0),
        ),
    ];
    for (case, args) in cases {
        assert_refused(case, args, "the box must be finite with lo <= hi");
    }
    for (case, yaw, pitch) in [
        ("NaN yaw", nan, ELEVATION),
        ("infinite yaw", -inf, ELEVATION),
        ("NaN pitch", AZIMUTH, nan),
        ("infinite pitch", AZIMUTH, inf),
    ] {
        let args = (LO, HI, yaw, pitch, 1.0, 1.0);
        assert_refused(case, args, "the yaw and pitch must be finite");
    }
    for aspect in [0.0, -0.0, -1.0, nan, inf] {
        let args = (LO, HI, AZIMUTH, ELEVATION, aspect, 1.0);
        assert_refused(&format!("aspect {aspect}"), args, "the aspect must be");
    }
    for zoom in [0.0, -2.0, nan, inf] {
        let args = (LO, HI, AZIMUTH, ELEVATION, 1.0, zoom);
        assert_refused(&format!("zoom {zoom}"), args, "the zoom must be");
    }
    assert_eq!(refusal(ok.0, ok.1, ok.2, ok.3, ok.4, ok.5), None);
}

#[test]
fn frame_box_panics_on_a_pitch_straight_along_the_camera_up() {
    // The portable cosine of these pitches is exactly zero, so the view
    // runs along +y and the camera has no right-hand direction.
    for pitch in [-90.0, 270.0, -450.0] {
        let args = (LO, HI, AZIMUTH, pitch, 1.0, 1.0);
        assert_refused(
            &format!("pitch {pitch}"),
            args,
            "looks straight along the camera's up",
        );
    }
}

#[test]
fn frame_box_panics_without_a_finite_framing_distance() {
    // A box as wide as the largest f64 can hold needs a distance past it.
    let wide = ([-f64::MAX, 0.0, 0.0], [f64::MAX, 1.0, 0.0]);
    let args = (wide.0, wide.1, AZIMUTH, ELEVATION, 1.0, 1.0);
    assert_refused("widest box", args, "no finite distance frames");
    // So does a box ten billion units wide on a frame nearly no wider than
    // a line.
    let args = (
        [-1e10, 0.0, 0.0],
        [1e10, 1.0, 0.0],
        AZIMUTH,
        ELEVATION,
        1e-300,
        1.0,
    );
    assert_refused("sliver frame", args, "no finite distance frames");
    // A zoom past the largest f64 over the framing distance.
    let args = (LO, HI, AZIMUTH, ELEVATION, 1.0, f64::MAX);
    assert_refused("huge zoom", args, "past the largest f64");
}

#[test]
fn frame_box_panics_when_zoom_puts_the_eye_on_the_target() {
    let args = (LO, HI, AZIMUTH, ELEVATION, 1.0, 1e-300);
    assert_refused("tiny zoom", args, "too close to the target");
}

#[test]
fn frame_box_frames_every_box_in_its_domain() {
    let boxes = [
        ("robot", LO, HI),
        ("point at the target", [0.0; 3], [0.0; 3]),
        (
            "point off the floor's origin",
            [2.0, -3.0, 5.0],
            [2.0, -3.0, 5.0],
        ),
        ("line along z", [0.0, 0.0, -10.0], [0.0, 0.0, 10.0]),
        ("flat sheet", [-1.0, 0.0, -1.0], [1.0, 0.0, 1.0]),
        ("speck", [0.0; 3], [1e-200, 1e-200, 1e-200]),
        (
            "far away",
            [1e6, -1e6, 1e6],
            [1e6 + 1.0, -1e6 + 2.0, 1e6 + 0.5],
        ),
        ("deep behind", [-1.0, 0.0, -1e9], [1.0, 1.0, -1e9 + 1.0]),
    ];
    let views = [
        (AZIMUTH, ELEVATION),
        (0.0, 0.0),
        (-180.0, 60.0),
        (90.0, -45.0),
        (-720.5, 89.0),
        (1e300, 1e20),
        (11.0, 90.0),
    ];
    for (name, lo, hi) in boxes {
        for (yaw, pitch) in views {
            for aspect in [1e-6, 9.0 / 16.0, 1.0, 16.0 / 9.0, 1e6] {
                let c = frame_box(lo, hi, yaw, pitch, aspect, 1.0);
                assert_eq!(c.aspect, aspect);
                for p in corners(lo, hi) {
                    let q = c.project(p);
                    assert!(
                        q[2] > 0.0 && q[0].abs() <= FILL && q[1].abs() <= FILL,
                        "{name} at yaw {yaw}, pitch {pitch}, aspect {aspect}: {p:?} lands at {q:?}"
                    );
                }
                let zoomed = frame_box(lo, hi, yaw, pitch, aspect, 4.0);
                let ratio = distance(&zoomed) / distance(&c);
                assert!(
                    (ratio - 4.0).abs() < 1e-9,
                    "{name} at yaw {yaw}, pitch {pitch}, aspect {aspect}: zoom 4 moves the eye {ratio} times as far"
                );
            }
        }
    }
}

#[test]
fn frame_box_looks_from_the_yaw_and_pitch_it_is_given() {
    for (yaw, pitch) in [
        (AZIMUTH, ELEVATION),
        (-135.0, 0.0),
        (180.0, 60.0),
        (45.0, 30.0),
    ] {
        let c = frame_box(LO, HI, yaw, pitch, 16.0 / 9.0, 2.0);
        let centre = [0.05, 0.95, 0.0];
        assert!(
            (0..3).all(|k| (c.target[k] - centre[k]).abs() < 1e-12),
            "the target {:?} is the box's x and y centre at z = 0",
            c.target
        );
        let d = distance(&c);
        let dir = [0, 1, 2].map(|k| (c.eye[k] - c.target[k]) / d);
        let (y, p) = (yaw * PI / 180.0, pitch * PI / 180.0);
        let want = [sin(y) * cos(p), sin(p), cos(y) * cos(p)];
        for k in 0..3 {
            assert!(
                (dir[k] - want[k]).abs() < 1e-9,
                "yaw {yaw}, pitch {pitch}: looks along {dir:?}, not {want:?}"
            );
        }
    }
}
