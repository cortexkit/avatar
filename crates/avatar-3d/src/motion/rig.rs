//! A robot's motion data: its mood postures, idle, reactions and status
//! loops, as the builder makes them.

use avatar_small_robots::layout::Status;
use avatar_small_robots::Mood;

use super::clip::Clip;
use super::idle::{Idle, MotionParams};
use super::math::V3;

/// The five reactions, each a meaning the robot expresses with its own
/// anatomy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reaction {
    /// A greeting: the camera-side arm waves; the happy face.
    Hello,
    /// "I have a question for you": the camera-side hand up and held; the
    /// waiting face; the lights pulse.
    Ask,
    /// Finished something: a jump with both arms thrown up.
    Done,
    /// Stuck and needs help: stomps and a "no" shake, then a slump.
    Blocked,
    /// The answer to being clicked: a jolt and a wobble back.
    Poke,
}

/// The reactions in their conventional order.
pub const REACTIONS: [Reaction; 5] = [
    Reaction::Hello,
    Reaction::Ask,
    Reaction::Done,
    Reaction::Blocked,
    Reaction::Poke,
];

impl Reaction {
    /// The reaction's name.
    pub fn name(self) -> &'static str {
        ["hello", "ask", "done", "blocked", "poke"][self.index()]
    }

    /// The reaction's place in [`REACTIONS`].
    pub fn index(self) -> usize {
        REACTIONS
            .iter()
            .position(|r| *r == self)
            .expect("every reaction is listed")
    }
}

/// The four status loops: gentle looping clips layered over idle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StatusLoop {
    /// A rhythmic tapping, 1.2 s.
    Working,
    /// Slow looks left and right, 4.0 s.
    Waiting,
    /// Slumped, with a short frustrated shake each cycle, 2.8 s.
    Blocked,
    /// Slow deep breaths and a drooping nod, 4.6 s.
    Sleepy,
}

/// The status loops in their conventional order.
pub const STATUS_LOOPS: [StatusLoop; 4] = [
    StatusLoop::Working,
    StatusLoop::Waiting,
    StatusLoop::Blocked,
    StatusLoop::Sleepy,
];

impl StatusLoop {
    /// The loop's name, without the `loop.` prefix its clip carries.
    pub fn name(self) -> &'static str {
        ["working", "waiting", "blocked", "sleepy"][self.index()]
    }

    /// The loop's place in [`STATUS_LOOPS`].
    pub fn index(self) -> usize {
        STATUS_LOOPS
            .iter()
            .position(|l| *l == self)
            .expect("every status loop is listed")
    }
}

/// A static offset of one bone from rest, applied with a weight.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BoneOffset {
    /// Index into the robot's bones.
    pub bone: usize,
    /// XYZ Euler angles in radians.
    pub rotation: V3,
    /// Model units.
    pub translation: V3,
    /// Factors minus one.
    pub scale: V3,
}

/// One mood as the 3D robot carries it: a posture, which blends, and the
/// 2D mood's light fields, which switch with its face.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct MoodPose {
    /// The mood.
    pub mood: Mood,
    /// Offsets from the neutral rest pose; empty for neutral.
    pub offsets: Vec<BoneOffset>,
    /// The status lights' key.
    pub status: Status,
    /// The four chest LEDs' keys.
    pub leds: [Status; 4],
    /// Whether the bulb shows the full status colour.
    pub lit: bool,
    /// Whether the beacon dome shows the full status colour.
    pub rays: bool,
    /// The hover jet's thrust: the flame's emissive scale and its length.
    pub thrust: f64,
    /// Whether hover gear lands in this mood.
    pub landed: bool,
}

/// One arm's bones and rest joints, for reactions and the floor rule.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ArmRig {
    /// −1 for the left (−x) arm, +1 for the right.
    pub side: f64,
    /// The `upperarm.X` bone, at the shoulder.
    pub upper: usize,
    /// The `forearm.X` bone, at the elbow.
    pub fore: usize,
    /// The `wrist.X` bone.
    pub wrist: usize,
    /// The `grip.X` bone, carrying the jaws.
    pub grip: usize,
    /// Shoulder, elbow and wrist at rest, in model units.
    pub joints: [V3; 3],
    /// Whether the forearm turns at the elbow; a stub or a telescope does
    /// not.
    pub elbow: bool,
}

/// Everything an actor needs to pose one robot.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Motion {
    /// The motion bytes' parameters.
    pub params: MotionParams,
    /// The rest height in model units: the highest point of every part.
    pub height: f64,
    /// The idle formulas' bones and amplitudes.
    pub idle: Idle,
    /// The seven moods, in `MOODS` order.
    pub moods: Vec<MoodPose>,
    /// The reactions, in [`REACTIONS`] order.
    pub reactions: Vec<Clip>,
    /// The status loops, in [`STATUS_LOOPS`] order.
    pub loops: Vec<Clip>,
    /// The arms, left then right, as the robot has them.
    pub arms: Vec<ArmRig>,
    /// The `exhaust.0` bone of hover gear, which shrinks toward the nozzle
    /// with thrust and is kept off the floor.
    pub exhaust: Option<usize>,
}

impl Motion {
    /// A reaction's clip.
    pub fn reaction(&self, r: Reaction) -> &Clip {
        &self.reactions[r.index()]
    }

    /// A status loop's clip.
    pub fn status_loop(&self, l: StatusLoop) -> &Clip {
        &self.loops[l.index()]
    }

    /// A mood's posture and light fields.
    pub fn mood(&self, m: Mood) -> &MoodPose {
        self.moods
            .iter()
            .find(|p| p.mood == m)
            .expect("every mood has a pose")
    }
}
