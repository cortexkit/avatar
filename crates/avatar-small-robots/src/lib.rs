//! The `creature.small-robots` avatar type.
//!
//! Every genome becomes a little machine: a painted hull that carries the
//! face, often a small chassis under it, running gear, arms that end in a
//! tool, and the antennae, bolts, panels, rivets and lights a small working
//! machine has. The hull takes the body colour the genome's `creature.wild`
//! has. Each mood poses the same machine again.
//!
//! This began as a port of a JavaScript design ("the reference" in this
//! crate's comments), which lives in the authors' private experiments
//! repository. The port read the same genes, built the same parts and drew
//! the same shapes. Version 1 was then refined in Rust after the port, so it
//! no longer matches the reference exactly: a few of its rules were changed
//! so that no genome draws a part that reads as clothing or a household
//! object, or a face that does not read. The fidelity harness in
//! `scripts/fidelity/` compares drawings with the reference, so it describes
//! the port, not the current drawing.
//!
//! Generation uses only arithmetic, `sqrt`, `floor` and rounding, with sine
//! and cosine from `avatar_svg::wild::math`, so every platform draws the
//! same tree.
//!
//! # Layout and layers
//!
//! [`layout::Layout::new`] reads the fitted parts of a decoded [`Creature`]
//! without applying the 240-unit picture fit. The layout gives rest geometry
//! and the resolved mood poses in robot units, with negative y pointing up.
//! [`Creature::face_layer`], [`Creature::stalk_eye_layer`],
//! [`Creature::head_marks`] and [`Creature::chassis_marks`] draw separate,
//! transparent trees. Each layer gives the origin of its square canvas in
//! the original drawing frame, so it can be projected back without scaling.
//!
//! ```
//! use avatar_small_robots::{decode, render_tree, Mood, GENOME_LEN};
//!
//! let genome = [7_u8; GENOME_LEN];
//! let robot = decode(&genome);
//! assert_eq!(robot.plan, "pod");
//! let tree = render_tree(&genome, Mood::Happy, None);
//! assert_eq!(tree.canvas, 240);
//! assert!(!tree.primitives.is_empty());
//! ```

mod arms;
mod body;
mod draw;
mod face;
mod face_draw;
mod gear;
mod geom;
mod hull;
mod kit;
pub mod layout;
mod marks;
mod mood;
mod parts;
mod robot;
#[cfg(test)]
mod tests;

pub use avatar_svg::{Mood, RenderTree, Rgb};
pub use face::{MouthKind, Tech};
pub use mood::{mood_name, MOODS};

/// The length of the stored genome this type reads: the same 16,384 bytes
/// `creature.wild` reads.
pub const GENOME_LEN: usize = 16_384;

/// The size, in pixels, whose detail [`render_tree`] draws when it is not
/// told a size.
pub const FULL_DETAIL_PX: u32 = 256;

/// What a genome builds: the robot's discrete traits, named as the
/// reference names them, and everything needed to draw it.
#[derive(Clone, Debug)]
pub struct Creature {
    /// The assembly plan: `pod`, `chibi`, `neck` or `stack`.
    pub plan: &'static str,
    /// The shape of the head's hull.
    pub hull: &'static str,
    /// The shape of the chassis, for every plan but a pod.
    pub chassis: Option<&'static str>,
    /// How the head is joined to the chassis, for every plan but a pod.
    pub neck: Option<&'static str>,
    /// The face unit. A screen the head has no room for becomes two lenses.
    pub face: Tech,
    /// For a screen: whether it is a pale panel, and the shape of its eyes.
    pub screen: Option<(bool, &'static str)>,
    /// What carries the mouth.
    pub mouth: MouthKind,
    /// What stands on top of the head.
    pub top: &'static str,
    /// What is bolted to the sides of the head.
    pub side: &'static str,
    /// How the arms are built.
    pub arms: &'static str,
    /// The tool on the right arm, and on the left one.
    pub hand: &'static str,
    pub hand_left: &'static str,
    /// The running gear.
    pub gear: &'static str,
    /// What is set into the chassis, or into a pod's hull.
    pub chest: &'static str,
    /// What trails behind: `none`, `cable` or `pipe`.
    pub back: &'static str,
    /// The metal of the limbs and fittings.
    pub metal: &'static str,
    /// Which of creature.wild's palette recipes the body colour comes from:
    /// 0 pastel, 2 vivid, 3 dark, 4 muted.
    pub palette_recipe: u8,
    /// The body colour, exactly as creature.wild resolves it.
    pub body: Rgb,
    robot: robot::Robot,
}

/// Builds the robot a stored genome describes.
pub fn decode(genome: &[u8; GENOME_LEN]) -> Creature {
    let c = robot::creature(genome);
    Creature {
        plan: c.plan,
        hull: c.head.kind,
        chassis: c.body_kind,
        neck: c.neck.as_ref().map(|n| n.kind),
        face: c.f.tech,
        screen: c.f.screen.as_ref().map(|_| (c.f.lcd, c.f.eye_style)),
        mouth: c.f.mouth.kind,
        top: c.top,
        side: c.side,
        arms: c.arms.style,
        hand: c.arms.hand,
        hand_left: c.arms.hand_l,
        gear: c.gear.kind,
        chest: c.chest.kind,
        back: c.back,
        metal: kit::METAL_NAMES[c.p.metal_kind],
        palette_recipe: c.p.recipe,
        body: c.p.body,
        robot: c,
    }
}

impl Creature {
    /// Draws the robot in `mood`. `detail_px` is the width in pixels the
    /// tree will be drawn at: smaller drawings leave out fine detail and
    /// keep lines about a pixel wide, as the reference does. `None` draws
    /// the detail of a [`FULL_DETAIL_PX`] drawing.
    pub fn render_tree(&self, mood: Mood, detail_px: Option<u32>) -> RenderTree {
        // The reference treats a size of 0 as its own 240-unit frame.
        let size = match detail_px {
            None => FULL_DETAIL_PX,
            Some(0) => 240,
            Some(n) => n,
        };
        robot::render(&self.robot, mood, f64::from(size))
    }
}

/// Draws the robot a stored genome describes in `mood`, at the detail of a
/// drawing `detail_px` pixels wide (see [`Creature::render_tree`]).
pub fn render_tree(genome: &[u8; GENOME_LEN], mood: Mood, detail_px: Option<u32>) -> RenderTree {
    decode(genome).render_tree(mood, detail_px)
}
