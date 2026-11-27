//! The motion scripts: every corpus genome in every mood runs every script,
//! poses depend on nothing but the events and the time, and the pinned
//! pose digests match.

use avatar_3d::motion::{pose_bytes, Actor, Pose};
use avatar_3d::robot::{build_with, BuildOptions, Robot, Skin};
use avatar_small_robots::{mood_name, Mood, MOODS};

use crate::cases;
use crate::scripts;

pub type V3 = [f64; 3];
pub type M4 = [f64; 16];

/// The point `p` carried by the column-major matrix `m`.
pub fn apply(m: &M4, p: V3) -> V3 {
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

/// The point `p` relative to bone `j`'s joint: what the inverse of its rest
/// matrix, a pure translation, makes of it.
pub fn local(robot: &Robot, j: usize, p: V3) -> V3 {
    let r = &robot.bones[j].rest;
    assert_eq!(
        &r[..12],
        &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        "a rest matrix with a rotation"
    );
    [p[0] - r[12], p[1] - r[13], p[2] - r[14]]
}

/// Every vertex of one part, posed: `M_b · R_b⁻¹ · v` for a rigid part,
/// `Σ w_j · M_j · R_j⁻¹ · v` for a skinned one.
pub fn posed(robot: &Robot, pose: &Pose, part: usize) -> Vec<V3> {
    let p = &robot.parts[part];
    match &p.skin {
        Skin::Rigid { bone } => p
            .mesh
            .positions
            .iter()
            .map(|v| apply(&pose.bones[*bone], local(robot, *bone, *v)))
            .collect(),
        Skin::Skinned { joints, weights } => p
            .mesh
            .positions
            .iter()
            .enumerate()
            .map(|(i, v)| {
                (0..4).fold([0.0; 3], |acc, k| {
                    let j = joints[i][k] as usize;
                    let q = apply(&pose.bones[j], local(robot, j, *v));
                    [0, 1, 2].map(|c| acc[c] + weights[i][k] * q[c])
                })
            })
            .collect(),
        _ => panic!("a part is rigid or skinned"),
    }
}

/// The corpus robots, untextured, with their project ids.
pub fn corpus_robots() -> Vec<(String, Robot)> {
    cases::corpus()
        .into_iter()
        .map(|(id, g)| (id, build_with(&g, BuildOptions::untextured())))
        .collect()
}

fn bytes(pose: &Pose) -> Vec<u8> {
    pose_bytes(pose).expect("a pose is finite")
}

#[test]
fn every_script_runs_and_its_log_outlasts_its_end() {
    for (id, robot) in corpus_robots() {
        for mood in MOODS {
            for script in scripts::scripts() {
                let label = format!("{id} {} {}", mood_name(mood), script.name());
                let actor = script.actor(&robot, mood);
                let end = script.end(&robot);
                for t in [0.0, end / 2.0, end] {
                    let pose = actor.evaluate(t);
                    assert_eq!(pose.bones.len(), robot.bones.len(), "{label}");
                    bytes(&pose);
                }
                // Past the end, an ended reaction contributes nothing: the
                // pose is the one the same events without it give.
                if let Some((_, reaction)) = script.reaction() {
                    let clip = robot.motion.reaction(reaction);
                    let after = end.max(script.reaction().unwrap().0 + clip.duration) + 1.0;
                    let mut without = Actor::new(&robot, mood);
                    if let Some(l) = script.status_loop() {
                        without.set_loop(0.0, Some(l));
                    }
                    if script.changes_mood() {
                        without.set_mood(scripts::MOOD_CHANGE_AT, scripts::next_mood(mood));
                    }
                    assert_eq!(
                        bytes(&actor.evaluate(after)),
                        bytes(&without.evaluate(after)),
                        "{label}: the ended reaction still contributes at {after}"
                    );
                }
                // A status loop keeps running past the script's end.
                if let Some(l) = script.status_loop() {
                    let period = robot.motion.status_loop(l).duration;
                    let after = end + 0.25 * period;
                    let mut without = Actor::new(&robot, mood);
                    if let Some((at, r)) = script.reaction() {
                        without.play(at, r);
                    }
                    if script.changes_mood() {
                        without.set_mood(scripts::MOOD_CHANGE_AT, scripts::next_mood(mood));
                    }
                    assert_ne!(
                        bytes(&actor.evaluate(after)),
                        bytes(&without.evaluate(after)),
                        "{label}: the loop stopped by {after}"
                    );
                }
            }
        }
    }
}

#[test]
fn poses_are_independent_of_evaluation_order() {
    for (id, robot) in corpus_robots() {
        for mood in MOODS {
            for script in scripts::scripts() {
                let label = format!("{id} {} {}", mood_name(mood), script.name());
                let times: Vec<f64> = script.pinned(&robot).iter().map(|p| p.1).collect();
                let fresh: Vec<Vec<u8>> = times
                    .iter()
                    .map(|&t| bytes(&script.actor(&robot, mood).evaluate(t)))
                    .collect();
                // One actor asked later times first, then earlier ones.
                let actor = script.actor(&robot, mood);
                for &t in times.iter().rev() {
                    actor.evaluate(t + 0.5);
                    actor.evaluate(t);
                }
                for (i, &t) in times.iter().enumerate() {
                    actor.evaluate(t + 1.0);
                    assert_eq!(bytes(&actor.evaluate(t)), fresh[i], "{label} at {t}");
                    actor.evaluate((t - 0.3).max(0.0));
                    assert_eq!(bytes(&actor.evaluate(t)), fresh[i], "{label} at {t}");
                }
            }
        }
    }
}

#[test]
fn pose_digests_equal_the_pins() {
    let committed = scripts::committed_poses();
    let lines = scripts::pose_lines();
    let labels = |lines: &[String]| -> Vec<String> {
        lines
            .iter()
            .map(|l| l.split('\t').next().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        labels(&lines),
        labels(&committed),
        "the pose label inventory differs from the pins"
    );
    // Every corpus genome, mood and script, at 0 and 2.5 s, and at a
    // reaction's peak and end.
    let reacting = scripts::scripts()
        .iter()
        .filter(|s| s.reaction().is_some())
        .count();
    assert_eq!(
        lines.len(),
        cases::corpus().len() * MOODS.len() * (2 * scripts::scripts().len() + 2 * reacting)
    );
    for (got, want) in lines.iter().zip(&committed) {
        assert_eq!(
            got, want,
            "rerun the robot_pins example with --write if the change is intended"
        );
    }
}

/// A mood's name, for messages.
pub fn name(m: Mood) -> &'static str {
    mood_name(m)
}
