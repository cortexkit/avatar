//! The `creature.pixel-critters` avatar type.
//!
//! A stored genome becomes a small pixel-art creature on a 24 × 24 grid,
//! like a sprite from a handheld game. The body takes the colour
//! creature.wild gives the same genome (bytes 160 to 162, read exactly as
//! wild reads them); everything else is read from byte 1024 on, bit by bit.
//!
//! This is a port of a JavaScript design ("the reference" in this crate's
//! comments), which lives in the authors' private experiments repository;
//! `scripts/fidelity/` compares the two drawing by drawing.
//!
//! How a creature is made:
//! 1. A ramp of colours is built round the body colour.
//! 2. The body is one or two lobes whose size, proportions and corners the
//!    genome sets, each rebuilt so its runs of cells change length steadily.
//! 3. Anatomy fills slots: ears, horns or antennae on top, arms or wings at
//!    the sides, legs below, a tail, a coat round the edge.
//! 4. The parts are stacked back to front; a cell is outline where it
//!    touches empty space or a part further back, and the rest is shaded from
//!    the upper left.
//! 5. A face (eyes, lids, brows, mouth) is drawn for each mood with a posture.
//! 6. The creature is fitted into a 24 × 24 frame, and its cells are merged
//!    into rectangles, one group per part.
//!
//! The generator uses only arithmetic, square roots, floor and rounding, so
//! every platform draws the same cells. The tree is a pixel grid
//! ([`RenderTree::pixel_grid`]): a rasterizer draws each cell as whole
//! pixels. The creature draws no ground shadow, so no primitive carries
//! [`avatar_svg::Role::GroundShadow`].
//!
//! ```
//! use avatar_pixel_critters::{render_tree, GENOME_LEN};
//! use avatar_svg::Mood;
//!
//! let tree = render_tree(&[7; GENOME_LEN], Mood::Happy);
//! assert_eq!(tree.pixel_grid, Some(24));
//! assert!(!tree.primitives.is_empty());
//! ```

mod body;
mod compose;
mod face;
mod fit;
mod genes;
mod grid;
mod palette;
mod pattern;
mod render;
mod sprite;

use avatar_svg::{Mood, RenderTree};

pub use genes::{
    Belly, BlockedFace, BodyGenes, BrowGene, Cheeks, CoatGenes, CoatKind, EarDetail, EyePlan,
    EyeStyle, FaceGenes, Genes, LegGenes, LegKind, LidGene, Marks, MouthKind, Muzzle, PatternGenes,
    Plan, Quirk, SideGenes, SideKind, TailGenes, TailKind, Temper, TopGenes, TopKind, WingKind,
};
pub use grid::Corner;
pub use palette::Palette;
pub use render::placed_svg;

/// The avatar type this crate draws.
pub const TYPE: &str = "creature.pixel-critters";
/// The only version of the type.
pub const VERSION: u32 = 1;
/// The length of a stored genome.
pub const GENOME_LEN: usize = 16_384;
/// Cells across the finished sprite, and across the tree's pixel grid.
pub const GRID_CELLS: u32 = grid::N as u32;

/// A decoded creature: its genes and its colours.
#[derive(Clone, Debug)]
pub struct Creature {
    pub genes: Genes,
    pub palette: Palette,
    bytes: Vec<u8>,
}

/// How the neutral sprite draws its eyes and mouth, which a small or dark
/// face can change from what the genes ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawnFace {
    /// The eye plan as laid out: one with no room for its eyes is drawn as
    /// `Close`.
    pub eye_plan: EyePlan,
    /// The kind of eye drawn.
    pub eye_kind: EyeStyle,
    /// The kind of mouth drawn.
    pub mouth: MouthKind,
}

/// Reads a genome's genes and colours.
pub fn decode(genome: &[u8; GENOME_LEN]) -> Creature {
    let mut reader = genes::Reader::new(genome);
    let palette = palette::build_palette(&mut reader);
    let genes = genes::read_genes(&mut reader);
    Creature {
        genes,
        palette,
        bytes: genome.to_vec(),
    }
}

impl Creature {
    fn memo(&self) -> sprite::Memo<'_> {
        sprite::Memo {
            genes: &self.genes,
            palette: &self.palette,
            bytes: &self.bytes,
            f0: None,
            l0: None,
            marks0: Vec::new(),
            anger: None,
        }
    }

    /// The creature in one mood as a render tree.
    pub fn render_tree(&self, mood: Mood) -> RenderTree {
        let mut memo = self.memo();
        let fit = fit::fit(&mut memo, Some(mood));
        let sprite = render::finish(&mut memo, &fit, mood);
        render::tree_of(&sprite, &self.palette)
    }

    /// How the neutral sprite draws its eyes and mouth.
    pub fn drawn_face(&self) -> DrawnFace {
        let mut memo = self.memo();
        let fit = fit::fit(&mut memo, Some(Mood::Neutral));
        let sprite = render::finish(&mut memo, &fit, Mood::Neutral);
        let face = &sprite.built.ctx.f;
        DrawnFace {
            eye_plan: face.plan,
            eye_kind: face.kind,
            mouth: face.mouth.kind,
        }
    }
}

/// Draws a stored genome in one mood.
pub fn render_tree(genome: &[u8; GENOME_LEN], mood: Mood) -> RenderTree {
    decode(genome).render_tree(mood)
}
