//! The cast's lifecycle: builds, replacement, removal, the checks of
//! `set_creatures`, and commands for keys not built yet.

use avatar_3d::motion::{ActorState, Reaction, StatusLoop};
use avatar_3d_stage::{BuildMode, CommandKind, CreatureSpec, Event, SkipReason, Target};
use avatar_small_robots::Mood;

use crate::support::{fresh_pose, genome, harness, harness_with, robot, spec, tag};

#[test]
fn an_unchanged_spec_keeps_its_robot_and_actor() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    let generation = h
        .drawn_generation("a")
        .expect("a robot built inline draws at once");
    h.stage.play("a", Reaction::Done);
    h.frame().unwrap();
    let played_at = h.last().unwrap().stage_time;

    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.frame().unwrap();
    let last = h.last().unwrap();
    assert_eq!(h.drawn_generation("a"), Some(generation));
    assert_eq!(h.builder.calls().len(), 1);
    assert_eq!(h.fake.uploads().len(), 1);
    assert_eq!(h.readies("a"), 1);

    // The actor is the one that took the reaction.
    let robot = robot(&genome(1));
    let mut actor = ActorState::new(&robot, Mood::Neutral);
    actor.play(played_at, Reaction::Done);
    assert_eq!(
        last.item("a").unwrap().pose,
        actor.evaluate(&robot, last.stage_time)
    );
}

#[test]
fn a_changed_spec_keeps_the_old_robot_until_the_new_one_takes_over() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.until_drawn(&["a"]);
    let old = h.drawn_generation("a").unwrap();
    h.stage
        .set_state("a", Mood::Happy, Some(StatusLoop::Working));

    h.builder.hold("a");
    h.stage.set_creatures(&[spec("a", 2)]);
    for _ in 0..10 {
        h.frame().unwrap();
        assert_eq!(h.drawn_generation("a"), Some(old));
    }
    h.builder.release("a");
    let before = h.fake.frame_count();
    h.until("the new robot", |h| h.drawn_generation("a") != Some(old));

    // No frame went without the creature.
    let frames = h.fake.frames();
    assert!(frames[before..].iter().all(|f| f.item("a").is_some()));
    let new = h.last().unwrap();
    let item = new.item("a").unwrap();
    let call = h.builder.calls_for("a").last().cloned().unwrap();
    assert_eq!(call.tag, tag(&genome(2)));
    assert_eq!(call.generation, item.generation);
    // The new robot starts in the key's mood and status loop.
    assert_eq!(
        item.pose,
        fresh_pose(
            2,
            Mood::Happy,
            Some(StatusLoop::Working),
            new.stage_time,
            new.stage_time
        )
    );
    assert!(h.fake.retired().iter().any(|r| r.generation == old));
    assert_eq!(h.readies("a"), 2);
}

#[test]
fn builds_finishing_in_reverse_order_each_appear() {
    let mut h = harness_with(Target::Image, BuildMode::Workers, 2, true);
    h.builder.hold("a");
    h.builder.hold("b");
    h.stage.set_creatures(&[spec("a", 1), spec("b", 2)]);
    h.frame().unwrap();
    h.builder
        .wait_for("both builds running", |calls| calls.len() == 2);

    h.builder.release("b");
    h.until_drawn(&["b"]);
    assert!(h.fake.frames().iter().all(|f| f.item("a").is_none()));
    assert_eq!((h.readies("a"), h.readies("b")), (0, 1));

    h.builder.release("a");
    h.until_drawn(&["a", "b"]);
    assert_eq!(h.last().unwrap().keys(), ["a", "b"]);
    assert_eq!((h.readies("a"), h.readies("b")), (1, 1));
}

#[test]
fn a_spec_replaced_while_building_never_appears() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.builder.hold("a");
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.builder
        .wait_for("the first build", |calls| calls.len() == 1);
    h.stage.set_creatures(&[spec("a", 2)]);
    h.frame().unwrap();
    h.builder.release("a");
    h.until_drawn(&["a"]);
    h.builder
        .wait_for("both builds", |calls| calls.iter().all(|c| c.finished));
    for _ in 0..5 {
        h.frame().unwrap();
    }

    let calls = h.builder.calls();
    let stale = calls
        .iter()
        .find(|c| c.tag == tag(&genome(1)))
        .unwrap()
        .generation;
    let fresh = calls
        .iter()
        .find(|c| c.tag == tag(&genome(2)))
        .unwrap()
        .generation;
    assert!(h.fake.uploads().iter().all(|u| u.generation != stale));
    for frame in h.fake.frames() {
        if let Some(item) = frame.item("a") {
            assert_eq!(item.generation, fresh);
        }
    }
    assert_eq!(h.readies("a"), 1);
}

#[test]
fn a_removed_key_vanishes_at_the_next_frame() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1), spec("b", 2)]);
    h.frame().unwrap();
    assert_eq!(h.last().unwrap().keys(), ["a", "b"]);
    let a = h.drawn_generation("a").unwrap();

    h.stage.set_creatures(&[spec("b", 2)]);
    h.frame().unwrap();
    assert_eq!(h.last().unwrap().keys(), ["b"]);
    assert!(h.fake.retired().iter().any(|r| r.generation == a));
    assert_eq!(h.fake.resident().len(), 1);

    // A drawn key whose new row is skipped is removed too.
    h.stage.set_creatures(&[spec("a", 1), spec("b", 2)]);
    h.frame().unwrap();
    assert_eq!(h.last().unwrap().keys(), ["a", "b"]);
    let mut bad = spec("a", 1);
    bad.avatar_type = "creature.unknown".into();
    h.stage.set_creatures(&[bad, spec("b", 2)]);
    h.frame().unwrap();
    assert_eq!(h.last().unwrap().keys(), ["b"]);
    let skipped: Vec<_> = h
        .events
        .iter()
        .filter(|e| matches!(e, Event::Skipped { .. }))
        .collect();
    assert_eq!(skipped.len(), 1);
    assert!(matches!(
        skipped[0],
        Event::Skipped { key, reason: SkipReason::UnknownType, .. } if key == "a"
    ));
}

#[test]
fn each_key_keeps_its_valid_row_and_reports_the_other_once() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    let with_version = |seed, version| CreatureSpec {
        avatar_version: version,
        ..spec("a", seed)
    };
    let short = CreatureSpec {
        genome: vec![0; 100],
        ..spec("a", 5)
    };
    let batches: [(Vec<CreatureSpec>, u32, SkipReason); 3] = [
        (
            vec![spec("a", 1), spec("a", 2)],
            1,
            SkipReason::DuplicateKey,
        ),
        (
            vec![with_version(3, 2), spec("a", 4)],
            4,
            SkipReason::UnknownVersion,
        ),
        (vec![spec("a", 4), short], 4, SkipReason::GenomeLength),
    ];
    for (batch, kept, reason) in batches {
        let before = h.events.len();
        h.stage.set_creatures(&batch);
        h.frame().unwrap();
        let time = h.t;
        h.frame().unwrap();
        let item_generation = h.drawn_generation("a").unwrap();
        let call = h
            .builder
            .calls()
            .into_iter()
            .find(|c| c.generation == item_generation)
            .unwrap();
        assert_eq!(call.tag, tag(&genome(kept)), "{reason:?}");
        assert_eq!(
            h.events[before..]
                .iter()
                .filter(|e| matches!(e, Event::Skipped { .. }))
                .collect::<Vec<_>>(),
            [&Event::Skipped {
                key: "a".into(),
                reason,
                time
            }]
        );
    }
    // The unchanged third batch kept the second's robot.
    assert_eq!(h.builder.calls().len(), 2);

    // An unknown type is reported as such whatever its genome.
    let before = h.events.len();
    h.stage.set_creatures(&[CreatureSpec {
        key: "x".into(),
        genome: vec![1; 100],
        avatar_type: "creature.unknown".into(),
        avatar_version: 1,
    }]);
    h.frame().unwrap();
    assert!(h.last().unwrap().items.is_empty());
    assert!(matches!(
        &h.events[before..],
        [Event::Skipped { key, reason: SkipReason::UnknownType, .. }] if key == "x"
    ));
}

#[test]
fn ready_comes_again_after_replacement_and_after_resume() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    assert_eq!(h.readies("a"), 1);
    assert!(matches!(
        h.events.last(),
        Some(Event::Ready { key, time }) if key == "a" && *time == h.t
    ));
    h.stage.set_creatures(&[spec("a", 2)]);
    h.frame().unwrap();
    assert_eq!(h.readies("a"), 2);
    h.stage.pause();
    assert_eq!(h.frame(), Err(avatar_3d_stage::RenderStatus::Paused));
    h.stage.resume();
    h.frame().unwrap();
    assert_eq!(h.readies("a"), 3);
    let generations: Vec<u64> = h.builder.calls().iter().map(|c| c.generation).collect();
    assert_eq!(generations.len(), 3);
    assert_eq!(h.drawn_generation("a"), generations.last().copied());
}

#[test]
fn a_set_state_for_an_unbuilt_key_is_kept_until_it_appears() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.builder.hold("a");
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.stage.set_state("a", Mood::Sad, Some(StatusLoop::Sleepy));
    h.frame().unwrap();
    h.builder.release("a");
    h.until_drawn(&["a"]);
    let first = h.last().unwrap();
    assert_eq!(
        first.item("a").unwrap().pose,
        fresh_pose(
            1,
            Mood::Sad,
            Some(StatusLoop::Sleepy),
            first.stage_time,
            first.stage_time
        )
    );
    assert!(!h
        .events
        .iter()
        .any(|e| matches!(e, Event::CommandDropped { .. })));
}

#[test]
fn a_play_for_an_unbuilt_or_unknown_key_is_dropped_and_reported() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.builder.hold("a");
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.stage.play("a", Reaction::Hello);
    h.stage.play("zz", Reaction::Hello);
    h.stage.set_state("zz", Mood::Happy, None);
    h.frame().unwrap();
    let time = h.t;
    let dropped = |key: &str, command| Event::CommandDropped {
        key: key.into(),
        command,
        time,
    };
    assert_eq!(
        h.events,
        [
            dropped("a", CommandKind::Play),
            dropped("zz", CommandKind::Play),
            dropped("zz", CommandKind::SetState)
        ]
    );

    // The dropped reaction never plays.
    h.builder.release("a");
    h.until_drawn(&["a"]);
    let first = h.last().unwrap();
    assert_eq!(
        first.item("a").unwrap().pose,
        fresh_pose(1, Mood::Neutral, None, first.stage_time, first.stage_time)
    );
}
