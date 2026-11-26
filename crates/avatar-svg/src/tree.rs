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
    /// The square canvas edge in SVG USER UNITS — the serializer's viewBox.
    ///
    /// **This is not the unit primitive coordinates are expressed in.** Those
    /// are 100x finer; divide them by [`RenderTree::extent`], never by this.
    /// Normalising geometry by `canvas` yields a drawing 100x too large,
    /// which on most surfaces looks like a plausible zoomed-in avatar rather
    /// than an error — an iOS consumer hit exactly that and had to read the
    /// serializer to find the factor.
    pub canvas: u32,
    /// Optional background drawn behind every primitive.
    pub background: Option<Rgb>,
    /// Independently drawable geometry in painter's order.
    pub primitives: Vec<Primitive>,
}

impl RenderTree {
    /// The coordinate extent primitives span, on both axes, from zero.
    ///
    /// This is the divisor a geometry consumer wants: multiply by a target
    /// edge length and divide by this to map a primitive into a view. It is
    /// exposed as a value rather than left to callers to derive, because the
    /// alternative is every consumer rediscovering the factor by reading the
    /// SVG serializer.
    pub fn extent(&self) -> i64 {
        i64::from(self.canvas) * COORDS_PER_CANVAS_UNIT
    }
}

/// Primitive coordinates carry two more decimal places than the SVG canvas,
/// which is what the serializer divides out when it formats a path.
pub const COORDS_PER_CANVAS_UNIT: i64 = 100;

/// One independently drawable item with its fully resolved transform chain.
///
/// `label` is a diagnostic name with no stability promise. Consumers that
/// change behavior based on a primitive's meaning must use `role` instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Primitive {
    pub shape: Shape,
    /// The complete SVG-order operation chain, or `None` for identity.
    pub transform: Option<Transform>,
    pub paint: Paint,
    pub stroke: Option<Stroke>,
    /// Per-element opacity; `None` means fully opaque.
    pub opacity: Option<Q32>,
    /// Structural clipping geometry that consumers must honor. It is in the
    /// same local coordinates as `shape`, so `transform` applies to both.
    pub clip: Option<Shape>,
    /// Optional rendering annotations that small native consumers may ignore.
    pub effects: Vec<Effect>,
    /// Diagnostic SVG class or id name; not a semantic contract.
    pub label: String,
    /// Semantic contract for consumer behavior; most primitives have no role.
    pub role: Option<Role>,
}

/// A primitive's semantic purpose, independent of its diagnostic label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    GroundShadow,
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
    /// A non-empty list of closed contours sharing one winding rule. Each
    /// starts with a move and continues with at least one line or cubic.
    CompoundPath {
        subpaths: Vec<Vec<Segment>>,
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
    /// Opaque stops are `None`; explicit opacities are in [0, 1).
    pub opacity_q32: Option<Q32>,
}

impl GradientStop {
    /// Clamp and quantize opacity, omitting a stop opacity that rounds to one.
    pub fn with_opacity(offset_q32: Q32, color: Rgb, opacity: f64) -> Self {
        let opacity = Q32((opacity.clamp(0.0, 1.0) * Q32::ONE.0 as f64).round() as i64);
        Self {
            offset_q32,
            color,
            opacity_q32: (opacity != Q32::ONE).then_some(opacity),
        }
    }

    pub(crate) fn checked_opacity(&self) -> Option<Q32> {
        if let Some(opacity) = self.opacity_q32 {
            assert!(
                (0..Q32::ONE.0).contains(&opacity.0),
                "gradient stop opacity must be in [0, 1)"
            );
        }
        self.opacity_q32
    }
}

/// Fill paint for a primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Paint {
    Solid {
        color: Rgb,
    },
    RadialGradient {
        stops: Vec<GradientStop>,
    },
    /// A radial gradient in the primitive's local, pre-transform coordinates.
    RadialGradientAt {
        cx: i64,
        cy: i64,
        r: i64,
        focal: Option<(i64, i64)>,
        stops: Vec<GradientStop>,
    },
    None,
}

impl Paint {
    /// Move an on-circle or exterior focus to the nearest strictly interior
    /// integer point, breaking ties by x then y.
    ///
    /// Panics unless the radius is positive and all inputs are within ±2³¹.
    pub fn radial_gradient_at(
        cx: i64,
        cy: i64,
        r: i64,
        focal: Option<(i64, i64)>,
        stops: Vec<GradientStop>,
    ) -> Self {
        assert!(r > 0, "radial gradient radius must be positive");
        let in_range = |coord: i64| coord.unsigned_abs() <= 1 << 31;
        assert!(
            in_range(cx)
                && in_range(cy)
                && in_range(r)
                && focal.is_none_or(|(fx, fy)| in_range(fx) && in_range(fy)),
            "radial gradient constructor coordinates must be within ±2³¹"
        );
        let focal = focal.map(|(fx, fy)| {
            if focus_inside(cx, cy, r, fx, fy) {
                return (fx, fy);
            }
            let dx = i128::from(fx) - i128::from(cx);
            let dy = i128::from(fy) - i128::from(cy);
            let (x, y) = nearest_interior_focus(dx, dy, i128::from(r));
            (cx + x as i64, cy + y as i64)
        });
        Self::RadialGradientAt {
            cx,
            cy,
            r,
            focal,
            stops,
        }
    }
}

pub(crate) fn assert_radial_gradient_at(cx: i64, cy: i64, r: i64, focal: Option<(i64, i64)>) {
    assert!(r > 0, "radial gradient radius must be positive");
    assert!(
        focal.is_none_or(|(fx, fy)| focus_inside(cx, cy, r, fx, fy)),
        "radial gradient focus must be strictly inside the circle"
    );
}

fn focus_inside(cx: i64, cy: i64, r: i64, fx: i64, fy: i64) -> bool {
    let dx = (i128::from(fx) - i128::from(cx)).abs();
    let dy = (i128::from(fy) - i128::from(cy)).abs();
    let r = i128::from(r);
    dx < r && dy < r && dx * dx + dy * dy < r * r
}

pub(crate) fn assert_compound_path(subpaths: &[Vec<Segment>]) {
    assert!(!subpaths.is_empty(), "compound path must have a subpath");
    for segments in subpaths {
        assert!(
            matches!(segments.as_slice(), [Segment::MoveTo { .. }, rest @ ..]
                if !rest.is_empty() && rest.iter().all(|s| matches!(s, Segment::LineTo { .. } | Segment::CubicTo { .. }))),
            "compound subpath must start with its only move and have a line or cubic"
        );
    }
}

fn nearest_interior_focus(dx: i128, dy: i128, r: i128) -> (i128, i128) {
    // Reflect into the target's quadrant: crossing an axis cannot improve
    // distance, and neither can going farther along an axis than the target.
    let (tx, ty) = (dx.abs(), dy.abs());
    let limit = r * r - 1; // Integer squared lengths must be strictly below r².
    let y_at = |x: i128| ty.min((limit - x * x).isqrt());
    let square = |n| n * n;
    let candidate = |x, y| {
        (
            square(tx - x) + square(ty - y),
            x * dx.signum(),
            y * dy.signum(),
        )
    };

    let max_x = tx.min(r - 1);
    // Seed the search near the continuous projection; pruning does not
    // depend on this estimate being exact.
    let seed = (tx * r / (tx * tx + ty * ty).isqrt()).min(max_x);
    let mut best = candidate(seed, y_at(seed));
    let mut intervals = vec![(0, max_x)];
    while let Some((lo, hi)) = intervals.pop() {
        // The feasible points over [lo, hi] lie in [lo, hi] × [0, y_at(lo)].
        // Distance to that rectangle is a lower bound; ties still need a look.
        let y = y_at(lo);
        if square(tx - hi) + square(ty - y) > best.0 {
            continue;
        }
        if lo == hi {
            best = best.min(candidate(lo, y));
        } else {
            let mid = (lo + hi) / 2;
            intervals.push((mid + 1, hi));
            intervals.push((lo, mid));
        }
    }
    (best.1, best.2)
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

#[cfg(test)]
#[path = "tree_feature_tests.rs"]
mod tree_features;
