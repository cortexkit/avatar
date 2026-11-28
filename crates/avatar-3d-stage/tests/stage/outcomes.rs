//! Early returns and render outcomes: pause in the throttle and at zero
//! size, resizes, invalid arguments, a busy or wrong-target call, and a
//! lost device.

use std::thread;
use std::time::Duration;

use avatar_3d::motion::Reaction;
use avatar_3d_stage::fake::{FakeRenderer, Step};
use avatar_3d_stage::{
    BuildMode, CommandKind, Event, Failure, InvalidArgument, RenderStatus, Stage, StageError,
    StageOptions, Target,
};
use avatar_small_robots::Mood;

use crate::support::{harness, spec, Harness, Unblock, HEIGHT, WAIT, WIDTH};

fn dropped_plays(h: &Harness) -> usize {
    h.events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::CommandDropped {
                    command: CommandKind::Play,
                    ..
                }
            )
        })
        .count()
}

#[test]
fn a_pause_frees_at_once_and_later_calls_return_paused() {
    // Inside the fps throttle.
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_max_fps(30);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.render_at(0.0).unwrap();
    assert!(h.fake.memory_bytes() > 0);
    h.stage.pause();
    assert_eq!(h.fake.releases(), 1);
    assert_eq!(h.fake.memory_bytes(), 0);
    assert!(h.fake.resident().is_empty());
    assert_eq!(h.render_at(0.01), Err(RenderStatus::Paused));
    assert_eq!(h.fake.releases(), 1);
    assert_eq!(h.stage.frame_stats().gpu_memory_bytes, 0);

    // At zero size.
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.stage.resize(0, 0, 1.0).unwrap();
    assert_eq!(h.frame(), Err(RenderStatus::NotDue));
    h.stage.pause();
    assert_eq!(h.frame(), Err(RenderStatus::Paused));
    assert_eq!(h.fake.releases(), 1);
    assert_eq!(h.fake.memory_bytes(), 0);
}

#[test]
fn a_pause_during_a_render_frees_when_that_render_ends() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.fake.hold_finish();
    let stage = &h.stage;
    thread::scope(|s| {
        let render = s.spawn(|| stage.render_to_image(200.0).map(|_| ()));
        let _unblock = Unblock(&h.fake);
        assert!(h.fake.wait_held(WAIT));
        stage.pause();
        assert_eq!(
            h.fake.releases(),
            0,
            "the running frame holds its resources"
        );
        h.fake.release_finish();
        assert_eq!(render.join().unwrap(), Ok(()));
    });
    assert_eq!(h.fake.releases(), 1);
    assert_eq!(h.fake.memory_bytes(), 0);
    assert_eq!(h.render_at(201.0), Err(RenderStatus::Paused));
    assert_eq!(h.fake.releases(), 1);
}

#[test]
fn a_resize_from_zero_applies_on_the_next_call() {
    let (renderer, fake) = FakeRenderer::new(Target::Image, true);
    let stage = Stage::new(renderer, 0, 0, 1.0, StageOptions::default()).unwrap();
    assert_eq!(stage.render_to_image(1.0), Err(RenderStatus::NotDue));
    stage.resize(30, 20, 2.0).unwrap();
    let image = stage.render_to_image(1.0).unwrap();
    assert_eq!((image.width, image.height), (30, 20));
    let frame = fake.last_frame().unwrap();
    assert_eq!(
        (frame.width_px, frame.height_px, frame.scale),
        (30, 20, 2.0)
    );
}

#[test]
fn a_non_finite_time_is_refused_and_a_pending_resize_waits() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.frame().unwrap();
    h.stage.resize(80, 60, 2.0).unwrap();
    h.stage.play("zz", Reaction::Poke);
    for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(h.render_at(t), Err(RenderStatus::InvalidArgument));
    }
    assert_eq!(h.fake.frame_count(), 1);
    assert_eq!(dropped_plays(&h), 0, "no command was taken");
    h.render_at(1000.0).unwrap();
    let frame = h.last().unwrap();
    assert_eq!(
        (frame.width_px, frame.height_px, frame.scale),
        (80, 60, 2.0)
    );
    assert_eq!(dropped_plays(&h), 1);
}

#[test]
fn invalid_scales_are_refused_and_change_nothing() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(h.stage.resize(10, 10, scale), Err(InvalidArgument));
    }
    h.frame().unwrap();
    let frame = h.last().unwrap();
    assert_eq!(
        (frame.width_px, frame.height_px, frame.scale),
        (WIDTH, HEIGHT, 1.0)
    );
    let (renderer, _) = FakeRenderer::new(Target::Image, true);
    assert_eq!(
        Stage::new(renderer, 10, 10, f64::NAN, StageOptions::default()).err(),
        Some(StageError::InvalidArgument)
    );
}

#[test]
fn a_second_render_call_is_busy_and_takes_nothing() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.frame().unwrap();
    h.fake.hold_finish();
    let stage = &h.stage;
    let fake = &h.fake;
    thread::scope(|s| {
        let render = s.spawn(|| stage.render_to_image(200.0).map(|_| ()));
        let _unblock = Unblock(fake);
        assert!(fake.wait_held(WAIT));
        stage.play("zz", Reaction::Poke);
        assert_eq!(stage.render_to_image(201.0), Err(RenderStatus::Busy));
        assert_eq!(stage.render_to_surface(201.0), Err(RenderStatus::Busy));
        fake.release_finish();
        assert_eq!(render.join().unwrap(), Ok(()));
    });
    assert_eq!(dropped_plays(&h), 0);
    h.render_at(202.0).unwrap();
    assert_eq!(dropped_plays(&h), 1);
}

#[test]
fn a_call_for_the_other_target_is_refused_and_takes_nothing() {
    for target in [Target::Image, Target::Surface] {
        let mut h = harness(target, BuildMode::Inline);
        h.stage.play("zz", Reaction::Poke);
        let wrong = match target {
            Target::Image => h.stage.render_to_surface(1.0),
            Target::Surface => h.stage.render_to_image(1.0).map(|_| ()),
        };
        assert_eq!(wrong, Err(RenderStatus::WrongTarget));
        assert_eq!(h.fake.frame_count(), 0);
        h.render_at(1.0).unwrap();
        assert_eq!(dropped_plays(&h), 1);
    }
}

#[test]
fn other_renderer_errors_pass_through() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.frame().unwrap();
    let error = avatar_3d_stage::RenderError::new("out of memory");
    h.fake.fail(Step::Submit, Failure::Render(error.clone()));
    assert_eq!(h.frame(), Err(RenderStatus::Render(error)));
    h.frame().unwrap();
    assert_eq!(h.fake.recoveries(), 0);
}

/// A stage with `a` and `b` drawn, a set_state for `a` and a play for an
/// unknown key queued, and a replacement build for `b` finished but not
/// landed.
fn lost_device_setup() -> (Harness, u64) {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.stage.set_creatures(&[spec("a", 1), spec("b", 2)]);
    h.until_drawn(&["a", "b"]);
    h.builder.hold("b");
    h.stage.set_creatures(&[spec("a", 1), spec("b", 3)]);
    h.frame().unwrap();
    h.builder
        .wait_for("b's new build", |calls| calls.len() == 3);
    let landing = h.builder.calls()[2].generation;
    h.stage.set_state("a", Mood::Happy, None);
    h.stage.play("zz", Reaction::Poke);
    h.builder.release("b");
    h.builder
        .wait_for("b's new build to finish", |calls| calls[2].finished);
    // Let the result reach the landing queue.
    thread::sleep(Duration::from_millis(20));
    (h, landing)
}

/// After the loss: the next frames rebuild both creatures in their latest
/// state, apply the queued commands once, and announce only robots a
/// successful frame showed.
fn check_recovery(mut h: Harness, landing: u64, lost_frame_drew_landing: bool) {
    let before = h.builder.calls().len();
    h.until_drawn(&["a", "b"]);
    assert_eq!(h.fake.recoveries(), 1);
    let rebuilt: Vec<_> = h.builder.calls()[before..].to_vec();
    assert_eq!(rebuilt.len(), 2);
    assert_eq!(
        rebuilt.iter().find(|c| c.key == "a").unwrap().mood,
        Mood::Happy
    );
    let frame = h.last().unwrap();
    assert_eq!(frame.item("a").unwrap().pose.face, Mood::Happy);
    for item in &frame.items {
        assert!(rebuilt.iter().any(|c| c.generation == item.generation));
    }
    // Each creature is announced once more, and the landing build never.
    assert_eq!((h.readies("a"), h.readies("b")), (2, 2));
    let drew_landing = h
        .fake
        .frames()
        .iter()
        .any(|f| f.item("b").is_some_and(|i| i.generation == landing));
    assert_eq!(drew_landing, lost_frame_drew_landing);
    assert_eq!(dropped_plays(&h), 1);
}

#[test]
fn a_device_lost_before_acquisition_commits_nothing_and_rebuilds_everything() {
    let (mut h, landing) = lost_device_setup();
    let frames = h.fake.frame_count();
    h.fake.fail(Step::Begin, Failure::DeviceLost);
    assert_eq!(h.frame(), Err(RenderStatus::DeviceLost));
    assert_eq!(h.fake.frame_count(), frames);
    assert_eq!(dropped_plays(&h), 0);
    assert!(h.fake.uploads().iter().all(|u| u.generation != landing));
    check_recovery(h, landing, false);
}

#[test]
fn a_device_lost_after_submit_keeps_its_commit_and_rebuilds_everything() {
    let (mut h, landing) = lost_device_setup();
    h.fake.fail(Step::Finish, Failure::DeviceLost);
    assert_eq!(h.frame(), Err(RenderStatus::DeviceLost));
    // The frame committed: the commands applied once and the landed build
    // was drawn, but no successful frame showed it.
    assert_eq!(dropped_plays(&h), 1);
    let lost = h.last().unwrap();
    assert!(!lost.finished);
    assert_eq!(lost.item("b").unwrap().generation, landing);
    assert_eq!(lost.item("a").unwrap().pose.face, Mood::Neutral);
    check_recovery(h, landing, true);
}

#[test]
fn a_failed_recovery_reports_the_device_lost_again() {
    let mut h = harness(Target::Surface, BuildMode::Inline);
    h.frame().unwrap();
    h.fake.fail(Step::Begin, Failure::DeviceLost);
    assert_eq!(h.frame(), Err(RenderStatus::DeviceLost));
    h.fake.fail(Step::Recover, Failure::DeviceLost);
    assert_eq!(h.frame(), Err(RenderStatus::DeviceLost));
    h.frame().unwrap();
    assert_eq!(h.fake.recoveries(), 1);
}
