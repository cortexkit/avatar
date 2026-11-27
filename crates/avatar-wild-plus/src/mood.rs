//! Moods: what each one does to the eyes, brows, mouth and posture, and how
//! a mood turns a part about its root.

use avatar_svg::Mood;

use crate::parts::Part;

/// What a mood does to the face and the posture.
pub struct MoodSpec {
    /// How closed the upper lids are, and their slant in degrees.
    pub lid: f64,
    pub ang: f64,
    /// Eye scale.
    pub es: f64,
    /// A squint from below.
    pub sq: f64,
    pub brows: usize,
    /// The mouth kind, or the creature's own.
    pub mouth: Option<u32>,
    pub adj: &'static str,
    pub gaze: [f64; 2],
    /// How far limbs droop (radians, positive is down).
    pub droop: f64,
    pub squash: [f64; 2],
    pub tilt: f64,
    pub lift: f64,
}

pub const MOODS: [Mood; 7] = [
    Mood::Neutral,
    Mood::Working,
    Mood::Happy,
    Mood::Waiting,
    Mood::Blocked,
    Mood::Sad,
    Mood::Sleepy,
];

pub fn spec(mood: Mood) -> MoodSpec {
    let base = MoodSpec {
        lid: 0.0,
        ang: 0.0,
        es: 1.0,
        sq: 0.0,
        brows: 0,
        mouth: None,
        adj: "",
        gaze: [0.0, 0.0],
        droop: 0.0,
        squash: [1.0, 1.0],
        tilt: 0.0,
        lift: 0.0,
    };
    match mood {
        Mood::Neutral => base,
        Mood::Working => MoodSpec {
            lid: 0.22,
            ang: 4.0,
            es: 0.96,
            sq: 0.1,
            brows: 1,
            mouth: Some(8),
            adj: "sweat",
            gaze: [0.0, 0.45],
            droop: -0.05,
            ..base
        },
        Mood::Happy => MoodSpec {
            es: 1.04,
            sq: 0.45,
            mouth: Some(2),
            adj: "sparkle",
            droop: -0.17,
            squash: [0.985, 1.025],
            lift: 3.0,
            ..base
        },
        Mood::Waiting => MoodSpec {
            es: 1.16,
            brows: 2,
            mouth: Some(6),
            adj: "dots",
            gaze: [0.45, -0.55],
            droop: -0.06,
            tilt: -3.5,
            ..base
        },
        Mood::Blocked => MoodSpec {
            lid: 0.38,
            ang: 14.0,
            brows: 3,
            mouth: Some(9),
            adj: "fume",
            droop: 0.08,
            squash: [1.025, 0.985],
            ..base
        },
        Mood::Sad => MoodSpec {
            lid: 0.3,
            ang: -12.0,
            es: 0.97,
            brows: 4,
            mouth: Some(7),
            adj: "tear",
            gaze: [0.0, 0.5],
            droop: 0.34,
            squash: [1.02, 0.965],
            ..base
        },
        Mood::Sleepy => MoodSpec {
            lid: 1.0,
            mouth: Some(10),
            adj: "zz",
            droop: 0.45,
            squash: [1.045, 0.93],
            ..base
        },
    }
}

/// How this creature carries a mood: which way it tilts and glances, how
/// far its limbs droop, how much its body sags or lifts.
#[derive(Clone, Copy)]
pub struct Temper {
    pub side: f64,
    pub tilt: f64,
    pub droop: f64,
    pub squash: f64,
}

/// How far a mood turns a part about its root, in radians, clockwise on the
/// page. `way` is the side of the axis the part is rooted on. A part that
/// perks is turned no further than its `perk_max`.
pub fn mood_turn(part: &Part, droop: f64, temper: &Temper, way: f64) -> f64 {
    let mut lean = droop * temper.droop * part.sway;
    if lean < 0.0 {
        if let Some(cap) = part.perk_max {
            lean = if -cap > lean { -cap } else { lean };
        }
    }
    lean * way + part.cock
}

/// The side of the axis a part in frame units is rooted on.
pub fn way_of(root_x: f64) -> f64 {
    if root_x > 120.5 {
        1.0
    } else if root_x < 119.5 {
        -1.0
    } else {
        0.4
    }
}
