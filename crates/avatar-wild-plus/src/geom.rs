//! Geometry kernels: polygons, splines, signed distances and the marching
//! squares tracer that welds masses into one mirror-symmetric silhouette.
//!
//! Every function mirrors the reference's arithmetic in the same
//! order, so the outlines agree to the last bit wherever the maths allows.

use avatar_svg::wild::math::{cos, sin};
use std::f64::consts::{PI, TAU};

/// A point in model or frame units.
pub type P = [f64; 2];

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

pub fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((bx - ax) * (bx - ax) + (by - ay) * (by - ay)).sqrt()
}

pub fn smooth01(t: f64) -> f64 {
    let t = clamp(t, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// JavaScript's `Math.min` over two numbers (no NaN reaches it here).
pub fn min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// JavaScript's `Math.max` over two numbers.
pub fn max(a: f64, b: f64) -> f64 {
    if b > a {
        b
    } else {
        a
    }
}

/// JavaScript's `Math.round`: halves go toward positive infinity.
pub fn js_round(v: f64) -> f64 {
    let f = v.floor();
    if v - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// JavaScript's `||` default for a number: zero (or NaN) becomes `fallback`.
pub fn or(v: f64, fallback: f64) -> f64 {
    if v == 0.0 || v.is_nan() {
        fallback
    } else {
        v
    }
}

pub fn rot(v: P, a: f64) -> P {
    let (c, s) = (cos(a), sin(a));
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

pub fn unit(x: f64, y: f64) -> P {
    let l = or((x * x + y * y).sqrt(), 1e-9);
    [x / l, y / l]
}

pub fn bbox_of(pts: &[P]) -> [f64; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    grow_box(&mut b, pts);
    b
}

pub fn grow_box(b: &mut [f64; 4], pts: &[P]) {
    for p in pts {
        if p[0] < b[0] {
            b[0] = p[0];
        }
        if p[1] < b[1] {
            b[1] = p[1];
        }
        if p[0] > b[2] {
            b[2] = p[0];
        }
        if p[1] > b[3] {
            b[3] = p[1];
        }
    }
}

pub fn centroid(poly: &[P]) -> P {
    let (mut a2, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let cr = p[0] * q[1] - q[0] * p[1];
        a2 += cr;
        cx += (p[0] + q[0]) * cr;
        cy += (p[1] + q[1]) * cr;
    }
    if a2.abs() < 1e-9 {
        let (mut sx, mut sy) = (0.0, 0.0);
        for p in poly {
            sx += p[0];
            sy += p[1];
        }
        return [sx / n as f64, sy / n as f64];
    }
    [cx / (3.0 * a2), cy / (3.0 * a2)]
}

/// Signed distance to a closed polygon: negative inside by winding number,
/// so a self-overlapping outline still counts as one solid shape.
pub fn poly_sdf(pts: &[P], x: f64, y: f64) -> f64 {
    let n = pts.len();
    let mut d2 = f64::INFINITY;
    let mut wn = 0i32;
    let mut p1 = pts[n - 1];
    for &p2 in pts {
        let (x1, y1) = (p1[0], p1[1]);
        let (ex, ey) = (p2[0] - x1, p2[1] - y1);
        let (px, py) = (x - x1, y - y1);
        let mut t = (px * ex + py * ey) / or(ex * ex + ey * ey, 1e-12);
        t = clamp(t, 0.0, 1.0);
        let (qx, qy) = (px - ex * t, py - ey * t);
        let q = qx * qx + qy * qy;
        if q < d2 {
            d2 = q;
        }
        let cross = ex * py - ey * px;
        if y1 <= y {
            if p2[1] > y && cross > 0.0 {
                wn += 1;
            }
        } else if p2[1] <= y && cross < 0.0 {
            wn -= 1;
        }
        p1 = p2;
    }
    (if wn != 0 { -1.0 } else { 1.0 }) * d2.sqrt()
}

/// Polynomial smooth minimum: shapes closer than `k` blend into each other.
pub fn smin(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return if a < b { a } else { b };
    }
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    b * (1.0 - h) + a * h - k * h * (1.0 - h)
}

/// Horizontal spans of a closed polygon at height `y`, left to right.
pub fn spans_at(outline: &[P], y: f64) -> Vec<[f64; 2]> {
    let mut xs = Vec::new();
    let n = outline.len();
    for i in 0..n {
        let (p, q) = (outline[i], outline[(i + 1) % n]);
        if (p[1] <= y && q[1] > y) || (q[1] <= y && p[1] > y) {
            xs.push(p[0] + (q[0] - p[0]) * (y - p[1]) / (q[1] - p[1]));
        }
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < xs.len() {
        out.push([xs[i], xs[i + 1]]);
        i += 2;
    }
    out
}

/// The span at height `y` that contains `x`, if any.
pub fn span_through(outline: &[P], y: f64, x: f64) -> Option<[f64; 2]> {
    spans_at(outline, y)
        .into_iter()
        .find(|s| s[0] <= x && x <= s[1])
}

fn ray_crossings(outline: &[P], ox: f64, oy: f64, dx: f64, dy: f64, far: bool) -> Option<f64> {
    let mut best: Option<f64> = None;
    let n = outline.len();
    for i in 0..n {
        let (x1, y1) = (outline[i][0] - ox, outline[i][1] - oy);
        let q = outline[(i + 1) % n];
        let (ex, ey) = (q[0] - ox - x1, q[1] - oy - y1);
        let den = dx * ey - dy * ex;
        if den.abs() < 1e-12 {
            continue;
        }
        let s = (dx * y1 - dy * x1) / den;
        if !(0.0..=1.0).contains(&s) {
            continue;
        }
        let (px, py) = (x1 + ex * s, y1 + ey * s);
        let t = if dx.abs() > dy.abs() {
            px / dx
        } else {
            py / dy
        };
        if t > 1e-9 {
            match best {
                None => best = Some(t),
                Some(b) if (far && t > b) || (!far && t < b) => best = Some(t),
                _ => {}
            }
        }
    }
    best
}

/// Nearest crossing of the ray from `(ox, oy)` along the unit `(dx, dy)`.
pub fn ray_hit(outline: &[P], ox: f64, oy: f64, dx: f64, dy: f64) -> Option<f64> {
    ray_crossings(outline, ox, oy, dx, dy, false)
}

/// Farthest crossing along the same ray: the outer skin, past any fold.
pub fn ray_hit_far(outline: &[P], ox: f64, oy: f64, dx: f64, dy: f64) -> Option<f64> {
    ray_crossings(outline, ox, oy, dx, dy, true)
}

/// Points evenly spaced by arc length along an open chain, ends included.
pub fn resample_open(pts: &[P], count: usize) -> Vec<P> {
    let mut cum = vec![0.0];
    for i in 1..pts.len() {
        cum.push(cum[i - 1] + dist(pts[i - 1][0], pts[i - 1][1], pts[i][0], pts[i][1]));
    }
    let total = cum[cum.len() - 1];
    let mut out = Vec::with_capacity(count);
    let mut seg = 0usize;
    for k in 0..count {
        let d = total * k as f64 / (count as f64 - 1.0);
        while seg + 2 < pts.len() && cum[seg + 1] < d {
            seg += 1;
        }
        let span = or(cum[seg + 1] - cum[seg], 1e-12);
        let t = clamp((d - cum[seg]) / span, 0.0, 1.0);
        out.push([
            lerp(pts[seg][0], pts[seg + 1][0], t),
            lerp(pts[seg][1], pts[seg + 1][1], t),
        ]);
    }
    out
}

pub fn chain_length(pts: &[P]) -> f64 {
    let mut total = 0.0;
    for i in 1..pts.len() {
        total += dist(pts[i - 1][0], pts[i - 1][1], pts[i][0], pts[i][1]);
    }
    total
}

/// A spline control point with an optional per-point tangent scale.
#[derive(Clone, Copy)]
pub struct Ctrl {
    pub x: f64,
    pub y: f64,
    pub tension: f64,
}

impl Ctrl {
    pub fn at(p: P) -> Self {
        Ctrl {
            x: p[0],
            y: p[1],
            tension: 1.0,
        }
    }
}

fn bezier(p1: (f64, f64), c1: (f64, f64), c2: (f64, f64), p2: (f64, f64), u: f64) -> P {
    let v = 1.0 - u;
    [
        v * v * v * p1.0 + 3.0 * v * v * u * c1.0 + 3.0 * v * u * u * c2.0 + u * u * u * p2.0,
        v * v * v * p1.1 + 3.0 * v * v * u * c1.1 + 3.0 * v * u * u * c2.1 + u * u * u * p2.1,
    ]
}

/// A closed Catmull-Rom spline through control points, sampled.
pub fn spline_closed(ctrl: &[Ctrl], per_segment: usize) -> Vec<P> {
    let n = ctrl.len();
    let mut out = Vec::with_capacity(n * per_segment);
    for i in 0..n {
        let (p0, p1, p2, p3) = (
            ctrl[(i + n - 1) % n],
            ctrl[i],
            ctrl[(i + 1) % n],
            ctrl[(i + 2) % n],
        );
        let (t1, t2) = (p1.tension, p2.tension);
        let c1 = (
            p1.x + (p2.x - p0.x) / 6.0 * t1,
            p1.y + (p2.y - p0.y) / 6.0 * t1,
        );
        let c2 = (
            p2.x - (p3.x - p1.x) / 6.0 * t2,
            p2.y - (p3.y - p1.y) / 6.0 * t2,
        );
        for s in 0..per_segment {
            let u = s as f64 / per_segment as f64;
            out.push(bezier((p1.x, p1.y), c1, c2, (p2.x, p2.y), u));
        }
    }
    out
}

pub fn spline_closed_pts(ctrl: &[P], per_segment: usize) -> Vec<P> {
    let c: Vec<Ctrl> = ctrl.iter().map(|&p| Ctrl::at(p)).collect();
    spline_closed(&c, per_segment)
}

/// An open Catmull-Rom spline through control points (ends repeated), sampled.
pub fn spline_open(ctrl: &[P], per_segment: usize) -> Vec<P> {
    let n = ctrl.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < n {
        let (p0, p1, p2, p3) = (
            ctrl[i.saturating_sub(1)],
            ctrl[i],
            ctrl[i + 1],
            ctrl[(i + 2).min(n - 1)],
        );
        let c1 = (p1[0] + (p2[0] - p0[0]) / 6.0, p1[1] + (p2[1] - p0[1]) / 6.0);
        let c2 = (p2[0] - (p3[0] - p1[0]) / 6.0, p2[1] - (p3[1] - p1[1]) / 6.0);
        for s in 0..per_segment {
            let u = s as f64 / per_segment as f64;
            out.push(bezier((p1[0], p1[1]), c1, c2, (p2[0], p2[1]), u));
        }
        i += 1;
    }
    out.push(ctrl[n - 1]);
    out
}

/// A mirror-symmetric blob around `(0, cy)`: an ellipse whose radius is
/// scaled by one factor per control direction, from the top of the axis down
/// the right side to the bottom; the left side mirrors.
pub fn blob_poly(cy: f64, rx: f64, ry: f64, factors: &[f64], per_segment: usize) -> Vec<P> {
    let m = factors.len();
    let mut right = Vec::with_capacity(m);
    for (j, f) in factors.iter().enumerate() {
        let a = -PI / 2.0 + PI * j as f64 / (m as f64 - 1.0);
        right.push([cos(a) * rx * f, cy + sin(a) * ry * f]);
    }
    let mut ctrl = right.clone();
    for j in (1..=m - 2).rev() {
        ctrl.push([-right[j][0], right[j][1]]);
    }
    spline_closed_pts(&ctrl, per_segment)
}

/// Squash everything below a ground line, as the wild body flattens a base.
pub fn flatten_base(pts: &mut [P], share: f64, squash: f64) {
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in pts.iter() {
        if p[1] < min_y {
            min_y = p[1];
        }
        if p[1] > max_y {
            max_y = p[1];
        }
    }
    let ground = max_y - (max_y - min_y) * share;
    for p in pts.iter_mut() {
        if p[1] > ground {
            p[1] = ground + (p[1] - ground) * squash;
        }
    }
}

pub fn flattened(mut pts: Vec<P>, share: f64, squash: f64) -> Vec<P> {
    flatten_base(&mut pts, share, squash);
    pts
}

/// One spoke of the wild body: angle, radius and tangent scale.
#[derive(Clone, Copy)]
pub struct Spoke {
    pub theta: f64,
    pub r: f64,
    pub tension: f64,
}

fn wrap_tau(t: f64) -> f64 {
    ((t % TAU) + TAU) % TAU
}

/// The wild body's control points: fold-mirrored, sorted, de-duplicated and
/// domesticated by `dom`.
pub fn prepared_points(spokes: &[Spoke], dom: f64) -> Vec<Ctrl> {
    let half: Vec<Spoke> = spokes
        .iter()
        .map(|s| {
            let mut t = wrap_tau(s.theta);
            if cos(t) < 0.0 {
                t = PI - t;
            }
            Spoke {
                theta: t,
                r: s.r,
                tension: s.tension,
            }
        })
        .collect();
    let mut all: Vec<Spoke> = half
        .iter()
        .copied()
        .chain(half.iter().map(|s| Spoke {
            theta: PI - s.theta,
            r: s.r,
            tension: s.tension,
        }))
        .map(|s| Spoke {
            theta: wrap_tau(s.theta),
            ..s
        })
        .collect();
    all.sort_by(|a, b| a.theta.total_cmp(&b.theta));
    let mut dedup: Vec<Spoke> = Vec::new();
    for s in all {
        if dedup.is_empty() || (s.theta - dedup[dedup.len() - 1].theta).abs() >= 1e-3 {
            dedup.push(s);
        }
    }
    let k = 0.55 * dom;
    let mut radii: Vec<f64> = dedup.iter().map(|s| s.r).collect();
    for _ in 0..2 {
        let prev = radii.clone();
        let n = prev.len();
        for i in 0..n {
            radii[i] = prev[i] * (1.0 - k) + (prev[(i + n - 1) % n] + prev[(i + 1) % n]) * 0.5 * k;
        }
    }
    let mut sum = 0.0;
    for r in &radii {
        sum += r;
    }
    let mean_r = sum / radii.len() as f64;
    let floor = mean_r * 0.55 * dom;
    for r in radii.iter_mut() {
        *r = max(*r, floor);
    }
    dedup
        .iter()
        .zip(&radii)
        .map(|(s, &r)| Ctrl {
            x: cos(s.theta) * r,
            y: sin(s.theta) * r,
            tension: s.tension + (1.0 - s.tension) * dom,
        })
        .collect()
}

/// The wild body outline in model space, stretched by `(sx, sy)`. `grounded`
/// squashes its base flat, as the wild body does for one that sits.
pub fn wild_body(spokes: &[Spoke], dom: f64, sx: f64, sy: f64, grounded: bool) -> Vec<P> {
    let mut pts = spline_closed(&prepared_points(spokes, dom), 9);
    if grounded {
        flatten_base(&mut pts, 0.14, 0.30 + 0.35 * dom);
    }
    for p in pts.iter_mut() {
        p[0] *= sx;
        p[1] *= sy;
    }
    pts
}

/// What a welded mass's signed distance is measured from.
#[derive(Clone)]
pub enum Sdf {
    Poly(Vec<P>),
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    Capsule {
        ax: f64,
        ay: f64,
        ex: f64,
        ey: f64,
        l2: f64,
        ra: f64,
        rb: f64,
    },
}

/// A welded mass: a signed distance, its bounding box (to skip it far away),
/// the radius over which it blends into what was welded before it, and
/// whether it also exists mirrored at -x.
#[derive(Clone)]
pub struct WShape {
    pub sdf: Sdf,
    pub bx: [f64; 4],
    pub k: f64,
    pub pair: bool,
}

impl WShape {
    pub fn eval(&self, x: f64, y: f64) -> f64 {
        match &self.sdf {
            Sdf::Poly(pts) => poly_sdf(pts, x, y),
            Sdf::Circle { cx, cy, r } => ((x - cx) * (x - cx) + (y - cy) * (y - cy)).sqrt() - r,
            Sdf::Capsule {
                ax,
                ay,
                ex,
                ey,
                l2,
                ra,
                rb,
            } => {
                let t = clamp(((x - ax) * ex + (y - ay) * ey) / l2, 0.0, 1.0);
                let (qx, qy) = (x - ax - ex * t, y - ay - ey * t);
                (qx * qx + qy * qy).sqrt() - (ra + (rb - ra) * t)
            }
        }
    }
}

pub fn poly_shape(pts: Vec<P>, k: f64, pair: bool) -> WShape {
    let bx = bbox_of(&pts);
    WShape {
        sdf: Sdf::Poly(pts),
        bx,
        k,
        pair,
    }
}

pub fn circle_shape(cx: f64, cy: f64, r: f64, k: f64, pair: bool) -> WShape {
    WShape {
        sdf: Sdf::Circle { cx, cy, r },
        bx: [cx - r, cy - r, cx + r, cy + r],
        k,
        pair,
    }
}

/// A round-ended bar from a to b whose radius runs from `ra` to `rb`.
#[allow(clippy::too_many_arguments)]
pub fn capsule_shape(
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    ra: f64,
    rb: f64,
    k: f64,
    pair: bool,
) -> WShape {
    let (ex, ey) = (bx - ax, by - ay);
    let l2 = or(ex * ex + ey * ey, 1e-12);
    let rm = max(ra, rb);
    WShape {
        sdf: Sdf::Capsule {
            ax,
            ay,
            ex,
            ey,
            l2,
            ra,
            rb,
        },
        bx: [
            min(ax, bx) - rm,
            min(ay, by) - rm,
            max(ax, bx) + rm,
            max(ay, by) + rm,
        ],
        k,
        pair,
    }
}

/// The welded field of a list of masses at one point.
pub fn field(shapes: &[WShape], x: f64, y: f64) -> f64 {
    let mut v = f64::INFINITY;
    for sh in shapes {
        let b = sh.bx;
        let ddy = if y < b[1] {
            b[1] - y
        } else if y > b[3] {
            y - b[3]
        } else {
            0.0
        };
        let mut ddx = if x < b[0] {
            b[0] - x
        } else if x > b[2] {
            x - b[2]
        } else {
            0.0
        };
        // A shape farther away than the blend radius cannot change the minimum.
        if v == f64::INFINITY {
            v = sh.eval(x, y);
        } else if (if ddx > ddy { ddx } else { ddy }) < v + sh.k {
            v = smin(v, sh.eval(x, y), sh.k);
        }
        if sh.pair {
            let mx = -x;
            ddx = if mx < b[0] {
                b[0] - mx
            } else if mx > b[2] {
                mx - b[2]
            } else {
                0.0
            };
            if (if ddx > ddy { ddx } else { ddy }) < v + sh.k {
                v = smin(v, sh.eval(mx, y), sh.k);
            }
        }
    }
    v
}

/// Traces the outer boundary of a mirror-symmetric field on the half plane
/// x >= 0 by marching squares, as a chain from the top of the axis round the
/// right side to the bottom of the axis. The field is evaluated only near the
/// boundary: a coarse cell whose corners all lie farther from it than the
/// cell's half diagonal cannot contain it.
pub fn trace_half(shapes: &[WShape], bx: [f64; 4], res: f64) -> Option<Vec<P>> {
    const C: usize = 4;
    let cf = C as f64;
    let h = max(bx[2], bx[3] - bx[1]) / res;
    let nx = C * (bx[2] / (cf * h)).ceil() as usize;
    let ny = C * ((bx[3] - bx[1]) / (cf * h)).ceil() as usize;
    let y0 = bx[1];
    let w = nx + 1;
    let mut memo = vec![f64::NAN; w * (ny + 1)];
    let val = |memo: &mut Vec<f64>, i: usize, j: usize| -> f64 {
        let q = j * w + i;
        let mut v = memo[q];
        if v.is_nan() {
            v = field(shapes, i as f64 * h, y0 + j as f64 * h);
            memo[q] = v;
        }
        v
    };
    let mut link_a = vec![-1i64; 2 * w * (ny + 1)];
    let mut link_b = vec![-1i64; 2 * w * (ny + 1)];
    let mut link = |a: usize, b: usize| {
        if link_a[a] < 0 {
            link_a[a] = b as i64;
        } else {
            link_b[a] = b as i64;
        }
        if link_a[b] < 0 {
            link_a[b] = a as i64;
        } else {
            link_b[b] = a as i64;
        }
    };
    let thr = cf * h * 0.7072;
    let mut jj = 0;
    while jj < ny {
        let mut ii = 0;
        while ii < nx {
            let a = val(&mut memo, ii, jj);
            let b = val(&mut memo, ii + C, jj);
            let c = val(&mut memo, ii + C, jj + C);
            let d = val(&mut memo, ii, jj + C);
            let far = min(min(a.abs(), b.abs()), min(c.abs(), d.abs())) > thr;
            if far && (a < 0.0) == (b < 0.0) && (b < 0.0) == (c < 0.0) && (c < 0.0) == (d < 0.0) {
                ii += C;
                continue;
            }
            for j in jj..jj + C {
                for i in ii..ii + C {
                    let v00 = val(&mut memo, i, j);
                    let v10 = val(&mut memo, i + 1, j);
                    let v11 = val(&mut memo, i + 1, j + 1);
                    let v01 = val(&mut memo, i, j + 1);
                    let kase = (v00 < 0.0) as u8
                        | ((v10 < 0.0) as u8) << 1
                        | ((v11 < 0.0) as u8) << 2
                        | ((v01 < 0.0) as u8) << 3;
                    if kase == 0 || kase == 15 {
                        continue;
                    }
                    let top = 2 * (j * w + i);
                    let left = top + 1;
                    let right = 2 * (j * w + i + 1) + 1;
                    let bottom = 2 * ((j + 1) * w + i);
                    match kase {
                        1 | 14 => link(left, top),
                        2 | 13 => link(top, right),
                        3 | 12 => link(left, right),
                        4 | 11 => link(right, bottom),
                        6 | 9 => link(top, bottom),
                        7 | 8 => link(left, bottom),
                        _ => {
                            // Two opposite corners inside: the cell's centre
                            // says whether they join.
                            let inside = (v00 + v10 + v11 + v01) / 4.0 < 0.0;
                            if (kase == 5) == inside {
                                link(top, right);
                                link(left, bottom);
                            } else {
                                link(left, top);
                                link(right, bottom);
                            }
                        }
                    }
                }
            }
            ii += C;
        }
        jj += C;
    }
    let vertex = |id: usize| -> P {
        let q = id >> 1;
        let i = q % w;
        let j = (q - i) / w;
        let fa = memo[q];
        if id & 1 == 1 {
            let fb = memo[q + w];
            return [i as f64 * h, y0 + (j as f64 + fa / (fa - fb)) * h];
        }
        let fb = memo[q + 1];
        [(i as f64 + fa / (fa - fb)) * h, y0 + j as f64 * h]
    };
    // Chains start and end where the boundary crosses the axis (column 0).
    let mut seen = vec![false; link_a.len()];
    let mut chains: Vec<Vec<P>> = Vec::new();
    for j in 0..ny {
        let start = 2 * (j * w) + 1;
        if link_a[start] < 0 || seen[start] {
            continue;
        }
        let mut pts = Vec::new();
        let mut prev: i64 = -1;
        let mut cur: i64 = start as i64;
        while cur >= 0 && !seen[cur as usize] {
            let c = cur as usize;
            seen[c] = true;
            pts.push(vertex(c));
            let next = if link_a[c] != prev {
                link_a[c]
            } else {
                link_b[c]
            };
            prev = cur;
            cur = next;
        }
        if pts.len() >= 2 {
            chains.push(pts);
        }
    }
    if chains.is_empty() {
        return None;
    }
    // The outer boundary is the chain whose two ends are farthest apart on
    // the axis: it runs from the crown of the body to its base.
    let mut best = 0;
    let mut best_span = -1.0;
    for (ci, c) in chains.iter().enumerate() {
        let span = (c[0][1] - c[c.len() - 1][1]).abs();
        if span > best_span {
            best = ci;
            best_span = span;
        }
    }
    let mut best = chains.swap_remove(best);
    if best[0][1] > best[best.len() - 1][1] {
        best.reverse();
    }
    best[0][0] = 0.0;
    let last = best.len() - 1;
    best[last][0] = 0.0;
    Some(best)
}

/// Smooths a half chain whose ends lie on the axis, as if its mirror image
/// continued it, so the mirrored outline has no kink where the halves meet.
pub fn smooth_half(pts: Vec<P>, passes: usize) -> Vec<P> {
    let mut cur = pts;
    for _ in 0..passes {
        let n = cur.len();
        let mut out = vec![[0.0; 2]; n];
        out[0] = [0.0, 0.5 * cur[0][1] + 0.5 * cur[1][1]];
        out[n - 1] = [0.0, 0.5 * cur[n - 1][1] + 0.5 * cur[n - 2][1]];
        for i in 1..n - 1 {
            out[i] = [
                cur[i][0] * 0.5 + (cur[i - 1][0] + cur[i + 1][0]) * 0.25,
                cur[i][1] * 0.5 + (cur[i - 1][1] + cur[i + 1][1]) * 0.25,
            ];
        }
        cur = out;
    }
    cur
}

/// The closed outline from a half chain: the right side, then its mirror.
pub fn mirror_half(half: &[P]) -> Vec<P> {
    let mut out = half.to_vec();
    for i in (1..half.len() - 1).rev() {
        out.push([-half[i][0], half[i][1]]);
    }
    out
}

/// Keeps the part of a polygon at or above the line `y = y_max`.
pub fn clip_below(poly: &[P], y_max: f64) -> Vec<P> {
    let mut out = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (ain, bin) = (a[1] <= y_max, b[1] <= y_max);
        if ain {
            out.push(a);
        }
        if ain != bin {
            let t = (y_max - a[1]) / (b[1] - a[1]);
            out.push([a[0] + (b[0] - a[0]) * t, y_max]);
        }
    }
    out
}

/// Every `step`-th point.
pub fn thin(pts: &[P], step: usize) -> Vec<P> {
    pts.iter().step_by(step).copied().collect()
}

pub fn poly_area(poly: &[P]) -> f64 {
    let mut a2 = 0.0;
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        a2 += p[0] * q[1] - q[0] * p[1];
    }
    a2.abs() / 2.0
}

/// One clean, mirror-symmetric loop around a set of masses.
pub fn traced_loop(shapes: &[WShape], res: f64, count: usize) -> Option<Vec<P>> {
    let (mut x1, mut y0, mut y1, mut k_max) = (0.0, f64::INFINITY, f64::NEG_INFINITY, 0.0);
    for sh in shapes {
        x1 = max(max(x1, sh.bx[2]), -sh.bx[0]);
        y0 = min(y0, sh.bx[1]);
        y1 = max(y1, sh.bx[3]);
        k_max = max(k_max, sh.k);
    }
    let pad = k_max + max(x1, y1 - y0) * 0.06;
    let half = trace_half(shapes, [0.0, y0 - pad, x1 + pad, y1 + pad], res)?;
    Some(mirror_half(&smooth_half(resample_open(&half, count), 1)))
}

/// A round shape with a scalloped edge, for fluffy things.
pub fn fluff_poly(cx: f64, cy: f64, r: f64, bumps: usize, depth: f64) -> Vec<P> {
    let mut pts = Vec::with_capacity(bumps * 5);
    for b in 0..bumps {
        for s in 0..5 {
            let a = TAU * (b as f64 + s as f64 / 5.0) / bumps as f64;
            // Half a sine per bump: 0 at the cusp, 1 at the crown.
            let lift = sin(PI * s as f64 / 5.0);
            let rr = r * (1.0 - depth + depth * lift);
            pts.push([cx + cos(a) * rr, cy + sin(a) * rr]);
        }
    }
    pts
}

/// An ellipse polygon whose `rx` runs along the unit vector `axis`.
pub fn ellipse_poly(cx: f64, cy: f64, rx: f64, ry: f64, axis: P, count: usize) -> Vec<P> {
    let mut pts = Vec::with_capacity(count);
    for i in 0..count {
        let a = TAU * i as f64 / count as f64;
        let (u, v) = (cos(a) * rx, sin(a) * ry);
        pts.push([
            cx + axis[0] * u - axis[1] * v,
            cy + axis[1] * u + axis[0] * v,
        ]);
    }
    pts
}
