//! Fitting cubic Beziers to a flattened contour.
//!
//! The wild pipeline extracts silhouettes by marching squares and spline
//! flattening, so it hands the renderer a polyline: one creature runs 1800
//! to 11600 line segments where the older generator's tree carries 200 to
//! 370 curves. Every consumer then walks that tessellation on every draw,
//! and a list of thirty avatars is on the order of 350,000 segments.
//!
//! The claim this module exists to test is that the same silhouette can be
//! carried as curves at a deviation nobody can see. It is MEASUREMENT
//! ONLY: nothing here is wired into the render path, because changing the
//! geometry and judging the geometry at the same time answers neither
//! question.
//!
//! The method is Schneider's: fit one cubic to the whole run, measure the
//! worst deviation, and split at that point if it exceeds tolerance. Fits
//! use chord-length parameterization with no reparameterization pass,
//! which is the cheaper half of the algorithm and enough to size the win.

/// A cubic in the plane, in the same units as the input points.
#[derive(Clone, Copy, Debug)]
pub struct Cubic {
    pub p0: (f64, f64),
    pub p1: (f64, f64),
    pub p2: (f64, f64),
    pub p3: (f64, f64),
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn norm(v: (f64, f64)) -> (f64, f64) {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    if len < 1e-12 {
        (0.0, 0.0)
    } else {
        (v.0 / len, v.1 / len)
    }
}

fn eval(c: &Cubic, t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        a * c.p0.0 + b * c.p1.0 + cc * c.p2.0 + d * c.p3.0,
        a * c.p0.1 + b * c.p1.1 + cc * c.p2.1 + d * c.p3.1,
    )
}

/// Chord-length parameters for a run of points, normalized to [0, 1].
fn parameterize(points: &[(f64, f64)]) -> Vec<f64> {
    let mut u = Vec::with_capacity(points.len());
    u.push(0.0);
    for i in 1..points.len() {
        let d = sub(points[i], points[i - 1]);
        u.push(u[i - 1] + (d.0 * d.0 + d.1 * d.1).sqrt());
    }
    let total = *u.last().unwrap_or(&0.0);
    if total > 0.0 {
        for value in &mut u {
            *value /= total;
        }
    }
    u
}

/// Least-squares fit of one cubic through `points`, with the end tangents
/// pinned to `t0` and `t3` so adjacent fits meet without a corner.
fn fit_one(points: &[(f64, f64)], u: &[f64], t0: (f64, f64), t3: (f64, f64)) -> Cubic {
    let p0 = points[0];
    let p3 = points[points.len() - 1];
    // Solve for the two tangent magnitudes that minimize squared error.
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (point, &t) in points.iter().zip(u.iter()) {
        let uu = 1.0 - t;
        let b0 = uu * uu * uu;
        let b1 = 3.0 * uu * uu * t;
        let b2 = 3.0 * uu * t * t;
        let b3 = t * t * t;
        let a0 = (t0.0 * b1, t0.1 * b1);
        let a1 = (t3.0 * b2, t3.1 * b2);
        c00 += a0.0 * a0.0 + a0.1 * a0.1;
        c01 += a0.0 * a1.0 + a0.1 * a1.1;
        c11 += a1.0 * a1.0 + a1.1 * a1.1;
        let tmp = (
            point.0 - (b0 * p0.0 + b1 * p0.0 + b2 * p3.0 + b3 * p3.0),
            point.1 - (b0 * p0.1 + b1 * p0.1 + b2 * p3.1 + b3 * p3.1),
        );
        x0 += a0.0 * tmp.0 + a0.1 * tmp.1;
        x1 += a1.0 * tmp.0 + a1.1 * tmp.1;
    }
    let det = c00 * c11 - c01 * c01;
    let chord = {
        let d = sub(p3, p0);
        (d.0 * d.0 + d.1 * d.1).sqrt()
    };
    // A singular system means the two tangents are parallel and no
    // least-squares solution exists; the Bezier-through-thirds heuristic is
    // the standard fallback and is exact for a straight run.
    let (alpha0, alpha1) = if det.abs() < 1e-12 {
        (chord / 3.0, chord / 3.0)
    } else {
        let a0 = (x0 * c11 - x1 * c01) / det;
        let a1 = (c00 * x1 - c01 * x0) / det;
        if a0 < 1e-9 || a1 < 1e-9 {
            (chord / 3.0, chord / 3.0)
        } else {
            (a0, a1)
        }
    };
    Cubic {
        p0,
        p1: (p0.0 + t0.0 * alpha0, p0.1 + t0.1 * alpha0),
        p2: (p3.0 + t3.0 * alpha1, p3.1 + t3.1 * alpha1),
        p3,
    }
}

/// Worst distance from any input point to the fitted curve, and where it
/// occurs. Distance is measured at the point's own parameter rather than
/// by true projection: the parameterization is what the fit used, so this
/// is the error the fit is responsible for.
fn max_error(points: &[(f64, f64)], u: &[f64], c: &Cubic) -> (f64, usize) {
    let mut worst = 0.0;
    let mut at = points.len() / 2;
    for (i, (point, &t)) in points.iter().zip(u.iter()).enumerate() {
        let q = eval(c, t);
        let d = sub(q, *point);
        let e = (d.0 * d.0 + d.1 * d.1).sqrt();
        if e > worst {
            worst = e;
            at = i;
        }
    }
    (worst, at)
}

/// One Newton-Raphson step per point, moving each parameter toward the
/// value that minimizes its distance to the curve.
fn reparameterize(points: &[(f64, f64)], u: &mut [f64], c: &Cubic) {
    for (point, t) in points.iter().zip(u.iter_mut()) {
        let q = eval(c, *t);
        let d1 = deriv1(c, *t);
        let d2 = deriv2(c, *t);
        let diff = sub(q, *point);
        let numerator = diff.0 * d1.0 + diff.1 * d1.1;
        let denominator = d1.0 * d1.0 + d1.1 * d1.1 + diff.0 * d2.0 + diff.1 * d2.1;
        if denominator.abs() > 1e-12 {
            *t = (*t - numerator / denominator).clamp(0.0, 1.0);
        }
    }
    // The endpoints are pinned by construction; letting them drift inward
    // would let the fit shorten the curve instead of shaping it.
    if let Some(first) = u.first_mut() {
        *first = 0.0;
    }
    if let Some(last) = u.last_mut() {
        *last = 1.0;
    }
}

fn deriv1(c: &Cubic, t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    let a = 3.0 * u * u;
    let b = 6.0 * u * t;
    let d = 3.0 * t * t;
    (
        a * (c.p1.0 - c.p0.0) + b * (c.p2.0 - c.p1.0) + d * (c.p3.0 - c.p2.0),
        a * (c.p1.1 - c.p0.1) + b * (c.p2.1 - c.p1.1) + d * (c.p3.1 - c.p2.1),
    )
}

fn deriv2(c: &Cubic, t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    (
        6.0 * u * (c.p2.0 - 2.0 * c.p1.0 + c.p0.0) + 6.0 * t * (c.p3.0 - 2.0 * c.p2.0 + c.p1.0),
        6.0 * u * (c.p2.1 - 2.0 * c.p1.1 + c.p0.1) + 6.0 * t * (c.p3.1 - 2.0 * c.p2.1 + c.p1.1),
    )
}

fn fit_run(points: &[(f64, f64)], tol: f64, t0: (f64, f64), t3: (f64, f64), out: &mut Vec<Cubic>) {
    if points.len() < 3 {
        // Two points carry no shape information, so the inherited tangents
        // must not be trusted here: a tangent that disagrees with the
        // chord throws the control points off the segment and bulges the
        // curve far from both endpoints. Because this base case never
        // measures its own error, that bulge is invisible to the recursion
        // and survives at any tolerance — which is exactly how a fit can
        // report more curves without less deviation. Blend each tangent
        // toward the chord and keep the handles short.
        let p0 = points[0];
        let p3 = points[points.len() - 1];
        let d = sub(p3, p0);
        let len = (d.0 * d.0 + d.1 * d.1).sqrt();
        let chord = norm(d);
        let blend = |t: (f64, f64), sign: f64| -> (f64, f64) {
            let candidate = (t.0 * sign, t.1 * sign);
            // A tangent pointing away from the chord is worse than no
            // tangent at all.
            if candidate.0 * chord.0 + candidate.1 * chord.1 <= 0.0 {
                chord
            } else {
                norm((candidate.0 + chord.0, candidate.1 + chord.1))
            }
        };
        let a = blend(t0, 1.0);
        let b = blend(t3, -1.0);
        let third = len / 3.0;
        out.push(Cubic {
            p0,
            p1: (p0.0 + a.0 * third, p0.1 + a.1 * third),
            p2: (p3.0 - b.0 * third, p3.1 - b.1 * third),
            p3,
        });
        return;
    }
    let mut u = parameterize(points);
    let mut curve = fit_one(points, &u, t0, t3);
    let (mut err, mut at) = max_error(points, &u, &curve);
    // Chord-length parameters are a guess at where each point lands on the
    // curve, and a wrong guess makes the error metric measure the wrong
    // distance: the fit then believes it is inside tolerance while the true
    // geometric deviation stays large, so tightening tolerance buys more
    // curves and no accuracy. Pulling each parameter onto its nearest point
    // of the curve before refitting is what makes the metric mean what it
    // claims.
    for _ in 0..4 {
        if err <= tol {
            break;
        }
        reparameterize(points, &mut u, &curve);
        curve = fit_one(points, &u, t0, t3);
        let next = max_error(points, &u, &curve);
        if next.0 >= err {
            // Refinement stopped helping; splitting is the remaining lever.
            break;
        }
        err = next.0;
        at = next.1;
    }
    if err <= tol {
        out.push(curve);
        return;
    }
    // Split at the worst point and fit each side. The shared tangent is the
    // centered difference there, so the two halves join smoothly.
    let split = at.clamp(1, points.len() - 2);
    let centre = norm(sub(points[split + 1], points[split - 1]));
    fit_run(&points[..=split], tol, t0, (-centre.0, -centre.1), out);
    fit_run(&points[split..], tol, centre, t3, out);
}

/// Fits a closed polyline, returning the cubics that replace it.
pub fn fit_closed(points: &[(f64, f64)], tol: f64) -> Vec<Cubic> {
    if points.len() < 4 {
        return Vec::new();
    }
    let n = points.len();
    // The ring starts and ends at the same place, so its two end tangents
    // are one tangent seen from both sides. Computing them independently
    // leaves the closing curve free to bulge, and because that curve is
    // emitted once no matter how finely the rest subdivides, the error it
    // introduces does not respond to tolerance at all — the fit reports
    // more curves without less deviation.
    // ...unless the seam itself lands on a corner, which forcing a shared
    // tangent would round away. The seam is an arbitrary index in a ring,
    // so a fitter that always smooths there damages one feature per shape
    // at a position nobody chose.
    let seam_in = norm(sub(points[0], points[n - 1]));
    let seam_out = norm(sub(points[1], points[0]));
    let seam_is_corner = seam_in.0 * seam_out.0 + seam_in.1 * seam_out.1 < 0.5;
    let (start, end) = if seam_is_corner {
        (seam_out, (-seam_in.0, -seam_in.1))
    } else {
        let tangent = norm(sub(points[1], points[n - 1]));
        (tangent, (-tangent.0, -tangent.1))
    };
    let mut ring: Vec<(f64, f64)> = points.to_vec();
    ring.push(points[0]);

    // Corners are features, not error. Reparameterization pulls each
    // sample toward its nearest point on a smooth curve, which rounds a
    // sharp turn away — so the runs are cut AT the corners first and each
    // side is fitted independently. Without this the fitter silently
    // changes the silhouette, which is the one thing a representation
    // change must never do.
    let mut cuts = vec![0usize];
    for i in 1..ring.len() - 1 {
        let incoming = norm(sub(ring[i], ring[i - 1]));
        let outgoing = norm(sub(ring[i + 1], ring[i]));
        let dot = incoming.0 * outgoing.0 + incoming.1 * outgoing.1;
        // About 60 degrees: gentle enough to keep organic curvature in one
        // run, sharp enough to preserve a spike tip.
        if dot < 0.5 {
            cuts.push(i);
        }
    }
    cuts.push(ring.len() - 1);
    cuts.dedup();

    let mut out = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b <= a {
            continue;
        }
        let run = &ring[a..=b];
        // A run's own end tangents, except at the ring seam where the
        // shared closure tangent applies.
        let t_start = if a == 0 {
            start
        } else {
            norm(sub(run[1], run[0]))
        };
        let t_end = if b == ring.len() - 1 {
            end
        } else {
            norm(sub(run[run.len() - 2], run[run.len() - 1]))
        };
        fit_run(run, tol, t_start, t_end, &mut out);
    }
    out
}

/// Worst deviation of the fitted curves from the original polyline,
/// sampled densely along each curve. This is the number that decides
/// whether a fit is invisible, so it is measured independently of the
/// error the fitter minimized.
pub fn deviation(points: &[(f64, f64)], curves: &[Cubic]) -> f64 {
    deviation_located(points, curves).0
}

/// Worst deviation plus which curve produced it, for diagnosing a fit that
/// does not improve with tolerance.
pub fn deviation_located(points: &[(f64, f64)], curves: &[Cubic]) -> (f64, usize) {
    let mut worst: f64 = 0.0;
    let mut at = 0usize;
    for (index, c) in curves.iter().enumerate() {
        for step in 0..=16 {
            let q = eval(c, step as f64 / 16.0);
            let mut best = f64::MAX;
            for w in points.windows(2) {
                best = best.min(point_segment_distance(q, w[0], w[1]));
            }
            if best > worst {
                worst = best;
                at = index;
            }
        }
    }
    (worst, at)
}

fn point_segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let ab = sub(b, a);
    let len2 = ab.0 * ab.0 + ab.1 * ab.1;
    let t = if len2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / len2).clamp(0.0, 1.0)
    };
    let q = (a.0 + ab.0 * t, a.1 + ab.1 * t);
    let d = sub(p, q);
    (d.0 * d.0 + d.1 * d.1).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A circle is where fitting should win biggest. Four cubics can
    /// approximate one to about a thousandth of its radius, but only with
    /// the reparameterization pass this fitter deliberately omits, so the
    /// property asserted here is the one that matters: a large reduction
    /// at a deviation far below one canvas unit. Tightening the fit is a
    /// known lever if the measured reduction on real contours is not
    /// enough.
    #[test]
    fn a_sampled_circle_collapses_to_few_curves() {
        let points: Vec<(f64, f64)> = (0..256)
            .map(|i| {
                let a = i as f64 / 256.0 * std::f64::consts::TAU;
                (100.0 * a.cos(), 100.0 * a.sin())
            })
            .collect();
        let curves = fit_closed(&points, 0.05);
        assert!(
            curves.len() * 8 < points.len(),
            "expected at least an 8x reduction, got {} curves from {} points",
            curves.len(),
            points.len()
        );
        assert!(deviation(&points, &curves) < 0.5);
    }

    /// A square must NOT be smoothed away: its corners are real features,
    /// and a fitter that rounds them is silently changing the silhouette.
    #[test]
    fn corners_survive_the_fit() {
        let mut points = Vec::new();
        for i in 0..40 {
            points.push((i as f64 * 2.5, 0.0));
        }
        for i in 0..40 {
            points.push((100.0, i as f64 * 2.5));
        }
        for i in 0..40 {
            points.push((100.0 - i as f64 * 2.5, 100.0));
        }
        for i in 0..40 {
            points.push((0.0, 100.0 - i as f64 * 2.5));
        }
        let curves = fit_closed(&points, 0.05);
        let dev = deviation(&points, &curves);
        assert!(dev < 1.0, "corners rounded away: deviation {dev:.3}");
    }
}
