//! The motion scripts every corpus genome runs in every mood, and the times
//! its poses are pinned at, shared by the `robot_pins` example and the
//! robot tests.
//!
//! The scripts are: idle only, to 3 s; each reaction started at 0, to its
//! end; each status loop started at 0, for two periods; a mood change at
//! 1 s to the next mood in `MOODS` order (sleepy wraps to neutral), to 3 s;
//! and one composed case (the working loop from 0, hello at 0.5 s and the
//! same mood change at 1 s), to the latest of 3 s, hello's end and two
//! working periods. A script's end bounds its sampling only: its events
//! stay as they are at every time asked for.

// The example and the tests each use only part of this module.
#![allow(dead_code)]

use avatar_3d::motion::{pose_bytes, Actor, Reaction, StatusLoop, REACTIONS, STATUS_LOOPS};
use avatar_3d::robot::{build_with, BuildOptions, Robot};
use avatar_small_robots::{mood_name, Mood, MOODS};
use sha2::{Digest, Sha256};

use crate::cases;

/// When the mood-change and composed scripts change the mood.
pub const MOOD_CHANGE_AT: f64 = 1.0;
/// When the composed script starts its reaction.
pub const COMPOSED_REACTION_AT: f64 = 0.5;
/// The end of the idle and mood-change scripts, and the least end of the
/// composed one.
pub const IDLE_END: f64 = 3.0;
/// A time pinned in every script, even past its end.
pub const LATE: f64 = 2.5;
/// The pose pins file, relative to the repository's `vectors/`.
pub const POSE_PINS: &str = "robots-3d/poses.tsv";
/// The header comment of the pose pins file.
pub const POSE_HEADER: &str =
    "# Pose digests of the 3D small robots' motion scripts. Written only by\n\
# `cargo run -p cortexkit-avatar-3d --example robot_pins -- --write`.";

/// One motion script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Idle,
    React(Reaction),
    Loop(StatusLoop),
    MoodChange,
    Composed,
}

/// Every script, in pin order.
pub fn scripts() -> Vec<Script> {
    let mut out = vec![Script::Idle];
    out.extend(REACTIONS.map(Script::React));
    out.extend(STATUS_LOOPS.map(Script::Loop));
    out.push(Script::MoodChange);
    out.push(Script::Composed);
    out
}

/// The mood after `m` in `MOODS` order; sleepy wraps to neutral.
pub fn next_mood(m: Mood) -> Mood {
    let i = MOODS.iter().position(|x| *x == m).expect("a listed mood");
    MOODS[(i + 1) % MOODS.len()]
}

impl Script {
    /// The script's name in pin labels.
    pub fn name(self) -> String {
        match self {
            Script::Idle => "idle".into(),
            Script::React(r) => r.name().into(),
            Script::Loop(l) => format!("loop.{}", l.name()),
            Script::MoodChange => "mood-change".into(),
            Script::Composed => "composed".into(),
        }
    }

    /// The reaction the script plays, with its start time.
    pub fn reaction(self) -> Option<(f64, Reaction)> {
        match self {
            Script::React(r) => Some((0.0, r)),
            Script::Composed => Some((COMPOSED_REACTION_AT, REACTIONS[0])),
            _ => None,
        }
    }

    /// The status loop the script runs from 0.
    pub fn status_loop(self) -> Option<StatusLoop> {
        match self {
            Script::Loop(l) => Some(l),
            Script::Composed => Some(STATUS_LOOPS[0]),
            _ => None,
        }
    }

    /// Whether the script changes the mood at [`MOOD_CHANGE_AT`].
    pub fn changes_mood(self) -> bool {
        matches!(self, Script::MoodChange | Script::Composed)
    }

    /// An actor for `robot` starting in `mood` with the script's events.
    pub fn actor(self, robot: &Robot, mood: Mood) -> Actor<'_> {
        let mut actor = Actor::new(robot, mood);
        if let Some(l) = self.status_loop() {
            actor.set_loop(0.0, Some(l));
        }
        if let Some((at, r)) = self.reaction() {
            actor.play(at, r);
        }
        if self.changes_mood() {
            actor.set_mood(MOOD_CHANGE_AT, next_mood(mood));
        }
        actor
    }

    /// The end of the script's sampling.
    pub fn end(self, robot: &Robot) -> f64 {
        let m = &robot.motion;
        match self {
            Script::Idle | Script::MoodChange => IDLE_END,
            Script::React(r) => m.reaction(r).duration,
            Script::Loop(l) => 2.0 * m.status_loop(l).duration,
            Script::Composed => {
                let (at, r) = self.reaction().expect("the composed script reacts");
                let period = m
                    .status_loop(self.status_loop().expect("the composed script loops"))
                    .duration;
                IDLE_END.max(at + m.reaction(r).duration).max(2.0 * period)
            }
        }
    }

    /// The times its poses are pinned at, labelled: 0 and 2.5 s, and the
    /// reaction's peak and end when it has one.
    pub fn pinned(self, robot: &Robot) -> Vec<(&'static str, f64)> {
        let mut out = vec![("t0", 0.0), ("t2.5", LATE)];
        if let Some((at, r)) = self.reaction() {
            let clip = robot.motion.reaction(r);
            out.push(("peak", at + clip.peak.expect("every reaction has a peak")));
            out.push(("end", at + clip.duration));
        }
        out
    }

    /// Every 1/120 s from 0 to the script's end, and the end itself.
    pub fn samples(self, robot: &Robot) -> Vec<f64> {
        let end = self.end(robot);
        let mut out: Vec<f64> = (0..)
            .map(|k| f64::from(k) / 120.0)
            .take_while(|t| *t <= end)
            .collect();
        if out.last() != Some(&end) {
            out.push(end);
        }
        out
    }
}

/// A pose's SHA-256 in hex.
pub fn pose_digest(actor: &Actor, t: f64) -> String {
    let bytes = pose_bytes(&actor.evaluate(t)).expect("a pose is finite");
    Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The pose pins file's lines: one labelled SHA-256 for every corpus
/// genome, mood, script and pinned time.
pub fn pose_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for (id, genome) in cases::corpus() {
        let robot = build_with(&genome, BuildOptions::untextured());
        for mood in MOODS {
            for script in scripts() {
                let actor = script.actor(&robot, mood);
                for (label, t) in script.pinned(&robot) {
                    lines.push(format!(
                        "pose/{id}/{}/{}/{label}\t{}",
                        mood_name(mood),
                        script.name(),
                        pose_digest(&actor, t)
                    ));
                }
            }
        }
    }
    lines
}

/// The committed pose pins' lines, without comments.
pub fn committed_poses() -> Vec<String> {
    let path = cases::vectors().join(POSE_PINS);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(str::to_owned)
        .collect()
}
