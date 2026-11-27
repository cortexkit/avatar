//! Points, smooth curves and pen strokes, as the reference builds them.
//!
//! Every expression keeps the reference's operation order, so the same
//! genome gives the same `f64` values here as in JavaScript. Only basic
//! arithmetic, `sqrt`, `floor` and the shared sine polynomial are used.

use avatar_svg::wild::math;
use avatar_svg::Segment;
use std::f64::consts::PI;

/// Degrees to radians, as the reference computes it.
pub const DEG: f64 = PI / 180.0;

/// A point of an outline. A sharp point is a corner: the curve through it
/// has no tangent there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
    pub sharp: bool,
}

/// A smooth point.
pub fn pt(x: f64, y: f64) -> Pt {
    Pt { x, y, sharp: false }
}

/// A corner point.
pub fn sharp(x: f64, y: f64) -> Pt {
    Pt { x, y, sharp: true }
}

impl Pt {
    /// The same place as a smooth point.
    pub fn smooth(self) -> Pt {
        pt(self.x, self.y)
    }

    /// The same place, sharp or smooth as given.
    pub fn with_sharp(self, sharp: bool) -> Pt {
        Pt { sharp, ..self }
    }
}

/// JavaScript's `Math.round`: the nearest integer, halves toward +∞.
pub fn js_round(v: f64) -> f64 {
    let f = v.floor();
    if v - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// A coordinate as the reference writes it (to a tenth of a unit), in the
/// render tree's hundredths.
pub fn tenth(v: f64) -> i64 {
    js_round(v * 10.0) as i64 * 10
}

/// A width or a translation as the reference writes it (to a hundredth of
/// a unit), in the render tree's hundredths.
pub fn hundredth(v: f64) -> i64 {
    js_round(v * 100.0) as i64
}

/// A value kept to a hundredth, as the reference keeps angles and moves.
pub fn round2(v: f64) -> f64 {
    js_round(v * 100.0) / 100.0
}

/// A value kept to a thousandth, as the reference keeps scales.
pub fn round3(v: f64) -> f64 {
    js_round(v * 1000.0) / 1000.0
}

pub fn sin(angle: f64) -> f64 {
    math::sin(angle)
}

pub fn cos(angle: f64) -> f64 {
    math::cos(angle)
}

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// `Math.min` and `Math.max` of two finite values.
pub fn min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

pub fn max(a: f64, b: f64) -> f64 {
    if b > a {
        b
    } else {
        a
    }
}

pub fn hyp(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

pub fn mix(p: Pt, q: Pt, t: f64) -> Pt {
    pt(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t)
}

/// 0 below `a`, 1 above `b`, and a smooth S-shaped step between them.
pub fn step(a: f64, b: f64, v: f64) -> f64 {
    let t = clamp((v - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A smooth hump: 1 at `c`, falling to 0 at `c - w` and `c + w`.
pub fn hump(v: f64, c: f64, w: f64) -> f64 {
    let u = (v - c) / w;
    if u.abs() >= 1.0 {
        0.0
    } else {
        (1.0 - u * u) * (1.0 - u * u)
    }
}

/// The unit vector from `a` towards `b`, or zero when they coincide.
pub fn unit_of(a: Pt, b: Pt) -> (f64, f64) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let l = hyp(dx, dy);
    if l < 1e-9 {
        (0.0, 0.0)
    } else {
        (dx / l, dy / l)
    }
}

/// The direction a turn of `degrees` from straight up points in: 0 is up,
/// 90 is to the right.
pub fn up_dir(degrees: f64) -> (f64, f64) {
    (sin(degrees * DEG), -cos(degrees * DEG))
}

/// One cubic stretch of a smooth curve.
#[derive(Clone, Copy, Debug)]
pub struct Seg {
    pub p: Pt,
    pub c1: (f64, f64),
    pub c2: (f64, f64),
    pub q: Pt,
}

/// A smooth curve through points, as a pen would join them. At a smooth
/// point the curve runs parallel to the line between its two neighbours,
/// and each handle is a third as long as its stretch.
pub fn segments(pts: &[Pt], closed: bool) -> Vec<Seg> {
    let n = pts.len();
    if n == 0 {
        return Vec::new();
    }
    let tangent = |i: usize| -> (f64, f64) {
        let p = pts[i];
        if p.sharp {
            return (0.0, 0.0);
        }
        let (a, b) = if closed {
            (pts[(i + n - 1) % n], pts[(i + 1) % n])
        } else {
            (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)])
        };
        unit_of(a, b)
    };
    let last = if closed { n } else { n - 1 };
    let mut segs = Vec::with_capacity(last);
    for i in 0..last {
        let p = pts[i];
        let q = pts[(i + 1) % n];
        let u = tangent(i);
        let v = tangent((i + 1) % n);
        let l = hyp(q.x - p.x, q.y - p.y) / 3.0;
        segs.push(Seg {
            p,
            c1: (p.x + u.0 * l, p.y + u.1 * l),
            c2: (q.x - v.0 * l, q.y - v.1 * l),
            q,
        });
    }
    segs
}

/// One contour of a drawn path, in render-tree hundredths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contour {
    pub segments: Vec<Segment>,
    pub closed: bool,
}

fn move_to(p: (f64, f64)) -> Segment {
    Segment::MoveTo {
        x: tenth(p.0),
        y: tenth(p.1),
    }
}

fn line_to(p: (f64, f64)) -> Segment {
    Segment::LineTo {
        x: tenth(p.0),
        y: tenth(p.1),
    }
}

/// The path of a list of stretches. A stretch whose handles lie on its
/// ends is written as a straight line, as the reference writes it.
pub fn contour_of(segs: &[Seg], closed: bool) -> Option<Contour> {
    let first = segs.first()?;
    let mut segments = vec![move_to((first.p.x, first.p.y))];
    for s in segs {
        let straight = (s.c1.0 - s.p.x).abs()
            + (s.c1.1 - s.p.y).abs()
            + (s.c2.0 - s.q.x).abs()
            + (s.c2.1 - s.q.y).abs()
            < 0.02;
        segments.push(if straight {
            line_to((s.q.x, s.q.y))
        } else {
            Segment::CubicTo {
                cp1x: tenth(s.c1.0),
                cp1y: tenth(s.c1.1),
                cp2x: tenth(s.c2.0),
                cp2y: tenth(s.c2.1),
                x: tenth(s.q.x),
                y: tenth(s.q.y),
            }
        });
    }
    Some(Contour { segments, closed })
}

/// The smooth curve through `pts` as a contour.
pub fn curve(pts: &[Pt], closed: bool) -> Option<Contour> {
    contour_of(&segments(pts, closed), closed)
}

/// An open polyline of straight lines, as `M…L…` in the reference.
pub fn polyline(pts: &[(f64, f64)]) -> Contour {
    let mut segments = vec![move_to(pts[0])];
    segments.extend(pts[1..].iter().map(|&p| line_to(p)));
    Contour {
        segments,
        closed: false,
    }
}

/// A falling drop: a point at the top and a round end below.
pub fn drop_contour(x: f64, y: f64, s: f64) -> Contour {
    let cubic = |c1: (f64, f64), c2: (f64, f64), to: (f64, f64)| Segment::CubicTo {
        cp1x: tenth(c1.0),
        cp1y: tenth(c1.1),
        cp2x: tenth(c2.0),
        cp2y: tenth(c2.1),
        x: tenth(to.0),
        y: tenth(to.1),
    };
    Contour {
        segments: vec![
            move_to((x, y - s)),
            cubic(
                (x + s * 0.7, y + s * 0.1),
                (x + s * 0.55, y + s),
                (x, y + s),
            ),
            cubic(
                (x - s * 0.55, y + s),
                (x - s * 0.7, y + s * 0.1),
                (x, y - s),
            ),
        ],
        closed: true,
    }
}

/// The same curve as a list of points along it, `per` to each stretch.
pub fn sample(pts: &[Pt], closed: bool, per: usize) -> Vec<Pt> {
    let segs = segments(pts, closed);
    let mut out = Vec::with_capacity(segs.len() * per + 1);
    for s in &segs {
        for k in 0..per {
            let t = k as f64 / per as f64;
            let u = 1.0 - t;
            let a = u * u * u;
            let b = 3.0 * u * u * t;
            let c = 3.0 * u * t * t;
            let e = t * t * t;
            out.push(pt(
                a * s.p.x + b * s.c1.0 + c * s.c2.0 + e * s.q.x,
                a * s.p.y + b * s.c1.1 + c * s.c2.1 + e * s.q.y,
            ));
        }
    }
    if !closed {
        if let Some(s) = segs.last() {
            out.push(s.q.smooth());
        }
    }
    out
}

/// How the width of a pen stroke runs from its start (0) to its end (1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Thin at both ends.
    Leaf,
    /// Full almost to both ends.
    Full,
    /// Pressed at the start, lifted at the end.
    Out,
    /// Lifted at the start, pressed at the end.
    Into,
    /// A marker: the same all along, with blunt ends.
    Even,
}

impl Profile {
    fn at(self, s: f64) -> f64 {
        match self {
            Profile::Leaf => 1.0 - (2.0 * s - 1.0) * (2.0 * s - 1.0),
            Profile::Full => {
                let u = (2.0 * s - 1.0) * (2.0 * s - 1.0);
                1.0 - u * u
            }
            Profile::Out => (1.0 - s) * (0.35 + 0.65 * (1.0 - s)),
            Profile::Into => s * (0.35 + 0.65 * s),
            Profile::Even => 1.0,
        }
    }
}

/// The outline of a stroke drawn by hand round the centre line through
/// `ctrl`: `hw` is half its greatest width and `profile` how the width runs.
pub fn stroke_pts(ctrl: &[Pt], hw: f64, profile: Profile, n: Option<usize>) -> Vec<Pt> {
    let mid: Vec<Pt> = if ctrl.len() > 2 || n.is_some() {
        sample(ctrl, false, n.unwrap_or(3))
    } else {
        ctrl.iter().map(|p| p.smooth()).collect()
    };
    let m = mid.len() - 1;
    let mut total = 0.0;
    let mut along = vec![0.0];
    for i in 1..=m {
        total += hyp(mid[i].x - mid[i - 1].x, mid[i].y - mid[i - 1].y);
        along.push(total);
    }
    let mut left = Vec::with_capacity(m + 1);
    let mut right = Vec::with_capacity(m + 1);
    for i in 0..=m {
        let t = unit_of(mid[i.saturating_sub(1)], mid[(i + 1).min(m)]);
        let h = hw * profile.at(if total != 0.0 { along[i] / total } else { 0.0 });
        left.push((mid[i].x - t.1 * h, mid[i].y + t.0 * h, h));
        right.push((mid[i].x + t.1 * h, mid[i].y - t.0 * h, h));
    }
    // An end with no width is one sharp point; an end with width has two corners.
    let mut out = Vec::with_capacity(2 * m + 4);
    if left[0].2 < 0.08 {
        out.push(sharp(mid[0].x, mid[0].y));
    } else {
        out.push(sharp(right[0].0, right[0].1));
        out.push(sharp(left[0].0, left[0].1));
    }
    for l in &left[1..m.max(1)] {
        out.push(pt(l.0, l.1));
    }
    if left[m].2 < 0.08 {
        out.push(sharp(mid[m].x, mid[m].y));
    } else {
        out.push(sharp(left[m].0, left[m].1));
        out.push(sharp(right[m].0, right[m].1));
    }
    for i in (1..m).rev() {
        out.push(pt(right[i].0, right[i].1));
    }
    out
}

/// A box as `[x0, y0, x1, y1]`.
pub type BoxF = [f64; 4];

pub fn box_of(pts: &[Pt]) -> BoxF {
    let mut b = [f64::INFINITY, f64::INFINITY, -f64::INFINITY, -f64::INFINITY];
    for p in pts {
        if p.x < b[0] {
            b[0] = p.x;
        }
        if p.x > b[2] {
            b[2] = p.x;
        }
        if p.y < b[1] {
            b[1] = p.y;
        }
        if p.y > b[3] {
            b[3] = p.y;
        }
    }
    b
}

pub fn join_box(a: Option<BoxF>, b: BoxF) -> BoxF {
    match a {
        None => b,
        Some(a) => [
            min(a[0], b[0]),
            min(a[1], b[1]),
            max(a[2], b[2]),
            max(a[3], b[3]),
        ],
    }
}

pub fn pad_box(b: BoxF, m: f64) -> BoxF {
    [b[0] - m, b[1] - m, b[2] + m, b[3] + m]
}

/// The corners of the square round a disc.
pub fn disc_pts(x: f64, y: f64, r: f64) -> [Pt; 4] {
    [
        pt(x - r, y - r),
        pt(x + r, y - r),
        pt(x + r, y + r),
        pt(x - r, y + r),
    ]
}

/// How wide a closed polyline is at one height, as a half-width about x = 0.
pub fn half_width_at(poly: &[Pt], y: f64) -> f64 {
    let mut w = 0.0;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        if (p.y - y) * (q.y - y) > 0.0 {
            continue;
        }
        let x = if p.y == q.y {
            max(p.x.abs(), q.x.abs())
        } else {
            (p.x + (q.x - p.x) * (y - p.y) / (q.y - p.y)).abs()
        };
        if x > w {
            w = x;
        }
    }
    w
}

/// How far a ray from (cx, cy) in the direction `d` runs before it leaves
/// a closed polyline.
pub fn ray_hit(poly: &[Pt], cx: f64, cy: f64, d: (f64, f64)) -> f64 {
    let mut far = 0.0;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        let ex = q.x - p.x;
        let ey = q.y - p.y;
        let den = d.0 * ey - d.1 * ex;
        if den.abs() < 1e-12 {
            continue;
        }
        let t = ((p.x - cx) * ey - (p.y - cy) * ex) / den;
        let s = ((p.x - cx) * d.1 - (p.y - cy) * d.0) / den;
        if t > far && (-1e-9..=1.0 + 1e-9).contains(&s) {
            far = t;
        }
    }
    far
}

/// Whether a point lies inside a closed polyline.
pub fn inside_poly(poly: &[Pt], p: Pt) -> bool {
    let mut odd = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (b.x - a.x) * (p.y - a.y) / (b.y - a.y) {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// One transform operation of the reference: a move, a clockwise turn in
/// degrees about (0, 0), or a scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    T(f64, f64),
    R(f64),
    S(f64, f64),
}

/// `[a, b, c, d, e, f]` with x' = a x + c y + e and y' = b x + d y + f.
pub type Matrix = [f64; 6];

const IDENT: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn mul_m(m: &Matrix, n: &Matrix) -> Matrix {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

pub fn apply_m(m: &Matrix, p: Pt) -> Pt {
    pt(
        m[0] * p.x + m[2] * p.y + m[4],
        m[1] * p.x + m[3] * p.y + m[5],
    )
}

/// Angles are kept to a hundredth of a degree, moves to a hundredth and
/// scales to a thousandth, in the matrix as in the drawing.
fn op_m(o: Op) -> Matrix {
    match o {
        Op::T(x, y) => [1.0, 0.0, 0.0, 1.0, round2(x), round2(y)],
        Op::R(d) => {
            let a = round2(d) * DEG;
            let c = cos(a);
            let s = sin(a);
            [c, s, -s, c, 0.0, 0.0]
        }
        Op::S(x, y) => [round3(x), 0.0, 0.0, round3(y), 0.0, 0.0],
    }
}

pub fn matrix_of(ops: &[Op]) -> Matrix {
    ops.iter().fold(IDENT, |m, &o| mul_m(&m, &op_m(o)))
}

pub fn through(m: &Matrix, pts: &[Pt]) -> Vec<Pt> {
    pts.iter().map(|&p| apply_m(m, p)).collect()
}
