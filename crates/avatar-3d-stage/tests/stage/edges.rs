//! Texture edges: the edge a robot's layers need where it is drawn, and how
//! the stage moves a staged robot's layers to it.

use avatar_3d::robot::camera::{frame_box, AZIMUTH, ELEVATION};
use avatar_3d::robot::Robot;
use avatar_3d_stage::{edge_target, BuildMode, StageView, Target, FIRST_EDGE, SHRINK_AFTER};

use crate::support::{
    genome, harness, harness_with, robot, spec, warm_reach, Harness, TextureWork,
};

/// The robot of seed 1 with its reach box worked out.
fn reaching_robot() -> Robot {
    warm_reach(&genome(1));
    robot(&genome(1))
}

/// The edge target of `robot` alone on a stage frame of `w` × `h` device
/// pixels, at the default view.
fn staged_target(robot: &Robot, w: u32, h: u32) -> u32 {
    let reach = robot.reach();
    let camera = frame_box(
        reach.lo,
        reach.hi,
        AZIMUTH,
        ELEVATION,
        f64::from(w) / f64::from(h),
        1.0,
    );
    edge_target(robot, &camera, [0.0; 3], h)
}

/// The smallest square frame edge, in steps of 8 px, whose target is `edge`.
fn square_for(robot: &Robot, edge: u32) -> u32 {
    (1..=512)
        .map(|k| 8 * k)
        .find(|&s| staged_target(robot, s, s) == edge)
        .unwrap_or_else(|| panic!("a square frame needs {edge} px"))
}

#[test]
fn the_edge_target_follows_the_projected_canvas() {
    let robot = reaching_robot();
    let reach = robot.reach();
    let camera = frame_box(reach.lo, reach.hi, AZIMUTH, ELEVATION, 1.0, 1.0);
    let target = |offset: [f64; 3], h: u32| edge_target(&robot, &camera, offset, h);

    // Powers of two from 64 to 512, growing with the frame.
    let mut last = 0;
    for h in (1..=200).map(|k| 10 * k) {
        let edge = target([0.0; 3], h);
        assert!(edge.is_power_of_two() && (64..=512).contains(&edge));
        assert!(edge >= last);
        last = edge;
    }
    assert_eq!((target([0.0; 3], 1), target([0.0; 3], 2000)), (64, 512));
    // The projected edge is proportional to the frame's height, so doubling
    // a frame doubles an edge above the floor, up to 512.
    for h in (1..=100).map(|k| 10 * k) {
        let edge = target([0.0; 3], h);
        if edge > 64 {
            assert_eq!(target([0.0; 3], 2 * h), (2 * edge).min(512), "at {h} px");
        }
    }
    // Farther from the camera, smaller; nearer, larger.
    let h = square_for(&robot, 256);
    assert_eq!(target([0.0; 3], h), 256);
    let back: [f64; 3] = std::array::from_fn(|k| -(camera.eye[k] - camera.target[k]));
    assert!(target(back, h) < 256);
    let near: [f64; 3] = std::array::from_fn(|k| 0.5 * (camera.eye[k] - camera.target[k]));
    assert!(target(near, h) > 256);
    // A box reaching behind the camera needs the largest edge.
    let behind: [f64; 3] = std::array::from_fn(|k| 2.0 * (camera.eye[k] - camera.target[k]));
    assert_eq!(target(behind, h), 512);
}

/// Renders until `done`, checking that every frame draws `a` from
/// resident layers.
fn until_checked(h: &mut Harness, what: &str, done: impl Fn(&Harness) -> bool) {
    h.until(what, |h| {
        let item = h.item("a");
        assert!(item.missing.is_empty());
        done(h)
    });
}

#[test]
fn a_staged_robot_moves_to_a_larger_edge_once_and_never_back_to_a_late_smaller_one() {
    let robot = reaching_robot();
    let (s256, s512) = (square_for(&robot, 256), square_for(&robot, 512));
    // Two workers, so a second texture job asked for while the first is
    // held would start, and show in the log.
    let mut h = harness_with(Target::Image, BuildMode::Workers, 2, true);
    h.builder.hold_textures();
    h.stage.resize(s256, s256, 1.0).unwrap();
    h.stage.set_view(StageView::Stage { key: "a".into() });
    h.stage.set_creatures(&[spec("a", 1)]);
    h.until_drawn(&["a"]);
    assert_eq!(h.item("a").edge, FIRST_EDGE);

    // The first drawn frame wants 256 px; the old layers keep showing
    // while the new ones are made.
    h.builder
        .wait_for_textures("the 256 px layers", |c| c.len() == 1);
    // A larger target meanwhile asks for nothing more while that one is
    // outstanding.
    h.stage.resize(s512, s512, 1.0).unwrap();
    for _ in 0..20 {
        h.frame().unwrap();
        let item = h.item("a");
        assert_eq!(item.edge, FIRST_EDGE);
        assert!(item.missing.is_empty());
    }
    let calls = h.builder.texture_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].px, matches!(calls[0].work, TextureWork::Layers(_))),
        (256, true)
    );

    // The 256 px layers land after 512 px was wanted: dropped, and the
    // 512 px ones are made next and installed.
    h.land_textures();
    until_checked(&mut h, "512 px layers", |h| h.item("a").edge == 512);
    let calls = h.builder.texture_calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].px, 512);
    let edges: Vec<u32> = h
        .fake
        .frames()
        .iter()
        .filter_map(|f| f.item("a"))
        .map(|i| i.edge)
        .collect();
    assert!(
        edges.iter().all(|e| *e == FIRST_EDGE || *e == 512),
        "{edges:?}"
    );
    assert!(h.fake.layer_changes().iter().all(|c| c.layers.edge != 256));
}

#[test]
fn a_smaller_target_moves_the_edge_after_two_seconds_at_or_below_half() {
    let robot = reaching_robot();
    let s512 = square_for(&robot, 512);
    let small = staged_target(&robot, 64, 48);
    assert!(2 * small <= 512);
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.resize(s512, s512, 1.0).unwrap();
    h.stage.set_view(StageView::Stage { key: "a".into() });
    h.stage.set_creatures(&[spec("a", 1)]);
    h.step(0.1);
    // Larger at once: asked on the first frame, installed on the next.
    h.step(0.1);
    assert_eq!(h.item("a").edge, 512);

    h.stage.resize(64, 48, 1.0).unwrap();
    h.step(0.1);
    let low_from = h.last().unwrap().stage_time;
    let calls = h.builder.texture_calls().len();
    loop {
        h.step(0.1);
        let t = h.last().unwrap().stage_time;
        let made = h.builder.texture_calls().len() > calls;
        assert_eq!(made, t - low_from >= SHRINK_AFTER, "at {t}");
        assert_eq!(h.item("a").edge, 512);
        if made {
            break;
        }
    }
    assert_eq!(h.builder.texture_calls().last().unwrap().px, small);
    h.step(0.1);
    assert_eq!(h.item("a").edge, small);
    assert!(h.item("a").missing.is_empty());
}

#[test]
fn staging_one_robot_changes_no_hidden_robots_edge() {
    let robot = reaching_robot();
    let s512 = square_for(&robot, 512);
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.resize(s512, s512, 1.0).unwrap();
    h.stage.set_creatures(&[spec("a", 1), spec("b", 2)]);
    h.step(0.1);
    let b = h.drawn_generation("b").unwrap();
    // The plaza works out no edge yet: both stay at the first edge.
    for _ in 0..30 {
        h.step(0.1);
    }
    assert_eq!(
        (h.item("a").edge, h.item("b").edge),
        (FIRST_EDGE, FIRST_EDGE)
    );
    assert!(h.builder.texture_calls().is_empty());

    h.stage.set_view(StageView::Stage { key: "a".into() });
    for _ in 0..30 {
        h.step(0.1);
    }
    assert_eq!(h.item("a").edge, 512);
    assert_eq!(h.fake.layers("b", b).unwrap().edge, FIRST_EDGE);
    assert!(h.builder.texture_calls().iter().all(|c| c.key == "a"));

    h.stage.set_view(StageView::Plaza);
    for _ in 0..30 {
        h.step(0.1);
    }
    assert_eq!((h.item("a").edge, h.item("b").edge), (512, FIRST_EDGE));
}
