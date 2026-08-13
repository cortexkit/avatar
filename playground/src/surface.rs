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
/// Sites are decoded on the RIGHT half and mirrored, so the pattern shares
/// the body's symmetry axis (grid-8 review: an asymmetric mottle on a
/// symmetric body reads as a symmetry BUG, not as texture).
pub fn decode_sites(b: &mut Bits) -> Vec<Site> {
    let pairs = 3 + (b.byte() % 4) as usize; // 3..=6 pairs -> 6..=12 cells
    let mut sites = Vec::with_capacity(pairs * 2);
    for _ in 0..pairs {
        let x = b.lin(0.04, 1.05);
        let y = b.lin(-1.05, 1.05);
        let shade = b.byte() % 3;
        sites.push(Site { x, y, shade });
        sites.push(Site { x: -x, y, shade });
    }
    sites
}

/// Polygon centroid (area-weighted; falls back to vertex mean for slivers).
fn centroid(poly: &[(f64, f64)]) -> (f64, f64) {
    let n = poly.len();
    let mut a2 = 0.0;
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % n];
        let cross = x1 * y2 - x2 * y1;
        a2 += cross;
        cx += (x1 + x2) * cross;
        cy += (y1 + y2) * cross;
    }
    if a2.abs() < 1e-9 {
        let m = poly.iter().fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
        return (m.0 / n as f64, m.1 / n as f64);
    }
    (cx / (3.0 * a2), cy / (3.0 * a2))
}

/// Lloyd relaxation: move each site to its cell centroid. Evens the cells
/// into organic, similar-sized regions (the "cracked glass" look of raw
/// random sites comes from wildly uneven cells). Mirror pairs stay mirrored
/// because the whole configuration is symmetric.
pub fn relax(sites: &mut Vec<Site>, bbox: (f64, f64, f64, f64), iterations: usize) {
    for _ in 0..iterations {
        let polys = cells(sites, bbox);
        for (site, poly) in sites.iter_mut().zip(polys.iter()) {
            if poly.len() >= 3 {
                let (cx, cy) = centroid(poly);
                site.x = cx;
                site.y = cy;
            }
        }
    }
}

/// Rounded, inset cell path: shrink toward the centroid (grout gap), then
/// round every corner with quadratics — pebbles, not shards.
pub fn cell_path(poly: &[(f64, f64)], inset: f64) -> String {
    let n = poly.len();
    if n < 3 {
        return String::new();
    }
    let (cx, cy) = centroid(poly);
    let pts: Vec<(f64, f64)> = poly
        .iter()
        .map(|&(x, y)| (cx + (x - cx) * inset, cy + (y - cy) * inset))
        .collect();
    let mut d = String::new();
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let cur = pts[i];
        let next = pts[(i + 1) % n];
        let a = (cur.0 + (prev.0 - cur.0) * 0.26, cur.1 + (prev.1 - cur.1) * 0.26);
        let b = (cur.0 + (next.0 - cur.0) * 0.26, cur.1 + (next.1 - cur.1) * 0.26);
        if i == 0 {
            d.push_str(&format!("M{:.1} {:.1} ", a.0, a.1));
        } else {
            d.push_str(&format!("L{:.1} {:.1} ", a.0, a.1));
        }
        d.push_str(&format!("Q{:.1} {:.1} {:.1} {:.1} ", cur.0, cur.1, b.0, b.1));
    }
    d.push('Z');
    d
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
