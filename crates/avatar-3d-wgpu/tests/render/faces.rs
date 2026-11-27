//! The face textures a draw selects follow the actor's face set and blink,
//! before, at and after a face swap.

use avatar_3d::motion::Actor;
use avatar_3d::robot::{build_with, BuildOptions, Robot, TextureContent};
use avatar_3d_wgpu::frame::{self, BLINK_SHOWS};
use avatar_small_robots::{decode, Mood, Tech, MOODS};

use crate::genomes;

/// The content of the texture a slot selects.
fn content(robot: &Robot, index: Option<usize>) -> TextureContent {
    robot.textures[index.expect("the slot shows a texture")]
        .binding
        .content
}

/// Changes the mood at 1 s and samples every 1/120 s up to 3 s, checking
/// at every sample that the head (and every stalk eye) shows the face set's
/// pair, blinking exactly when the blink is at least one half. Returns the
/// selections just before, at and after the swap.
fn check_swap(robot: &Robot) -> [(Mood, bool); 3] {
    let from = Mood::Neutral;
    let to = MOODS[1];
    let mut actor = Actor::new(robot, from);
    actor.set_mood(1.0, to);
    let mut seen = Vec::new();
    for i in 0..=360 {
        let t = f64::from(i) / 120.0;
        let pose = actor.evaluate(t);
        let blinking = pose.blink >= BLINK_SHOWS;
        let faces = frame::faces(robot, &pose);
        assert_eq!(
            content(robot, faces.head),
            TextureContent::Face {
                mood: pose.face,
                blinking
            },
            "the head at t = {t}"
        );
        for (eye, stalk) in faces.stalks.iter().enumerate() {
            if robot.part_named(["stalk.L.eye", "stalk.R.eye"][eye]) {
                assert_eq!(
                    content(robot, *stalk),
                    TextureContent::StalkEye {
                        eye,
                        mood: pose.face,
                        blinking
                    },
                    "stalk eye {eye} at t = {t}"
                );
            } else {
                assert_eq!(*stalk, None, "a robot without stalks shows no stalk eye");
            }
        }
        seen.push((pose.face, blinking));
    }
    let swap = seen
        .iter()
        .position(|s| s.0 == to)
        .expect("the face swaps within two seconds of the mood change");
    assert!(swap > 0 && seen[..swap].iter().all(|s| s.0 == from));
    assert!(
        seen[swap..].iter().all(|s| s.0 == to),
        "the face swaps once"
    );
    let after = seen[swap..]
        .iter()
        .position(|s| !s.1)
        .expect("the eyes open again after the swap");
    [seen[swap - 1], seen[swap], seen[swap + after]]
}

#[test]
fn the_drawn_face_follows_the_face_set_and_blink_across_a_swap() {
    let robot = build_with(&genomes::corpus()[0], BuildOptions::untextured());
    let [before, at, after] = check_swap(&robot);
    assert_eq!(before.0, Mood::Neutral);
    // The new face first shows while the blinking textures do, so no frame
    // shows two faces.
    assert_eq!(at, (MOODS[1], true));
    assert_eq!(after, (MOODS[1], false));
}

#[test]
fn stalk_eyes_swap_with_the_face() {
    let genome = (0..2000)
        .map(|i| genomes::random(3301, i))
        .find(|g| decode(g).face == Tech::Stalk)
        .expect("a stalk robot among the first 2000 genomes of seed 3301");
    let robot = build_with(&genome, BuildOptions::untextured());
    assert!(robot.part_named("stalk.L.eye") && robot.part_named("stalk.R.eye"));
    let [_, at, after] = check_swap(&robot);
    assert_eq!(at, (MOODS[1], true));
    assert_eq!(after, (MOODS[1], false));
}

/// Whether a robot has a part of this name.
trait PartNamed {
    fn part_named(&self, name: &str) -> bool;
}

impl PartNamed for Robot {
    fn part_named(&self, name: &str) -> bool {
        self.parts.iter().any(|p| p.name == name)
    }
}
