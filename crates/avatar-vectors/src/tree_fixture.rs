use avatar_svg::{
    Bam16, Effect, FillRule, GradientStop, Paint, Primitive, RenderTree, Rgb, Role, Segment, Shape,
    Stroke, StrokeCap, StrokeJoin, Transform, TransformOp, Q32,
};
use serde::Deserialize;

pub(crate) fn decode_tree_fixture(bytes: &[u8]) -> Result<RenderTree, serde_json::Error> {
    serde_json::from_slice::<FixtureTree>(bytes).map(Into::into)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureTree {
    canvas: u32,
    background: Option<FixtureRgb>,
    primitives: Vec<FixturePrimitive>,
}

impl From<FixtureTree> for RenderTree {
    fn from(tree: FixtureTree) -> Self {
        Self {
            canvas: tree.canvas,
            background: tree.background.map(Into::into),
            primitives: tree.primitives.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixturePrimitive {
    shape: FixtureShape,
    transform: Option<FixtureTransform>,
    paint: FixturePaint,
    stroke: Option<FixtureStroke>,
    opacity: Option<i64>,
    clip: Option<FixtureShape>,
    effects: Vec<FixtureEffect>,
    label: String,
    role: Option<FixtureRole>,
}

impl From<FixturePrimitive> for Primitive {
    fn from(primitive: FixturePrimitive) -> Self {
        Self {
            shape: primitive.shape.into(),
            transform: primitive.transform.map(Into::into),
            paint: primitive.paint.into(),
            stroke: primitive.stroke.map(Into::into),
            opacity: primitive.opacity.map(Q32),
            clip: primitive.clip.map(Into::into),
            effects: primitive.effects.into_iter().map(Into::into).collect(),
            label: primitive.label,
            role: primitive.role.map(Into::into),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureRole {
    GroundShadow,
}

impl From<FixtureRole> for Role {
    fn from(role: FixtureRole) -> Self {
        match role {
            FixtureRole::GroundShadow => Self::GroundShadow,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureFillRule {
    NonZero,
    EvenOdd,
}

impl From<FixtureFillRule> for FillRule {
    fn from(fill_rule: FixtureFillRule) -> Self {
        match fill_rule {
            FixtureFillRule::NonZero => Self::NonZero,
            FixtureFillRule::EvenOdd => Self::EvenOdd,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureSegment {
    #[serde(rename = "move_to")]
    Move { x: i64, y: i64 },
    #[serde(rename = "cubic_to")]
    Cubic {
        cp1x: i64,
        cp1y: i64,
        cp2x: i64,
        cp2y: i64,
        x: i64,
        y: i64,
    },
    #[serde(rename = "line_to")]
    Line { x: i64, y: i64 },
}

impl From<FixtureSegment> for Segment {
    fn from(segment: FixtureSegment) -> Self {
        match segment {
            FixtureSegment::Move { x, y } => Self::MoveTo { x, y },
            FixtureSegment::Cubic {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            } => Self::CubicTo {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            },
            FixtureSegment::Line { x, y } => Self::LineTo { x, y },
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureShape {
    Path {
        segments: Vec<FixtureSegment>,
        closed: bool,
        fill_rule: FixtureFillRule,
    },
    Circle {
        cx: i64,
        cy: i64,
        r: i64,
    },
    Ellipse {
        cx: i64,
        cy: i64,
        rx: i64,
        ry: i64,
    },
    Rect {
        x: i64,
        y: i64,
        w: i64,
        h: i64,
    },
}

impl From<FixtureShape> for Shape {
    fn from(shape: FixtureShape) -> Self {
        match shape {
            FixtureShape::Path {
                segments,
                closed,
                fill_rule,
            } => Self::Path {
                segments: segments.into_iter().map(Into::into).collect(),
                closed,
                fill_rule: fill_rule.into(),
            },
            FixtureShape::Circle { cx, cy, r } => Self::Circle { cx, cy, r },
            FixtureShape::Ellipse { cx, cy, rx, ry } => Self::Ellipse { cx, cy, rx, ry },
            FixtureShape::Rect { x, y, w, h } => Self::Rect { x, y, w, h },
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRgb {
    r: u8,
    g: u8,
    b: u8,
}

impl From<FixtureRgb> for Rgb {
    fn from(color: FixtureRgb) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureGradientStop {
    offset_q32: i64,
    color: FixtureRgb,
}

impl From<FixtureGradientStop> for GradientStop {
    fn from(stop: FixtureGradientStop) -> Self {
        Self {
            offset_q32: Q32(stop.offset_q32),
            color: stop.color.into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixturePaint {
    Solid { color: FixtureRgb },
    RadialGradient { stops: Vec<FixtureGradientStop> },
    None,
}

impl From<FixturePaint> for Paint {
    fn from(paint: FixturePaint) -> Self {
        match paint {
            FixturePaint::Solid { color } => Self::Solid {
                color: color.into(),
            },
            FixturePaint::RadialGradient { stops } => Self::RadialGradient {
                stops: stops.into_iter().map(Into::into).collect(),
            },
            FixturePaint::None => Self::None,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureStrokeCap {
    Butt,
    Round,
    Square,
}

impl From<FixtureStrokeCap> for StrokeCap {
    fn from(cap: FixtureStrokeCap) -> Self {
        match cap {
            FixtureStrokeCap::Butt => Self::Butt,
            FixtureStrokeCap::Round => Self::Round,
            FixtureStrokeCap::Square => Self::Square,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureStrokeJoin {
    Miter,
    Round,
    Bevel,
}

impl From<FixtureStrokeJoin> for StrokeJoin {
    fn from(join: FixtureStrokeJoin) -> Self {
        match join {
            FixtureStrokeJoin::Miter => Self::Miter,
            FixtureStrokeJoin::Round => Self::Round,
            FixtureStrokeJoin::Bevel => Self::Bevel,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureStroke {
    color: FixtureRgb,
    width: i64,
    cap: FixtureStrokeCap,
    join: FixtureStrokeJoin,
}

impl From<FixtureStroke> for Stroke {
    fn from(stroke: FixtureStroke) -> Self {
        Self {
            color: stroke.color.into(),
            width: stroke.width,
            cap: stroke.cap.into(),
            join: stroke.join.into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureEffect {
    Lit {
        light_y: i64,
    },
    Masked {
        mask: Vec<FixturePrimitive>,
    },
    Turbulence {
        seed: u8,
        freq_h: i64,
        freq_v: i64,
        octaves: u8,
        scale: i64,
    },
}

impl From<FixtureEffect> for Effect {
    fn from(effect: FixtureEffect) -> Self {
        match effect {
            FixtureEffect::Lit { light_y } => Self::Lit { light_y },
            FixtureEffect::Masked { mask } => Self::Masked {
                mask: mask.into_iter().map(Into::into).collect(),
            },
            FixtureEffect::Turbulence {
                seed,
                freq_h,
                freq_v,
                octaves,
                scale,
            } => Self::Turbulence {
                seed,
                freq_h: Q32(freq_h),
                freq_v: Q32(freq_v),
                octaves,
                scale: Q32(scale),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureTransformOp {
    Translate { tx: i64, ty: i64 },
    Rotate { angle: u16, cx: i64, cy: i64 },
    Scale { sx: i64, sy: i64 },
}

impl From<FixtureTransformOp> for TransformOp {
    fn from(operation: FixtureTransformOp) -> Self {
        match operation {
            FixtureTransformOp::Translate { tx, ty } => Self::Translate { tx, ty },
            FixtureTransformOp::Rotate { angle, cx, cy } => Self::Rotate {
                angle: Bam16(angle),
                cx,
                cy,
            },
            FixtureTransformOp::Scale { sx, sy } => Self::Scale {
                sx: Q32(sx),
                sy: Q32(sy),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureTransform {
    ops: Vec<FixtureTransformOp>,
}

impl From<FixtureTransform> for Transform {
    fn from(transform: FixtureTransform) -> Self {
        Self {
            ops: transform.ops.into_iter().map(Into::into).collect(),
        }
    }
}
