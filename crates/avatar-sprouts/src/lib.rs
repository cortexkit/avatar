//! The `creature.sprouts` avatar type.
//!
//! A sprout is a small plant creature: a body that is a plant organ (a
//! bulb, a seed, a tuber, a gourd, a cactus column and so on) with a face on
//! it, shoots that grow out of it, and roots it stands on. The body takes
//! the colour `creature.wild` gives the same genome; every other gene is
//! read from byte 1,024 on.
//!
//! Shoots grow from an L-system whose rules the genome writes, and a turtle
//! turns the grown strings into stems, leaves, flowers, fruit, thorns and
//! tendrils. Each mood draws the same grown plant in another posture: it
//! wilts, perks up, opens or closes its flowers, folds its leaves for the
//! night, bristles.
//!
//! This is a port of the approved JavaScript reference. Generation uses
//! `f64` arithmetic, square roots and rounding only, with sine and cosine
//! from `avatar_svg::wild::math`, in the reference's operation order: the
//! discrete choices that depend on geometry come out the same on every
//! platform and as in the reference.
//!
//! [`render_tree`] draws a genome in one mood. It draws the same tree at
//! every output size: the reference changes nothing but the size of its
//! picture.

mod body;
mod creature;
mod face;
mod geometry;
mod growth;
mod limits;
mod maths;
mod mood;
mod organs;
mod palette;
mod reader;
mod render;
mod roots;
mod skin;

use avatar_svg::{Mood, RenderTree, Rgb};

pub use crate::reader::GENOME_LEN;

/// The creature type this crate draws.
pub const TYPE: &str = "creature.sprouts";

/// Draws a genome in one mood, on the 240-unit canvas with no background.
///
/// This is the generator itself, with no type or version routing.
pub fn render_tree(genome: &[u8; GENOME_LEN], mood: Mood) -> RenderTree {
    let creature = creature::creature(genome);
    render::render(&creature, mood_index(mood))
}

fn mood_index(mood: Mood) -> usize {
    match mood {
        Mood::Neutral => 0,
        Mood::Working => 1,
        Mood::Happy => 2,
        Mood::Waiting => 3,
        Mood::Blocked => 4,
        Mood::Sad => 5,
        Mood::Sleepy => 6,
    }
}

/// What the plant's shoots bear, over all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bears {
    pub leaves: bool,
    pub flowers: bool,
    pub fruit: bool,
    pub thorns: bool,
    pub tendrils: bool,
}

/// The discrete traits a genome decodes to, named as the reference names
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprout {
    /// The body kind, one of `bulb`, `seed`, `tuber`, `gourd`, `pumpkin`,
    /// `cone`, `column`, `taproot`, `pod`, `rosette`, `berry` and `bush`.
    pub body: &'static str,
    /// creature.wild's palette recipe: 0 pastel, 2 vivid, 3 dark, 4 muted.
    pub recipe: u8,
    /// The family of greens the leaves are drawn in.
    pub foliage: &'static str,
    /// The body colour, as creature.wild gives it.
    pub body_rgb: Rgb,
    /// The shoots, by place: `apex` (a stem on top that continues the
    /// body), `stalk` (the stalk a fruit hung from), `ear` or `arm` (a
    /// mirrored pair), and `tail`.
    pub shoots: Vec<&'static str>,
    pub bears: Bears,
    /// How many lobes ring a rosette.
    pub lobes: Option<u32>,
    pub leaf_form: Option<&'static str>,
    pub flower_form: Option<&'static str>,
    pub fruit_form: Option<&'static str>,
    pub roots: &'static str,
    pub coat: &'static str,
    pub skin: &'static str,
    /// `two`, `one` or `three` eyes.
    pub eye_plan: &'static str,
    pub eye_style: &'static str,
    pub mouth: &'static str,
}

/// Decodes a genome into its discrete traits.
pub fn decode(genome: &[u8; GENOME_LEN]) -> Sprout {
    let c = creature::creature(genome);
    Sprout {
        body: c.b.kind.name(),
        recipe: c.p.recipe,
        foliage: palette::FOLIAGE[c.p.foliage],
        body_rgb: c.p.body,
        shoots: c.s.iter().map(|s| s.name.name()).collect(),
        bears: Bears {
            leaves: c.has.l,
            flowers: c.has.u,
            fruit: c.has.k,
            thorns: c.has.t,
            tendrils: c.has.c,
        },
        lobes: c.lobes.as_ref().map(|x| x.n),
        leaf_form: c.o.leaf.map(|l| l.form.name),
        flower_form: c.o.flower.map(|f| f.form),
        fruit_form: c.o.fruit.map(|f| f.form),
        roots: roots::ROOT_LAYOUTS[c.rt.layout],
        coat: c.k.kind,
        skin: c.skin.kind,
        eye_plan: c.f.plan,
        eye_style: c.f.style,
        mouth: c.f.mouth,
    }
}

impl Sprout {
    /// Whether one of the shoots grows at `place`.
    pub fn grows(&self, place: &str) -> bool {
        self.shoots.contains(&place)
    }
}
