//! Surface patterns: REAL Voronoi, computed, not premade.
//!
//! TOG shipped a premade voronoi asset because EVM gas made computing one
//! impossible. Off-chain we can afford exact cells: each site's cell is the
//! bounding box clipped by the perpendicular-bisector half-plane against
//! every other site (Sutherland-Hodgman, O(n^2) with n <= 12 — trivial).
//! Cells clip to the body outline via the SVG clipPath the renderer already
//! maintains, and shade alternation derives from the palette so the mottle
//! belongs to the creature instead of sitting on it.

use crate::genome::Bits;

/// One Voronoi site with its shade role.
pub struct Site {
    pub x: f64,
    pub y: f64,
    /// 0 = darker mottle, 1 = lighter mottle, 2 = skip (body shows through).
    pub shade: u8,
}

/// Decode sites in unit-square coordinates ([-1,1]^2, scaled by the caller).
pub fn decode_sites(b: &mut Bits) -> Vec<Site> {
    let n = 5 + (b.byte() % 8) as usize; // 5..=12 sites
    let mut sites = Vec::with_capacity(n);
    for _ in 0..n {
        sites.push(Site {
            x: b.lin(-1.05, 1.05),
            y: b.lin(-1.05, 1.05),
            shade: b.byte() % 3,
        });
    }
    sites
}

/// Sutherland-Hodgman: clip `poly` to the half-plane of points closer to
/// site `a` than site `b` (the perpendicular bisector).
fn clip_halfplane(poly: &[(f64, f64)], a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    // Inside test: dot(p - mid, b - a) <= 0  (closer to a).
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let dir = (b.0 - a.0, b.1 - a.1);
    let side = |p: (f64, f64)| (p.0 - mid.0) * dir.0 + (p.1 - mid.1) * dir.1;
    let n = poly.len();
    let mut out = Vec::with_capacity(n + 2);
    for i in 0..n {
        let cur = poly[i];
        let nxt = poly[(i + 1) % n];
        let sc = side(cur);
        let sn = side(nxt);
        if sc <= 0.0 {
            out.push(cur);
        }
        if (sc < 0.0 && sn > 0.0) || (sc > 0.0 && sn < 0.0) {
            let t = sc / (sc - sn);
            out.push((cur.0 + (nxt.0 - cur.0) * t, cur.1 + (nxt.1 - cur.1) * t));
        }
    }
    out
}

/// Exact Voronoi cells within `bbox` (min_x, min_y, max_x, max_y).
/// Returns one polygon per site (possibly empty when crowded out).
pub fn cells(sites: &[Site], bbox: (f64, f64, f64, f64)) -> Vec<Vec<(f64, f64)>> {
    let base = vec![
        (bbox.0, bbox.1),
        (bbox.2, bbox.1),
        (bbox.2, bbox.3),
        (bbox.0, bbox.3),
    ];
    sites
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut poly = base.clone();
            for (j, o) in sites.iter().enumerate() {
                if i != j && !poly.is_empty() {
                    poly = clip_halfplane(&poly, (s.x, s.y), (o.x, o.y));
                }
            }
            poly
        })
        .collect()
}
