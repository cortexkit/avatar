//! Byte-stable SVG projection of the geometry-primary render tree.
//!
//! The serializer performs representation-only work such as collapsing the
//! repeated body path into `<use>` elements. It never consults a phenotype or
//! re-derives geometry resolved by the tree builder.

use avatar_phenotype::{Bam16, Rgb, Q32};

use crate::fixed::{bam_degrees_q32, div_i64, q32_mul, CENTER, Q32_ONE};
use crate::format::{format_milli, format_q32_decimal};
use crate::tree::{Effect, Paint, Primitive, Segment, Shape, TransformOp};
use crate::voronoi::VORONOI_PATTERNS;

/// Serializes a render tree to the corpus-pinned SVG byte representation.
pub fn serialize_svg(tree: &crate::tree::RenderTree) -> Vec<u8> {
    let body: Vec<&Primitive> = tree
        .primitives
        .iter()
        .filter(|primitive| primitive.label.starts_with("av-body-"))
        .collect();
    let Some(base) = body.first().copied() else {
        return Vec::new();
    };
    let Some(root) = base.transform.as_ref() else {
        return Vec::new();
    };
    let root_ops = &root.ops;
    let root_values = root_placement(root_ops);

    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">\n",
        tree.canvas, tree.canvas
    );
    if let Some(background) = tree.background {
        output.push_str(&format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
            tree.canvas,
            tree.canvas,
            hex(background)
        ));
    }
    output.push_str(&format!(
        "<g transform=\"translate({},{}) scale({}, {})\">\n",
        format_milli(root_values.tx),
        format_milli(root_values.ty),
        format_q32_decimal(root_values.sx),
        format_q32_decimal(root_values.sy),
    ));
    output.push_str("<defs>\n");

    if let Some(pattern) = body
        .iter()
        .copied()
        .find(|primitive| primitive.label == "av-body-pattern")
    {
        if let Some(mask) = masked_primitives(&pattern.effects) {
            output.push_str(&format!(
                "  <mask id=\"av-pattern\">{}</mask>\n",
                serialize_mask(mask, root_ops)
            ));
        }
    }
    if let Some(gradient) = body
        .iter()
        .copied()
        .find(|primitive| primitive.label == "av-body-gradient")
    {
        if let Paint::RadialGradient { stops } = &gradient.paint {
            if stops.len() >= 2 {
                output.push_str("  <radialGradient id=\"av-gradox\" cx=\"50%\" cy=\"50%\" r=\"50%\" fx=\"50%\" fy=\"50%\" spreadMethod=\"pad\">\n");
                output.push_str(&format!(
                    "    <stop offset=\"89%\" stop-color=\"{}\"/><stop offset=\"100%\" stop-color=\"{}\"/>\n",
                    hex(stops[0].color),
                    hex(stops[1].color),
                ));
                output.push_str("  </radialGradient>\n");
            }
        }
    }

    let light_y = body
        .iter()
        .flat_map(|primitive| primitive.effects.iter())
        .find_map(|effect| match effect {
            Effect::Lit { light_y } => Some(*light_y),
            _ => None,
        })
        .unwrap_or(CENTER);
    output.push_str("  <filter id=\"av-light\">\n");
    output.push_str(
        "    <feDiffuseLighting in=\"SourceGraphic\" result=\"light\" lighting-color=\"white\">\n",
    );
    output.push_str(&format!(
        "      <fePointLight x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
        format_milli(CENTER),
        format_milli(light_y),
        format_milli(div_i64(crate::fixed::CANVAS_MILLI, 21)),
    ));
    output.push_str("    </feDiffuseLighting>\n");
    output.push_str("    <feComposite in=\"SourceGraphic\" in2=\"light\" operator=\"arithmetic\" k1=\"0.9\" k2=\"1.1\" k3=\"0\" k4=\"0\"/>\n");
    output.push_str("  </filter>\n");
    output.push_str(&format!(
        "  <path id=\"av-bp\" d=\"{}\"/>\n",
        path_data(&base.shape, &base.label)
    ));
    output.push_str("</defs>\n");
    output.push_str(&serialize_body(&body));

    let face: Vec<&Primitive> = tree
        .primitives
        .iter()
        .filter(|primitive| !primitive.label.starts_with("av-body-"))
        .collect();
    if !face.is_empty() {
        output.push_str(&serialize_face(&face, root_ops));
    }
    output.push_str("</g>\n</svg>\n");
    output.into_bytes()
}

fn serialize_body(body: &[&Primitive]) -> String {
    if body.len() < 3 {
        return String::new();
    }
    let Some(base_stroke) = body[0].stroke else {
        return String::new();
    };
    let Paint::Solid { color: base_fill } = body[0].paint else {
        return String::new();
    };
    let Paint::Solid { color: lit_fill } = body[1].paint else {
        return String::new();
    };
    let mut output = format!(
        "<use href=\"#av-bp\" stroke-width=\"{}\" stroke=\"{}\" fill=\"{}\"/>\n",
        format_milli(base_stroke.width),
        hex(base_stroke.color),
        hex(base_fill),
    );
    output.push_str("<g filter=\"url(#av-light)\">\n");
    output.push_str(&format!(
        "  <use href=\"#av-bp\" fill=\"{}\"/>\n",
        hex(lit_fill)
    ));
    output.push_str("  <use href=\"#av-bp\" fill=\"url(#av-gradox)\"/>\n");
    if let Some(pattern) = body.get(3) {
        if let Paint::Solid { color } = pattern.paint {
            output.push_str(&format!(
                "  <use href=\"#av-bp\" fill=\"{}\" mask=\"url(#av-pattern)\"/>\n",
                hex(color)
            ));
        }
    }
    output.push_str("</g>\n");
    output
}

fn serialize_mask(primitives: &[Primitive], root_ops: &[TransformOp]) -> String {
    let Some(primitive) = primitives.first() else {
        return String::new();
    };
    let local = primitive
        .transform
        .as_ref()
        .map(|transform| transform.ops.get(root_ops.len()..).unwrap_or_default())
        .unwrap_or_default();
    let mut tx = 0;
    let mut ty = 0;
    let mut sx = Q32_ONE;
    let mut sy = Q32_ONE;
    let mut angle = Bam16(0);
    let mut cx = CENTER;
    let mut cy = CENTER;
    for operation in local {
        match *operation {
            TransformOp::Translate { tx: x, ty: y } => {
                tx = x;
                ty = y;
            }
            TransformOp::Scale { sx: x, sy: y } => {
                sx = x.0;
                sy = y.0;
            }
            TransformOp::Rotate {
                angle: value,
                cx: x,
                cy: y,
            } => {
                angle = value;
                cx = x;
                cy = y;
            }
        }
    }
    let degrees = if primitive.label.starts_with("av-pattern-voronoi-") {
        div_i64(i64::from(angle.0) * 360, 65_536).to_string()
    } else if primitive.label == "av-pattern-cut-flipped" {
        "180".to_owned()
    } else {
        "0".to_owned()
    };
    format!(
        "<g><g transform=\"translate({}, {})\"><g transform=\"scale({} {}) rotate({} {} {})\"><path d=\"{}\" fill=\"{}\"/></g></g></g>",
        format_milli(tx),
        format_milli(ty),
        format_q32_decimal(sx),
        format_q32_decimal(sy),
        degrees,
        format_milli(cx),
        format_milli(cy),
        path_data(&primitive.shape, &primitive.label),
        paint_value(&primitive.paint),
    )
}

fn serialize_face(primitives: &[&Primitive], root_ops: &[TransformOp]) -> String {
    let mut output = String::from("<g id=\"av-face\">\n");
    let eyeballs: Vec<&Primitive> = primitives
        .iter()
        .copied()
        .filter(|primitive| primitive.label == "av-eyeball")
        .collect();
    if eyeballs.len() != 2 {
        output.push_str("</g>\n");
        return output;
    }
    let Some(shade) = primitives
        .iter()
        .copied()
        .find(|primitive| primitive.label == "av-iris-shade")
    else {
        output.push_str("</g>\n");
        return output;
    };
    let Some((seed, freq_h, freq_v, octaves, scale)) = turbulence_effect(&shade.effects) else {
        output.push_str("</g>\n");
        return output;
    };

    output.push_str("  <defs>\n");
    output.push_str(&format!(
        "    <clipPath id=\"av-eye-left-clip\"><path d=\"{}\"/></clipPath>\n",
        path_data(&eyeballs[0].shape, "av-eye-clip")
    ));
    output.push_str(&format!(
        "    <clipPath id=\"av-eye-right-clip\"><path d=\"{}\"/></clipPath>\n",
        path_data(&eyeballs[1].shape, "av-eye-clip")
    ));
    output.push_str(&format!(
        "    <filter id=\"av-eye-noise\"><feTurbulence type=\"fractalNoise\" stitchTiles=\"stitch\" baseFrequency=\"{} {}\" numOctaves=\"{}\" seed=\"{}\" result=\"warp\"/><feDisplacementMap xChannelSelector=\"R\" yChannelSelector=\"G\" scale=\"{}\" in=\"SourceGraphic\" in2=\"warp\"/></filter>\n",
        format_q32_decimal(freq_h.0),
        format_q32_decimal(freq_v.0),
        octaves,
        seed,
        format_milli(q32_mul(100, scale.0)),
    ));
    output.push_str("  </defs>\n");

    if let Some(mouth) = primitives
        .iter()
        .copied()
        .find(|primitive| primitive.label.starts_with("av-mouth-"))
    {
        output.push_str(&serialize_mouth(mouth));
    }
    output.push_str(&serialize_eye(EyeSide::Left, primitives, root_ops));
    output.push_str(&serialize_eye(EyeSide::Right, primitives, root_ops));
    for brow in primitives
        .iter()
        .copied()
        .filter(|primitive| primitive.label.starts_with("av-brow-"))
    {
        output.push_str(&serialize_brow(brow, root_ops));
    }
    output.push_str("</g>\n");
    output
}

fn serialize_mouth(primitive: &Primitive) -> String {
    let family = primitive.label.rsplit('-').next().unwrap_or("0");
    let Some(stroke) = primitive.stroke else {
        return String::new();
    };
    format!(
        "  <g id=\"av-mouth\" data-family=\"{}\"><path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/></g>\n",
        family,
        path_data(&primitive.shape, &primitive.label),
        paint_value(&primitive.paint),
        hex(stroke.color),
        format_milli(stroke.width),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EyeSide {
    Left,
    Right,
}

fn serialize_eye(side: EyeSide, primitives: &[&Primitive], root_ops: &[TransformOp]) -> String {
    let eyeballs: Vec<&Primitive> = primitives
        .iter()
        .copied()
        .filter(|primitive| primitive.label == "av-eyeball")
        .collect();
    let eye_index = if side == EyeSide::Left { 0 } else { 1 };
    let Some(first) = eyeballs.get(eye_index).copied() else {
        return String::new();
    };
    let eye: Vec<&Primitive> = primitives
        .iter()
        .copied()
        .filter(|primitive| {
            primitive.transform == first.transform
                && matches!(
                    primitive.label.as_str(),
                    "av-eyeball"
                        | "av-iris"
                        | "av-iris-shade"
                        | "av-pupil"
                        | "av-highlight"
                        | "av-upper-lid"
                        | "av-lower-lid"
                        | "av-lash"
                )
        })
        .collect();
    let local = first
        .transform
        .as_ref()
        .and_then(|transform| transform.ops.get(root_ops.len()..))
        .unwrap_or_default();
    let Some(TransformOp::Rotate { angle, cx, cy }) = local.last().copied() else {
        return String::new();
    };
    let side_name = if side == EyeSide::Left {
        "left"
    } else {
        "right"
    };
    let mut output = format!(
        "  <g id=\"av-eye-{}\" transform=\"rotate({},{},{})\">\n",
        side_name,
        format_q32_decimal(bam_degrees_q32(angle)),
        format_milli(cx),
        format_milli(cy),
    );
    if let Some(eyeball) = eye
        .iter()
        .copied()
        .find(|primitive| primitive.label == "av-eyeball")
    {
        if let Some(stroke) = eyeball.stroke {
            output.push_str(&format!(
                "    <path class=\"av-eyeball\" d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                path_data(&eyeball.shape, &eyeball.label),
                paint_value(&eyeball.paint),
                hex(stroke.color),
                format_milli(stroke.width),
            ));
        }
    }
    output.push_str(&format!(
        "    <g clip-path=\"url(#av-eye-{}-clip)\">\n",
        side_name
    ));
    for primitive in &eye {
        match (&*primitive.label, &primitive.shape) {
            ("av-iris", Shape::Circle { cx, cy, r }) => {
                if let Some(stroke) = primitive.stroke {
                    output.push_str(&format!(
                        "      <circle class=\"av-iris\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                        format_milli(*cx),
                        format_milli(*cy),
                        format_milli(*r),
                        paint_value(&primitive.paint),
                        hex(stroke.color),
                        format_milli(stroke.width),
                    ));
                }
            }
            ("av-iris-shade", Shape::Ellipse { cx, cy, rx, ry }) => {
                output.push_str(&format!(
                    "      <ellipse class=\"av-iris-shade\" cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\" opacity=\"{}\" filter=\"url(#av-eye-noise)\"/>\n",
                    format_milli(*cx),
                    format_milli(*cy),
                    format_milli(*rx),
                    format_milli(*ry),
                    paint_value(&primitive.paint),
                    format_q32_decimal(primitive.opacity.unwrap_or(Q32::ONE).0),
                ));
            }
            ("av-pupil", Shape::Circle { cx, cy, r }) => {
                output.push_str(&format!(
                    "      <circle class=\"av-pupil\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>\n",
                    format_milli(*cx),
                    format_milli(*cy),
                    format_milli(*r),
                    paint_value(&primitive.paint),
                ));
            }
            ("av-highlight", Shape::Circle { cx, cy, r }) => {
                output.push_str(&format!(
                    "      <circle class=\"av-highlight\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" opacity=\"{}\"/>\n",
                    format_milli(*cx),
                    format_milli(*cy),
                    format_milli(*r),
                    paint_value(&primitive.paint),
                    format_q32_decimal(primitive.opacity.unwrap_or(Q32::ONE).0),
                ));
            }
            _ => {}
        }
    }
    output.push_str("    </g>\n");
    for primitive in &eye {
        match primitive.label.as_str() {
            "av-upper-lid" | "av-lower-lid" => {
                let Some(stroke) = primitive.stroke else {
                    continue;
                };
                output.push_str(&format!(
                    "    <path class=\"{}\" d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                    primitive.label,
                    path_data(&primitive.shape, &primitive.label),
                    paint_value(&primitive.paint),
                    hex(stroke.color),
                    format_milli(stroke.width),
                ));
            }
            "av-lash" => {
                let Some(stroke) = primitive.stroke else {
                    continue;
                };
                output.push_str(&format!(
                    "    <path class=\"av-lash\" d=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" fill=\"{}\"/>\n",
                    path_data(&primitive.shape, &primitive.label),
                    hex(stroke.color),
                    format_milli(stroke.width),
                    paint_value(&primitive.paint),
                ));
            }
            _ => {}
        }
    }
    output.push_str("  </g>\n");
    output
}

fn serialize_brow(primitive: &Primitive, root_ops: &[TransformOp]) -> String {
    let side = if primitive.label.ends_with("left") {
        "left"
    } else {
        "right"
    };
    let local = primitive
        .transform
        .as_ref()
        .and_then(|transform| transform.ops.get(root_ops.len()..))
        .unwrap_or_default();
    let mut tx = 0;
    let mut ty = 0;
    let mut angle = Bam16(0);
    let mut cx = 0;
    let mut cy = 0;
    for operation in local {
        match *operation {
            TransformOp::Translate { tx: x, ty: y } => {
                tx = x;
                ty = y;
            }
            TransformOp::Rotate {
                angle: value,
                cx: x,
                cy: y,
            } => {
                angle = value;
                cx = x;
                cy = y;
            }
            TransformOp::Scale { .. } => {}
        }
    }
    format!(
        "  <path id=\"av-brow-{}\" d=\"{}\" transform=\"translate({},{}) rotate({},{},{})\" fill=\"{}\"/>\n",
        side,
        path_data(&primitive.shape, &primitive.label),
        format_milli(tx),
        format_milli(ty),
        format_q32_decimal(bam_degrees_q32(angle)),
        format_milli(cx),
        format_milli(cy),
        paint_value(&primitive.paint),
    )
}

fn path_data(shape: &Shape, label: &str) -> String {
    if let Some(index) = label
        .strip_prefix("av-pattern-voronoi-")
        .and_then(|index| index.parse::<usize>().ok())
    {
        return VORONOI_PATTERNS[index].to_owned();
    }
    let Shape::Path {
        segments, closed, ..
    } = shape
    else {
        return String::new();
    };
    if matches!(label, "av-pattern-cut" | "av-pattern-cut-flipped") {
        let [Segment::MoveTo { y: bottom, .. }, Segment::CubicTo {
            cp1x: a,
            cp1y: b,
            cp2x: c,
            cp2y: d,
            x: e,
            y: f,
        }, Segment::CubicTo {
            cp1x: g,
            cp1y: h,
            cp2x: i,
            cp2y: j,
            x: k,
            y: l,
        }] = segments.as_slice()
        else {
            return String::new();
        };
        return format!(
            "M 0, {} C {} {}, {} {}, {} {}C {} {}, {} {}, {} {}Z",
            format_milli(*bottom),
            format_milli(*a),
            format_milli(*b),
            format_milli(*c),
            format_milli(*d),
            format_milli(*e),
            format_milli(*f),
            format_milli(*g),
            format_milli(*h),
            format_milli(*i),
            format_milli(*j),
            format_milli(*k),
            format_milli(*l),
        );
    }

    let compact = label.starts_with("av-body-")
        || label.starts_with("av-brow-")
        || label == "av-pattern-circler";
    let lash = label == "av-lash";
    let mut output = String::new();
    for segment in segments {
        match *segment {
            Segment::MoveTo { x, y } => {
                if compact {
                    output.push_str(&format!("M{},{}", format_milli(x), format_milli(y)));
                } else {
                    output.push_str(&format!("M{} {}", format_milli(x), format_milli(y)));
                }
            }
            Segment::LineTo { x, y } => {
                if compact {
                    output.push_str(&format!("L{},{}", format_milli(x), format_milli(y)));
                } else {
                    output.push_str(&format!(" L{} {}", format_milli(x), format_milli(y)));
                }
            }
            Segment::CubicTo {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            } => {
                if compact {
                    output.push_str(&format!(
                        "C{} {}, {} {}, {} {}",
                        format_milli(cp1x),
                        format_milli(cp1y),
                        format_milli(cp2x),
                        format_milli(cp2y),
                        format_milli(x),
                        format_milli(y),
                    ));
                } else if lash {
                    output.push_str(&format!(
                        " C{} {}, {} {}, {} {}",
                        format_milli(cp1x),
                        format_milli(cp1y),
                        format_milli(cp2x),
                        format_milli(cp2y),
                        format_milli(x),
                        format_milli(y),
                    ));
                } else {
                    output.push_str(&format!(
                        " C{} {},{} {},{} {}",
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
    }
    if *closed {
        output.push_str(" Z");
    }
    output
}

fn masked_primitives(effects: &[Effect]) -> Option<&[Primitive]> {
    effects.iter().find_map(|effect| match effect {
        Effect::Masked { mask } => Some(mask.as_slice()),
        _ => None,
    })
}

fn turbulence_effect(effects: &[Effect]) -> Option<(u8, Q32, Q32, u8, Q32)> {
    effects.iter().find_map(|effect| match effect {
        Effect::Turbulence {
            seed,
            freq_h,
            freq_v,
            octaves,
            scale,
        } => Some((*seed, *freq_h, *freq_v, *octaves, *scale)),
        _ => None,
    })
}

struct RootPlacement {
    tx: i64,
    ty: i64,
    sx: i64,
    sy: i64,
}

fn root_placement(ops: &[TransformOp]) -> RootPlacement {
    let mut values = RootPlacement {
        tx: 0,
        ty: 0,
        sx: Q32_ONE,
        sy: Q32_ONE,
    };
    for operation in ops {
        match *operation {
            TransformOp::Translate { tx, ty } => {
                values.tx = tx;
                values.ty = ty;
            }
            TransformOp::Scale { sx, sy } => {
                values.sx = sx.0;
                values.sy = sy.0;
            }
            TransformOp::Rotate { .. } => {}
        }
    }
    values
}

fn paint_value(paint: &Paint) -> String {
    match paint {
        Paint::Solid { color } => hex(*color),
        Paint::RadialGradient { .. } => "url(#av-gradox)".to_owned(),
        Paint::None => "none".to_owned(),
    }
}

fn hex(color: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}
