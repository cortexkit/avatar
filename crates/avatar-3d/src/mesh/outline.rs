//! Simple polygons: validation, winding, mitred insets and triangulation.

use super::{count, dot2, length, orient, sub2, unit2, unit_circle, MeshError, CREASE_COS};

/// A simple polygon in the plane, checked and normalised for meshing.
///
/// The accepted domain: at least three finite points forming a simple polygon
/// of either winding; concave outlines are allowed. A last point equal to the
/// first is dropped as a repeated closing point. Anything else is rejected,
/// never repaired: consecutive equal points, an edge that crosses or touches
/// another, an edge that doubles back along the one before it, and an outline
/// with no area. Three or more consecutive collinear points are allowed.
///
/// The kept points wind counter-clockwise (positive area with x right and y
/// up). A clockwise input is reversed with its first point kept first.
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    points: Vec<[f64; 2]>,
}

impl Outline {
    /// Checks and normalises an outline; see the type's documentation.
    pub fn new(points: &[[f64; 2]]) -> Result<Self, MeshError> {
        let mut points = points.to_vec();
        if points.iter().flatten().any(|v| !v.is_finite()) {
            return Err(MeshError::NonFinite);
        }
        if points.len() >= 2 && points[0] == points[points.len() - 1] {
            points.pop();
        }
        let area = simple_area(&points)?;
        if area < 0.0 {
            points[1..].reverse();
        }
        Ok(Self { points })
    }

    /// The normalised points, counter-clockwise, without a closing repeat.
    pub fn points(&self) -> &[[f64; 2]] {
        &self.points
    }

    /// The enclosed area, always positive.
    pub fn area(&self) -> f64 {
        signed_area(&self.points)
    }

    /// The outline inset by `distance`: every edge moved inward by that
    /// distance along its normal, each corner mitred. This is the front cap
    /// of an extrusion whose bevel is `distance`.
    ///
    /// The inset must fit: every edge keeps its direction and at least a
    /// quarter of its length, and the result stays simple. Otherwise, or for
    /// a negative distance, this is an error.
    pub fn inset(&self, distance: f64) -> Result<Outline, MeshError> {
        let distance = length(distance, "inset distance", 0.0, true)?;
        let shape = Insets::new(&self.points);
        if !shape.fits(&[distance]) {
            return Err(MeshError::BevelTooWide);
        }
        Ok(Outline {
            points: shape.at(distance),
        })
    }

    /// The widest bevel an extrusion of this outline accepts with
    /// `bevel_steps` steps: the largest value, found by bisection, at which
    /// every bevel ring's inset fits as [`Outline::inset`] requires. Zero when
    /// `bevel_steps` is out of range. An extrusion also needs twice the bevel
    /// to stay below its depth.
    pub fn max_bevel(&self, bevel_steps: u32) -> f64 {
        let Ok(steps) = count(bevel_steps, "bevel steps", 1, 1) else {
            return 0.0;
        };
        let shape = Insets::new(&self.points);
        let rings = bevel_rings(steps);
        let fits = |bevel: f64| shape.fits(&ring_offsets(&rings, bevel));
        // Each edge's length along its own direction shrinks linearly with the
        // inset, so the quarter-length rule gives an upper bound directly.
        let n = self.points.len();
        let mut upper = f64::INFINITY;
        for i in 0..n {
            let edge = sub2(self.points[(i + 1) % n], self.points[i]);
            let rate = dot2(sub2(shape.miters[(i + 1) % n], shape.miters[i]), edge);
            if rate > 0.0 {
                upper = upper.min(0.75 * dot2(edge, edge) / rate);
            }
        }
        if !upper.is_finite() {
            return 0.0;
        }
        if fits(upper) {
            return upper;
        }
        let (mut low, mut high) = (0.0, upper);
        for _ in 0..64 {
            let middle = 0.5 * (low + high);
            if fits(middle) {
                low = middle;
            } else {
                high = middle;
            }
        }
        low
    }

    /// Whether every inset offset fits, as [`Outline::inset`] requires.
    pub(super) fn fits(&self, offsets: &[f64]) -> bool {
        Insets::new(&self.points).fits(offsets)
    }
}

/// The sine and cosine of each bevel ring's angle round its quarter-round,
/// from the cap (0) to the side wall (a quarter turn).
pub(super) fn bevel_rings(steps: usize) -> Vec<[f64; 2]> {
    unit_circle(4 * steps)[..=steps].to_vec()
}

/// Inset of each bevel ring: `bevel` at the cap down to zero at the wall.
pub(super) fn ring_offsets(rings: &[[f64; 2]], bevel: f64) -> Vec<f64> {
    rings.iter().map(|[s, _]| bevel * (1.0 - s)).collect()
}

/// An outline's edge normals and corner mitres, from which its insets follow.
pub(super) struct Insets<'a> {
    points: &'a [[f64; 2]],
    /// Outward unit normal of edge i, from point i to point i + 1.
    pub normals: Vec<[f64; 2]>,
    /// The vector a corner moves inward per unit inset: it meets both edges'
    /// inset lines.
    pub miters: Vec<[f64; 2]>,
}

impl<'a> Insets<'a> {
    pub fn new(points: &'a [[f64; 2]]) -> Self {
        let n = points.len();
        let normals: Vec<[f64; 2]> = (0..n)
            .map(|i| {
                let e = sub2(points[(i + 1) % n], points[i]);
                unit2([e[1], -e[0]])
            })
            .collect();
        let miters = (0..n)
            .map(|i| {
                let (a, b) = (normals[(i + n - 1) % n], normals[i]);
                let k = 1.0 + dot2(a, b);
                [(a[0] + b[0]) / k, (a[1] + b[1]) / k]
            })
            .collect();
        Self {
            points,
            normals,
            miters,
        }
    }

    /// Whether the corner at point i turns gently enough to share a normal.
    pub fn smooth(&self, i: usize) -> bool {
        let n = self.points.len();
        dot2(self.normals[(i + n - 1) % n], self.normals[i]) >= CREASE_COS
    }

    /// The outline inset by `offset`.
    pub fn at(&self, offset: f64) -> Vec<[f64; 2]> {
        if offset == 0.0 {
            return self.points.to_vec();
        }
        self.points
            .iter()
            .zip(&self.miters)
            .map(|(p, m)| [p[0] - offset * m[0], p[1] - offset * m[1]])
            .collect()
    }

    /// Whether every offset gives a fitting inset: each edge keeps its
    /// direction and at least a quarter of its length, and the ring is simple.
    pub fn fits(&self, offsets: &[f64]) -> bool {
        let n = self.points.len();
        let deepest = offsets.iter().copied().fold(0.0, f64::max);
        // Edge length along its own direction is linear in the offset, so
        // checking the deepest ring covers every shallower one.
        let ring = self.at(deepest);
        for i in 0..n {
            let edge = sub2(self.points[(i + 1) % n], self.points[i]);
            let inset = sub2(ring[(i + 1) % n], ring[i]);
            let kept = dot2(inset, edge);
            if kept.is_nan() || kept < 0.25 * dot2(edge, edge) {
                return false;
            }
        }
        offsets
            .iter()
            .filter(|offset| **offset > 0.0)
            .all(|offset| simple_area(&self.at(*offset)).is_ok_and(|area| area > 0.0))
    }
}

/// Checks that `points` form a simple polygon and returns its signed area.
pub(super) fn simple_area(points: &[[f64; 2]]) -> Result<f64, MeshError> {
    let n = points.len();
    if n < 3 {
        return Err(MeshError::TooFewPoints { needed: 3 });
    }
    if points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(MeshError::NonFinite);
    }
    for i in 0..n {
        if points[i] == points[(i + 1) % n] {
            return Err(MeshError::RepeatedPoint { index: (i + 1) % n });
        }
    }
    for i in 0..n {
        let (a, b, c) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
        if orient(a, b, c) == 0.0 && dot2(sub2(b, a), sub2(c, b)) < 0.0 {
            return Err(MeshError::NotSimple);
        }
    }
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (p, q) = ((points[i], points[i + 1]), (points[j], points[(j + 1) % n]));
            if segments_touch(p, q) {
                return Err(MeshError::NotSimple);
            }
        }
    }
    let area = signed_area(points);
    if area == 0.0 {
        return Err(MeshError::ZeroArea);
    }
    Ok(area)
}

fn signed_area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    let mut twice = 0.0;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        twice += a[0] * b[1] - b[0] * a[1];
    }
    0.5 * twice
}

/// Whether two segments share any point.
fn segments_touch(p: ([f64; 2], [f64; 2]), q: ([f64; 2], [f64; 2])) -> bool {
    let d1 = orient(q.0, q.1, p.0);
    let d2 = orient(q.0, q.1, p.1);
    let d3 = orient(p.0, p.1, q.0);
    let d4 = orient(p.0, p.1, q.1);
    let opposite = |a: f64, b: f64| (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0);
    if opposite(d1, d2) && opposite(d3, d4) {
        return true;
    }
    (d1 == 0.0 && within(q, p.0))
        || (d2 == 0.0 && within(q, p.1))
        || (d3 == 0.0 && within(p, q.0))
        || (d4 == 0.0 && within(p, q.1))
}

/// Whether `c`, collinear with segment `s`, lies within its bounding box.
fn within(s: ([f64; 2], [f64; 2]), c: [f64; 2]) -> bool {
    c[0] >= s.0[0].min(s.1[0])
        && c[0] <= s.0[0].max(s.1[0])
        && c[1] >= s.0[1].min(s.1[1])
        && c[1] <= s.0[1].max(s.1[1])
}

/// Triangulates a counter-clockwise simple polygon by ear clipping. Each step
/// clips the ear with the best shape (largest area over squared longest
/// edge), the lowest index winning a tie, so the result is deterministic.
/// Every triangle winds counter-clockwise.
pub(super) fn triangulate(points: &[[f64; 2]]) -> Result<Vec<[usize; 3]>, MeshError> {
    let n = points.len();
    let mut next: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();
    let mut prev: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
    let mut alive = vec![true; n];
    let mut left = n;
    let mut out = Vec::with_capacity(n.saturating_sub(2));
    while left > 3 {
        let mut best: Option<(f64, usize)> = None;
        for v in (0..n).filter(|v| alive[*v]) {
            let (u, w) = (prev[v], next[v]);
            let (a, b, c) = (points[u], points[v], points[w]);
            let twice = orient(a, b, c);
            if twice <= 0.0 {
                continue;
            }
            let blocked = (0..n).any(|x| {
                alive[x]
                    && x != u
                    && x != v
                    && x != w
                    && orient(a, b, points[x]) >= 0.0
                    && orient(b, c, points[x]) >= 0.0
                    && orient(c, a, points[x]) >= 0.0
            });
            if blocked {
                continue;
            }
            let longest = [sub2(b, a), sub2(c, b), sub2(a, c)]
                .iter()
                .map(|e| dot2(*e, *e))
                .fold(0.0, f64::max);
            let quality = twice / longest;
            if best.is_none_or(|(q, _)| quality > q) {
                best = Some((quality, v));
            }
        }
        let Some((_, v)) = best else {
            return Err(MeshError::Degenerate);
        };
        let (u, w) = (prev[v], next[v]);
        out.push([u, v, w]);
        next[u] = w;
        prev[w] = u;
        alive[v] = false;
        left -= 1;
    }
    if let Some(v) = (0..n).find(|v| alive[*v]) {
        let (u, w) = (prev[v], next[v]);
        if orient(points[u], points[v], points[w]) <= 0.0 {
            return Err(MeshError::Degenerate);
        }
        out.push([u, v, w]);
    }
    Ok(out)
}
