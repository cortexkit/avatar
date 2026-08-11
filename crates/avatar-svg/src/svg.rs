//! SVG document assembly for the body pass.
//!
//! Element order, attribute order, whitespace, and ids are pinned: the byte
//! shape is the compatibility surface, not an accident of serialization.
//! Ids use a fixed `av-` prefix with no instance number (TOG's
//! `bodylayer.tsx` interpolated a per-instance id), which keeps the output
//! byte-stable per phenotype; embedders needing DOM-unique ids rewrite or
//! wrap them.

use avatar_phenotype::{Phenotype, Rgb, Q32};
use avatar_tables::div_round_half_away;

use crate::fixed::{q32_mul, CANVAS_MILLI, CENTER, Q32_ONE};
use crate::format::{format_milli, format_q32_decimal};
use crate::geometry::{body_geometry, BodyGeometry};
use crate::pattern::mask_content;

/// The v1 viewBox edge in canvas units. Geometry always runs on the
/// 1_000_000 milliunit canvas; this only fixes the serialized viewBox.
pub(crate) const CANVAS_UNITS: u32 = 10_000;

/// Render-time inputs that are not part of identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderParams {
    pub mood: Mood,
    /// Fetus→adult morph factor. v1 renders only the adult shape; any other
    /// value is rejected.
    pub growth: Q32,
    /// Output viewBox edge in canvas units. v1 pins 10_000.
    pub canvas: u32,
}

/// The closed mood vocabulary (design note 001 §5). The body pass accepts
/// every mood and applies none — the field exists so the render signature is
/// final before the face pass lands.
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
    /// The growth morph is not specified yet. v1 fails closed on any growth
    /// other than 1 rather than silently drawing the wrong shape.
    UnsupportedGrowth(Q32),
    /// v1 serializes the pinned 10_000-unit canvas only; widening the canvas
    /// range comes with a spec change.
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

/// Renders the body SVG (background + body, no face) for a phenotype.
pub fn render_svg(phenotype: &Phenotype, params: &RenderParams) -> Result<Vec<u8>, RenderError> {
    if params.growth != Q32::ONE {
        return Err(RenderError::UnsupportedGrowth(params.growth));
    }
    if params.canvas != CANVAS_UNITS {
        return Err(RenderError::UnsupportedCanvas(params.canvas));
    }
    let geometry = body_geometry(phenotype);
    Ok(document(phenotype, &geometry))
}

fn document(phenotype: &Phenotype, geometry: &BodyGeometry) -> Vec<u8> {
    let palette = &phenotype.palette;
    let placement = placement(geometry);
    let mask = mask_content(
        &phenotype.pattern,
        geometry.body_width,
        geometry.body_height,
        geometry.top,
    );
    let stroke_width = stroke_width();

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {CANVAS_UNITS} {CANVAS_UNITS}\">\n"
    ));
    svg.push_str(&format!(
        "<rect width=\"{CANVAS_UNITS}\" height=\"{CANVAS_UNITS}\" fill=\"{}\"/>\n",
        hex(palette.back)
    ));
    svg.push_str(&format!(
        "<g transform=\"translate({},{}) scale({}, {})\">\n",
        format_milli(placement.tx),
        format_milli(placement.ty),
        format_q32_decimal(placement.scale),
        format_q32_decimal(placement.scale),
    ));
    svg.push_str("<defs>\n");
    if let Some(mask) = &mask {
        svg.push_str(&format!("  <mask id=\"av-pattern\">{mask}</mask>\n"));
    }
    svg.push_str(
        "  <radialGradient id=\"av-gradox\" cx=\"50%\" cy=\"50%\" r=\"50%\" fx=\"50%\" fy=\"50%\" spreadMethod=\"pad\">\n",
    );
    svg.push_str(&format!(
        "    <stop offset=\"89%\" stop-color=\"{}\"/><stop offset=\"100%\" stop-color=\"{}\"/>\n",
        hex(palette.primary),
        hex(palette.primary_darkish),
    ));
    svg.push_str("  </radialGradient>\n");
    svg.push_str("  <filter id=\"av-light\">\n");
    svg.push_str(
        "    <feDiffuseLighting in=\"SourceGraphic\" result=\"light\" lighting-color=\"white\">\n",
    );
    svg.push_str(&format!(
        "      <fePointLight x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
        format_milli(CENTER),
        format_milli(light_y(geometry)),
        format_milli(light_z()),
    ));
    svg.push_str("    </feDiffuseLighting>\n");
    svg.push_str(
        "    <feComposite in=\"SourceGraphic\" in2=\"light\" operator=\"arithmetic\" k1=\"0.9\" k2=\"1.1\" k3=\"0\" k4=\"0\"/>\n",
    );
    svg.push_str("  </filter>\n");
    svg.push_str(&format!("  <path id=\"av-bp\" d=\"{}\"/>\n", geometry.path));
    svg.push_str("</defs>\n");
    svg.push_str(&format!(
        "<use href=\"#av-bp\" stroke-width=\"{}\" stroke=\"{}\" fill=\"{}\"/>\n",
        format_milli(stroke_width),
        hex(palette.dark),
        hex(palette.primary),
    ));
    svg.push_str("<g filter=\"url(#av-light)\">\n");
    svg.push_str(&format!(
        "  <use href=\"#av-bp\" fill=\"{}\"/>\n",
        hex(palette.primary)
    ));
    svg.push_str("  <use href=\"#av-bp\" fill=\"url(#av-gradox)\"/>\n");
    if mask.is_some() {
        svg.push_str(&format!(
            "  <use href=\"#av-bp\" fill=\"{}\" mask=\"url(#av-pattern)\"/>\n",
            hex(palette.harmonic3)
        ));
    }
    svg.push_str("</g>\n</g>\n</svg>\n");
    svg.into_bytes()
}

/// Stroke width: canvas/255 in milliunits, one pinned division.
fn stroke_width() -> i64 {
    div_round_half_away(i128::from(CANVAS_MILLI), 255) as i64
}

/// Point-light position: x at the canvas center, y at the body's vertical
/// midpoint, z at canvas/21 — all pinned divisions.
fn light_y(geometry: &BodyGeometry) -> i64 {
    geometry.top + div_round_half_away(i128::from(geometry.body_height), 2) as i64
}

fn light_z() -> i64 {
    div_round_half_away(i128::from(CANVAS_MILLI), 21) as i64
}

/// The centering transform applied around the body group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Placement {
    tx: i64,
    ty: i64,
    scale: i64,
}

/// Derives the centering transform.
///
/// When the body (plus an 8×stroke-width margin for the outline) overflows
/// the canvas, the larger dimension sets the scale and the body is centered;
/// otherwise scale stays 1 and only the vertical centering translate runs.
fn placement(geometry: &BodyGeometry) -> Placement {
    let stroke_margin = 8 * stroke_width();
    let limit = CANVAS_MILLI - stroke_margin;
    if geometry.body_height > limit || geometry.body_width > limit {
        let dimension = if geometry.body_width > geometry.body_height {
            geometry.body_width
        } else {
            geometry.body_height
        };
        let scale = div_round_half_away(
            i128::from(limit) * i128::from(Q32_ONE),
            i128::from(dimension),
        ) as i64;
        let scaled_width = q32_mul(geometry.body_width, scale);
        let scaled_height = q32_mul(geometry.body_height, scale);
        Placement {
            tx: div_round_half_away(i128::from(geometry.body_width - scaled_width), 2) as i64,
            ty: div_round_half_away(i128::from(CANVAS_MILLI - scaled_height), 2) as i64
                - geometry.top,
            scale,
        }
    } else {
        Placement {
            tx: 0,
            ty: div_round_half_away(i128::from(CANVAS_MILLI - geometry.body_height), 2) as i64
                - geometry.top,
            scale: Q32_ONE,
        }
    }
}

fn hex(color: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroke_width_is_the_pinned_division() {
        // 1_000_000 / 255 = 3921.568… → 3922 half away from zero.
        assert_eq!(stroke_width(), 3922);
    }

    #[test]
    fn light_z_is_the_pinned_division() {
        // 1_000_000 / 21 = 47619.04… → 47619.
        assert_eq!(light_z(), 47_619);
    }

    #[test]
    fn hex_colors_are_lowercase() {
        assert_eq!(
            hex(Rgb {
                r: 0,
                g: 171,
                b: 255
            }),
            "#00abff"
        );
        assert_eq!(
            hex(Rgb {
                r: 255,
                g: 255,
                b: 255
            }),
            "#ffffff"
        );
    }

    #[test]
    fn placement_is_identity_for_fitting_bodies() {
        let geometry = BodyGeometry {
            path: String::new(),
            body_width: 500_000,
            body_height: 600_000,
            top: 200_000,
            bottom: 800_000,
            max_width_y: 0,
            min_width_y: 0,
            all_points: Vec::new(),
        };
        let placement = placement(&geometry);
        assert_eq!(placement.scale, Q32_ONE);
        assert_eq!(placement.tx, 0);
        // ty = (1_000_000 − 600_000)/2 − 200_000 = 0.
        assert_eq!(placement.ty, 0);
    }

    #[test]
    fn placement_scales_overflowing_bodies_down() {
        let geometry = BodyGeometry {
            path: String::new(),
            body_width: 1_200_000,
            body_height: 600_000,
            top: 200_000,
            bottom: 800_000,
            max_width_y: 0,
            min_width_y: 0,
            all_points: Vec::new(),
        };
        let placement = placement(&geometry);
        assert!(placement.scale < Q32_ONE);
        // Width dominates: the scaled width lands on the limit within one
        // milliunit of rounding, and tx centers whatever remains.
        let limit = CANVAS_MILLI - 8 * 3922;
        let scaled_width = q32_mul(1_200_000, placement.scale);
        assert!((scaled_width - limit).abs() <= 1);
        let scaled_height = q32_mul(600_000, placement.scale);
        assert_eq!(
            placement.tx,
            div_round_half_away(i128::from(1_200_000 - scaled_width), 2) as i64
        );
        assert_eq!(
            placement.ty,
            div_round_half_away(i128::from(CANVAS_MILLI - scaled_height), 2) as i64 - 200_000
        );
    }
}
