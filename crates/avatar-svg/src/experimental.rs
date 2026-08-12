//! Unstable developer preview for the dormant asymmetry and layer genome regions.
//!
//! `ExperimentalOptions::asymmetry_strength` asks how much right-side control
//! point drift keeps a creature recognizable, while `ExperimentalOptions::layers`
//! asks whether small, offset harmonic body echoes add useful depth. This module
//! is not covered by golden vectors, has no cross-language port, and makes no
//! compatibility promise. Graduating either preview into the stable pipeline
//! requires activating its genome region, minting a generator version,
//! implementing cross-language ports, and extending the conformance corpus.
//!
//! The stable SVG serializer deliberately reproduces the established body
//! emission structure rather than projecting arbitrary render trees. The
//! developer-only projection below exists so the example harness can display
//! experimental primitives without changing that compatibility-sensitive path.

use avatar_phenotype::{Bam16, Phenotype, Rgb, Q32};

use crate::build::{body_stroke_width, build_render_tree};
use crate::fixed::{bam_degrees_q32, div_i64, q32_mul, CENTER, Q32_ONE};
use crate::format::{format_milli, format_q32_decimal};
use crate::geometry::{
    body_geometry, build_spline, flatten, min_width_y, mirror_ring, rescale, BodyGeometry,
};
use crate::tree::{
    Effect, FillRule, Paint, Primitive, RenderTree, Segment, Shape, StrokeCap, StrokeJoin,
    Transform, TransformOp,
};
use crate::{RenderError, RenderParams};

const Q32_LAYER_SCALE_START: i64 = 2_362_232_013; // 0.55
const Q32_LAYER_SCALE_STEP: i64 = 773_094_113; // 0.18
const Q32_LAYER_JITTER: i64 = 429_496_730; // 0.10
const LAYER_ROTATION_LIMIT: i64 = 8_192; // 45 degrees in BAM16

/// Developer-only controls for previews of dormant phenotype regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExperimentalOptions {
    /// Right-side control-point displacement strength; zero preserves the stable tree.
    pub asymmetry_strength: Q32,
    /// Draws one or two small body echoes behind the main body.
    pub layers: bool,
    /// Echo paint scheme: which palette family the layer fills come from.
    pub layer_paint: LayerPaint,
}

/// Candidate paint schemes for layer echoes, compared by eye before any
/// graduation: harmonic fills read as separate objects on busy bodies, so a
/// body-ramp variant tests whether echoes read better as the creature's own
/// depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayerPaint {
    /// harmonic2 / harmonic1 — spike-1 scheme, distinct hues.
    #[default]
    Harmonic,
    /// primary_darker / primary_darkest — the body's own shade ramp.
    BodyRamp,
}

/// Builds a developer-only render tree with optional asymmetry and body layers.
///
/// The off state delegates directly to [`crate::build_render_tree`], making the
/// experiment incapable of drifting the stable tree when both controls are off.
pub fn build_render_tree_experimental(
    phenotype: &Phenotype,
    params: &RenderParams,
    options: &ExperimentalOptions,
) -> Result<RenderTree, RenderError> {
    if options.asymmetry_strength == Q32(0) && !options.layers {
        return build_render_tree(phenotype, params);
    }

    // Reuse the stable builder for every established primitive, then replace the
    // body outline and prepend layer primitives only where the preview differs.
    let mut tree = build_render_tree(phenotype, params)?;
    let geometry = if options.asymmetry_strength == Q32(0) {
        body_geometry(phenotype)
    } else {
        body_geometry_with_asymmetry(phenotype, options.asymmetry_strength)
    };

    if options.asymmetry_strength != Q32(0) {
        for primitive in &mut tree.primitives {
            if primitive.label.starts_with("av-body-") {
                primitive.shape = geometry.shape.clone();
            }
        }
    }

    if options.layers {
        let root_ops = tree
            .primitives
            .iter()
            .find(|primitive| primitive.label == "av-body-base")
            .and_then(|primitive| primitive.transform.as_ref())
            .expect("the stable body base always has a root transform")
            .ops
            .clone();
        let layers = layer_primitives(phenotype, &geometry, &root_ops, options.layer_paint);
        tree.primitives.splice(0..0, layers);
    }

    Ok(tree)
}

/// Produces a readable, deliberately non-canonical SVG projection of any render tree.
///
/// This is only for developer previews. It draws every primitive in painter's
/// order and intentionally skips lighting and turbulence effects; stable output
/// remains the responsibility of [`crate::serialize_svg`].
pub fn serialize_svg_experimental(tree: &RenderTree) -> Vec<u8> {
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">\n",
        tree.canvas, tree.canvas
    );
    if let Some(background) = tree.background {
        output.push_str(&format!(
            "  <rect width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
            tree.canvas,
            tree.canvas,
            hex(background)
        ));
    }

    let definitions = preview_definitions(&tree.primitives);
    if !definitions.is_empty() {
        output.push_str("  <defs>\n");
        output.push_str(&definitions);
        output.push_str("  </defs>\n");
    }
    for (index, primitive) in tree.primitives.iter().enumerate() {
        output.push_str(&preview_primitive(primitive, index, true));
    }
    output.push_str("</svg>\n");
    output.into_bytes()
}

fn body_geometry_with_asymmetry(phenotype: &Phenotype, strength: Q32) -> BodyGeometry {
    let mut ring = mirror_ring(&phenotype.body);
    let right_start = ring.len() / 2;
    let body_size_twenty_fifth = div_i64(phenotype.body.size.0, 25);

    for (index, point) in ring[right_start..].iter_mut().take(8).enumerate() {
        point.x += asymmetry_displacement(
            phenotype.asymmetry.deltas[index],
            strength,
            body_size_twenty_fifth,
        );
        point.y += asymmetry_displacement(
            phenotype.asymmetry.deltas[(index + 3) % 8],
            strength,
            body_size_twenty_fifth,
        );
    }

    let mut spline = build_spline(&ring);
    let mut bounds = flatten(&spline);
    let body_width = rescale(&mut spline, &mut bounds);
    let top = bounds.min_y;
    let bottom = bounds.max_y;
    let body_height = bottom - top;
    BodyGeometry {
        shape: spline.shape(),
        body_width,
        body_height,
        top,
        bottom,
        max_width_y: bounds.max_width_y,
        min_width_y: min_width_y(&bounds.samples, top, body_height),
        all_points: bounds.samples,
    }
}

fn asymmetry_displacement(delta: Q32, strength: Q32, body_size_twenty_fifth: i64) -> i64 {
    let centered = 2 * (delta.0 - Q32_ONE / 2);
    q32_mul(q32_mul(body_size_twenty_fifth, centered), strength.0)
}

fn layer_primitives(
    phenotype: &Phenotype,
    geometry: &BodyGeometry,
    root_ops: &[TransformOp],
    paint: LayerPaint,
) -> Vec<Primitive> {
    let count = 1 + usize::from(phenotype.layers.count % 2);
    let body_center_y = geometry.top + div_i64(geometry.body_height, 2);
    (0..count)
        .map(|index| {
            let scale = layer_scale(phenotype.layers.params[index], index);
            let offset = layer_offset(phenotype.layers.params[index + 4], geometry.body_width);
            let angle = layer_angle(phenotype.layers.params[index + 2]);
            let mut ops = root_ops.to_vec();
            // SVG applies transform-list operations right to left. This sequence
            // scales about the body center, rotates around that center, and then
            // offsets the resulting layer before the stable root placement.
            ops.extend([
                TransformOp::Translate { tx: offset, ty: 0 },
                TransformOp::Rotate {
                    angle,
                    cx: CENTER,
                    cy: body_center_y,
                },
                TransformOp::Translate {
                    tx: CENTER,
                    ty: body_center_y,
                },
                TransformOp::Scale {
                    sx: Q32(scale),
                    sy: Q32(scale),
                },
                TransformOp::Translate {
                    tx: -CENTER,
                    ty: -body_center_y,
                },
            ]);
            Primitive {
                shape: geometry.shape.clone(),
                transform: Some(Transform { ops }),
                paint: Paint::Solid {
                    color: match (paint, index) {
                        (LayerPaint::Harmonic, 0) => phenotype.palette.harmonic2,
                        (LayerPaint::Harmonic, _) => phenotype.palette.harmonic1,
                        (LayerPaint::BodyRamp, 0) => phenotype.palette.primary_darker,
                        (LayerPaint::BodyRamp, _) => phenotype.palette.primary_darkest,
                    },
                },
                stroke: Some(crate::Stroke::new(
                    phenotype.palette.dark,
                    body_stroke_width(),
                )),
                opacity: None,
                clip: None,
                effects: Vec::new(),
                label: format!("av-experimental-layer-{}", index + 1),
            }
        })
        .collect()
}

fn layer_scale(param: Q32, index: usize) -> i64 {
    let base = Q32_LAYER_SCALE_START - Q32_LAYER_SCALE_STEP * index as i64;
    base + q32_mul(Q32_LAYER_JITTER, centered_unit(param))
}

fn layer_offset(param: Q32, body_width: i64) -> i64 {
    q32_mul(div_i64(body_width, 6), centered_unit(param))
}

fn layer_angle(param: Q32) -> Bam16 {
    Bam16(q32_mul(LAYER_ROTATION_LIMIT, centered_unit(param)) as i16 as u16)
}

fn centered_unit(param: Q32) -> i64 {
    2 * param.0 - Q32_ONE
}

fn preview_definitions(primitives: &[Primitive]) -> String {
    let mut output = String::new();
    for (index, primitive) in primitives.iter().enumerate() {
        if let Paint::RadialGradient { stops } = &primitive.paint {
            output.push_str(&format!(
                "    <radialGradient id=\"av-experimental-gradient-{index}\" cx=\"50%\" cy=\"50%\" r=\"50%\">\n"
            ));
            for stop in stops {
                output.push_str(&format!(
                    "      <stop offset=\"{}\" stop-color=\"{}\"/>\n",
                    format_q32_decimal(stop.offset_q32.0),
                    hex(stop.color)
                ));
            }
            output.push_str("    </radialGradient>\n");
        }
        if let Some(clip) = &primitive.clip {
            output.push_str(&format!(
                "    <clipPath id=\"av-experimental-clip-{index}\">{}</clipPath>\n",
                preview_shape(clip)
            ));
        }
        if let Some(mask) = masked_primitives(&primitive.effects) {
            output.push_str(&format!("    <mask id=\"av-experimental-mask-{index}\">\n"));
            for mask_primitive in mask {
                output.push_str(&preview_primitive(mask_primitive, index, false));
            }
            output.push_str("    </mask>\n");
        }
    }
    output
}

fn preview_primitive(primitive: &Primitive, index: usize, include_mask: bool) -> String {
    let mut attributes = format!(" class=\"{}\"", primitive.label);
    if let Some(transform) = &primitive.transform {
        attributes.push_str(&format!(
            " transform=\"{}\"",
            preview_transform(&transform.ops)
        ));
    }
    attributes.push_str(&format!(
        " fill=\"{}\"",
        preview_paint(&primitive.paint, index)
    ));
    if let Some(stroke) = primitive.stroke {
        attributes.push_str(&format!(
            " stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
            hex(stroke.color),
            format_milli(stroke.width),
            stroke_cap(stroke.cap),
            stroke_join(stroke.join),
        ));
    }
    if let Some(opacity) = primitive.opacity {
        attributes.push_str(&format!(" opacity=\"{}\"", format_q32_decimal(opacity.0)));
    }
    if primitive.clip.is_some() {
        attributes.push_str(&format!(
            " clip-path=\"url(#av-experimental-clip-{index})\""
        ));
    }
    if include_mask && masked_primitives(&primitive.effects).is_some() {
        attributes.push_str(&format!(" mask=\"url(#av-experimental-mask-{index})\""));
    }
    format!(
        "  {}\n",
        preview_shape_with_attributes(&primitive.shape, &attributes)
    )
}

fn preview_shape_with_attributes(shape: &Shape, attributes: &str) -> String {
    match shape {
        Shape::Path {
            fill_rule: FillRule::EvenOdd,
            ..
        } => format!(
            "<path d=\"{}\" fill-rule=\"evenodd\"{attributes}/>",
            preview_path(shape)
        ),
        Shape::Path { .. } => format!("<path d=\"{}\"{attributes}/>", preview_path(shape)),
        Shape::Circle { cx, cy, r } => format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{attributes}/>",
            format_milli(*cx),
            format_milli(*cy),
            format_milli(*r),
        ),
        Shape::Ellipse { cx, cy, rx, ry } => format!(
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{attributes}/>",
            format_milli(*cx),
            format_milli(*cy),
            format_milli(*rx),
            format_milli(*ry),
        ),
        Shape::Rect { x, y, w, h } => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{attributes}/>",
            format_milli(*x),
            format_milli(*y),
            format_milli(*w),
            format_milli(*h),
        ),
    }
}

fn preview_shape(shape: &Shape) -> String {
    preview_shape_with_attributes(shape, "")
}

fn preview_path(shape: &Shape) -> String {
    let Shape::Path {
        segments, closed, ..
    } = shape
    else {
        return String::new();
    };
    let mut output = String::new();
    for segment in segments {
        match *segment {
            Segment::MoveTo { x, y } => {
                output.push_str(&format!("M{} {}", format_milli(x), format_milli(y)));
            }
            Segment::LineTo { x, y } => {
                output.push_str(&format!(" L{} {}", format_milli(x), format_milli(y)));
            }
            Segment::CubicTo {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            } => {
                output.push_str(&format!(
                    " C{} {}, {} {}, {} {}",
                    format_milli(cp1x),
                    format_milli(cp1y),
                    format_milli(cp2x),
                    format_milli(cp2y),
                    format_milli(x),
                    format_milli(y),
                ));
            }
        }
    }
    if *closed {
        output.push_str(" Z");
    }
    output
}

fn preview_transform(ops: &[TransformOp]) -> String {
    ops.iter()
        .map(|operation| match *operation {
            TransformOp::Translate { tx, ty } => {
                format!("translate({} {})", format_milli(tx), format_milli(ty))
            }
            TransformOp::Rotate { angle, cx, cy } => format!(
                "rotate({} {} {})",
                format_q32_decimal(bam_degrees_q32(angle)),
                format_milli(cx),
                format_milli(cy),
            ),
            TransformOp::Scale { sx, sy } => format!(
                "scale({} {})",
                format_q32_decimal(sx.0),
                format_q32_decimal(sy.0),
            ),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn preview_paint(paint: &Paint, index: usize) -> String {
    match paint {
        Paint::Solid { color } => hex(*color),
        Paint::RadialGradient { .. } => format!("url(#av-experimental-gradient-{index})"),
        Paint::None => "none".to_owned(),
    }
}

fn masked_primitives(effects: &[Effect]) -> Option<&[Primitive]> {
    effects.iter().find_map(|effect| match effect {
        Effect::Masked { mask } => Some(mask.as_slice()),
        Effect::Lit { .. } | Effect::Turbulence { .. } => None,
    })
}

fn stroke_cap(cap: StrokeCap) -> &'static str {
    match cap {
        StrokeCap::Butt => "butt",
        StrokeCap::Round => "round",
        StrokeCap::Square => "square",
    }
}

fn stroke_join(join: StrokeJoin) -> &'static str {
    match join {
        StrokeJoin::Miter => "miter",
        StrokeJoin::Round => "round",
        StrokeJoin::Bevel => "bevel",
    }
}

fn hex(color: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}
