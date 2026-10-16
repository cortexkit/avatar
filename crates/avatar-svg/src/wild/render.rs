//! RenderTree construction helpers for wild floating-point geometry.
//!
//! Geometry stays in `f64` until these helpers create a tree primitive. The
//! shared milliunit boundary gives native drawing and SVG the same coordinates.

use super::math;
use crate::{
    Bam16, Effect, FillRule, Paint, Primitive, Rgb, Segment, Shape, Stroke, StrokeCap, StrokeJoin,
    Transform, TransformOp, Q32,
};

/// Converts one wild coordinate into the render contract's milliunits.
pub fn mu(value: f64) -> i64 {
    let scaled = value * 100.0;
    if scaled >= 0.0 {
        (scaled + 0.5).floor() as i64
    } else {
        (scaled - 0.5).ceil() as i64
    }
}

pub fn move_to(x: f64, y: f64) -> Segment {
    Segment::MoveTo { x: mu(x), y: mu(y) }
}

pub fn line_to(x: f64, y: f64) -> Segment {
    Segment::LineTo { x: mu(x), y: mu(y) }
}

pub fn cubic_to(cp1: (f64, f64), cp2: (f64, f64), to: (f64, f64)) -> Segment {
    Segment::CubicTo {
        cp1x: mu(cp1.0),
        cp1y: mu(cp1.1),
        cp2x: mu(cp2.0),
        cp2y: mu(cp2.1),
        x: mu(to.0),
        y: mu(to.1),
    }
}

/// Converts an SVG quadratic segment to its exactly equivalent cubic segment.
pub fn quadratic_to(from: (f64, f64), control: (f64, f64), to: (f64, f64)) -> Segment {
    cubic_to(
        (
            from.0 + (control.0 - from.0) * (2.0 / 3.0),
            from.1 + (control.1 - from.1) * (2.0 / 3.0),
        ),
        (
            to.0 + (control.0 - to.0) * (2.0 / 3.0),
            to.1 + (control.1 - to.1) * (2.0 / 3.0),
        ),
        to,
    )
}

/// Approximates an SVG elliptical arc with cubic segments, preserving its SVG
/// endpoint and flags. RenderTree paths are cubic-only, so this conversion is
/// the adapter between the old markup vocabulary and the tree contract.
pub fn arc_to(
    from: (f64, f64),
    mut rx: f64,
    mut ry: f64,
    _angle_deg: f64,
    large_arc: bool,
    sweep: bool,
    to: (f64, f64),
) -> Vec<Segment> {
    rx = rx.abs();
    ry = ry.abs();
    if rx == 0.0 || ry == 0.0 || from == to {
        return vec![line_to(to.0, to.1)];
    }

    // All wild eye arcs have zero rotation; only their sweep varies.
    let (sin_phi, cos_phi) = (0.0, 1.0);
    let dx = (from.0 - to.0) / 2.0;
    let dy = (from.1 - to.1) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }

    let numerator = (rx * rx) * (ry * ry) - (rx * rx) * (y1p * y1p) - (ry * ry) * (x1p * x1p);
    let denominator = (rx * rx) * (y1p * y1p) + (ry * ry) * (x1p * x1p);
    let factor = if denominator == 0.0 {
        0.0
    } else {
        (numerator / denominator).max(0.0).sqrt()
    };
    let factor = if large_arc == sweep { -factor } else { factor };
    let cxp = factor * rx * y1p / ry;
    let cyp = -factor * ry * x1p / rx;
    let center = (
        cos_phi * cxp - sin_phi * cyp + (from.0 + to.0) / 2.0,
        sin_phi * cxp + cos_phi * cyp + (from.1 + to.1) / 2.0,
    );

    let start_vector = ((x1p - cxp) / rx, (y1p - cyp) / ry);
    let end_vector = ((-x1p - cxp) / rx, (-y1p - cyp) / ry);
    let start = math::atan2(start_vector.1, start_vector.0);
    let mut delta = signed_angle(start_vector, end_vector);
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }
    let count = (delta.abs() / (std::f64::consts::FRAC_PI_2)).ceil() as usize;
    let step = delta / count as f64;
    let map = |point: (f64, f64)| -> (f64, f64) {
        (
            center.0 + rx * cos_phi * point.0 - ry * sin_phi * point.1,
            center.1 + rx * sin_phi * point.0 + ry * cos_phi * point.1,
        )
    };

    (0..count)
        .map(|index| {
            let theta = start + step * index as f64;
            let next = theta + step;
            let alpha = (4.0 / 3.0) * math::tan(step / 4.0);
            let (start_sin, start_cos) = (math::sin(theta), math::cos(theta));
            let (end_sin, end_cos) = (math::sin(next), math::cos(next));
            let end = if index + 1 == count {
                to
            } else {
                map((end_cos, end_sin))
            };
            cubic_to(
                map((start_cos - alpha * start_sin, start_sin + alpha * start_cos)),
                map((end_cos + alpha * end_sin, end_sin - alpha * end_cos)),
                end,
            )
        })
        .collect()
}

fn signed_angle(from: (f64, f64), to: (f64, f64)) -> f64 {
    math::atan2(from.0 * to.1 - from.1 * to.0, from.0 * to.0 + from.1 * to.1)
}

pub fn path(segments: Vec<Segment>, closed: bool) -> Shape {
    Shape::Path {
        segments,
        closed,
        fill_rule: FillRule::NonZero,
    }
}

pub fn polygon(points: &[(f64, f64)]) -> Shape {
    let mut segments = Vec::with_capacity(points.len());
    for (index, &(x, y)) in points.iter().enumerate() {
        segments.push(if index == 0 {
            move_to(x, y)
        } else {
            line_to(x, y)
        });
    }
    path(segments, true)
}

/// Builds the rounded, inset Voronoi path that the prior SVG emitter wrote as
/// quadratic commands. Keeping this conversion at the tree boundary leaves
/// surface generation responsible only for polygon coordinates.
pub fn rounded_polygon(points: &[(f64, f64)], inset: f64) -> Shape {
    let count = points.len();
    if count < 3 {
        return path(Vec::new(), false);
    }
    let mut twice_area = 0.0;
    let (mut center_x, mut center_y) = (0.0, 0.0);
    for index in 0..count {
        let current = points[index];
        let next = points[(index + 1) % count];
        let cross = current.0 * next.1 - next.0 * current.1;
        twice_area += cross;
        center_x += (current.0 + next.0) * cross;
        center_y += (current.1 + next.1) * cross;
    }
    let center = if twice_area.abs() < 1e-9 {
        let sum = points
            .iter()
            .fold((0.0, 0.0), |sum, point| (sum.0 + point.0, sum.1 + point.1));
        (sum.0 / count as f64, sum.1 / count as f64)
    } else {
        (center_x / (3.0 * twice_area), center_y / (3.0 * twice_area))
    };
    let points: Vec<(f64, f64)> = points
        .iter()
        .map(|point| {
            (
                center.0 + (point.0 - center.0) * inset,
                center.1 + (point.1 - center.1) * inset,
            )
        })
        .collect();
    let mut segments = Vec::with_capacity(count * 2);
    for index in 0..count {
        let previous = points[(index + count - 1) % count];
        let current = points[index];
        let next = points[(index + 1) % count];
        let start = (
            current.0 + (previous.0 - current.0) * 0.5,
            current.1 + (previous.1 - current.1) * 0.5,
        );
        let end = (
            current.0 + (next.0 - current.0) * 0.5,
            current.1 + (next.1 - current.1) * 0.5,
        );
        if index == 0 {
            segments.push(move_to(start.0, start.1));
        } else {
            segments.push(line_to(start.0, start.1));
        }
        segments.push(quadratic_to(start, current, end));
    }
    path(segments, true)
}

pub fn circle(cx: f64, cy: f64, r: f64) -> Shape {
    Shape::Circle {
        cx: mu(cx),
        cy: mu(cy),
        r: mu(r),
    }
}

pub fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Shape {
    Shape::Ellipse {
        cx: mu(cx),
        cy: mu(cy),
        rx: mu(rx),
        ry: mu(ry),
    }
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Shape {
    Shape::Rect {
        x: mu(x),
        y: mu(y),
        w: mu(w),
        h: mu(h),
    }
}

pub fn filled(
    shape: Shape,
    fill: &str,
    opacity: f64,
    stroke: Option<Stroke>,
    label: &str,
) -> Primitive {
    let (color, color_opacity) = css_color(fill);
    Primitive {
        shape,
        transform: None,
        paint: Paint::Solid { color },
        stroke,
        opacity: opacity_q32(opacity * color_opacity),
        clip: None,
        effects: Vec::new(),
        label: label.to_owned(),
        role: None,
    }
}

pub fn stroked(shape: Shape, stroke: Stroke, label: &str) -> Primitive {
    Primitive {
        shape,
        transform: None,
        paint: Paint::None,
        stroke: Some(stroke),
        opacity: None,
        clip: None,
        effects: Vec::new(),
        label: label.to_owned(),
        role: None,
    }
}

pub fn stroke(color: &str, width: f64, cap: StrokeCap, join: StrokeJoin) -> Stroke {
    let (color, _) = css_color(color);
    Stroke {
        color,
        width: mu(width),
        cap,
        join,
    }
}

pub fn opacity(value: f64) -> Option<Q32> {
    opacity_q32(value)
}

pub fn css_rgb(value: &str) -> Rgb {
    css_color(value).0
}

pub fn css_color(value: &str) -> (Rgb, f64) {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let parse = |range| u8::from_str_radix(&hex[range], 16).expect("valid hex palette color");
        return (
            Rgb {
                r: parse(0..2),
                g: parse(2..4),
                b: parse(4..6),
            },
            1.0,
        );
    }
    if value == "white" {
        return (
            Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            1.0,
        );
    }
    if let Some(args) = value
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let values: Vec<f64> = args
            .split(',')
            .map(|value| value.trim().parse().expect("valid rgba palette color"))
            .collect();
        return (
            Rgb {
                r: values[0].round() as u8,
                g: values[1].round() as u8,
                b: values[2].round() as u8,
            },
            values[3],
        );
    }
    let args = value
        .strip_prefix("hsl(")
        .and_then(|value| value.strip_suffix(')'))
        .expect("tree colors must be CSS hex, rgba, or hsl");
    let tokens: Vec<&str> = args.split_whitespace().collect();
    let hue: f64 = tokens[0].parse().expect("valid hsl hue");
    let saturation: f64 = tokens[1]
        .trim_end_matches('%')
        .parse::<f64>()
        .expect("valid hsl saturation")
        / 100.0;
    let lightness: f64 = tokens[2]
        .trim_end_matches('%')
        .parse::<f64>()
        .expect("valid hsl lightness")
        / 100.0;
    let alpha = match tokens.get(3..) {
        Some(["/", alpha]) => alpha.parse().expect("valid hsl alpha"),
        None | Some([]) => 1.0,
        _ => panic!("valid hsl alpha syntax"),
    };
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let hue_sector = hue.rem_euclid(360.0) / 60.0;
    let second = chroma * (1.0 - (hue_sector.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match hue_sector as u8 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let offset = lightness - chroma / 2.0;
    (
        Rgb {
            r: ((red + offset) * 255.0).round() as u8,
            g: ((green + offset) * 255.0).round() as u8,
            b: ((blue + offset) * 255.0).round() as u8,
        },
        alpha,
    )
}

pub fn transform(operations: Vec<TransformOp>) -> Transform {
    Transform { ops: operations }
}

pub fn translate(x: f64, y: f64) -> TransformOp {
    TransformOp::Translate {
        tx: mu(x),
        ty: mu(y),
    }
}

pub fn scale(x: f64, y: f64) -> TransformOp {
    TransformOp::Scale {
        sx: Q32((x * (1_u64 << 32) as f64).round() as i64),
        sy: Q32((y * (1_u64 << 32) as f64).round() as i64),
    }
}

pub fn rotate(angle_degrees: f64, cx: f64, cy: f64) -> TransformOp {
    TransformOp::Rotate {
        angle: Bam16(
            ((angle_degrees.rem_euclid(360.0) / 360.0 * 65_536.0).round() as u32 % 65_536) as u16,
        ),
        cx: mu(cx),
        cy: mu(cy),
    }
}

/// Prepends a group transform to primitives and their mask geometry so the
/// effect remains in the same coordinate space as the primitive it masks.
pub fn prepend_transform(primitives: &mut [Primitive], operations: &[TransformOp]) {
    for primitive in primitives {
        prepend_primitive_transform(primitive, operations);
    }
}

fn prepend_primitive_transform(primitive: &mut Primitive, operations: &[TransformOp]) {
    let mut combined = operations.to_vec();
    if let Some(transform) = primitive.transform.take() {
        combined.extend(transform.ops);
    }
    primitive.transform = if combined.is_empty() {
        None
    } else {
        Some(transform(combined))
    };
    for effect in &mut primitive.effects {
        if let Effect::Masked { mask } = effect {
            prepend_transform(mask, operations);
        }
    }
}

fn opacity_q32(value: f64) -> Option<Q32> {
    let value = value.clamp(0.0, 1.0);
    if value >= 1.0 {
        None
    } else {
        Some(Q32((value * (1_u64 << 32) as f64).round() as i64))
    }
}
