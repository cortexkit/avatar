//! Geometry-primary render contract from design note 006.
//!
//! Desktop and other native consumers draw this flat list directly. SVG is
//! only one byte-stable projection of the same resolved geometry.

use avatar_phenotype::{Bam16, Rgb, Q32};
use avatar_tables::{cos_q30, sin_q30};

use crate::fixed::q32_mul;

/// A complete avatar drawing in painter's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderTree {
    /// The square canvas edge in SVG user units.
    pub canvas: u32,
    /// Optional background drawn behind every primitive.
    pub background: Option<Rgb>,
    /// Independently drawable geometry in painter's order.
    pub primitives: Vec<Primitive>,
}

/// One independently drawable item with its fully resolved transform chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Primitive {
    pub shape: Shape,
    /// The complete SVG-order operation chain, or `None` for identity.
    pub transform: Option<Transform>,
    pub paint: Paint,
    pub stroke: Option<Stroke>,
    /// Per-element opacity; `None` means fully opaque.
    pub opacity: Option<Q32>,
    /// Structural clipping geometry that consumers must honor.
    pub clip: Option<Shape>,
    /// Optional rendering annotations that small native consumers may ignore.
    pub effects: Vec<Effect>,
    /// Stable semantic SVG class or id name.
    pub label: String,
}

/// Winding rule used to fill a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// One absolute path segment in canonical milliunits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    MoveTo {
        x: i64,
        y: i64,
    },
    CubicTo {
        cp1x: i64,
        cp1y: i64,
        cp2x: i64,
        cp2y: i64,
        x: i64,
        y: i64,
    },
    LineTo {
        x: i64,
        y: i64,
    },
}

/// Drawable geometry in pre-transform canonical milliunits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Path {
        segments: Vec<Segment>,
        closed: bool,
        fill_rule: FillRule,
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

/// One radial-gradient stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GradientStop {
    pub offset_q32: Q32,
    pub color: Rgb,
}

/// Fill paint for a primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Paint {
    Solid { color: Rgb },
    RadialGradient { stops: Vec<GradientStop> },
    None,
}

/// Stroke endpoint treatment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeCap {
    Butt,
    Round,
    Square,
}

/// Stroke corner treatment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeJoin {
    Miter,
    Round,
    Bevel,
}

/// Per-primitive stroke styling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub color: Rgb,
    pub width: i64,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
}

impl Stroke {
    pub(crate) fn new(color: Rgb, width: i64) -> Self {
        Self {
            color,
            width,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
        }
    }
}

/// Optional rendering behavior attached to a drawable primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Membership in the body lighting group.
    ///
    /// Design note 006 requires the resolved light position to ride the tree:
    /// rebuilding it from rounded path geometry can differ by one milliunit.
    Lit { light_y: i64 },
    /// Pattern mask geometry, already carrying its resolved transforms.
    Masked { mask: Vec<Primitive> },
    /// Iris displacement annotation.
    Turbulence {
        seed: u8,
        freq_h: Q32,
        freq_v: Q32,
        octaves: u8,
        scale: Q32,
    },
}

/// One transform operation, preserving serializer order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformOp {
    Translate { tx: i64, ty: i64 },
    Rotate { angle: Bam16, cx: i64, cy: i64 },
    Scale { sx: Q32, sy: Q32 },
}

/// Q32.32 affine coefficients with milliunit translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Affine {
    pub a: Q32,
    pub b: Q32,
    pub c: Q32,
    pub d: Q32,
    pub tx: i64,
    pub ty: i64,
}

/// A serializer-order transform operation chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transform {
    pub ops: Vec<TransformOp>,
}

impl Transform {
    /// Composes the operation chain into one affine transform.
    ///
    /// Products narrow once per Q32 multiplication, matching the geometry
    /// helpers used to resolve the tree.
    pub fn affine(&self) -> Affine {
        let mut result = Affine {
            a: Q32::ONE,
            b: Q32(0),
            c: Q32(0),
            d: Q32::ONE,
            tx: 0,
            ty: 0,
        };
        for operation in &self.ops {
            let next = match *operation {
                TransformOp::Translate { tx, ty } => Affine {
                    a: Q32::ONE,
                    b: Q32(0),
                    c: Q32(0),
                    d: Q32::ONE,
                    tx,
                    ty,
                },
                TransformOp::Scale { sx, sy } => Affine {
                    a: sx,
                    b: Q32(0),
                    c: Q32(0),
                    d: sy,
                    tx: 0,
                    ty: 0,
                },
                TransformOp::Rotate { angle, cx, cy } => {
                    let cosine = i64::from(cos_q30(angle.0)) * 4;
                    let sine = i64::from(sin_q30(angle.0)) * 4;
                    Affine {
                        a: Q32(cosine),
                        b: Q32(sine),
                        c: Q32(-sine),
                        d: Q32(cosine),
                        tx: cx - q32_mul(cx, cosine) + q32_mul(cy, sine),
                        ty: cy - q32_mul(cx, sine) - q32_mul(cy, cosine),
                    }
                }
            };
            result = compose_affine(result, next);
        }
        result
    }
}

fn compose_affine(lhs: Affine, rhs: Affine) -> Affine {
    Affine {
        a: Q32(q32_mul(lhs.a.0, rhs.a.0) + q32_mul(lhs.c.0, rhs.b.0)),
        b: Q32(q32_mul(lhs.b.0, rhs.a.0) + q32_mul(lhs.d.0, rhs.b.0)),
        c: Q32(q32_mul(lhs.a.0, rhs.c.0) + q32_mul(lhs.c.0, rhs.d.0)),
        d: Q32(q32_mul(lhs.b.0, rhs.c.0) + q32_mul(lhs.d.0, rhs.d.0)),
        tx: lhs.tx + q32_mul(rhs.tx, lhs.a.0) + q32_mul(rhs.ty, lhs.c.0),
        ty: lhs.ty + q32_mul(rhs.tx, lhs.b.0) + q32_mul(rhs.ty, lhs.d.0),
    }
}
