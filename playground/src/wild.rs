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
    /// Symmetry flag decoded from the genome: mirrored bodies read tidier,
    /// free bodies read wilder. Both exist in the population.
    pub mirrored: bool,
}

/// Decode a wild body from the bit source. `spoke_count` in 6..=10 gives
/// TOG-like entropy without polygon soup.
pub fn decode(b: &mut Bits) -> WildBody {
    let spoke_count = 5 + (b.byte() % 9) as usize; // 5..=13
    let mirrored = b.byte() % 2 == 0; // half tidy, half free
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
    WildBody { spokes, mirrored }
}

/// The cute-ification pass. Returns the outline as dense sampled points.
///
/// Steps, each scaled by `dom` in 0..1:
/// 1. Radius smoothing: circular moving-average of spoke radii — needles
///    become lobes, deep concavities shallow out.
/// 2. Tension floor: corners round toward arcs.
/// 3. Ground flatten: y below the sit-line compresses, weight settles.
pub fn outline(body: &WildBody, dom: f64, cx: f64, cy: f64, scale: f64) -> Vec<(f64, f64)> {
    // Mirroring: reflect spokes with theta in (0, PI) across the y axis,
    // replacing the originals in (PI, TAU), then sort by angle.
    let mut spokes: Vec<Spoke> = if body.mirrored {
        let mut half: Vec<Spoke> = body
            .spokes
            .iter()
            .filter(|s| {
                let t = s.theta.rem_euclid(std::f64::consts::TAU);
                (t <= std::f64::consts::PI * 0.5) || (t >= std::f64::consts::PI * 1.5)
            })
            .copied()
            .collect();
        if half.len() < 3 {
            half = body.spokes.clone(); // degenerate mirror set: keep free form
        } else {
            let mirrored: Vec<Spoke> = half
                .iter()
                .filter(|s| {
                    let t = s.theta.rem_euclid(std::f64::consts::TAU);
                    !(t < 0.03 || (t - std::f64::consts::PI).abs() < 0.03)
                })
                .map(|s| Spoke {
                    theta: std::f64::consts::PI - s.theta,
                    r: s.r,
                    tension: s.tension,
                })
                .collect();
            half.extend(mirrored);
        }
        half
    } else {
        body.spokes.clone()
    };
    for s in &mut spokes {
        s.theta = s.theta.rem_euclid(std::f64::consts::TAU);
    }
    spokes.sort_by(|a, b| a.theta.partial_cmp(&b.theta).unwrap());
    spokes.dedup_by(|a, b| (a.theta - b.theta).abs() < 1e-6);

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
