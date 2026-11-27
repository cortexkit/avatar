//! The part-mapping table: what every distinct value of every 2D catalogue
//! becomes in 3D.
//!
//! Each entry pairs a catalogue field and value
//! ([`avatar_small_robots::layout::CATALOGUES`]) with one [`Kind`]. Repeated
//! catalogue names, such as the two `none` decals, have one entry. Fields
//! named `component` are not catalogues: they are the members assemblies are
//! made of.

/// The [`crate::mesh`] solid a drawable entry is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Solid {
    /// An outline extruded with bevelled edges.
    Extrusion,
    /// A half outline turned on a lathe.
    Lathe,
    /// A tube along a path, with round ends.
    Tube,
    /// Capsule-shaped rods.
    Rod,
    /// A sphere or an ellipsoid.
    Sphere,
    /// A cylinder.
    Cylinder,
    /// A torus.
    Torus,
    /// A belt round a stadium.
    Belt,
    /// A box with rounded edges.
    Block,
    /// Several of the solids above in one mesh.
    Compound,
}

/// How a drawable entry follows the skeleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Binding {
    /// On one bone.
    Rigid,
    /// Skinned over several bones.
    Skinned,
}

/// Which texture a surface contribution is drawn into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Texture {
    /// The head's marks texture.
    HeadMarks,
    /// The chassis's marks texture.
    ChassisMarks,
    /// The face textures (and stalk-eye textures) of every mood.
    Face,
}

/// What one catalogue value becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// A part with geometry: one solid on a bone, with a finish.
    Drawable {
        /// The solid it is built from.
        solid: Solid,
        /// How it follows the skeleton.
        binding: Binding,
        /// One of [`super::FINISHES`]; `metal` means the robot's metal
        /// finish.
        finish: &'static str,
    },
    /// Several parts placed by the layout. Each member names the role of its
    /// parts: a `component` entry, or a hull field (`head`, `chassis`,
    /// `pelvis`) whose own entry builds it.
    Assembly {
        /// The members' roles.
        members: &'static [&'static str],
    },
    /// Painted onto a hull or the face: no geometry of its own.
    Surface {
        /// The texture it appears in.
        texture: Texture,
    },
    /// A material choice: the finish a metal kind selects.
    Finish {
        /// One of [`super::FINISHES`].
        finish: &'static str,
    },
    /// Nothing: no geometry and no texture.
    Absence,
}

/// One row of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Entry {
    /// The catalogue field, or `component`.
    pub field: &'static str,
    /// The value.
    pub value: &'static str,
    /// What it becomes.
    pub kind: Kind,
}

const fn drawable(
    field: &'static str,
    value: &'static str,
    solid: Solid,
    finish: &'static str,
) -> Entry {
    Entry {
        field,
        value,
        kind: Kind::Drawable {
            solid,
            binding: Binding::Rigid,
            finish,
        },
    }
}

const fn skinned(
    field: &'static str,
    value: &'static str,
    solid: Solid,
    finish: &'static str,
) -> Entry {
    Entry {
        field,
        value,
        kind: Kind::Drawable {
            solid,
            binding: Binding::Skinned,
            finish,
        },
    }
}

const fn assembly(
    field: &'static str,
    value: &'static str,
    members: &'static [&'static str],
) -> Entry {
    Entry {
        field,
        value,
        kind: Kind::Assembly { members },
    }
}

const fn surface(field: &'static str, value: &'static str, texture: Texture) -> Entry {
    Entry {
        field,
        value,
        kind: Kind::Surface { texture },
    }
}

const fn absence(field: &'static str, value: &'static str) -> Entry {
    Entry {
        field,
        value,
        kind: Kind::Absence,
    }
}

const fn finish(field: &'static str, value: &'static str, finish: &'static str) -> Entry {
    Entry {
        field,
        value,
        kind: Kind::Finish { finish },
    }
}

use Solid::*;
use Texture::{Face, HeadMarks};

/// Every entry. Box-like hulls are extruded with bevelled edges and round
/// ones are their outline's half turned on a lathe; fittings are rods,
/// tubes, balls and blocks.
pub const MAPPING: &[Entry] = &[
    // The assembly plan arranges the hulls.
    assembly("plan", "pod", &["head"]),
    assembly("plan", "chibi", &["head", "chassis"]),
    assembly("plan", "neck", &["head", "chassis"]),
    assembly("plan", "stack", &["head", "chassis", "waist", "pelvis"]),
    // Head hulls.
    drawable("head", "box", Extrusion, "paint"),
    drawable("head", "soft", Extrusion, "paint"),
    drawable("head", "capsule", Lathe, "paint"),
    drawable("head", "dome", Lathe, "paint"),
    drawable("head", "cup", Extrusion, "paint"),
    drawable("head", "bucket", Extrusion, "paint"),
    drawable("head", "pyramid", Extrusion, "paint"),
    drawable("head", "octagon", Extrusion, "paint"),
    drawable("head", "hexagon", Extrusion, "paint"),
    drawable("head", "ball", Lathe, "paint"),
    drawable("head", "egg", Lathe, "paint"),
    drawable("head", "acorn", Lathe, "paint"),
    drawable("head", "tee", Extrusion, "paint"),
    drawable("head", "shield", Extrusion, "paint"),
    drawable("head", "can", Lathe, "paint"),
    // Chassis hulls.
    drawable("chassis", "box", Extrusion, "paint"),
    drawable("chassis", "barrel", Lathe, "paint"),
    drawable("chassis", "vee", Extrusion, "paint"),
    drawable("chassis", "bell", Lathe, "paint"),
    drawable("chassis", "ball", Lathe, "paint"),
    drawable("chassis", "can", Lathe, "paint"),
    drawable("chassis", "octagon", Extrusion, "paint"),
    drawable("chassis", "capsule", Lathe, "paint"),
    // The lower segment of a stack.
    drawable("pelvis", "box", Extrusion, "paint"),
    drawable("pelvis", "soft", Extrusion, "paint"),
    drawable("pelvis", "ball", Lathe, "paint"),
    drawable("pelvis", "bucket", Lathe, "paint"),
    // Necks; a flush neck has no size.
    absence("neck", "flush"),
    drawable("neck", "collar", Cylinder, "metal"),
    drawable("neck", "piston", Compound, "metal"),
    skinned("neck", "bellows", Lathe, "metal"),
    drawable("neck", "ball", Sphere, "metal"),
    skinned("neck", "spring", Tube, "metal"),
    // Face units are drawn into the face textures, except stalks, whose
    // eyes are parts carrying their own textures.
    surface("face", "screen", Face),
    surface("face", "lens", Face),
    surface("face", "lamp", Face),
    surface("face", "mono", Face),
    surface("face", "tri", Face),
    surface("face", "odd", Face),
    assembly("face", "stalk", &["stalk", "stalk-eye"]),
    surface("screen_eye", "dot", Face),
    surface("screen_eye", "pill", Face),
    surface("screen_eye", "square", Face),
    surface("screen_eye", "wide", Face),
    surface("screen_eye", "ring", Face),
    surface("screen_eye", "pupil", Face),
    surface("mouth", "line", Face),
    surface("mouth", "strip", Face),
    surface("mouth", "slot", Face),
    surface("mouth", "grid", Face),
    absence("mouth", "none"),
    // What stands on the head.
    absence("top", "none"),
    drawable("top", "rod", Rod, "metal"),
    assembly("top", "twin", &["mast"]),
    drawable("top", "zigzag", Rod, "metal"),
    drawable("top", "coil", Tube, "metal"),
    assembly("top", "loop", &["mast", "loop"]),
    assembly("top", "dish", &["mast", "dish", "lamp"]),
    assembly("top", "bulb", &["mast", "bulb"]),
    assembly("top", "beacon", &["mast", "beacon"]),
    assembly("top", "triple", &["mast"]),
    assembly("top", "ears", &["mast"]),
    drawable("tip", "ball", Sphere, "glow"),
    drawable("tip", "fork", Rod, "metal"),
    drawable("tip", "arrow", Lathe, "glow"),
    absence("tip", "bare"),
    // Side fittings; vents are painted on the head.
    absence("side", "none"),
    drawable("side", "bolts", Block, "metal"),
    assembly("side", "rods", &["side", "side-cap"]),
    drawable("side", "cones", Lathe, "metal"),
    surface("side", "vents", HeadMarks),
    drawable("side", "knobs", Block, "metal"),
    drawable("side", "fins", Block, "metal"),
    // Arms: jointed ones are an upper and a lower segment.
    absence("arms", "none"),
    assembly("arms", "rod", &["arm-upper", "arm-lower", "elbow"]),
    assembly("arms", "shell", &["arm-upper", "arm-lower", "elbow"]),
    skinned("arms", "hose", Tube, "metal"),
    assembly("arms", "telescope", &["arm-upper", "arm-lower"]),
    drawable("arms", "stub", Rod, "metal"),
    // Hands; jawed hands have their jaws on the grip bone.
    assembly("hand", "clamp", &["hand", "jaw"]),
    assembly("hand", "pincer", &["hand", "jaw"]),
    assembly("hand", "claw", &["hand", "jaw"]),
    drawable("hand", "mitt", Block, "metal"),
    drawable("hand", "ball", Sphere, "metal"),
    assembly("hand", "magnet", &["hand", "hand-tip"]),
    assembly("hand", "sucker", &["hand", "hand-cup"]),
    drawable("hand", "fork", Rod, "metal"),
    // Running gear.
    assembly("gear", "biped", &["leg", "foot"]),
    assembly("gear", "piston", &["leg", "foot"]),
    assembly("gear", "spring", &["leg", "foot"]),
    assembly("gear", "quad", &["leg", "foot"]),
    assembly("gear", "hexa", &["leg", "foot"]),
    assembly("gear", "tripod", &["leg", "foot"]),
    assembly("gear", "treads", &["tread"]),
    assembly("gear", "track", &["track", "wheel"]),
    assembly("gear", "wheels", &["wheel"]),
    assembly("gear", "discs", &["wheel"]),
    assembly("gear", "mono", &["fork", "wheel"]),
    assembly("gear", "ball", &["cup", "wheel"]),
    assembly("gear", "jet", &["nozzle", "flame"]),
    assembly("gear", "rings", &["nozzle", "ring"]),
    assembly("gear", "pogo", &["leg", "foot"]),
    assembly("gear", "hoses", &["leg", "foot"]),
    drawable("foot", "pad", Block, "metal"),
    drawable("foot", "dome", Lathe, "paint"),
    drawable("foot", "block", Block, "paint"),
    drawable("foot", "toes", Compound, "metal"),
    drawable("foot", "ball", Sphere, "metal"),
    drawable("foot", "roller", Cylinder, "rubber"),
    drawable("foot", "sucker", Lathe, "rubber"),
    // What is set into the chassis, or a pod's hull.
    absence("chest", "none"),
    assembly("chest", "light", &["chest-rim", "lamp"]),
    drawable("chest", "panel", Block, "paint"),
    drawable("chest", "vent", Block, "paint"),
    assembly("chest", "gauge", &["dial", "needle"]),
    assembly("chest", "leds", &["led"]),
    drawable("chest", "hatch", Block, "paint"),
    drawable("chest", "stencil", Block, "paint"),
    drawable("chest", "hazard", Block, "paint"),
    drawable("chest", "grille", Block, "paint"),
    // Head marks.
    absence("decal", "none"),
    surface("decal", "number", HeadMarks),
    surface("decal", "star", HeadMarks),
    surface("decal", "bars", HeadMarks),
    surface("decal", "arrow", HeadMarks),
    surface("decal", "bolt", HeadMarks),
    surface("decal", "delta", HeadMarks),
    absence("decor", "plain"),
    surface("decor", "stripe", HeadMarks),
    absence("decor", "dipped"),
    absence("wear", "none"),
    surface("wear", "dent", HeadMarks),
    surface("wear", "scratch", HeadMarks),
    // What trails behind.
    absence("back", "none"),
    assembly("back", "cable", &["cable", "plug"]),
    drawable("back", "pipe", Compound, "metal"),
    // Metal kinds select a finish.
    finish("metal", "steel", "metal"),
    finish("metal", "gunmetal", "metal"),
    finish("metal", "brass", "metal"),
    finish("metal", "copper", "metal"),
    finish("metal", "enamel", "paint"),
    // The members assemblies are made of. The hulls a plan arranges are
    // built by their own fields' entries.
    drawable("component", "waist", Cylinder, "metal"),
    drawable("component", "stalk", Rod, "metal"),
    drawable("component", "stalk-eye", Extrusion, "metal"),
    drawable("component", "mast", Rod, "metal"),
    drawable("component", "loop", Torus, "metal"),
    drawable("component", "dish", Lathe, "metal"),
    drawable("component", "lamp", Sphere, "glow"),
    drawable("component", "bulb", Sphere, "glow"),
    drawable("component", "beacon", Lathe, "glow"),
    drawable("component", "side", Rod, "metal"),
    drawable("component", "side-cap", Sphere, "vinyl"),
    drawable("component", "arm-upper", Rod, "metal"),
    drawable("component", "arm-lower", Rod, "metal"),
    drawable("component", "elbow", Sphere, "metal"),
    drawable("component", "hand", Compound, "metal"),
    drawable("component", "jaw", Rod, "metal"),
    drawable("component", "hand-tip", Rod, "metal"),
    drawable("component", "hand-cup", Lathe, "rubber"),
    drawable("component", "leg", Tube, "metal"),
    drawable("component", "foot", Block, "metal"),
    drawable("component", "tread", Belt, "rubber"),
    drawable("component", "track", Belt, "rubber"),
    drawable("component", "wheel", Cylinder, "rubber"),
    drawable("component", "fork", Rod, "metal"),
    drawable("component", "cup", Lathe, "metal"),
    drawable("component", "nozzle", Lathe, "metal"),
    drawable("component", "flame", Lathe, "glow"),
    drawable("component", "ring", Torus, "glow"),
    drawable("component", "chest-rim", Cylinder, "metal"),
    drawable("component", "dial", Cylinder, "paint"),
    drawable("component", "needle", Rod, "paint"),
    drawable("component", "led", Sphere, "glow"),
    drawable("component", "cable", Tube, "rubber"),
    drawable("component", "plug", Block, "metal"),
];

/// The entry for a field and value.
pub fn entry(field: &str, value: &str) -> Option<&'static Entry> {
    MAPPING
        .iter()
        .find(|e| e.field == field && e.value == value)
}
