//! Drawn moods: a face changes only to a resident set and only under a
//! blink, or a second late; the lights follow the face; a robot's first
//! frames show the key's latest mood; and a face set no frame has drawn or
//! needed for ten seconds is released.

use avatar_3d::motion::{Reaction, StatusLoop, REACTIONS};
use avatar_3d::robot::{Robot, TextureContent};
use avatar_3d_stage::fake::face_bindings;
use avatar_3d_stage::{BuildMode, Target, LATE_AFTER, RELEASE_AFTER, SHUT};
use avatar_small_robots::{Mood, MOODS};

use crate::support::{
    fresh_pose, genome, harness, harness_with, robot, spec, Harness, TextureWork,
};

/// One frame at 30 Hz.
const STEP_30: f64 = 1.0 / 30.0;

/// A mood's place in `MOODS`.
fn index(mood: Mood) -> usize {
    MOODS
        .iter()
        .position(|m| *m == mood)
        .expect("a listed mood")
}

/// Whether every face binding of `mood`, open and blinking, is resident
/// for the robot `key` draws in the latest frame.
fn resident(h: &Harness, robot: &Robot, key: &str, mood: Mood) -> bool {
    let generation = h.item(key).generation;
    let layers = h
        .fake
        .layers(key, generation)
        .expect("a drawn robot has layers");
    [false, true]
        .into_iter()
        .flat_map(|shut| face_bindings(robot, mood, shut))
        .all(|b| layers.resident.contains(&b))
}

/// The number of stalk eyes `robot` has.
fn stalk_eyes(robot: &Robot) -> usize {
    robot
        .textures
        .iter()
        .filter(|t| {
            matches!(
                t.binding.content,
                TextureContent::StalkEye {
                    mood: Mood::Neutral,
                    blinking: false,
                    ..
                }
            )
        })
        .count()
}

/// The stage times of the next `n` frames, `dt` apart in host time, as the
/// stage will add them up.
fn ahead(h: &Harness, dt: f64, n: usize) -> Vec<f64> {
    let (mut target, mut stage) = (h.t, h.last().expect("a frame").stage_time);
    (0..n)
        .map(|_| {
            let next = target + dt;
            stage += next - target;
            target = next;
            stage
        })
        .collect()
}

/// Checks the face rules on every frame of one creature, and keeps what
/// the checks need from one frame to the next.
struct Watch {
    key: &'static str,
    robot: Robot,
    /// The drawn mood of the frame before.
    drawn: Mood,
    /// The face the pose asks for, and the stage time of the first frame
    /// that asked for it.
    posed: (Mood, f64),
    /// The stage time each set was first seen resident; minus infinity for
    /// the sets resident when watching began.
    drawable: [Option<f64>; 7],
    /// Late switches seen, and the stage's count when watching began.
    late: u64,
    late_before: u64,
    /// Every switch seen: its stage time, the mood switched to, and whether
    /// it was late.
    switches: Vec<(f64, Mood, bool)>,
}

impl Watch {
    fn new(h: &Harness, key: &'static str, seed: u32) -> Self {
        let robot = robot(&genome(seed));
        let item = h.item(key);
        let drawable = std::array::from_fn(|i| {
            resident(h, &robot, key, MOODS[i]).then_some(f64::NEG_INFINITY)
        });
        Self {
            key,
            drawn: item.pose.face,
            posed: (item.posed_face, f64::NEG_INFINITY),
            drawable,
            late: 0,
            late_before: h.stage.frame_stats().late_face_switches,
            switches: Vec::new(),
            robot,
        }
    }

    /// Checks the latest frame.
    fn check(&mut self, h: &Harness) {
        let frame = h.last().expect("a frame");
        let t = frame.stage_time;
        let item = h.item(self.key);
        for (i, mood) in MOODS.iter().enumerate() {
            if resident(h, &self.robot, self.key, *mood) {
                self.drawable[i].get_or_insert(t);
            } else {
                self.drawable[i] = None;
            }
        }
        if item.posed_face != self.posed.0 {
            self.posed = (item.posed_face, t);
        }
        let drawn = item.pose.face;
        let shut = item.pose.blink >= SHUT;

        // The head and each stalk eye show the drawn mood, blinking as the
        // pose blinks, each from a resident layer.
        assert!(
            item.missing.is_empty(),
            "at {t}: {:?} not resident",
            item.missing
        );
        assert_eq!(
            item.faces,
            face_bindings(&self.robot, drawn, shut),
            "at {t}"
        );
        assert_eq!(item.faces.len(), 1 + stalk_eyes(&self.robot));
        // The lights are the drawn mood's.
        let lights = self.robot.motion.mood(drawn);
        assert_eq!(
            (
                item.pose.status,
                item.pose.leds,
                item.pose.lit,
                item.pose.rays
            ),
            (lights.status, lights.leds, lights.lit, lights.rays),
            "at {t}"
        );

        if drawn != self.drawn {
            assert_eq!(drawn, item.posed_face, "at {t}: only to the posed face");
            let late = !shut;
            if late {
                let drawable = self.drawable[index(drawn)].expect("a drawn set is resident");
                let since = self.posed.1.max(drawable);
                assert!(
                    t - since >= LATE_AFTER,
                    "at {t}: off a blink only {LATE_AFTER} s after {since}"
                );
                self.late += 1;
            }
            self.switches.push((t, drawn, late));
        }
        assert_eq!(
            h.stage.frame_stats().late_face_switches - self.late_before,
            self.late,
            "at {t}: the late counter counts exactly the late switches"
        );
        self.drawn = drawn;
    }

    /// Renders `n` frames `dt` apart, checking each.
    fn run(&mut self, h: &mut Harness, n: usize, dt: f64) {
        for _ in 0..n {
            h.step(dt);
            self.check(h);
        }
    }
}

/// The first seeds whose robots have no stalk eyes and stalk eyes.
fn plain_and_stalked_seeds() -> [u32; 2] {
    let eyes = |seed: &u32| stalk_eyes(&robot(&genome(*seed)));
    let plain = (1..=200)
        .find(|s| eyes(s) == 0)
        .expect("a seed without stalk eyes");
    let stalked = (1..=200)
        .find(|s| eyes(s) > 0)
        .expect("a seed among the first 200 grows stalk eyes");
    [plain, stalked]
}

#[test]
fn the_drawn_mood_changes_only_on_a_blink_or_a_second_late() {
    for seed in plain_and_stalked_seeds() {
        drawn_mood_at_30_fps(seed);
    }
}

/// A robot through a mood change and each reaction at 30 fps, with its face
/// sets rasterized behind a gate, then a set landing after the swap blink
/// that a later blink switches in, and one no blink switches in for a
/// second.
fn drawn_mood_at_30_fps(seed: u32) {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.stage.set_creatures(&[spec("a", seed)]);
    h.until_drawn(&["a"]);
    h.stage.set_max_fps(30);
    h.step(STEP_30);
    let mut w = Watch::new(&h, "a", seed);
    let robot = w.robot.clone();
    let mut calls = 0;

    // A mood change whose set lands within half a second.
    h.builder.hold_textures();
    h.stage.set_state("a", Mood::Happy, None);
    calls += 1;
    w.run(&mut h, 6, STEP_30);
    assert_eq!(w.posed.0, Mood::Neutral, "held until its set is drawable");
    h.land_textures(calls);
    w.run(&mut h, 60, STEP_30);
    assert_eq!(w.drawn, Mood::Happy);

    // Each reaction; one whose face set is not resident waits for it.
    for reaction in REACTIONS {
        h.builder.hold_textures();
        if let Some(face) = robot.motion.reaction(reaction).face {
            if !resident(&h, &robot, "a", face) {
                calls += 1;
            }
        }
        h.stage.play("a", reaction);
        w.run(&mut h, 6, STEP_30);
        h.land_textures(calls);
        w.run(&mut h, 90, STEP_30);
    }
    assert_eq!(w.drawn, Mood::Happy);

    // Held past its half second, the change reaches the actor without its
    // set, and the swap blink passes with nothing to switch to.
    h.builder.hold_textures();
    h.stage.set_state("a", Mood::Sad, None);
    calls += 1;
    w.run(&mut h, 1, STEP_30);
    let drained = h.last().unwrap().stage_time;
    while h.last().unwrap().stage_time - drained < 0.8 {
        w.run(&mut h, 1, STEP_30);
    }
    assert_eq!((w.posed.0, w.drawn), (Mood::Sad, Mood::Happy));
    // The set lands four frames before an idle blink begins; that blink
    // switches it in, not late.
    let times = ahead(&h, STEP_30, 600);
    let shut = |k: usize| robot.motion.params.blink(times[k]) >= SHUT;
    let k = (13..times.len())
        .find(|&k| shut(k) && !(k - 4..k).any(shut))
        .expect("an idle blink within 20 s");
    w.run(&mut h, k - 4, STEP_30);
    h.land_textures(calls);
    let switches = w.switches.len();
    w.run(&mut h, 8, STEP_30);
    assert_eq!(w.switches[switches..], [(times[k], Mood::Sad, false)]);

    // A set that lands where no blink comes for over a second switches in
    // a second after it became drawable, counted late.
    h.builder.hold_textures();
    h.stage.set_state("a", Mood::Blocked, None);
    calls += 1;
    w.run(&mut h, 1, STEP_30);
    let drained = h.last().unwrap().stage_time;
    while h.last().unwrap().stage_time - drained < 0.8 {
        w.run(&mut h, 1, STEP_30);
    }
    let times = ahead(&h, STEP_30, 900);
    let shut = |k: usize| robot.motion.params.blink(times[k]) >= SHUT;
    let landing = (1..times.len() - 45)
        .find(|&k| !(k..k + 45).any(shut))
        .expect("a second and a half without a blink within 30 s");
    w.run(&mut h, landing, STEP_30);
    h.land_textures(calls);
    let switches = w.switches.len();
    w.run(&mut h, 40, STEP_30);
    let late = w.switches[switches..].to_vec();
    assert_eq!(late.len(), 1);
    let (at, mood, was_late) = late[0];
    assert_eq!((mood, was_late), (Mood::Blocked, true));
    assert!(at - times[landing] >= LATE_AFTER && at - times[landing] < LATE_AFTER + 2.0 * STEP_30);
    assert!(w.late >= 1);
}

#[test]
fn at_one_frame_a_second_a_resident_set_waits_for_a_blink_or_a_second() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    // Happy's set is made resident by a change there and back.
    h.stage.set_state("a", Mood::Happy, None);
    for _ in 0..90 {
        h.frame().unwrap();
    }
    h.stage.set_state("a", Mood::Neutral, None);
    for _ in 0..90 {
        h.frame().unwrap();
    }
    let robot = robot(&genome(1));
    assert_eq!(h.item("a").pose.face, Mood::Neutral);
    assert!(resident(&h, &robot, "a", Mood::Happy));

    h.stage.set_max_fps(1);
    h.step(1.0);
    let mut w = Watch::new(&h, "a", 1);
    let held = h.stage.frame_stats().held_face_commands;
    // The change takes effect at frame k; frame k + 1, the first that
    // poses the new face, has no blink.
    let times = ahead(&h, 1.0, 8);
    let k = (0..6)
        .find(|&k| robot.motion.params.blink(times[k + 1]) < SHUT)
        .expect("a frame without a blink");
    w.run(&mut h, k, 1.0);
    h.stage.set_state("a", Mood::Happy, None);
    w.run(&mut h, 1, 1.0);
    w.run(&mut h, 1, 1.0);
    let first = h.item("a");
    assert_eq!(
        (first.posed_face, first.pose.face),
        (Mood::Happy, Mood::Neutral)
    );
    assert!(first.pose.blink < SHUT);
    // About a second later it switches, on a blink or late: the step's
    // floating-point sum may fall an ulp short of the second, which defers
    // a late switch by one frame.
    w.run(&mut h, 1, 1.0);
    if h.item("a").pose.face != Mood::Happy {
        w.run(&mut h, 1, 1.0);
    }
    assert_eq!(h.item("a").pose.face, Mood::Happy);
    assert_eq!(h.stage.frame_stats().held_face_commands, held, "not held");
}

#[test]
fn a_mood_change_while_the_build_is_held_first_draws_the_latest_mood() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.builder.hold("a");
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.builder.wait_for("the build", |calls| calls.len() == 1);
    h.stage
        .set_state("a", Mood::Happy, Some(StatusLoop::Working));
    h.frame().unwrap();
    h.builder.release("a");
    h.until_drawn(&["a"]);

    let first = h.last().unwrap();
    let item = h.item("a");
    assert_eq!(
        (item.pose.face, item.posed_face),
        (Mood::Happy, Mood::Happy)
    );
    assert!(item.missing.is_empty());
    assert_eq!(
        item.pose,
        fresh_pose(
            1,
            Mood::Happy,
            Some(StatusLoop::Working),
            first.stage_time,
            first.stage_time
        )
    );
    // Built in the mood of the time, then the latest mood's set first.
    assert_eq!(h.builder.calls()[0].mood, Mood::Neutral);
    assert!(h
        .builder
        .texture_calls()
        .iter()
        .any(|c| c.generation == item.generation && c.work == TextureWork::Faces(Mood::Happy)));
}

#[test]
fn a_spec_change_with_a_mood_change_first_draws_the_latest_mood() {
    // Two workers: the old robot's face set waits behind the gate on one,
    // while the new robot builds on the other.
    let mut h = harness_with(Target::Image, BuildMode::Workers, 2, true);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.until_drawn(&["a"]);
    let old = h.drawn_generation("a").unwrap();
    h.builder.hold("a");
    h.builder.hold_textures();
    h.stage.set_creatures(&[spec("a", 2)]);
    h.stage.set_state("a", Mood::Sad, None);
    for _ in 0..10 {
        h.frame().unwrap();
        let item = h.item("a");
        assert_eq!((item.generation, item.pose.face), (old, Mood::Neutral));
    }
    h.builder.release("a");
    h.until("the new robot", |h| h.drawn_generation("a") != Some(old));
    let item = h.item("a");
    assert_eq!((item.pose.face, item.posed_face), (Mood::Sad, Mood::Sad));
    assert!(item.missing.is_empty());
    assert_eq!(h.builder.calls().last().unwrap().mood, Mood::Sad);
}

#[test]
fn resume_after_a_mood_change_while_paused_first_draws_the_latest_mood() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.until_drawn(&["a"]);
    h.stage.pause();
    assert!(h.frame().is_err());
    h.stage.set_state("a", Mood::Blocked, None);
    h.stage.resume();
    let before = h.fake.frame_count();
    h.until("a back", |h| {
        h.fake.frame_count() > before && h.last().is_some_and(|f| f.item("a").is_some())
    });
    let item = h.item("a");
    assert_eq!(
        (item.pose.face, item.posed_face),
        (Mood::Blocked, Mood::Blocked)
    );
    assert!(item.missing.is_empty());
}

#[test]
fn a_set_unused_for_ten_seconds_is_released_and_one_used_within_ten_is_kept() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    let robot = robot(&genome(1));
    let waiting = |h: &Harness| resident(h, &robot, "a", Mood::Waiting);
    let mut last_used = f64::NEG_INFINITY;
    // Renders a frame 0.1 s on; the stage time, whether the waiting set is
    // resident, and the layer bytes.
    let mut step = |h: &mut Harness| {
        h.step(0.1);
        let item = h.item("a");
        let t = h.last().unwrap().stage_time;
        if item.pose.face == Mood::Waiting || item.posed_face == Mood::Waiting {
            last_used = t;
        }
        (t, waiting(h), h.fake.layer_bytes(), last_used)
    };

    // The ask reaction shows the waiting face for a while.
    h.stage.play("a", Reaction::Ask);
    let mut now = step(&mut h);
    assert!(now.1, "its set is rasterized and resident at once");
    for _ in 0..40 {
        now = step(&mut h);
        assert!(now.1);
    }
    assert!(now.3.is_finite(), "the waiting face was shown");
    assert_eq!(h.item("a").pose.face, Mood::Neutral, "and is gone again");
    while now.0 - now.3 < RELEASE_AFTER - 1.5 {
        now = step(&mut h);
        assert!(now.1, "kept while used within 10 s");
    }
    // Used again within ten seconds: kept, and never rasterized again.
    let calls = h.builder.texture_calls().len();
    h.stage.play("a", Reaction::Ask);
    for _ in 0..30 {
        now = step(&mut h);
        assert!(now.1);
    }
    assert_eq!(h.builder.texture_calls().len(), calls);

    // Unused for ten seconds: released, and the layers shrink.
    let mut before = now;
    loop {
        now = step(&mut h);
        if !now.1 {
            break;
        }
        assert!(now.0 - now.3 < RELEASE_AFTER, "released by {}", now.0);
        before = now;
    }
    assert!(now.0 - now.3 >= RELEASE_AFTER);
    assert!(before.0 - before.3 < RELEASE_AFTER);
    assert!(now.2 < before.2, "layer storage falls");
    // The drawn mood's set stays.
    assert!(resident(&h, &robot, "a", h.item("a").pose.face));
    assert_eq!(index(h.item("a").pose.face), index(Mood::Neutral));
}
