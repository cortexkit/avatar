//! Generic SVG projection for arbitrary render trees.
//!
//! The classic serializer preserves a historical, corpus-pinned SVG layout for
//! one avatar vocabulary. This module projects the render contract directly so
//! preview tools can use the same geometry without inheriting that layout.

use avatar_phenotype::Q32;

use crate::tree::{
    Effect, FillRule, Paint, Primitive, RenderTree, Segment, Shape, StrokeCap, StrokeJoin,
    TransformOp,
};

/// Serializes every primitive in a render tree without interpreting its label.
///
/// Labels are emitted only as CSS classes. Effects that have no portable SVG
/// equivalent are intentionally ignored here: lighting and turbulence belong to
/// the classic renderer's fixed filter vocabulary rather than to this generic
/// projection.
pub fn serialize_tree(tree: &RenderTree) -> Vec<u8> {
    serialize_tree_with_id_prefix(tree, "")
}

/// Serializes a tree fragment that will share one SVG document with siblings.
///
/// The caller supplies a document-local prefix so definition ids remain unique
/// when several independently built trees are embedded in one review sheet.
pub fn serialize_tree_with_id_prefix(tree: &RenderTree, prefix: &str) -> Vec<u8> {
    let mut defs = String::new();
    let mut content = String::new();

    let mut index = 0;
    while index < tree.primitives.len() {
        let effects = &tree.primitives[index].effects;
        let mut end = index + 1;
        while end < tree.primitives.len()
            && tree.primitives[end].effects.as_slice() == effects.as_slice()
        {
            end += 1;
        }
        let mut members = String::new();
        for (member_index, primitive) in tree.primitives[index..end].iter().enumerate() {
            let primitive_index = index + member_index;
            let key = if prefix.is_empty() {
                format!("primitive-{primitive_index}")
            } else {
                format!("{prefix}-primitive-{primitive_index}")
            };
            members.push_str(&serialize_primitive(
                primitive,
                &key,
                &mut defs,
                false,
                i64::from(tree.canvas),
            ));
            members.push('\n');
        }
        let group_key = if prefix.is_empty() {
            format!("group-{index}")
        } else {
            format!("{prefix}-group-{index}")
        };
        content.push_str(&wrap_effects(
            members,
            effects,
            &group_key,
            &mut defs,
            i64::from(tree.canvas),
        ));
        index = end;
    }

    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">\n",
        tree.canvas, tree.canvas
    );
    if !defs.is_empty() {
        output.push_str("<defs>\n");
        output.push_str(&defs);
        output.push_str("</defs>\n");
    }
    if let Some(background) = tree.background {
        output.push_str(&format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
            tree.canvas,
            tree.canvas,
            hex(background.r, background.g, background.b),
        ));
    }
    output.push_str(&content);
    output.push_str("</svg>\n");
    output.into_bytes()
}

fn serialize_primitive(
    primitive: &Primitive,
    key: &str,
    defs: &mut String,
    include_effects: bool,
    mask_extent: i64,
) -> String {
    let paint = paint_value(&primitive.paint, &format!("gradient-{key}"), defs);
    let mut attributes = format!("fill=\"{paint}\"");
    if let Some(stroke) = primitive.stroke {
        attributes.push_str(&format!(
            " stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
            hex(stroke.color.r, stroke.color.g, stroke.color.b),
            milli(stroke.width),
            stroke_cap(stroke.cap),
            stroke_join(stroke.join),
        ));
    }
    if let Some(opacity) = primitive.opacity {
        attributes.push_str(&format!(" opacity=\"{}\"", q32(opacity)));
    }
    if !primitive.label.is_empty() {
        attributes.push_str(&format!(" class=\"{}\"", xml_escape(&primitive.label)));
    }
    if let Some(transform) = &primitive.transform {
        attributes.push_str(&format!(
            " transform=\"{}\"",
            transform_value(&transform.ops)
        ));
    }

    let mut element = shape_element(&primitive.shape, &attributes);
    if let Some(clip) = &primitive.clip {
        let clip_id = format!("clip-{key}");
        defs.push_str(&format!(
            "<clipPath id=\"{clip_id}\" clipPathUnits=\"userSpaceOnUse\">{}</clipPath>\n",
            shape_element(clip, "")
        ));
        element = format!("<g clip-path=\"url(#{clip_id})\">{element}</g>");
    }

    if include_effects {
        wrap_effects(element, &primitive.effects, key, defs, mask_extent)
    } else {
        element
    }
}

/// Consecutive primitives with the same effects share one SVG group. This
/// preserves their painter order while giving a mask a non-degenerate bounds
/// box when one member is a stroked horizontal or vertical line.
fn wrap_effects(
    mut content: String,
    effects: &[Effect],
    key: &str,
    defs: &mut String,
    mask_extent: i64,
) -> String {
    for (effect_index, effect) in effects.iter().enumerate().rev() {
        if let Effect::Masked { mask } = effect {
            let mask_id = format!("mask-{key}-{effect_index}");
            if is_clip_mask(mask) {
                defs.push_str(&format!(
                    "<clipPath id=\"{mask_id}\" clipPathUnits=\"userSpaceOnUse\">{}</clipPath>\n",
                    clip_shape(&mask[0]),
                ));
                content = format!("<g clip-path=\"url(#{mask_id})\">{content}</g>");
                continue;
            }
            let mut mask_content = String::new();
            for (mask_index, mask_primitive) in mask.iter().enumerate() {
                mask_content.push_str(&serialize_primitive(
                    mask_primitive,
                    &format!("{key}-mask-{effect_index}-{mask_index}"),
                    defs,
                    true,
                    mask_extent,
                ));
            }
            // A stroked line can have a zero-height or zero-width object box.
            // Use a canvas-bounded user-space region so the mask cannot
            // collapse and erase a stroke merely because its fill is empty.
            defs.push_str(&format!(
                "<mask id=\"{mask_id}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" maskUnits=\"userSpaceOnUse\" mask-type=\"luminance\" maskContentUnits=\"userSpaceOnUse\">{mask_content}</mask>\n",
                -mask_extent,
                -mask_extent,
                mask_extent * 3,
                mask_extent * 3,
            ));
            content = format!("<g mask=\"url(#{mask_id})\">{content}</g>");
        }
    }
    content
}

/// A one-shape opaque solid mask is geometric clipping, not luminance work.
/// Emitting a clip path avoids allocating a full off-screen mask surface.
fn is_clip_mask(mask: &[Primitive]) -> bool {
    matches!(
        mask,
        [primitive]
            if matches!(primitive.paint, Paint::Solid { .. })
                && primitive.opacity.is_none()
                && primitive.stroke.is_none()
                && primitive.clip.is_none()
                && primitive.effects.is_empty()
    )
}

fn clip_shape(primitive: &Primitive) -> String {
    let attributes = primitive
        .transform
        .as_ref()
        .map(|transform| format!("transform=\"{}\"", transform_value(&transform.ops)))
        .unwrap_or_default();
    shape_element(&primitive.shape, &attributes)
}

fn paint_value(paint: &Paint, gradient_id: &str, defs: &mut String) -> String {
    match paint {
        Paint::Solid { color } => hex(color.r, color.g, color.b),
        Paint::RadialGradient { stops } => {
            defs.push_str(&format!("<radialGradient id=\"{gradient_id}\">"));
            for stop in stops {
                defs.push_str(&format!(
                    "<stop offset=\"{}\" stop-color=\"{}\"/>",
                    q32(stop.offset_q32),
                    hex(stop.color.r, stop.color.g, stop.color.b),
                ));
            }
            defs.push_str("</radialGradient>\n");
            format!("url(#{gradient_id})")
        }
        Paint::None => "none".to_owned(),
    }
}

fn shape_element(shape: &Shape, attributes: &str) -> String {
    let suffix = if attributes.is_empty() {
        String::new()
    } else {
        format!(" {attributes}")
    };
    match shape {
        Shape::Path {
            segments,
            closed,
            fill_rule,
        } => {
            let fill_rule = match fill_rule {
                FillRule::NonZero => "nonzero",
                FillRule::EvenOdd => "evenodd",
            };
            format!(
                "<path d=\"{}\" fill-rule=\"{fill_rule}\"{suffix}/>",
                path_data(segments, *closed)
            )
        }
        Shape::Circle { cx, cy, r } => format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{suffix}/>",
            milli(*cx),
            milli(*cy),
            milli(*r),
        ),
        Shape::Ellipse { cx, cy, rx, ry } => format!(
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{suffix}/>",
            milli(*cx),
            milli(*cy),
            milli(*rx),
            milli(*ry),
        ),
        Shape::Rect { x, y, w, h } => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{suffix}/>",
            milli(*x),
            milli(*y),
            milli(*w),
            milli(*h),
        ),
    }
}

fn path_data(segments: &[Segment], closed: bool) -> String {
    let mut output = String::new();
    for segment in segments {
        match *segment {
            Segment::MoveTo { x, y } => output.push_str(&format!("M{} {}", milli(x), milli(y))),
            Segment::LineTo { x, y } => output.push_str(&format!(" L{} {}", milli(x), milli(y))),
            Segment::CubicTo {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            } => output.push_str(&format!(
                " C{} {},{} {},{} {}",
                milli(cp1x),
                milli(cp1y),
                milli(cp2x),
                milli(cp2y),
                milli(x),
                milli(y),
            )),
        }
    }
    if closed {
        output.push_str(" Z");
    }
    output
}

fn transform_value(operations: &[TransformOp]) -> String {
    operations
        .iter()
        .map(|operation| match *operation {
            TransformOp::Translate { tx, ty } => format!("translate({} {})", milli(tx), milli(ty)),
            TransformOp::Rotate { angle, cx, cy } => format!(
                "rotate({} {} {})",
                bam_degrees(angle.0),
                milli(cx),
                milli(cy),
            ),
            TransformOp::Scale { sx, sy } => format!("scale({} {})", q32(sx), q32(sy)),
        })
        .collect::<Vec<_>>()
        .join(" ")
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

fn hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn milli(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let value = value.unsigned_abs();
    let whole = value / 100;
    let fraction = value % 100;
    if fraction == 0 {
        format!("{sign}{whole}")
    } else if fraction.is_multiple_of(10) {
        format!("{sign}{whole}.{}", fraction / 10)
    } else {
        format!("{sign}{whole}.{fraction:02}")
    }
}

fn q32(value: Q32) -> String {
    const SCALE: i128 = 1_i128 << 32;
    const DIGITS: i128 = 1_000_000_000;
    let raw = i128::from(value.0);
    let sign = if raw < 0 { "-" } else { "" };
    let absolute = raw.abs();
    let whole = absolute / SCALE;
    let mut fraction = ((absolute % SCALE) * DIGITS + SCALE / 2) / SCALE;
    let mut whole = whole;
    if fraction == DIGITS {
        whole += 1;
        fraction = 0;
    }
    if fraction == 0 {
        format!("{sign}{whole}")
    } else {
        let fraction = format!("{fraction:09}");
        format!("{sign}{whole}.{}", fraction.trim_end_matches('0'))
    }
}

fn bam_degrees(angle: u16) -> String {
    const SCALE: i64 = 1_000_000;
    let scaled = (i64::from(angle) * 360 * SCALE + 32_768) / 65_536;
    let whole = scaled / SCALE;
    let fraction = scaled % SCALE;
    if fraction == 0 {
        whole.to_string()
    } else {
        let fraction = format!("{fraction:06}");
        format!("{whole}.{}", fraction.trim_end_matches('0'))
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use avatar_phenotype::{Rgb, Q32};

    use super::serialize_tree;
    use crate::tree::{
        Effect, FillRule, GradientStop, Paint, Primitive, RenderTree, Segment, Shape, Stroke,
        StrokeCap, StrokeJoin,
    };

    fn primitive(shape: Shape, paint: Paint, label: &str) -> Primitive {
        Primitive {
            shape,
            transform: None,
            paint,
            stroke: None,
            opacity: None,
            clip: None,
            effects: Vec::new(),
            label: label.to_owned(),
        }
    }

    #[test]
    fn serializes_arbitrary_primitives_without_label_vocabulary() {
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        let black = Rgb { r: 0, g: 0, b: 0 };
        let mut path = primitive(
            Shape::Path {
                segments: vec![
                    Segment::MoveTo { x: 100, y: 100 },
                    Segment::LineTo { x: 900, y: 100 },
                    Segment::LineTo { x: 500, y: 900 },
                ],
                closed: true,
                fill_rule: FillRule::EvenOdd,
            },
            Paint::Solid { color: black },
            "independent-label",
        );
        path.stroke = Some(Stroke {
            color: white,
            width: 20,
            cap: StrokeCap::Round,
            join: StrokeJoin::Bevel,
        });
        let mut circle = primitive(
            Shape::Circle {
                cx: 500,
                cy: 500,
                r: 200,
            },
            Paint::RadialGradient {
                stops: vec![
                    GradientStop {
                        offset_q32: Q32(0),
                        color: white,
                    },
                    GradientStop {
                        offset_q32: Q32::ONE,
                        color: black,
                    },
                ],
            },
            "another-label",
        );
        let shared_mask = Effect::Masked {
            mask: vec![primitive(
                Shape::Rect {
                    x: 0,
                    y: 0,
                    w: 1_000,
                    h: 1_000,
                },
                Paint::Solid { color: white },
                "mask-label",
            )],
        };
        path.effects.push(shared_mask.clone());
        circle.effects.push(shared_mask);
        let svg = String::from_utf8(serialize_tree(&RenderTree {
            canvas: 10,
            background: None,
            primitives: vec![path, circle],
        }))
        .unwrap();

        assert!(svg.contains("class=\"independent-label\""));
        assert!(svg.contains("class=\"another-label\""));
        assert!(svg.contains("fill-rule=\"evenodd\""));
        assert!(svg.contains("stroke-linecap=\"round\""));
        assert!(svg.contains("radialGradient id=\"gradient-primitive-1\""));
        assert!(svg.contains("clipPath id=\"mask-group-0-0\""));
        assert_eq!(svg.matches("<mask id=").count(), 0);
        assert!(!svg.contains("av-body"));
    }
}
