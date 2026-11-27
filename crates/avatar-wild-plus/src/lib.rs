//! The `creature.wild-plus` avatar type.
//!
//! A stored 16,384-byte genome becomes the render tree that draws a
//! `creature.wild-plus` creature: the wild body as the head, a body plan that
//! adds more masses (a torso, segments, lobes, a long neck, tentacles or
//! legs) welded into one mirror-symmetric silhouette, limbs of many kinds, a
//! crest, a tail and feet, a coat on the outline, a hide pattern, a belly, a
//! face and a mood. Bytes 0..255 keep the meaning the wild type gives them;
//! everything new is read from byte 1024 onward.
//!
//! This is a port of a JavaScript design ("the reference" in this crate's
//! comments), which lives in the authors' private experiments repository;
//! `scripts/fidelity/` compares the two drawing by drawing. The port follows
//! the reference's arithmetic in the same order, so its discrete choices
//! match exactly and its drawings match to the pixel tolerance the
//! comparison sets.
//!
//! Masses are composed in "model space", the unit-less ring coordinates of
//! the wild body (x right, y down, the head's ring centre at the origin).
//! The finished creature is fitted into the 240-unit frame of the canvas,
//! and everything after the fit works in frame units.
//!
//! Geometry uses `f64` with only basic arithmetic, `sqrt` and the shared
//! polynomial sine and cosine of `avatar_svg::wild::math`, so every platform
//! draws the same tree.

mod anatomy;
mod body;
mod coat;
mod creature;
mod draw;
mod face;
mod face_draw;
mod genes;
mod geom;
mod mood;
mod palette;
mod parts;
mod render;
mod surface;

use avatar_svg::wild::GENOME_LEN;
use avatar_svg::{Mood, RenderTree, Rgb};

pub use anatomy::{CRESTS, TAILS};
pub use body::PLANS;
pub use coat::COATS;
pub use face::{EYE_PLANS, EYE_STYLES, MOUTH_PLANS};
pub use palette::SCHEMES;
pub use parts::LIMB_KINDS;
pub use surface::SURFACES;

/// The discrete choices one genome decodes to: what the creature is made
/// of, before it is drawn in a mood.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creature {
    /// The body plan, one of [`PLANS`].
    pub plan: &'static str,
    /// The anatomy in the order it was built: one note for the body plan's
    /// masses, the limb kinds (`"2 pairs of horn, antenna"`), a crest
    /// (`"comb crest"`), arms, a tail and feet.
    pub anatomy: Vec<String>,
    /// The coat on the outline, one of [`COATS`].
    pub coat: &'static str,
    /// The hide pattern, one of [`SURFACES`].
    pub surface: &'static str,
    /// The second pattern a pebbled hide carries, one of [`SURFACES`].
    pub surface_second: &'static str,
    /// The patterns drawn, then the belly's kind where it has one.
    pub surface_notes: Vec<String>,
    /// The wild palette recipe: 0 pastel, 2 vivid, 3 dark, 4 muted.
    pub recipe: u32,
    /// The colour scheme index into [`SCHEMES`].
    pub scheme: usize,
    /// Where the eyes are, one of [`EYE_PLANS`].
    pub eye_plan: &'static str,
    /// How the eyes are drawn, one of [`EYE_STYLES`].
    pub eye_style: &'static str,
    /// The mouth plan, one of [`MOUTH_PLANS`].
    pub mouth: &'static str,
    /// The nose: 0 none, 1 dot, 2 triangle, 3 oval, 4 nostrils, 5 ball.
    pub nose: usize,
    /// The resolved body colour.
    pub body: Rgb,
}

/// Decodes a genome into the creature it draws.
pub fn decode(bytes: &[u8; GENOME_LEN]) -> Creature {
    let c = creature::creature(bytes);
    Creature {
        plan: PLANS[c.b.plan],
        anatomy: c.b.notes.clone(),
        coat: COATS[c.coat_kind],
        surface: SURFACES[c.surface.kind],
        surface_second: SURFACES[c.surface.second],
        surface_notes: c.surface.notes.clone(),
        recipe: c.w.recipe,
        scheme: c.p.scheme,
        eye_plan: EYE_PLANS[c.face.eye_plan],
        eye_style: EYE_STYLES[c.face.style],
        mouth: MOUTH_PLANS[c.face.mouth_plan],
        nose: c.face.nose,
        body: c.p.body,
    }
}

/// Draws one genome in one mood on the 240-unit canvas with no background.
///
/// `detail_px` is the size the drawing will be shown at. This type draws the
/// same tree at every size, so it is accepted for a uniform generator
/// signature and otherwise ignored. The ground shadow is the only primitive
/// with `Role::GroundShadow`.
pub fn render_tree(bytes: &[u8; GENOME_LEN], mood: Mood, detail_px: Option<u32>) -> RenderTree {
    let _ = detail_px;
    render::render(&creature::creature(bytes), mood)
}

#[cfg(test)]
mod tests;
