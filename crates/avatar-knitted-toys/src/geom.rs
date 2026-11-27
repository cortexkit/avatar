//! Arithmetic and polygon helpers the generator shares.
//!
//! Every function here uses only `+ - * /`, `sqrt`, comparisons and the
//! polynomial sine, cosine and arctangent of `avatar_svg::wild::math`, so the
//! same genome gives the same numbers on every platform. The operation order
//! is the reference's, so the port and the reference compute the same values
//! to the last bit.

use avatar_svg::wild::math;

pub use std::f64::consts::{PI, TAU};

/// A point in toy units, y pointing down.
pub type Pt = [f64; 2];

pub fn sin(angle: f64) -> f64 {
    math::sin(angle)
}

pub fn cos(angle: f64) -> f64 {
    math::cos(angle)
}

pub fn atan2(y: f64, x: f64) -> f64 {
    math::atan2(y, x)
}

/// JavaScript's `Math.round`: the nearest integer, halves toward positive
/// infinity. Computed as ceil(x), less one when that is more than a half
/// above x, which is exact for every finite input.
pub fn js_round(x: f64) -> f64 {
    let r = x.ceil();
    if r - 0.5 > x {
        r - 1.0
    } else {
        r
    }
}

/// Round to the nearest integer with halves to even, as Rust's `{:.0}`
/// formatting rounds.
pub fn round_half_even(x: f64) -> f64 {
    let f = x.floor();
    let d = x - f;
    if d > 0.5 || (d == 0.5 && f % 2.0 != 0.0) {
        f + 1.0
    } else {
        f
    }
}

pub fn hyp(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

pub fn sq(x: f64) -> f64 {
    x * x
}

/// x to the power k/16 for x in [0, 1], by k - 1 multiplications and four
/// square roots: no maths-library power function.
pub fn pw16(x: f64, k: u32) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut y = x;
    for _ in 1..k {
        y *= x;
    }
    y.sqrt().sqrt().sqrt().sqrt()
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

/// `Math.min` of two numbers that are never NaN.
pub fn min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `Math.max` of two numbers that are never NaN.
pub fn max(a: f64, b: f64) -> f64 {
    if b > a {
        b
    } else {
        a
    }
}

pub fn min_of(values: impl IntoIterator<Item = f64>) -> f64 {
    values.into_iter().fold(f64::INFINITY, min)
}

pub fn max_of(values: impl IntoIterator<Item = f64>) -> f64 {
    values.into_iter().fold(f64::NEG_INFINITY, max)
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

pub fn add(a: Pt, b: Pt) -> Pt {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn sub(a: Pt, b: Pt) -> Pt {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn scl(a: Pt, k: f64) -> Pt {
    [a[0] * k, a[1] * k]
}

pub fn dist(a: Pt, b: Pt) -> f64 {
    hyp(a[0] - b[0], a[1] - b[1])
}

pub fn mixp(a: Pt, b: Pt, t: f64) -> Pt {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// The direction of `a`; a zero vector is returned unchanged.
pub fn unit(a: Pt) -> Pt {
    let mut l = hyp(a[0], a[1]);
    if l == 0.0 || l.is_nan() {
        l = 1.0;
    }
    [a[0] / l, a[1] / l]
}

pub fn perp(a: Pt) -> Pt {
    [-a[1], a[0]]
}

pub fn flip_x(p: Pt) -> Pt {
    [-p[0], p[1]]
}

pub fn dir_of(angle: f64) -> Pt {
    [cos(angle), sin(angle)]
}

/// `p` turned by `angle` about `c` (the origin when `None`).
pub fn rot(p: Pt, angle: f64, c: Option<Pt>) -> Pt {
    let s = sin(angle);
    let co = cos(angle);
    let (cx, cy) = c.map_or((0.0, 0.0), |c| (c[0], c[1]));
    let x = p[0] - cx;
    let y = p[1] - cy;
    [cx + x * co - y * s, cy + x * s + y * co]
}

/// The bounding box `[x0, y0, x1, y1]` of several point lists.
pub fn bbox_of<'a>(lists: impl IntoIterator<Item = &'a [Pt]>) -> [f64; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for list in lists {
        for p in list {
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
    }
    b
}

/// Signed distance from `p` to a closed polygon: negative inside, by the
/// nonzero winding rule SVG fills and clips with.
pub fn sdist(p: Pt, poly: &[Pt]) -> f64 {
    let [x, y] = p;
    let mut d2 = f64::INFINITY;
    let mut wn = 0_i32;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let ex = b[0] - a[0];
        let ey = b[1] - a[1];
        let px = x - a[0];
        let py = y - a[1];
        let mut den = ex * ex + ey * ey;
        if den == 0.0 {
            den = 1e-12;
        }
        let t = clamp((px * ex + py * ey) / den, 0.0, 1.0);
        let qx = px - ex * t;
        let qy = py - ey * t;
        d2 = min(d2, qx * qx + qy * qy);
        let cr = ex * py - ey * px;
        if a[1] <= y && b[1] > y && cr > 0.0 {
            wn += 1;
        } else if b[1] <= y && a[1] > y && cr < 0.0 {
            wn -= 1;
        }
    }
    (if wn != 0 { -1.0 } else { 1.0 }) * d2.sqrt()
}

/// Every x where a closed polygon crosses the line at height y, sorted.
pub fn crossings(poly: &[Pt], y: f64) -> Vec<f64> {
    let mut xs = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if (a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y) {
            xs.push(a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]));
        }
    }
    xs.sort_by(|p, q| p.total_cmp(q));
    xs
}

pub fn poly_len(p: &[Pt], closed: bool) -> f64 {
    let mut s = 0.0;
    for i in 1..p.len() {
        s += dist(p[i - 1], p[i]);
    }
    if closed && p.len() > 1 {
        s += dist(p[p.len() - 1], p[0]);
    }
    s
}

/// Points every `step` along a polyline, closed or open.
pub fn resample(p: &[Pt], step: f64, closed: bool) -> Vec<Pt> {
    let mut pts = p.to_vec();
    if closed {
        pts.push(p[0]);
    }
    let total = poly_len(&pts, false);
    let n = max(if closed { 8.0 } else { 2.0 }, js_round(total / step)) as usize;
    let mut out = Vec::new();
    let mut seg = 0;
    let mut acc = 0.0;
    let count = if closed { n } else { n + 1 };
    for k in 0..count {
        let d = total * k as f64 / n as f64;
        while seg + 2 < pts.len() && acc + dist(pts[seg], pts[seg + 1]) < d {
            acc += dist(pts[seg], pts[seg + 1]);
            seg += 1;
        }
        let mut l = dist(pts[seg], pts[seg + 1]);
        if l == 0.0 {
            l = 1e-9;
        }
        out.push(mixp(pts[seg], pts[seg + 1], clamp((d - acc) / l, 0.0, 1.0)));
    }
    out
}

/// Outward normals of a closed polygon drawn clockwise on screen (y down).
pub fn closed_normals(p: &[Pt]) -> Vec<Pt> {
    let n = p.len();
    (0..n)
        .map(|i| {
            let a = p[(i + n - 1) % n];
            let b = p[(i + 1) % n];
            let t = unit(sub(b, a));
            [t[1], -t[0]]
        })
        .collect()
}

pub fn signed_area(p: &[Pt]) -> f64 {
    let mut s = 0.0;
    let n = p.len();
    for i in 0..n {
        let a = p[i];
        let b = p[(i + 1) % n];
        s += a[0] * b[1] - b[0] * a[1];
    }
    s / 2.0
}

/// Catmull-Rom through `c`, sampled `per` points per span.
pub fn spline(c: &[Pt], closed: bool, per: usize) -> Vec<Pt> {
    let n = c.len() as i64;
    let get = |i: i64| -> Pt {
        if closed {
            c[(((i % n) + n) % n) as usize]
        } else {
            c[i.clamp(0, n - 1) as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::new();
    for i in 0..spans {
        let p0 = get(i - 1);
        let p1 = get(i);
        let p2 = get(i + 1);
        let p3 = get(i + 2);
        for k in 0..per {
            let t = k as f64 / per as f64;
            let t2 = t * t;
            let t3 = t2 * t;
            let at = |j: usize| {
                0.5 * (2.0 * p1[j]
                    + (-p0[j] + p2[j]) * t
                    + (2.0 * p0[j] - 5.0 * p1[j] + 4.0 * p2[j] - p3[j]) * t2
                    + (-p0[j] + 3.0 * p1[j] - 3.0 * p2[j] + p3[j]) * t3)
            };
            out.push([at(0), at(1)]);
        }
    }
    if !closed {
        out.push(c[c.len() - 1]);
    }
    out
}

/// A quadratic Bézier sampled at n + 1 points.
pub fn quad(p0: Pt, p1: Pt, p2: Pt, n: usize) -> Vec<Pt> {
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            [
                u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
            ]
        })
        .collect()
}

/// n points round an ellipse, turned by `tilt` when it is not zero.
pub fn ellipse_pts(c: Pt, rx: f64, ry: f64, n: usize, tilt: f64) -> Vec<Pt> {
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * TAU;
            let mut p = [rx * cos(a), ry * sin(a)];
            if tilt != 0.0 {
                p = rot(p, tilt, None);
            }
            [c[0] + p[0], c[1] + p[1]]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_round_rounds_halves_up() {
        assert_eq!(js_round(0.5), 1.0);
        assert_eq!(js_round(-0.5), 0.0);
        assert_eq!(js_round(-1.5), -1.0);
        assert_eq!(js_round(2.4999), 2.0);
        assert_eq!(js_round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(js_round(-2.6), -3.0);
    }

    #[test]
    fn round_half_even_matches_rust_formatting() {
        for x in [0.5, 1.5, 2.5, 3.5, 2.4, 2.6, 359.5] {
            assert_eq!(
                round_half_even(x),
                format!("{x:.0}").parse::<f64>().unwrap()
            );
        }
    }
}
