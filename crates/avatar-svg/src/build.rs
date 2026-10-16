//! Assembly of the geometry-primary render contract.
//!
//! Geometry is resolved here once; consumers receive painter-order primitives
//! rather than rebuilding the SVG scene graph. In particular, design note 006
//! requires derived values such as the body light position to ride the tree.

use avatar_phenotype::{Phenotype, Q32};
use avatar_tables::div_round_half_away;

use crate::fixed::{div_i64, q32_mul, CANVAS_MILLI, Q32_ONE};
use crate::geometry::{body_geometry, BodyGeometry};
use crate::pattern::pattern_mask_primitives;
use crate::svg::{RenderError, RenderParams, CANVAS_UNITS};
use crate::tree::{
    Effect, GradientStop, Paint, Primitive, RenderTree, Stroke, Transform, TransformOp,
};
use crate::{brows, eyes, face, mood, mouth};

/// Builds the primary, geometry-first avatar representation in painter's order.
pub fn build_render_tree(
    phenotype: &Phenotype,
    params: &RenderParams,
) -> Result<RenderTree, RenderError> {
    build_tree(phenotype, params, true)
}

pub(crate) fn build_body_render_tree(
    phenotype: &Phenotype,
    params: &RenderParams,
) -> Result<RenderTree, RenderError> {
    build_tree(phenotype, params, false)
}

fn build_tree(
    phenotype: &Phenotype,
    params: &RenderParams,
    include_face: bool,
) -> Result<RenderTree, RenderError> {
    validate(params)?;
    let geometry = body_geometry(phenotype);
    let placement = body_placement(&geometry);
    let root_ops = vec![
        TransformOp::Translate {
            tx: placement.tx,
            ty: placement.ty,
        },
        TransformOp::Scale {
            sx: Q32(placement.scale),
            sy: Q32(placement.scale),
        },
    ];
    let root_transform = Transform {
        ops: root_ops.clone(),
    };
    let body_shape = geometry.shape.clone();
    let body_stroke = Stroke::new(phenotype.palette.dark, body_stroke_width());
    let light_y = geometry.top + div_i64(geometry.body_height, 2);

    let mut primitives = vec![
        Primitive {
            shape: body_shape.clone(),
            transform: Some(root_transform.clone()),
            paint: Paint::Solid {
                color: phenotype.palette.primary,
            },
            stroke: Some(body_stroke),
            opacity: None,
            clip: None,
            effects: Vec::new(),
            label: "av-body-base".to_owned(),
            role: None,
        },
        Primitive {
            shape: body_shape.clone(),
            transform: Some(root_transform.clone()),
            paint: Paint::Solid {
                color: phenotype.palette.primary,
            },
            stroke: None,
            opacity: None,
            clip: None,
            effects: vec![Effect::Lit { light_y }],
            label: "av-body-lit-primary".to_owned(),
            role: None,
        },
        Primitive {
            shape: body_shape.clone(),
            transform: Some(root_transform.clone()),
            paint: Paint::RadialGradient {
                stops: vec![
                    GradientStop {
                        offset_q32: Q32(3_822_520_893),
                        color: phenotype.palette.primary,
                    },
                    GradientStop {
                        offset_q32: Q32::ONE,
                        color: phenotype.palette.primary_darkish,
                    },
                ],
            },
            stroke: None,
            opacity: None,
            clip: None,
            effects: vec![Effect::Lit { light_y }],
            label: "av-body-gradient".to_owned(),
            role: None,
        },
    ];

    if let Some(mask) = pattern_mask_primitives(
        &phenotype.pattern,
        geometry.body_width,
        geometry.body_height,
        geometry.top,
        &root_ops,
    ) {
        primitives.push(Primitive {
            shape: body_shape,
            transform: Some(root_transform),
            paint: Paint::Solid {
                color: phenotype.palette.harmonic3,
            },
            stroke: None,
            opacity: None,
            clip: None,
            effects: vec![Effect::Lit { light_y }, Effect::Masked { mask }],
            label: "av-body-pattern".to_owned(),
            role: None,
        });
    }

    if include_face {
        primitives.extend(face_primitives(phenotype, &geometry, params, &root_ops));
    }

    Ok(RenderTree {
        canvas: params.canvas,
        background: Some(phenotype.palette.back),
        primitives,
    })
}

fn face_primitives(
    phenotype: &Phenotype,
    body: &BodyGeometry,
    params: &RenderParams,
    root_ops: &[TransformOp],
) -> Vec<Primitive> {
    let scaled_mood = mood::deltas(params.mood).scaled(&phenotype.mood_response);
    let dimensions = eyes::preliminary_dimensions(&phenotype.face, body.body_width);
    let rect = face::resolve_face_rect(body, &phenotype.face, dimensions.upper + dimensions.lower);
    let eye_pair = eyes::EyePair::new(&phenotype.face, rect, dimensions, scaled_mood);
    let mouth_rect = face::render_mouth_rect(body, rect, &eye_pair, &phenotype.face);
    let root_transform = Transform {
        ops: root_ops.to_vec(),
    };

    let mut result = vec![mouth::mouth_primitive(
        &phenotype.face.mouth,
        mouth_rect,
        scaled_mood,
        &phenotype.palette,
        root_transform,
    )];
    result.extend(eyes::eye_primitives(
        eye_pair.left,
        &phenotype.face,
        &phenotype.palette,
        root_ops,
    ));
    result.extend(eyes::eye_primitives(
        eye_pair.right,
        &phenotype.face,
        &phenotype.palette,
        root_ops,
    ));
    result.extend(brows::brow_primitives(
        &phenotype.face,
        rect,
        &eye_pair,
        scaled_mood,
        &phenotype.palette,
        root_ops,
    ));
    result
}

fn validate(params: &RenderParams) -> Result<(), RenderError> {
    if params.growth != Q32::ONE {
        return Err(RenderError::UnsupportedGrowth(params.growth));
    }
    if params.canvas != CANVAS_UNITS {
        return Err(RenderError::UnsupportedCanvas(params.canvas));
    }
    Ok(())
}

/// Stroke width: canvas/255 in milliunits, one pinned division.
pub(crate) fn body_stroke_width() -> i64 {
    div_round_half_away(i128::from(CANVAS_MILLI), 255) as i64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BodyPlacement {
    tx: i64,
    ty: i64,
    scale: i64,
}

fn body_placement(geometry: &BodyGeometry) -> BodyPlacement {
    let stroke_margin = 8 * body_stroke_width();
    let limit = CANVAS_MILLI - stroke_margin;
    if geometry.body_height > limit || geometry.body_width > limit {
        let dimension = geometry.body_width.max(geometry.body_height);
        let scale = div_round_half_away(
            i128::from(limit) * i128::from(Q32_ONE),
            i128::from(dimension),
        ) as i64;
        let scaled_width = q32_mul(geometry.body_width, scale);
        let scaled_height = q32_mul(geometry.body_height, scale);
        BodyPlacement {
            tx: div_i64(geometry.body_width - scaled_width, 2),
            ty: div_i64(CANVAS_MILLI - scaled_height, 2) - geometry.top,
            scale,
        }
    } else {
        BodyPlacement {
            tx: 0,
            ty: div_i64(CANVAS_MILLI - geometry.body_height, 2) - geometry.top,
            scale: Q32_ONE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{FillRule, Shape};

    fn geometry(body_width: i64, body_height: i64) -> BodyGeometry {
        BodyGeometry {
            shape: Shape::Path {
                segments: Vec::new(),
                closed: false,
                fill_rule: FillRule::NonZero,
            },
            body_width,
            body_height,
            top: 200_000,
            bottom: 200_000 + body_height,
            max_width_y: 0,
            min_width_y: 0,
            all_points: Vec::new(),
        }
    }

    #[test]
    fn body_stroke_width_is_the_pinned_division() {
        assert_eq!(body_stroke_width(), 3922);
    }

    #[test]
    fn placement_is_identity_for_fitting_bodies() {
        let placement = body_placement(&geometry(500_000, 600_000));
        assert_eq!(placement.scale, Q32_ONE);
        assert_eq!(placement.tx, 0);
        assert_eq!(placement.ty, 0);
    }

    #[test]
    fn placement_scales_overflowing_bodies_down() {
        let geometry = geometry(1_200_000, 600_000);
        let placement = body_placement(&geometry);
        assert!(placement.scale < Q32_ONE);
        let limit = CANVAS_MILLI - 8 * 3922;
        let scaled_width = q32_mul(geometry.body_width, placement.scale);
        assert!((scaled_width - limit).abs() <= 1);
        let scaled_height = q32_mul(geometry.body_height, placement.scale);
        assert_eq!(placement.tx, div_i64(geometry.body_width - scaled_width, 2));
        assert_eq!(
            placement.ty,
            div_i64(CANVAS_MILLI - scaled_height, 2) - geometry.top
        );
    }
}
