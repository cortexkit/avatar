//! Face commands held until their face set is drawable: when they reach
//! the actor, and in which order.

use avatar_3d::motion::{ActorState, Pose, Reaction, StatusLoop};
use avatar_3d_stage::fake::FakeItem;
use avatar_3d_stage::{BuildMode, Target, HOLD_AT_MOST};
use avatar_small_robots::Mood;

use crate::support::{genome, harness, robot, spec, Harness};

/// Whether `item` moves as `reference` poses: the same bones, blink, glow
/// and thrust, and the face the pose asks for.
fn moves_as(item: &FakeItem, reference: &Pose) -> bool {
    item.pose.bones == reference.bones
        && item.pose.blink == reference.blink
        && item.pose.glow == reference.glow
        && item.pose.thrust == reference.thrust
        && item.posed_face == reference.face
}

/// A stage drawing `a` of seed 1, built and shown in neutral, and the actor
/// state the stage gave it.
fn shown() -> (Harness, ActorState) {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.until_drawn(&["a"]);
    let actor = ActorState::new(&robot(&genome(1)), Mood::Neutral);
    (h, actor)
}

/// Checks that the latest frame moves as `actor` at its stage time.
fn assert_moves_as(h: &Harness, actor: &ActorState, what: &str) {
    let t = h.last().unwrap().stage_time;
    let reference = actor.evaluate(&robot(&genome(1)), t);
    assert!(moves_as(&h.item("a"), &reference), "at {t}: {what}");
}

#[test]
fn held_commands_reach_the_actor_once_their_set_is_drawable() {
    let (mut h, base) = shown();
    h.builder.hold_textures();
    h.stage
        .set_state("a", Mood::Happy, Some(StatusLoop::Working));
    h.stage.play("a", Reaction::Hello);
    for _ in 0..6 {
        h.frame().unwrap();
        assert_moves_as(&h, &base, "neither reached the actor");
    }
    assert_eq!(h.stage.frame_stats().held_face_commands, 2);

    h.land_textures(1);
    h.frame().unwrap();
    let landed = h.last().unwrap().stage_time;
    let mut actor = base.clone();
    actor.set_mood(landed, Mood::Happy);
    actor.set_loop(landed, Some(StatusLoop::Working));
    actor.play(landed, Reaction::Hello);
    for _ in 0..120 {
        assert_moves_as(&h, &actor, "both reached it with their set");
        assert!(h.item("a").missing.is_empty());
        h.frame().unwrap();
    }
}

#[test]
fn held_past_half_a_second_a_command_applies_on_the_first_frame_at_or_after_it() {
    for fps in [30u32, 1] {
        let (mut h, base) = shown();
        h.stage.set_max_fps(fps);
        let dt = 1.0 / f64::from(fps);
        h.builder.hold_textures();
        h.stage.set_state("a", Mood::Happy, None);
        h.step(dt);
        let drained = h.last().unwrap().stage_time;
        assert_moves_as(&h, &base, "held when taken");
        let applied = loop {
            h.step(dt);
            let t = h.last().unwrap().stage_time;
            if t - drained >= HOLD_AT_MOST {
                break t;
            }
            assert_moves_as(&h, &base, "held under half a second");
        };
        let mut actor = base.clone();
        actor.set_mood(applied, Mood::Happy);
        for _ in 0..(2 * fps) {
            assert_moves_as(&h, &actor, "applied at the first frame at or after it");
            // Its set is still not drawable: the face stays neutral.
            let item = h.item("a");
            assert_eq!(item.pose.face, Mood::Neutral, "{fps} fps");
            assert!(item.missing.is_empty());
            h.step(dt);
        }
    }
}

#[test]
fn a_held_mood_replaced_by_the_mood_shown_ends_there() {
    let (mut h, base) = shown();
    h.builder.hold_textures();
    h.stage.set_state("a", Mood::Happy, None);
    h.frame().unwrap();
    h.stage.set_state("a", Mood::Neutral, None);
    for _ in 0..30 {
        h.frame().unwrap();
    }
    h.land_textures(1);
    for _ in 0..120 {
        h.frame().unwrap();
        assert_moves_as(&h, &base, "never happy");
        assert_eq!(h.item("a").pose.face, Mood::Neutral);
    }
}

#[test]
fn a_held_set_state_then_a_play_reach_the_actor_in_that_order() {
    let (mut h, base) = shown();
    h.builder.hold_textures();
    h.stage.set_state("a", Mood::Happy, None);
    // Done needs no face set, but waits behind the held set_state.
    h.stage.play("a", Reaction::Done);
    for _ in 0..6 {
        h.frame().unwrap();
        assert_moves_as(&h, &base, "the play waits behind the set_state");
    }
    h.land_textures(1);
    h.frame().unwrap();
    let landed = h.last().unwrap().stage_time;
    let mut actor = base.clone();
    actor.set_mood(landed, Mood::Happy);
    actor.set_loop(landed, None);
    actor.play(landed, Reaction::Done);
    for _ in 0..60 {
        assert_moves_as(&h, &actor, "both in the frame the set landed");
        h.frame().unwrap();
    }
}

#[test]
fn held_set_state_play_set_state_deliver_the_play_then_the_last_set_state() {
    let (mut h, base) = shown();
    h.builder.hold_textures();
    h.stage.set_state("a", Mood::Happy, None);
    h.stage.play("a", Reaction::Hello);
    h.stage.set_state("a", Mood::Sad, None);
    for _ in 0..6 {
        h.frame().unwrap();
        assert_moves_as(&h, &base, "all held");
    }
    // Hello's face and sad's.
    h.land_textures(2);
    h.frame().unwrap();
    let landed = h.last().unwrap().stage_time;
    let mut actor = base.clone();
    actor.play(landed, Reaction::Hello);
    actor.set_mood(landed, Mood::Sad);
    actor.set_loop(landed, None);
    for _ in 0..180 {
        assert_moves_as(&h, &actor, "the play then sad, and happy never");
        h.frame().unwrap();
    }
    assert_eq!(h.stage.frame_stats().held_face_commands, 3);
}

#[test]
fn a_request_superseded_before_its_set_lands_never_switches_in() {
    let (mut h, _) = shown();
    h.builder.hold_textures();
    h.stage.set_state("a", Mood::Happy, None);
    // Past half a second, happy reaches the actor without its set.
    for _ in 0..45 {
        h.frame().unwrap();
    }
    assert_eq!(h.item("a").posed_face, Mood::Happy);
    h.stage.set_state("a", Mood::Sad, None);
    for _ in 0..45 {
        h.frame().unwrap();
    }
    assert_eq!(h.item("a").posed_face, Mood::Sad);
    h.land_textures(2);
    for _ in 0..180 {
        h.frame().unwrap();
        assert_ne!(h.item("a").pose.face, Mood::Happy);
        assert!(h.item("a").missing.is_empty());
    }
    assert_eq!(h.item("a").pose.face, Mood::Sad);
    assert!(h
        .fake
        .frames()
        .iter()
        .filter_map(|f| f.item("a"))
        .all(|i| i.pose.face != Mood::Happy));
}
