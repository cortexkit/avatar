//! Outlines, paths, boxes and transforms, as the reference works them out.
//!
//! Paths are kept in frame units until they become tree shapes; then every
//! coordinate is rounded to a tenth of a unit, as the reference writes it.

use avatar_svg::{Bam16, FillRule, Segment, Shape, Subpath, TransformOp, Q32};

use crate::kit::{cos, hyp, js_round, max, min, mix, sin, DEG, P2};

/// A corner of an outline: where it is, how far from the corner the
/// rounding starts along the edge that arrives (`cin`), and along the edge
/// that leaves (`cout`, the same as `cin` when unset).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cp {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) cin: f64,
    pub(crate) cout: Option<f64>,
}

/// A corner cut the same along both edges.
pub(crate) const fn cp(x: f64, y: f64, cut: f64) -> Cp {
    Cp {
        x,
        y,
        cin: cut,
        cout: None,
    }
}

/// A corner cut differently along the edge that arrives and the one that leaves.
pub(crate) const fn cp2(x: f64, y: f64, cin: f64, cout: f64) -> Cp {
    Cp {
        x,
        y,
        cin,
        cout: Some(cout),
    }
}

/// A corner joined to a point given as a pair, as `[...p, cut]` is.
pub(crate) const fn cpp(p: P2, cut: f64) -> Cp {
    cp(p[0], p[1], cut)
}

/// The handle length that makes a cubic follow a quarter of an ellipse.
const KAPPA: f64 = 0.5523;

fn or_tiny(v: f64) -> f64 {
    if v == 0.0 || v.is_nan() {
        1e-9
    } else {
        v
    }
}

struct Corner {
    p: P2,
    a: P2,
    b: P2,
    round: bool,
}

fn corners_of(pts: &[Cp]) -> Vec<Corner> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let p = pts[i];
            let a = pts[(i + n - 1) % n];
            let b = pts[(i + 1) % n];
            let la = or_tiny(hyp(a.x - p.x, a.y - p.y));
            let lb = or_tiny(hyp(b.x - p.x, b.y - p.y));
            let cin = min(p.cin, la * 0.5);
            let cout = min(p.cout.unwrap_or(p.cin), lb * 0.5);
            Corner {
                p: [p.x, p.y],
                a: [p.x + (a.x - p.x) / la * cin, p.y + (a.y - p.y) / la * cin],
                b: [p.x + (b.x - p.x) / lb * cout, p.y + (b.y - p.y) / lb * cout],
                round: cin > 0.05 && cout > 0.05,
            }
        })
        .collect()
}

/// The closed outline of a polygon whose corners are cut and rounded.
pub(crate) fn poly_path(pts: &[Cp]) -> Path {
    let cs = corners_of(pts);
    let mut d = Path::new();
    d.move_to(cs[0].b);
    for i in 1..=cs.len() {
        let c = &cs[i % cs.len()];
        d.line_to(c.a);
        if c.round {
            d.cubic_to(mix(c.a, c.p, KAPPA), mix(c.b, c.p, KAPPA), c.b);
        } else if c.a[0] != c.b[0] || c.a[1] != c.b[1] {
            d.line_to(c.b);
        }
    }
    d.close();
    d
}

/// The same outline as a dense closed polyline, `per` points to a rounded corner.
pub(crate) fn poly_sample(pts: &[Cp], per: u32) -> Vec<P2> {
    let mut out = Vec::new();
    for c in corners_of(pts) {
        if !c.round {
            out.push(c.a);
            if c.a[0] != c.b[0] || c.a[1] != c.b[1] {
                out.push(c.b);
            }
            continue;
        }
        let c1 = mix(c.a, c.p, KAPPA);
        let c2 = mix(c.b, c.p, KAPPA);
        for k in 0..=per {
            let t = f64::from(k) / f64::from(per);
            let u = 1.0 - t;
            let a = u * u * u;
            let b = 3.0 * u * u * t;
            let e = 3.0 * u * t * t;
            let g = t * t * t;
            out.push([
                a * c.a[0] + b * c1[0] + e * c2[0] + g * c.b[0],
                a * c.a[1] + b * c1[1] + e * c2[1] + g * c.b[1],
            ]);
        }
    }
    out
}

/// A four-sided outline, mirror-symmetric, standing on `y1` with its top at
/// `y0`: half-widths at the bottom and the top, then how far each corner is
/// cut along the horizontal edge and along the side.
#[allow(clippy::too_many_arguments)]
pub(crate) fn quad(
    y0: f64,
    y1: f64,
    wb: f64,
    wt: f64,
    bx: f64,
    by: f64,
    tx: f64,
    ty: f64,
) -> Vec<Cp> {
    vec![
        cp2(-wb, y1, bx, by),
        cp2(-wt, y0, ty, tx),
        cp2(wt, y0, tx, ty),
        cp2(wb, y1, by, bx),
    ]
}

/// A rectangle with rounded corners.
pub(crate) fn rrect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Vec<Cp> {
    vec![cp(x0, y1, r), cp(x0, y0, r), cp(x1, y0, r), cp(x1, y1, r)]
}

pub(crate) fn shift_pts(pts: &[Cp], dx: f64, dy: f64) -> Vec<Cp> {
    pts.iter()
        .map(|p| Cp {
            x: p.x + dx,
            y: p.y + dy,
            ..*p
        })
        .collect()
}

/// One piece of path data.
#[derive(Clone, Copy, Debug)]
enum Seg {
    M(P2),
    L(P2),
    C(P2, P2, P2),
}

#[derive(Clone, Debug)]
struct Sub {
    segs: Vec<Seg>,
    closed: bool,
}

/// Path data in frame units: one or more subpaths, each opened by a move.
#[derive(Clone, Debug, Default)]
pub(crate) struct Path {
    subs: Vec<Sub>,
}

impl Path {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn move_to(&mut self, p: P2) {
        self.subs.push(Sub {
            segs: vec![Seg::M(p)],
            closed: false,
        });
    }

    fn last(&mut self) -> &mut Sub {
        self.subs.last_mut().expect("a path segment follows a move")
    }

    pub(crate) fn line_to(&mut self, p: P2) {
        self.last().segs.push(Seg::L(p));
    }

    pub(crate) fn cubic_to(&mut self, c1: P2, c2: P2, p: P2) {
        self.last().segs.push(Seg::C(c1, c2, p));
    }

    /// A curve from `p0` to `p2` that bends towards `p1`: the quadratic,
    /// written as the cubic it equals.
    pub(crate) fn quad_to(&mut self, p0: P2, p1: P2, p2: P2) {
        self.cubic_to(mix(p0, p1, 2.0 / 3.0), mix(p2, p1, 2.0 / 3.0), p2);
    }

    pub(crate) fn close(&mut self) {
        self.last().closed = true;
    }

    /// Appends another path's subpaths, as joining two path strings does.
    pub(crate) fn append(&mut self, other: Path) {
        self.subs.extend(other.subs);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }

    /// The tree shape of this path, or `None` when nothing in it draws.
    pub(crate) fn shape(&self, fill_rule: FillRule) -> Option<Shape> {
        let subs: Vec<&Sub> = self.subs.iter().filter(|s| s.segs.len() > 1).collect();
        let last = subs.last()?;
        let segments =
            |sub: &Sub| -> Vec<Segment> { sub.segs.iter().map(|s| segment(*s)).collect() };
        if subs[..subs.len() - 1].iter().all(|s| !s.closed) {
            Some(Shape::Path {
                segments: subs.iter().flat_map(|s| segments(s)).collect(),
                closed: last.closed,
                fill_rule,
            })
        } else {
            Some(Shape::CompoundPath {
                subpaths: subs
                    .iter()
                    .map(|s| Subpath {
                        segments: segments(s),
                        closed: s.closed,
                    })
                    .collect(),
                fill_rule,
            })
        }
    }
}

/// A coordinate in the tree's hundredths of a unit, rounded to the tenth
/// of a unit the reference writes.
pub(crate) fn h1(v: f64) -> i64 {
    (js_round(v * 10.0) * 10.0) as i64
}

/// A length rounded to the hundredth of a unit the reference writes.
pub(crate) fn h2(v: f64) -> i64 {
    js_round(v * 100.0) as i64
}

fn segment(s: Seg) -> Segment {
    match s {
        Seg::M(p) => Segment::MoveTo {
            x: h1(p[0]),
            y: h1(p[1]),
        },
        Seg::L(p) => Segment::LineTo {
            x: h1(p[0]),
            y: h1(p[1]),
        },
        Seg::C(a, b, p) => Segment::CubicTo {
            cp1x: h1(a[0]),
            cp1y: h1(a[1]),
            cp2x: h1(b[0]),
            cp2y: h1(b[1]),
            x: h1(p[0]),
            y: h1(p[1]),
        },
    }
}

/// A polyline through `pts`, closed if asked.
pub(crate) fn line_path(pts: &[P2], closed: bool) -> Path {
    let mut d = Path::new();
    d.move_to(pts[0]);
    for p in &pts[1..] {
        d.line_to(*p);
    }
    if closed {
        d.close();
    }
    d
}

/// A point on the quadratic from `p0` to `p2` that bends towards `p1`.
pub(crate) fn quad_at(p0: P2, p1: P2, p2: P2, t: f64) -> P2 {
    let u = 1.0 - t;
    [
        u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
        u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
    ]
}

/// A box as `[x0, y0, x1, y1]`.
pub(crate) type Bx = [f64; 4];

pub(crate) fn box_of(pts: &[P2]) -> Bx {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in pts {
        if p[0] < b[0] {
            b[0] = p[0];
        }
        if p[0] > b[2] {
            b[2] = p[0];
        }
        if p[1] < b[1] {
            b[1] = p[1];
        }
        if p[1] > b[3] {
            b[3] = p[1];
        }
    }
    b
}

pub(crate) fn join_box(a: Option<Bx>, b: Option<Bx>) -> Option<Bx> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some([
            min(a[0], b[0]),
            min(a[1], b[1]),
            max(a[2], b[2]),
            max(a[3], b[3]),
        ]),
    }
}

pub(crate) fn pad_box(b: Bx, m: f64) -> Bx {
    [b[0] - m, b[1] - m, b[2] + m, b[3] + m]
}

/// The corner points of the box round a disc, for measuring.
pub(crate) fn disc_pts(x: f64, y: f64, r: f64) -> [P2; 4] {
    [
        [x - r, y - r],
        [x + r, y - r],
        [x + r, y + r],
        [x - r, y + r],
    ]
}

/// How wide a closed polyline is at one height, as a half-width about x = 0.
pub(crate) fn half_width_at(poly: &[P2], y: f64) -> f64 {
    let mut w = 0.0;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        if (p[1] - y) * (q[1] - y) > 0.0 {
            continue;
        }
        let x = if p[1] == q[1] {
            max(p[0].abs(), q[0].abs())
        } else {
            (p[0] + (q[0] - p[0]) * (y - p[1]) / (q[1] - p[1])).abs()
        };
        if x > w {
            w = x;
        }
    }
    w
}

/// How high a closed polyline reaches at one x.
pub(crate) fn top_at(poly: &[P2], x: f64) -> f64 {
    let mut y = f64::INFINITY;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        if (p[0] - x) * (q[0] - x) > 0.0 {
            continue;
        }
        let v = if p[0] == q[0] {
            min(p[1], q[1])
        } else {
            p[1] + (q[1] - p[1]) * (x - p[0]) / (q[0] - p[0])
        };
        if v < y {
            y = v;
        }
    }
    y
}

/// An affine map `[a, b, c, d, e, f]`: x' = a x + c y + e, y' = b x + d y + f.
pub(crate) type M6 = [f64; 6];
pub(crate) const IDENT: M6 = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub(crate) fn mul_m(m: M6, n: M6) -> M6 {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

pub(crate) fn apply_m(m: M6, p: P2) -> P2 {
    [
        m[0] * p[0] + m[2] * p[1] + m[4],
        m[1] * p[0] + m[3] * p[1] + m[5],
    ]
}

pub(crate) fn through(m: M6, pts: &[P2]) -> Vec<P2> {
    pts.iter().map(|p| apply_m(m, *p)).collect()
}

/// One transform operation as the reference writes it: a move, a clockwise
/// turn in degrees about the origin, or a scale.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Op {
    T(f64, f64),
    R(f64),
    S(f64, f64),
}

/// Angles are kept to a hundredth of a degree, scales to a thousandth and
/// moves to a hundredth of a unit, in the drawing and in what is measured.
fn deg2(v: f64) -> f64 {
    js_round(v * 100.0) / 100.0
}
fn sc3(v: f64) -> f64 {
    js_round(v * 1000.0) / 1000.0
}
fn pos2(v: f64) -> f64 {
    js_round(v * 100.0) / 100.0
}

fn op_m(o: Op) -> M6 {
    match o {
        Op::T(x, y) => [1.0, 0.0, 0.0, 1.0, pos2(x), pos2(y)],
        Op::R(deg) => {
            let a = deg2(deg) * DEG;
            let c = cos(a);
            let s = sin(a);
            [c, s, -s, c, 0.0, 0.0]
        }
        Op::S(x, y) => [sc3(x), 0.0, 0.0, sc3(y), 0.0, 0.0],
    }
}

pub(crate) fn matrix_of(ops: &[Op]) -> M6 {
    ops.iter().fold(IDENT, |m, o| mul_m(m, op_m(*o)))
}

/// The tree operations for `ops`, leaving out those the reference leaves
/// out of its text because they do nothing at its precision.
pub(crate) fn tree_ops(ops: &[Op]) -> Vec<TransformOp> {
    let mut out = Vec::new();
    for o in ops {
        match *o {
            Op::T(x, y) => {
                let (tx, ty) = (js_round(x * 100.0) as i64, js_round(y * 100.0) as i64);
                if tx != 0 || ty != 0 {
                    out.push(TransformOp::Translate { tx, ty });
                }
            }
            Op::R(deg) => {
                let hundredths = js_round(deg * 100.0);
                if hundredths != 0.0 {
                    let bam = (hundredths * 65_536.0 / 36_000.0).round() as i64;
                    out.push(TransformOp::Rotate {
                        angle: Bam16(bam.rem_euclid(65_536) as u16),
                        cx: 0,
                        cy: 0,
                    });
                }
            }
            Op::S(x, y) => {
                let (sx, sy) = (js_round(x * 1000.0) as i64, js_round(y * 1000.0) as i64);
                if sx != 1000 || sy != 1000 {
                    out.push(TransformOp::Scale {
                        sx: thousandths_q32(sx),
                        sy: thousandths_q32(sy),
                    });
                }
            }
        }
    }
    out
}

/// A number of thousandths as the nearest Q32.32 value.
fn thousandths_q32(k: i64) -> Q32 {
    let num = i128::from(k) << 32;
    let half = if num < 0 { -500 } else { 500 };
    Q32(((num + half) / 1000) as i64)
}

/// The direction a turn of `degrees` from straight up points in.
pub(crate) fn up_dir(degrees: f64) -> P2 {
    [sin(degrees * DEG), -cos(degrees * DEG)]
}

/// A point `along` units from `o` in the direction `u`, and `across` units
/// to the right of it.
pub(crate) fn place(o: P2, u: P2, across: f64, along: f64) -> P2 {
    [
        o[0] + u[0] * along - u[1] * across,
        o[1] + u[1] * along + u[0] * across,
    ]
}

pub(crate) fn unit_of(a: P2, b: P2) -> P2 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let l = or_tiny(hyp(dx, dy));
    [dx / l, dy / l]
}
