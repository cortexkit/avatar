//! Extremity-into-body merging: SDF smooth-min union + marching squares.
//!
//! TOG could never afford this: the body and every extremity become signed
//! distance fields, blended with a smooth minimum (metaball-style weld), and
//! the merged silhouette is re-extracted as ONE closed contour — "extract
//! all points, merge, respline" done properly. The weld radius k turns hard
//! grafts into organic fillets: an antenna stops being a layer on the body
//! and becomes body.

/// Precomputed polygon for SDF queries.
pub struct Poly {
    pts: Vec<(f64, f64)>,
}

impl Poly {
    /// Downsamples to every `step`-th point: SDF accuracy tolerates it and
    /// the grid loop is O(grid · edges).
    pub fn new(outline: &[(f64, f64)], step: usize) -> Self {
        let pts: Vec<(f64, f64)> = outline.iter().copied().step_by(step.max(1)).collect();
        Poly { pts }
    }

    /// Signed distance: negative inside, positive outside.
    ///
    /// Inside is the NONZERO WINDING rule — the same rule SVG uses for fill
    /// and clip-path — not crossing parity. On self-crossing outlines
    /// (tension overshoot makes them, by design) the two disagree: parity
    /// declared the star-notch interior "inside" while the renderer clipped
    /// it, so faces were placed in regions the body never painted — and the
    /// checker, sharing this SDF, certified the faceless result. The
    /// renderer's rule is ground truth; the SDF must speak it.
    pub fn sdf(&self, x: f64, y: f64) -> f64 {
        let n = self.pts.len();
        let mut d2 = f64::MAX;
        let mut winding = 0i32;
        for i in 0..n {
            let (x1, y1) = self.pts[i];
            let (x2, y2) = self.pts[(i + 1) % n];
            // Distance to segment.
            let (ex, ey) = (x2 - x1, y2 - y1);
            let (px, py) = (x - x1, y - y1);
            let t = ((px * ex + py * ey) / (ex * ex + ey * ey).max(1e-12)).clamp(0.0, 1.0);
            let (qx, qy) = (px - ex * t, py - ey * t);
            d2 = d2.min(qx * qx + qy * qy);
            // Winding accumulation (Sunday's algorithm).
            let cross = ex * py - ey * px;
            if y1 <= y && y2 > y && cross > 0.0 {
                winding += 1;
            } else if y2 <= y && y1 > y && cross < 0.0 {
                winding -= 1;
            }
        }
        let d = d2.sqrt();
        if winding != 0 {
            -d
        } else {
            d
        }
    }
}

/// Polynomial smooth minimum: fillet weld of two SDFs with radius `k`.
fn smin(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b * (1.0 - h) + a * h - k * h * (1.0 - h)
}

/// Marching squares over the blended field: extracts the zero contour as
/// closed loops, returns the longest (the merged silhouette).
pub fn merged_outline(
    shapes: &[Poly],
    bbox: (f64, f64, f64, f64),
    weld: f64,
    resolution: usize,
) -> Vec<(f64, f64)> {
    let (min_x, min_y, max_x, max_y) = bbox;
    let pad = weld * 2.0 + 6.0;
    let (x0, y0) = (min_x - pad, min_y - pad);
    let (x1, y1) = (max_x + pad, max_y + pad);
    let n = resolution;
    let dx = (x1 - x0) / n as f64;
    let dy = (y1 - y0) / n as f64;

    // Field sampling.
    let mut field = vec![0.0_f64; (n + 1) * (n + 1)];
    for j in 0..=n {
        for i in 0..=n {
            let x = x0 + i as f64 * dx;
            let y = y0 + j as f64 * dy;
            let mut v = f64::MAX;
            for p in shapes {
                v = smin(v, p.sdf(x, y), weld);
            }
            field[j * (n + 1) + i] = v;
        }
    }
    let at = |i: usize, j: usize| field[j * (n + 1) + i];

    // Segment extraction (16-case, linear interpolation).
    let lerp = |a: f64, b: f64| a / (a - b);
    let mut segs: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let v00 = at(i, j);
            let v10 = at(i + 1, j);
            let v01 = at(i, j + 1);
            let v11 = at(i + 1, j + 1);
            let gx = x0 + i as f64 * dx;
            let gy = y0 + j as f64 * dy;
            let mut pts_on_edges: Vec<(f64, f64)> = Vec::new();
            if (v00 < 0.0) != (v10 < 0.0) {
                pts_on_edges.push((gx + dx * lerp(v00, v10), gy));
            }
            if (v10 < 0.0) != (v11 < 0.0) {
                pts_on_edges.push((gx + dx, gy + dy * lerp(v10, v11)));
            }
            if (v01 < 0.0) != (v11 < 0.0) {
                pts_on_edges.push((gx + dx * lerp(v01, v11), gy + dy));
            }
            if (v00 < 0.0) != (v01 < 0.0) {
                pts_on_edges.push((gx, gy + dy * lerp(v00, v01)));
            }
            // 2 crossings = one segment; 4 = ambiguous saddle, pair them
            // arbitrarily (fine at this resolution for organic blobs).
            if pts_on_edges.len() == 2 {
                segs.push((pts_on_edges[0], pts_on_edges[1]));
            } else if pts_on_edges.len() == 4 {
                crate::coverage::hit(&crate::coverage::COUNTERS.saddle);
                segs.push((pts_on_edges[0], pts_on_edges[1]));
                segs.push((pts_on_edges[2], pts_on_edges[3]));
            }
        }
    }

    // Chain segments into loops by endpoint proximity.
    let eps = (dx.min(dy)) * 0.5;
    let mut used = vec![false; segs.len()];
    let mut best_loop: Vec<(f64, f64)> = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        let mut chain = vec![segs[start].0, segs[start].1];
        used[start] = true;
        loop {
            let tail = *chain.last().unwrap();
            // NEAREST candidate wins, with lowest index as the tiebreak —
            // not "first within eps". Both rules are deterministic, but at a
            // saddle where two segments qualify, first-by-index chains into
            // an arbitrary branch while nearest follows the contour. The
            // tiebreak is pinned because ports must chain identically:
            // squared distances compare exactly in fixed point (no sqrt
            // needed for an ordering), so this rule survives the port.
            let eps2 = eps * eps;
            let mut best: Option<(f64, usize, bool)> = None;
            let mut candidates = 0usize;
            for (idx, seg) in segs.iter().enumerate() {
                if used[idx] {
                    continue;
                }
                let d0 = (seg.0 .0 - tail.0).powi(2) + (seg.0 .1 - tail.1).powi(2);
                let d1 = (seg.1 .0 - tail.0).powi(2) + (seg.1 .1 - tail.1).powi(2);
                let (d, head_end) = if d0 <= d1 { (d0, true) } else { (d1, false) };
                if d < eps2 {
                    candidates += 1;
                    if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                        best = Some((d, idx, head_end));
                    }
                }
            }
            if candidates > 1 {
                crate::coverage::hit(&crate::coverage::COUNTERS.chain_tiebreak);
            }
            let extended = match best {
                Some((_, idx, head_end)) => {
                    chain.push(if head_end { segs[idx].1 } else { segs[idx].0 });
                    used[idx] = true;
                    true
                }
                None => false,
            };
            if !extended {
                break;
            }
            let head = chain[0];
            let tail = *chain.last().unwrap();
            if ((head.0 - tail.0).powi(2) + (head.1 - tail.1).powi(2)).sqrt() < eps {
                break; // closed
            }
        }
        // Strict `>` pins the tiebreak on equal length: the EARLIEST loop in
        // grid-scan order wins. Ports iterate the same order, so they select
        // the same loop.
        if chain.len() > best_loop.len() {
            best_loop = chain;
        }
    }

    // Respline: resample by arc length, then one smoothing pass (the raw
    // marching-squares contour carries grid stair noise).
    resample_smooth(&best_loop, 160)
}

/// Even arc-length resampling + neighbor smoothing.
fn resample_smooth(pts: &[(f64, f64)], target: usize) -> Vec<(f64, f64)> {
    if pts.len() < 8 {
        return pts.to_vec();
    }
    let mut total = 0.0;
    let n = pts.len();
    let mut cum = Vec::with_capacity(n + 1);
    cum.push(0.0);
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        total += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        cum.push(total);
    }
    let mut out = Vec::with_capacity(target);
    for k in 0..target {
        let d = total * k as f64 / target as f64;
        let idx = cum.partition_point(|&c| c <= d).saturating_sub(1).min(n - 1);
        let seg_len = (cum[idx + 1] - cum[idx]).max(1e-12);
        let t = (d - cum[idx]) / seg_len;
        let a = pts[idx];
        let b = pts[(idx + 1) % n];
        out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
    }
    // Light circular smoothing: two 3-tap passes.
    for _ in 0..2 {
        let prev = out.clone();
        let m = prev.len();
        for i in 0..m {
            let a = prev[(i + m - 1) % m];
            let b = prev[i];
            let c = prev[(i + 1) % m];
            out[i] = (b.0 * 0.5 + (a.0 + c.0) * 0.25, b.1 * 0.5 + (a.1 + c.1) * 0.25);
        }
    }
    out
}
