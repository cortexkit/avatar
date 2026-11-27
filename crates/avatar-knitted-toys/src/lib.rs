//! The `creature.knitted-toys` avatar type: crocheted plush creatures in the
//! manner of amigurumi, drawn from a stored 16,384-byte genome.
//!
//! This is a port of a JavaScript design ("the reference" in this crate's
//! comments), which lives in the authors' private experiments repository;
//! `scripts/fidelity/` compares the two drawing by drawing.
//!
//! - [`decode`] reads a toy's traits: bytes 1024 to 1279 for the toy, plus
//!   creature.wild's colour bytes 160 to 163 for its yarn.
//! - [`render_tree`] builds the toy's pieces (head, body) and its sewn-on
//!   parts in toy space, poses it for the mood, fits it into a 200-unit
//!   frame and draws it as a render tree.
//!
//! The small irregularities of the handwork (a stitch pulled a little off
//! its column, the tufts of a pompom) come from fixed hashes of each part's
//! name, not from the genome, so a bit flip never reshuffles them.
//!
//! Generation uses only arithmetic, square roots, rounding and the
//! polynomial sine, cosine and arctangent of `avatar_svg::wild::math`, so it
//! gives the same tree on every platform.

mod build;
mod decode;
mod dna;
mod draw;
mod emit;
mod face;
mod geom;
mod knit;
mod palette;
mod parts;
mod piece;
mod pose;

use avatar_svg::{Mood, RenderTree, Rgb};

pub use decode::{
    decode, Arms, Brows, Cheeks, Colour, Ears, Eyes, FaceGenes, Legs, Mouth, Nose, Patches,
    PieceGenes, Snout, Spoke, Stripes, Tail, Temper, Top, Traits,
};

/// The size a tree draws for when no size is given: what the toy shows at
/// 256 pixels, with every stitch.
pub const DEFAULT_DETAIL_PX: u32 = 256;

/// The frame's edge in SVG user units.
pub const CANVAS: u32 = 200;

/// The colour of the toy's main yarn: creature.wild's body colour for the
/// same genome, dyed less strongly when wild's recipe is vivid.
pub fn main_yarn(traits: &Traits) -> Rgb {
    palette::palette(traits).main.hex
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

/// Draws a genome's toy in one mood.
///
/// `detail_px` is the size in pixels the tree will be shown at, which sets
/// what the drawing keeps: at 100 px and up every stitch, from 56 px the
/// rows as grooves, and below that plain yarn with thicker embroidery.
/// Line widths have a floor of about a screen pixel, and an eye a least
/// size below 40 px. `None` draws what the toy shows at 256 px, and so does
/// `Some(0)`.
pub fn render_tree(genome: &[u8; 16_384], mood: Mood, detail_px: Option<u32>) -> RenderTree {
    let toy = build::build(decode(genome));
    // A drawing 0 pixels wide has no pixel to size lines by: a screen pixel
    // would be infinitely many frame units, which overflows when a length
    // is stored. It is drawn at full detail instead, as `None` is.
    let detail_px = detail_px.filter(|&px| px != 0);
    let px = f64::from(detail_px.unwrap_or(DEFAULT_DETAIL_PX));
    RenderTree {
        canvas: CANVAS,
        background: None,
        pixel_grid: None,
        primitives: draw::draw(&toy, mood_index(mood), px),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_detail_size_of_0_draws_what_no_size_draws() {
        let genome = [7_u8; 16_384];
        for mood in [
            Mood::Neutral,
            Mood::Working,
            Mood::Happy,
            Mood::Waiting,
            Mood::Blocked,
            Mood::Sad,
            Mood::Sleepy,
        ] {
            assert_eq!(
                render_tree(&genome, mood, Some(0)),
                render_tree(&genome, mood, None),
                "{mood:?}"
            );
        }
    }
}
