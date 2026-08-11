//! Existing SVG entry points, implemented as render-tree build and serialization wrappers.

use avatar_phenotype::{Phenotype, Q32};

use crate::build::{build_body_render_tree, build_render_tree};
use crate::serialize::serialize_svg;

/// The v1 viewBox edge in canvas units.
pub(crate) const CANVAS_UNITS: u32 = 10_000;

/// Render-time inputs that are not part of identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderParams {
    pub mood: Mood,
    /// Fetus→adult morph factor. v1 renders only the adult shape.
    pub growth: Q32,
    /// Output viewBox edge in canvas units. v1 pins 10_000.
    pub canvas: u32,
}

/// The closed mood vocabulary from design note 001.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Neutral,
    Working,
    Happy,
    Waiting,
    Blocked,
    Sad,
    Sleepy,
}

/// Why a render was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The growth morph is not specified yet.
    UnsupportedGrowth(Q32),
    /// v1 serializes only the pinned 10_000-unit canvas.
    UnsupportedCanvas(u32),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedGrowth(growth) => write!(
                formatter,
                "growth morph is unimplemented; v1 renders growth = 1 only (got {})",
                growth.0
            ),
            Self::UnsupportedCanvas(canvas) => write!(
                formatter,
                "v1 renders the pinned {CANVAS_UNITS}-unit canvas only (got {canvas})"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

/// Renders the complete body and mood-reactive face.
pub fn render_svg(phenotype: &Phenotype, params: &RenderParams) -> Result<Vec<u8>, RenderError> {
    build_render_tree(phenotype, params).map(|tree| serialize_svg(&tree))
}

/// Renders only the background and body for faceless or tiny-size variants.
pub fn render_body_svg(
    phenotype: &Phenotype,
    params: &RenderParams,
) -> Result<Vec<u8>, RenderError> {
    build_body_render_tree(phenotype, params).map(|tree| serialize_svg(&tree))
}
