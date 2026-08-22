//! Wild bodies: TOG-class silhouette entropy under a cute-ification pass.
//!
//! The blob (body.rs) proved the face grammar but collapsed shape entropy to
//! ~4 perceptual dimensions — every creature is the same species in
//! different hats (Ufuk's "2021 NFT" critique, correct). TOG's actual genius
//! was free spline bodies: hundreds of genome bits landing in SILHOUETTE,
//! the channel humans discriminate best. This module brings that back and
//! adds what TOG never had: a domestication pass that keeps wild shapes
//! endearing (radius smoothing kills needles into lobes, a tension floor
//! rounds corners, ground-flattening gives sit-weight).
//!
//! domestication 0.0 = raw TOG chaos; 1.0 = near-blob. The dial is itself a
//! candidate genome axis: the fleet should SPAN scrappy-weird to soft-round.

use crate::genome::Bits;

/// One radial spoke of a wild body.
#[derive(Clone, Copy, Debug)]
pub struct Spoke {
    /// Angle in radians (evenly spaced base + genome jitter).
    pub theta: f64,
    /// Radius in unit-cell terms.
    pub r: f64,
    /// Catmull-Rom tangent scale at this point (rounding weight).
    pub tension: f64,
}

/// A wild body: spokes around a center, plus derived vertical extents.
pub struct WildBody {
    pub spokes: Vec<Spoke>,
    /// Kept for the grid-7 renderer; decode() now always sets true. The
    /// free-form experiment produced unusable characters (grid-7 review:
    /// horizontal symmetry is load-bearing — TOG mirrored for a reason).
    /// Asymmetry re-enters later as SUBTLE post-mirror displacement, never
    /// as free topology.
    pub mirrored: bool,
}

/// Decode a wild body from the bit source. Spoke count reaches 13 (26 ring
/// points after mirroring) — more point information than TOG's 1024-bit
/// region, per the redesign direction.
pub fn decode(b: &mut Bits) -> WildBody {
    // 5..=16 half-side spokes (10..=32 ring points). Widened from 5..=13
    // with budget recovered from the surface region: silhouette is the
    // channel humans discriminate first, so it should be the best-funded
    // one — the inverse of TOG/v1, which spent its bits on face micro-detail
    // that avatar sizes cannot resolve.
    let spoke_count = 5 + (b.byte() % 12) as usize;
    let _legacy = b.byte(); // was the mirror flag; kept so seeds stay stable
    let mut spokes = Vec::with_capacity(spoke_count);
    let n = spoke_count as f64;
    for i in 0..spoke_count {
        let base = (i as f64 / n) * std::f64::consts::TAU;
        let jitter = (b.lin(-0.45, 0.45)) * std::f64::consts::TAU / n;
        // Radius range is DEEP on purpose: near-center spokes carve real
        // concavities. Tension reaches TOG-overshoot territory (>2 loops the
        // spline past its control point — the abstract-art accidents that
        // made TOGs unique; fill-rule turns self-crossings into cutouts).
        spokes.push(Spoke {
            theta: base + jitter,
            r: b.lin(0.12, 1.35),
            tension: b.lin(0.4, 2.6),
        });
    }
    WildBody { spokes, mirrored: true }
}

/// One genome-driven extremity: a curved, tapered capsule lobe grafted onto
/// the ring — continuous shape parameters, not a curated part family. This
/// is the "points create extremities" experiment: TOG could only afford a
/// fixed ring; free computation lets points GROW LIMBS.
#[derive(Clone, Copy, Debug)]
pub struct Extremity {
    /// Anchor angle on the ring (radians; mirrored pairs share one decode).
    pub theta: f64,
    /// Length relative to the body's mean radius (can exceed it: limbs
    /// stick OUT — that is their job).
    pub len: f64,
    /// Base half-width relative to mean radius.
    pub width: f64,
    /// Progressive bend along the spine (radians over the full length).
    pub curl: f64,
    /// Tip width as a fraction of base width (0 = pointed, 1 = capsule).
    pub taper: f64,
}

/// Decode 0..=3 extremity PAIRS (mirrored with the body).
pub fn decode_extremities(b: &mut Bits) -> Vec<Extremity> {
    // 0..=4 pairs (was 0..=3): limbs proved the strongest species-level
    // lever in grid 8, so recovered budget buys one more pair.
    let count = (b.byte() % 5) as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        // Anchor in the upper-and-side band (SVG y-down: PI..TAU is the
        // upper half); feet territory stays clear so ground-sit survives.
        let theta = b.lin(std::f64::consts::PI * 0.95, std::f64::consts::TAU * 1.02);
        out.push(Extremity {
            theta,
            len: b.lin(0.35, 1.15),
            width: b.lin(0.10, 0.30),
            curl: b.lin(-1.3, 1.3),
            taper: b.lin(0.15, 0.85),
        });
    }
    out
}

/// Sampled closed outline of one extremity capsule in ring coordinates
/// (unit-cell frame, before the body's normalize step — caller transforms).
/// `ring_r` is the body radius at the anchor angle.
pub fn extremity_outline(e: &Extremity, ring_r: f64, mean_r: f64, mirror: bool) -> Vec<(f64, f64)> {
    let theta = if mirror {
        std::f64::consts::PI - e.theta
    } else {
        e.theta
    };
    let curl = if mirror { -e.curl } else { e.curl };
    // Root sits INSIDE the body (85% of ring radius) so the graft is covered.
    let root = (theta.cos() * ring_r * 0.85, theta.sin() * ring_r * 0.85);
    let len = e.len * mean_r;
    let steps = 14usize;
    let mut spine = Vec::with_capacity(steps + 1);
    let mut dir = theta;
    let mut p = root;
    for i in 0..=steps {
        spine.push(p);
        if i < steps {
            dir += curl / steps as f64;
            p = (
                p.0 + dir.cos() * len / steps as f64,
                p.1 + dir.sin() * len / steps as f64,
            );
        }
    }
    // Offset left/right of the spine by the tapering width; rounded tip.
    // The half-width is capped by the spine's radius of curvature
    // (ρ = len/|curl|): a wider offset loops back over itself at strong
    // curls, and the self-crossing capsule renders as detached shards under
    // the nonzero fill rule (grid-10 zoom review, seed 4009's wings).
    let curvature_cap = if e.curl.abs() > 1e-6 {
        0.8 * len / e.curl.abs()
    } else {
        f64::MAX
    };
    let base_w = if e.width * mean_r > curvature_cap {
        crate::coverage::hit(&crate::coverage::COUNTERS.curvature_clamp);
        curvature_cap
    } else {
        e.width * mean_r
    };
    let mut left = Vec::with_capacity(steps + 1);
    let mut right = Vec::with_capacity(steps + 1);
    for (i, &(x, y)) in spine.iter().enumerate() {
        let t = i as f64 / steps as f64;
        let w = base_w * (1.0 - (1.0 - e.taper) * t);
        // Perpendicular from local direction: estimate from neighbors.
        let (dx, dy) = if i == 0 {
            let n = spine[1];
            (n.0 - x, n.1 - y)
        } else {
            let pr = spine[i - 1];
            (x - pr.0, y - pr.1)
        };
        let l = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (nx, ny) = (-dy / l, dx / l);
        left.push((x + nx * w, y + ny * w));
        right.push((x - nx * w, y - ny * w));
    }
    // Tip cap: arc via three points beyond the last spine point.
    let tip = spine[steps];
    let tip_w = base_w * e.taper;
    let (tdx, tdy) = {
        let pr = spine[steps - 1];
        let l = ((tip.0 - pr.0).powi(2) + (tip.1 - pr.1).powi(2)).sqrt().max(1e-9);
        ((tip.0 - pr.0) / l, (tip.1 - pr.1) / l)
    };
    let mut out = Vec::new();
    out.extend(left.iter().copied());
    out.push((tip.0 + tdx * tip_w, tip.1 + tdy * tip_w));
    out.extend(right.iter().rev().copied());
    out
}

/// Raw geometry in ring coordinates: sampled outline + EXACT bounds.
pub struct RawBody {
    pub pts: Vec<(f64, f64)>,
    /// Exact bbox (min_x, min_y, max_x, max_y) from bezier tangent roots
    /// (dx/dt = 0, dy/dt = 0 per segment), post-ground-flatten — the true
    /// spline extremes, not a sample or control-point approximation (TOG's
    /// rescale lesson: sample bboxes under-measure overshoot lobes).
    pub bbox: (f64, f64, f64, f64),
}

/// Raw (un-normalized) outline in ring coordinates — lets callers recover
/// the ring->cell transform that `outline` applies internally.
pub fn outline_raw(body: &WildBody, dom: f64) -> Vec<(f64, f64)> {
    outline_impl(body, dom, None)
}

/// Exact per-coordinate cubic extremes: roots of the derivative quadratic
/// inside (0,1), evaluated. Returns candidate values including endpoints.
fn cubic_extremes(p0: f64, c1: f64, c2: f64, p3: f64) -> [f64; 4] {
    let a = p3 - 3.0 * c2 + 3.0 * c1 - p0;
    let b = 2.0 * (c2 - 2.0 * c1 + p0);
    let c = c1 - p0;
    let eval = |t: f64| {
        let v = 1.0 - t;
        v * v * v * p0 + 3.0 * v * v * t * c1 + 3.0 * v * t * t * c2 + t * t * t * p3
    };
    let mut out = [p0, p3, p0, p3];
    let mut roots: Vec<f64> = Vec::with_capacity(2);
    if a.abs() < 1e-12 {
        if b.abs() > 1e-12 {
            roots.push(-c / b);
        }
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let sq = disc.sqrt();
            roots.push((-b + sq) / (2.0 * a));
            roots.push((-b - sq) / (2.0 * a));
        }
    }
    for (slot, t) in roots.into_iter().filter(|t| *t > 0.0 && *t < 1.0).enumerate() {
        out[2 + slot.min(1)] = eval(t);
    }
    out
}

/// Full raw build: sampled outline + exact tangent-root bounds, both after
/// the ground flatten. This is the geometry source for the unified
/// scale-to-fit transform (one transform for body, extremities, sites).
pub fn build_raw(body: &WildBody, dom: f64) -> RawBody {
    let pts = outline_impl(body, dom, None);
    // Recompute the segment control points exactly as outline_impl does, to
    // extract true extremes. (Duplication kept local and cheap: playground.)
    let segs = control_segments(body, dom);
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for s in &segs {
        for v in cubic_extremes(s[0].0, s[1].0, s[2].0, s[3].0) {
            min_x = min_x.min(v);
            max_x = max_x.max(v);
        }
        for v in cubic_extremes(s[0].1, s[1].1, s[2].1, s[3].1) {
            min_y = min_y.min(v);
            max_y = max_y.max(v);
        }
    }
    // Ground flatten mapping applied to the y extremes (monotonic in y).
    let h = max_y - min_y;
    let ground = max_y - h * 0.14;
    let squash = 0.30 + 0.35 * dom;
    let max_y_flat = ground + (max_y - ground) * squash;
    RawBody {
        pts,
        bbox: (min_x, min_y, max_x, max_y_flat),
    }
}

/// The cute-ification pass. Returns the outline as dense sampled points.
///
/// Steps, each scaled by `dom` in 0..1:
/// 1. Radius smoothing: circular moving-average of spoke radii — needles
///    become lobes, deep concavities shallow out.
/// 2. Tension floor: corners round toward arcs.
/// 3. Ground flatten: y below the sit-line compresses, weight settles.
pub fn outline(body: &WildBody, dom: f64, cx: f64, cy: f64, scale: f64) -> Vec<(f64, f64)> {
    outline_impl(body, dom, Some((cx, cy, scale)))
}

/// Prepared spoke ring → cubic segment control points [p1, c1, c2, p2].
/// Shared by the sampler and the exact-extremes bbox so they can never
/// disagree about the curve.
fn control_segments(body: &WildBody, dom: f64) -> Vec<[(f64, f64); 4]> {
    let pts = prepared_points(body, dom);
    let n = pts.len();
    let mut segs = Vec::with_capacity(n);
    for i in 0..n {
        let p0 = pts[(i + n - 1) % n];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let p3 = pts[(i + 2) % n];
        let t1 = p1.2;
        let t2 = p2.2;
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0 * t1, p1.1 + (p2.1 - p0.1) / 6.0 * t1);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0 * t2, p2.1 - (p3.1 - p1.1) / 6.0 * t2);
        segs.push([(p1.0, p1.1), c1, c2, (p2.0, p2.1)]);
    }
    segs
}

/// Spoke preparation shared by the sampler and the segment builder: mirror,
/// sort, dedup, domestication (radius smoothing + floor, tension pull),
/// producing (x, y, tension) ring points.
fn prepared_points(body: &WildBody, dom: f64) -> Vec<(f64, f64, f64)> {
    // Mirroring by FOLDING: every spoke maps into the right half-plane
    // (θ → π−θ when cos θ < 0), then the folded set is reflected. Every
    // decoded spoke contributes to the silhouette and symmetry cannot
    // degenerate. (The previous filter-the-right-half approach had a
    // fallback — keep the raw set when fewer than 3 spokes landed right of
    // the axis — which silently produced FREE-FORM bodies: the seed-3003
    // asymmetry, found by the probe, 19.05 cell units.)
    let mut spokes: Vec<Spoke> = if body.mirrored {
        let mut half: Vec<Spoke> = body
            .spokes
            .iter()
            .map(|s| {
                let t = s.theta.rem_euclid(std::f64::consts::TAU);
                let folded = if t.cos() < 0.0 { std::f64::consts::PI - t } else { t };
                Spoke {
                    theta: folded,
                    r: s.r,
                    tension: s.tension,
                }
            })
            .collect();
        let mirrored: Vec<Spoke> = half
            .iter()
            .map(|s| Spoke {
                theta: std::f64::consts::PI - s.theta,
                r: s.r,
                tension: s.tension,
            })
            .collect();
        half.extend(mirrored);
        half
    } else {
        body.spokes.clone()
    };
    for s in &mut spokes {
        s.theta = s.theta.rem_euclid(std::f64::consts::TAU);
    }
    spokes.sort_by(|a, b| a.theta.partial_cmp(&b.theta).unwrap());
    // Tolerance covers the vertical-pole mirror duplicates (θ ≈ π−θ) and
    // jitter near-collisions; a doubled point makes the spline kink.
    spokes.dedup_by(|a, b| (a.theta - b.theta).abs() < 1e-3);

    // 1. Radius smoothing (domestication): two passes of circular 3-tap
    // weighted average, weight rising with dom, PLUS a radius floor — dom
    // lifts the deepest concavities toward the mean.
    let k = 0.55 * dom;
    let mut radii: Vec<f64> = spokes.iter().map(|s| s.r).collect();
    for _ in 0..2 {
        let prev = radii.clone();
        let n = prev.len();
        for i in 0..n {
            let a = prev[(i + n - 1) % n];
            let b_ = prev[i];
            let c = prev[(i + 1) % n];
            radii[i] = b_ * (1.0 - k) + (a + c) * 0.5 * k;
        }
    }
    let mean_r = radii.iter().sum::<f64>() / radii.len() as f64;
    let floor = mean_r * 0.55 * dom;
    for r in &mut radii {
        *r = r.max(floor);
    }
    // 2. Tension DOMESTICATION: dom pulls overshoot tensions toward 1.0
    // (calm arcs); at dom 0 the raw 0.4..2.6 range survives, loops included.
    let pts: Vec<(f64, f64, f64)> = spokes
        .iter()
        .zip(radii.iter())
        .map(|(s, &r)| {
            (
                s.theta.cos() * r,
                s.theta.sin() * r,
                s.tension + (1.0 - s.tension) * dom,
            )
        })
        .collect();
    pts
}

fn outline_impl(body: &WildBody, dom: f64, normalize: Option<(f64, f64, f64)>) -> Vec<(f64, f64)> {
    let pts = prepared_points(body, dom);
    // Closed Catmull-Rom through the spoke tips, tension-scaled tangents,
    // sampled densely.
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 18);
    for i in 0..n {
        let p0 = pts[(i + n - 1) % n];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let p3 = pts[(i + 2) % n];
        let t1 = p1.2;
        let t2 = p2.2;
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0 * t1, p1.1 + (p2.1 - p0.1) / 6.0 * t1);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0 * t2, p2.1 - (p3.1 - p1.1) / 6.0 * t2);
        for step in 0..18 {
            let u = step as f64 / 18.0;
            let v = 1.0 - u;
            let x = v * v * v * p1.0 + 3.0 * v * v * u * c1.0 + 3.0 * v * u * u * c2.0 + u * u * u * p2.0;
            let y = v * v * v * p1.1 + 3.0 * v * v * u * c1.1 + 3.0 * v * u * u * c2.1 + u * u * u * p2.1;
            out.push((x, y));
        }
    }

    // 3. Ground flatten + normalize to the cell.
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &out {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let h = max_y - min_y;
    let ground = max_y - h * 0.14;
    let squash = 0.30 + 0.35 * dom;
    for p in &mut out {
        if p.1 > ground {
            p.1 = ground + (p.1 - ground) * squash;
        }
    }
    let Some((cx, cy, scale)) = normalize else {
        return out;
    };
    // Re-measure and scale uniformly into the cell.
    let (mut min_x2, mut max_x2, mut min_y2, mut max_y2) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &out {
        min_x2 = min_x2.min(x);
        max_x2 = max_x2.max(x);
        min_y2 = min_y2.min(y);
        max_y2 = max_y2.max(y);
    }
    let w = (max_x2 - min_x2).max(1e-6);
    let h2 = (max_y2 - min_y2).max(1e-6);
    let s = (scale / w).min(scale * 1.15 / h2);
    let mx = (min_x2 + max_x2) / 2.0;
    let my = (min_y2 + max_y2) / 2.0;
    out.iter_mut()
        .map(|p| {
            *p = (cx + (p.0 - mx) * s, cy + (p.1 - my) * s);
        })
        .count();
    out
}

/// Distance from the ring origin to the outline along `theta`, taking the
/// NEAREST crossing.
///
/// Extremity roots used the nearest spoke's raw radius, but the rendered
/// outline is the post-domestication curve — smoothing, the concavity
/// floor and tension all move it, and where the curve fell inside the raw
/// radius the root landed outside the body and the limb rendered detached
/// (seed 5012). Measuring the actual outline is the only way a graft can
/// be guaranteed covered; nearest crossing keeps the root inside even on
/// concave, self-crossing shapes.
pub fn radius_along(outline: &[(f64, f64)], theta: f64) -> Option<f64> {
    let (dx, dy) = (theta.cos(), theta.sin());
    let n = outline.len();
    let mut best: Option<f64> = None;
    for i in 0..n {
        let (x1, y1) = outline[i];
        let (x2, y2) = outline[(i + 1) % n];
        let (ex, ey) = (x2 - x1, y2 - y1);
        // Solve p1 + s*e = t*d for s in [0,1], t > 0.
        let den = dx * ey - dy * ex;
        if den.abs() < 1e-12 {
            continue;
        }
        let s = (dx * y1 - dy * x1) / den;
        if !(0.0..=1.0).contains(&s) {
            continue;
        }
        let px = x1 + ex * s;
        let py = y1 + ey * s;
        let t = if dx.abs() > dy.abs() { px / dx } else { py / dy };
        if t > 1e-9 && best.map(|b| t < b).unwrap_or(true) {
            best = Some(t);
        }
    }
    best
}

/// ALL horizontal spans of the outline at height `y`, left to right.
///
/// A concave body crosses a scanline more than twice, and a walk that takes
/// the widest span at each height can hop between disjoint masses — which
/// renders as horizontal striping when consecutive samples land on
/// different legs. Callers that walk a band need every span so they can
/// stay on one of them.
pub fn extents_at(outline: &[(f64, f64)], y: f64) -> Vec<(f64, f64)> {
    let mut xs: Vec<f64> = Vec::new();
    let n = outline.len();
    for i in 0..n {
        let (x1, y1) = outline[i];
        let (x2, y2) = outline[(i + 1) % n];
        if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
            let t = (y - y1) / (y2 - y1);
            xs.push(x1 + (x2 - x1) * t);
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect()
}

/// Horizontal extents of the outline at height `y` (for face mounting):
/// returns (left_x, right_x) of the widest crossing span, or None.
pub fn extent_at(outline: &[(f64, f64)], y: f64) -> Option<(f64, f64)> {
    let mut xs: Vec<f64> = Vec::new();
    let n = outline.len();
    for i in 0..n {
        let (x1, y1) = outline[i];
        let (x2, y2) = outline[(i + 1) % n];
        if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
            let t = (y - y1) / (y2 - y1);
            xs.push(x1 + (x2 - x1) * t);
        }
    }
    if xs.len() < 2 {
        return None;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Widest span among crossings (concave shapes cross more than twice).
    let mut best = (xs[0], xs[xs.len() - 1]);
    let mut best_w = 0.0;
    for w in xs.windows(2) {
        if w[1] - w[0] > best_w {
            best_w = w[1] - w[0];
            best = (w[0], w[1]);
        }
    }
    Some(best)
}

/// Bounding box helper: (min_x, min_y, max_x, max_y).
pub fn bbox(outline: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let (mut a, mut b, mut c, mut d) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in outline {
        a = a.min(x);
        b = b.min(y);
        c = c.max(x);
        d = d.max(y);
    }
    (a, b, c, d)
}

/// Symmetry probe: max distance from any outline point to the nearest point
/// of the x-mirrored outline (0 = perfectly symmetric). Diagnostic only.
pub fn asymmetry(outline: &[(f64, f64)], axis_x: f64) -> f64 {
    let mut worst = 0.0_f64;
    for &(x, y) in outline {
        let mx = 2.0 * axis_x - x;
        let mut best = f64::MAX;
        for &(ox, oy) in outline {
            let d = ((ox - mx).powi(2) + (oy - y).powi(2)).sqrt();
            if d < best {
                best = d;
            }
        }
        worst = worst.max(best);
    }
    worst
}
