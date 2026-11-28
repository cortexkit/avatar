//! Pause and resume: what is freed, what survives, and how creatures come
//! back.

use avatar_3d::motion::{Reaction, StatusLoop};
use avatar_3d_stage::{BuildMode, Event, RenderStatus, Target};
use avatar_small_robots::Mood;

use crate::support::{fresh_pose, harness, spec};

#[test]
fn a_pause_during_a_build_frees_everything_and_discards_the_late_build() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.until_drawn(&["a"]);
    h.stage
        .set_state("a", Mood::Happy, Some(StatusLoop::Working));
    h.builder.hold("b");
    h.stage.set_creatures(&[spec("a", 1), spec("b", 2)]);
    h.frame().unwrap();
    h.builder.wait_for("b's build", |calls| calls.len() == 2);
    let late = h.builder.calls()[1].generation;
    let paused_at = h.last().unwrap().stage_time;

    h.stage.pause();
    assert_eq!(h.frame(), Err(RenderStatus::Paused));
    assert_eq!(h.fake.memory_bytes(), 0);
    assert!(h.fake.resident().is_empty());
    assert_eq!(h.stage.frame_stats().gpu_memory_bytes, 0);

    h.builder.release("b");
    h.builder
        .wait_for("b's build to land", |calls| calls[1].finished);
    let frames = h.fake.frame_count();
    for _ in 0..5 {
        assert_eq!(h.frame(), Err(RenderStatus::Paused));
    }
    assert_eq!(h.fake.frame_count(), frames);
    assert_eq!(h.fake.memory_bytes(), 0);

    // A minute later on the host's clock.
    h.stage.resume();
    let t = h.t + 60.0;
    h.render_at(t).unwrap();
    assert_eq!(
        h.last().unwrap().stage_time,
        paused_at,
        "the first frame after resume is the clock's origin"
    );
    h.until_drawn(&["a", "b"]);
    assert!(h.fake.uploads().iter().all(|u| u.generation != late));
    assert_eq!((h.readies("a"), h.readies("b")), (2, 1));

    // Each creature is back in its latest mood, its status loop started
    // when it reappeared, with no reaction playing.
    let frame = h.last().unwrap();
    let appeared = h
        .fake
        .frames()
        .iter()
        .find(|f| f.item("a").is_some() && f.target_time >= t)
        .unwrap()
        .stage_time;
    assert_eq!(
        frame.item("a").unwrap().pose,
        fresh_pose(
            1,
            Mood::Happy,
            Some(StatusLoop::Working),
            appeared,
            frame.stage_time
        )
    );
}

#[test]
fn commands_wait_while_paused_and_apply_after_resume() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.stage.pause();
    h.stage.set_state("a", Mood::Sad, None);
    h.stage.play("a", Reaction::Done);
    h.stage.play("zz", Reaction::Done);
    for _ in 0..3 {
        assert_eq!(h.frame(), Err(RenderStatus::Paused));
    }
    let before = h.events.len();
    assert_eq!(h.readies("a"), 1);
    h.stage.resume();
    h.frame().unwrap();
    // Applied in order at the first frame: the mood is kept for the robot
    // being rebuilt, and the plays are dropped, as for any key not built.
    let frame = h.last().unwrap();
    assert_eq!(
        frame.item("a").unwrap().pose,
        fresh_pose(1, Mood::Sad, None, frame.stage_time, frame.stage_time)
    );
    let dropped = h.events[before..]
        .iter()
        .filter(|e| matches!(e, Event::CommandDropped { .. }))
        .count();
    assert_eq!(dropped, 2);
    assert_eq!(h.readies("a"), 2);
}

#[test]
fn pause_and_resume_before_any_frame_draw_as_usual() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.pause();
    h.stage.resume();
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    assert_eq!(h.last().unwrap().keys(), ["a"]);
    // Pausing twice frees once.
    h.stage.pause();
    h.stage.pause();
    assert_eq!(h.frame(), Err(RenderStatus::Paused));
    assert_eq!(h.fake.releases(), 2);
}
