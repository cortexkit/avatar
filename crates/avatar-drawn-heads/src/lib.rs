//! The `creature.drawn-heads` avatar type: a flat, front-facing head drawn in
//! ink, with ears, a neck, eyes with lids, brows, a nose, a mouth, hair,
//! often facial hair, and the lines and spots a face collects with age.
//!
//! This is a port of a JavaScript design ("the reference" in this crate's
//! comments), which lives in the authors' private experiments repository;
//! `scripts/fidelity/` compares the two drawing by drawing. Every gene is read
//! where the reference reads it, every value is computed in the reference's
//! operation order, and the render tree holds the same elements in the same
//! painter's order, written to the same precision as the reference's SVG. Generation uses only `+ - * /`, `sqrt`, rounding
//! and the shared sine polynomial in [`avatar_svg::wild::math`], so it gives
//! the same tree on every platform.
//!
//! The head reads the same stored 16,384-byte genome as `creature.wild`. Its
//! body colour is the wild body colour (bytes 160 to 162); every other gene
//! is read from byte 1024 on. The reference draws no ground shadow, so no
//! primitive carries [`avatar_svg::Role::GroundShadow`].

mod beard;
mod creature;
mod face;
mod genes;
mod geom;
mod hair;
mod head;
mod mood;
mod palette;
mod render;

use avatar_svg::{Mood, RenderTree};

pub use avatar_svg::wild::GENOME_LEN;
pub use beard::{Beard, BeardKind};
pub use creature::{decode, Creature, Fit};
pub use face::{Brows, Eyes, Mouth, Nose};
pub use hair::{Hair, HairStyle};
pub use head::{Ears, FaceLayout, Head, Neck};
pub use mood::{mood_name, Marks, Temper, MOODS};
pub use palette::Palette;

/// The type name a stored row carries.
pub const CREATURE_TYPE: &str = "creature.drawn-heads";

/// Draws one genome in one mood on the 240-unit canvas with no background.
///
/// `detail_px` is the size in pixels the image will be shown at. Lines are
/// kept about a pixel wide, so they are heavier in frame units at small
/// sizes, and fine detail is left out below 96 px and 56 px. `None` draws
/// what the reference draws at 256 px.
pub fn render_tree(genome: &[u8; GENOME_LEN], mood: Mood, detail_px: Option<u32>) -> RenderTree {
    render::render(&decode(genome), mood, detail_px)
}

impl Creature {
    /// Draws this head in one mood; see [`render_tree`].
    pub fn render(&self, mood: Mood, detail_px: Option<u32>) -> RenderTree {
        render::render(self, mood, detail_px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avatar_svg::{serialize_tree, Role};

    /// Genomes that reach many branches: a few fixed byte patterns and a
    /// simple deterministic sequence.
    fn genomes() -> Vec<[u8; GENOME_LEN]> {
        let mut out = Vec::new();
        for fill in [0u8, 0x55, 0xaa, 0xff] {
            out.push([fill; GENOME_LEN]);
        }
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..24 {
            let mut g = [0u8; GENOME_LEN];
            for b in g.iter_mut() {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                *b = (state >> 56) as u8;
            }
            out.push(g);
        }
        out
    }

    #[test]
    fn the_reference_draws_no_ground_shadow_so_no_primitive_has_that_role() {
        for genome in genomes() {
            for mood in MOODS {
                let tree = render_tree(&genome, mood, None);
                assert!(!tree.primitives.is_empty());
                assert!(tree
                    .primitives
                    .iter()
                    .all(|p| p.role != Some(Role::GroundShadow)));
            }
        }
    }

    #[test]
    fn no_detail_size_draws_what_256_px_draws() {
        for genome in genomes().iter().take(6) {
            for mood in MOODS {
                assert_eq!(
                    render_tree(genome, mood, None),
                    render_tree(genome, mood, Some(256))
                );
            }
        }
    }

    #[test]
    fn small_sizes_leave_detail_out() {
        // Below 96 px strands, age lines and freckles go; below 56 px glints
        // and curls go too. A smaller drawing never has more primitives.
        for genome in genomes() {
            for mood in MOODS {
                let large = render_tree(&genome, mood, Some(256)).primitives.len();
                let middle = render_tree(&genome, mood, Some(96)).primitives.len();
                let small = render_tree(&genome, mood, Some(48)).primitives.len();
                assert!(
                    large >= middle && middle >= small,
                    "{large} {middle} {small}"
                );
            }
        }
    }

    #[test]
    fn every_tree_serializes() {
        for genome in genomes() {
            for mood in MOODS {
                let svg = serialize_tree(&render_tree(&genome, mood, None));
                assert!(svg.starts_with(b"<svg"));
            }
        }
    }
}
