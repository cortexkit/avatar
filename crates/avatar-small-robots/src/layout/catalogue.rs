//! Decoder catalogues and their reachable values.

pub use crate::arms::{ARM_NAMES, HAND_NAMES};
pub use crate::body::{BODY_NAMES, NECK_NAMES, PELVIS_NAMES, PLAN_NAMES};
pub use crate::face::SCREEN_EYES;
pub use crate::gear::{FOOT_NAMES, GEAR_NAMES};
pub use crate::hull::HULL_NAMES;
pub use crate::kit::METAL_NAMES;
pub use crate::marks::{CHEST_NAMES, DECAL_NAMES, DECOR_NAMES, WEAR_NAMES};
pub use crate::parts::{BACK_NAMES, SIDE_NAMES, TIP_NAMES, TOP_NAMES};

use crate::{MouthKind, Tech};

/// The face units in decoder pick order.
pub const FACE_KINDS: [Tech; 7] = Tech::ALL;
/// All mouth kinds; `None` is a construction state, never a decoded mouth.
pub const MOUTH_KINDS: [MouthKind; 5] = [
    MouthKind::Line,
    MouthKind::Strip,
    MouthKind::Slot,
    MouthKind::Grid,
    MouthKind::None,
];

const FACE_NAMES: [&str; FACE_KINDS.len()] = {
    let mut names = [""; FACE_KINDS.len()];
    let mut i = 0;
    while i < names.len() {
        names[i] = FACE_KINDS[i].name();
        i += 1;
    }
    names
};
const MOUTH_NAMES: [&str; MOUTH_KINDS.len()] = {
    let mut names = [""; MOUTH_KINDS.len()];
    let mut i = 0;
    while i < names.len() {
        names[i] = MOUTH_KINDS[i].name();
        i += 1;
    }
    names
};

/// A decoder catalogue, including entries that never survive decoding.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Catalogue {
    /// Name of the decoded field or fitting category.
    pub field: &'static str,
    /// The catalogue in decoder order where it is picked by index.
    pub values: &'static [&'static str],
    /// Values that never appear, with the reason for each exclusion.
    pub exclusions: &'static [(&'static str, &'static str)],
}

impl Catalogue {
    /// Whether a name belongs to the catalogue and can appear on a decoded robot.
    pub fn is_reachable(&self, value: &str) -> bool {
        self.values.contains(&value) && !self.exclusions.iter().any(|(name, _)| *name == value)
    }
}

const fn catalogue(field: &'static str, values: &'static [&'static str]) -> Catalogue {
    Catalogue {
        field,
        values,
        exclusions: &[],
    }
}

/// All named robot-part catalogues and globally unreachable values. A pick
/// restricted to some plans, hulls or fittings is still reachable if any
/// decoded robot can carry it.
pub const CATALOGUES: &[Catalogue] = &[
    catalogue("plan", &PLAN_NAMES),
    catalogue("head", &HULL_NAMES),
    catalogue("chassis", &BODY_NAMES),
    catalogue("pelvis", &PELVIS_NAMES),
    catalogue("neck", &NECK_NAMES),
    catalogue("face", &FACE_NAMES),
    Catalogue {
        exclusions: &[("wide", "screen eyes always convert wide to dot")],
        ..catalogue("screen_eye", &SCREEN_EYES)
    },
    Catalogue {
        exclusions: &[(
            "none",
            "construction state; screens use line and the other faces always pick a mouth",
        )],
        ..catalogue("mouth", &MOUTH_NAMES)
    },
    catalogue("top", &TOP_NAMES),
    catalogue("tip", &TIP_NAMES),
    catalogue("side", &SIDE_NAMES),
    Catalogue {
        exclusions: &[("none", "arm style has zero pick weight")],
        ..catalogue("arms", &ARM_NAMES)
    },
    catalogue("hand", &HAND_NAMES),
    catalogue("gear", &GEAR_NAMES),
    catalogue("foot", &FOOT_NAMES),
    catalogue("chest", &CHEST_NAMES),
    catalogue("decal", &DECAL_NAMES),
    Catalogue {
        exclusions: &[(
            "dipped",
            "the old dipped scheme always leaves the paint plain",
        )],
        ..catalogue("decor", &DECOR_NAMES)
    },
    catalogue("wear", &WEAR_NAMES),
    catalogue("back", &BACK_NAMES),
    catalogue("metal", &METAL_NAMES),
];
