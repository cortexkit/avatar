//! Turning the reference's SVG vocabulary into render-tree primitives.
//!
//! The reference writes coordinates with one decimal (`num`) or two
//! (`num2`), rounding with JavaScript's `Math.round`. Tree coordinates are
//! hundredths of a canvas unit, so both precisions are exact in the tree, and
//! the helpers here round the way the reference's text does.

use avatar_svg::wild::render::opacity;
use avatar_svg::{
    Bam16, FillRule, Paint, Primitive, Rgb, Segment, Shape, Stroke, StrokeCap, StrokeJoin, Subpath,
    TransformOp, Q32,
};

use crate::geom::{js_round, P};

/// A coordinate written with one decimal, in tree units.
pub fn q1(v: f64) -> i64 {
    js_round(v * 10.0) as i64 * 10
}

/// A coordinate written with two decimals, in tree units.
pub fn q2(v: f64) -> i64 {
    js_round(v * 100.0) as i64
}

/// A plain number as the reference writes it in an attribute, in tree units
/// (exact for the reference's literal widths and offsets).
pub fn lit(v: f64) -> i64 {
    q2(v)
}

/// A number written with two decimals, as a value.
pub fn n2(v: f64) -> f64 {
    js_round(v * 100.0) / 100.0
}

/// One path command in a local coordinate frame.
#[derive(Clone, Copy)]
pub enum Cmd {
    M(f64, f64),
    L(f64, f64),
    Q(f64, f64, f64, f64),
    C(f64, f64, f64, f64, f64, f64),
    Z,
}

/// Path data being built, as the reference's `d` string would be.
#[derive(Default)]
pub struct PathB {
    subs: Vec<Subpath>,
    cur: Vec<Segment>,
}

impl PathB {
    pub fn new() -> Self {
        Self::default()
    }
    fn flush(&mut self) {
        if !self.cur.is_empty() {
            self.subs.push(Subpath {
                segments: std::mem::take(&mut self.cur),
                closed: false,
            });
        }
    }
    pub fn mq(&mut self, x: i64, y: i64) {
        self.flush();
        self.cur.push(Segment::MoveTo { x, y });
    }
    pub fn lq(&mut self, x: i64, y: i64) {
        self.cur.push(Segment::LineTo { x, y });
    }
    pub fn m1(&mut self, x: f64, y: f64) {
        self.mq(q1(x), q1(y));
    }
    pub fn l1(&mut self, x: f64, y: f64) {
        self.lq(q1(x), q1(y));
    }
    pub fn m2(&mut self, x: f64, y: f64) {
        self.mq(q2(x), q2(y));
    }
    pub fn l2(&mut self, x: f64, y: f64) {
        self.lq(q2(x), q2(y));
    }
    pub fn cq(&mut self, c: [i64; 6]) {
        self.cur.push(Segment::CubicTo {
            cp1x: c[0],
            cp1y: c[1],
            cp2x: c[2],
            cp2y: c[3],
            x: c[4],
            y: c[5],
        });
    }
    /// A cubic from the current point, written with two decimals.
    pub fn cubic_to(&mut self, c1: P, c2: P, end: P) {
        self.cq([
            q2(c1[0]),
            q2(c1[1]),
            q2(c2[0]),
            q2(c2[1]),
            q2(end[0]),
            q2(end[1]),
        ]);
    }
    /// A quadratic from `(x0, y0)` written as the cubic it equals.
    #[allow(clippy::too_many_arguments)]
    pub fn quad_to(&mut self, x0: f64, y0: f64, qx: f64, qy: f64, x1: f64, y1: f64) {
        self.cq([
            q2(x0 + (qx - x0) * 2.0 / 3.0),
            q2(y0 + (qy - y0) * 2.0 / 3.0),
            q2(x1 + (qx - x1) * 2.0 / 3.0),
            q2(y1 + (qy - y1) * 2.0 / 3.0),
            q2(x1),
            q2(y1),
        ]);
    }
    pub fn z(&mut self) {
        if !self.cur.is_empty() {
            self.subs.push(Subpath {
                segments: std::mem::take(&mut self.cur),
                closed: true,
            });
        }
    }
    pub fn is_empty(&self) -> bool {
        self.subs.is_empty() && self.cur.is_empty()
    }
    pub fn shape(mut self) -> Shape {
        self.flush();
        if self.subs.len() == 1 {
            let s = self.subs.pop().expect("one subpath");
            Shape::Path {
                segments: s.segments,
                closed: s.closed,
                fill_rule: FillRule::NonZero,
            }
        } else {
            Shape::CompoundPath {
                subpaths: self.subs,
                fill_rule: FillRule::NonZero,
            }
        }
    }
}

/// `pathOf`: points joined by lines, one decimal.
pub fn path_of(pts: &[P], closed: bool) -> Shape {
    let mut d = PathB::new();
    for (i, p) in pts.iter().enumerate() {
        if i == 0 {
            d.m1(p[0], p[1]);
        } else {
            d.l1(p[0], p[1]);
        }
    }
    if closed {
        d.z();
    }
    d.shape()
}

/// `localPath`: commands in local units mapped to the frame by `x` and `y`,
/// with quadratics written as cubics. `shear = (x0, k)` drops each point by
/// `k` for every unit it lies right of `x0`.
pub fn local_path(
    d: &mut PathB,
    cmds: &[Cmd],
    x: &dyn Fn(f64) -> f64,
    y: &dyn Fn(f64) -> f64,
    shear: Option<(f64, f64)>,
) {
    let (x0, k) = shear.unwrap_or((0.0, 0.0));
    let py = |vx: f64, vy: f64| y(vy) + k * (x(vx) - x0);
    let (mut cx, mut cy) = (0.0, 0.0);
    for c in cmds {
        match *c {
            Cmd::M(a, b) => {
                cx = x(a);
                cy = py(a, b);
                d.m2(cx, cy);
            }
            Cmd::L(a, b) => {
                cx = x(a);
                cy = py(a, b);
                d.l2(cx, cy);
            }
            Cmd::Q(qa, qb, a, b) => {
                let (ex, ey) = (x(a), py(a, b));
                d.quad_to(cx, cy, x(qa), py(qa, qb), ex, ey);
                cx = ex;
                cy = ey;
            }
            Cmd::C(a1, b1, a2, b2, a, b) => {
                cx = x(a);
                cy = py(a, b);
                d.cubic_to([x(a1), py(a1, b1)], [x(a2), py(a2, b2)], [cx, cy]);
            }
            Cmd::Z => d.z(),
        }
    }
}

pub fn local_shape(
    cmds: &[Cmd],
    x: &dyn Fn(f64) -> f64,
    y: &dyn Fn(f64) -> f64,
    shear: Option<(f64, f64)>,
) -> Shape {
    let mut d = PathB::new();
    local_path(&mut d, cmds, x, y, shear);
    d.shape()
}

pub fn circle(cx: i64, cy: i64, r: i64) -> Shape {
    Shape::Circle { cx, cy, r }
}

pub fn ellipse(cx: i64, cy: i64, rx: i64, ry: i64) -> Shape {
    Shape::Ellipse { cx, cy, rx, ry }
}

pub fn stroke(color: Rgb, width: i64, cap: StrokeCap, join: StrokeJoin) -> Stroke {
    Stroke {
        color,
        width,
        cap,
        join,
    }
}

/// A round-capped, round-joined stroke, the reference's usual line.
pub fn round_stroke(color: Rgb, width: i64) -> Stroke {
    stroke(color, width, StrokeCap::Round, StrokeJoin::Round)
}

/// A stroke with SVG's default butt caps and miter joins.
pub fn plain_stroke(color: Rgb, width: i64) -> Stroke {
    stroke(color, width, StrokeCap::Butt, StrokeJoin::Miter)
}

pub fn prim(
    shape: Shape,
    paint: Paint,
    stroke: Option<Stroke>,
    op: Option<f64>,
    label: &str,
) -> Primitive {
    Primitive {
        shape,
        transform: None,
        paint,
        stroke,
        opacity: op.and_then(opacity),
        clip: None,
        effects: Vec::new(),
        label: label.to_owned(),
        role: None,
    }
}

pub fn solid(color: Rgb) -> Paint {
    Paint::Solid { color }
}

/// A filled shape.
pub fn filled(shape: Shape, color: Rgb, op: Option<f64>, label: &str) -> Primitive {
    prim(shape, solid(color), None, op, label)
}

/// A shape with only a stroke.
pub fn lined(shape: Shape, stroke: Stroke, op: Option<f64>, label: &str) -> Primitive {
    prim(shape, Paint::None, Some(stroke), op, label)
}

/// A rotation as the reference writes it: degrees with two decimals.
pub fn rotate(deg: f64, cx: i64, cy: i64) -> TransformOp {
    let d = n2(deg);
    TransformOp::Rotate {
        angle: Bam16(((d.rem_euclid(360.0) / 360.0 * 65_536.0).round() as u32 % 65_536) as u16),
        cx,
        cy,
    }
}

pub fn translate(tx: i64, ty: i64) -> TransformOp {
    TransformOp::Translate { tx, ty }
}

/// A scale as the reference writes it: factors with two decimals.
pub fn scale(sx: f64, sy: f64) -> TransformOp {
    let q = |v: f64| Q32((n2(v) * (1_u64 << 32) as f64).round() as i64);
    TransformOp::Scale {
        sx: q(sx),
        sy: q(sy),
    }
}

/// A Q32 fraction from a float.
pub fn q32(v: f64) -> Q32 {
    Q32((v * Q32::ONE.0 as f64).round() as i64)
}
