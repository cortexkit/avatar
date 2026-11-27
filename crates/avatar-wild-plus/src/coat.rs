//! The coat: the edge of the silhouette, grown outward from the body's
//! outline as wool, fur, spines, warts or lumps.

use crate::genes::Reader;
use crate::geom::{clamp, dist, js_round, lerp, max, min, or, smooth01, unit, P};

pub const COATS: [&str; 6] = ["smooth", "woolly", "shaggy", "spiny", "warty", "lumpy"];
const COAT_WEIGHTS: [f64; 6] = [8.0, 3.0, 3.0, 2.0, 2.0, 2.0];

/// One segment of the body path's right half.
#[derive(Clone, Copy)]
pub enum Seg {
    L(P),
    C(P, P, P),
}

impl Seg {
    pub fn end(&self) -> P {
        match *self {
            Seg::L(p) | Seg::C(_, _, p) => p,
        }
    }
}

/// The coat's genes.
pub struct CoatGenes {
    pub kind: usize,
    pub chord: f64,
    pub amp: f64,
    pub reach: f64,
    pub region: usize,
    pub lean: f64,
}

pub fn coat_genes(r: &Reader) -> CoatGenes {
    let kind = r.pick(1552, &COAT_WEIGHTS);
    if kind == 0 {
        return CoatGenes {
            kind,
            chord: 0.0,
            amp: 0.0,
            reach: 0.0,
            region: 0,
            lean: 0.0,
        };
    }
    let size = r.unit(1553);
    let tall = r.unit(1554);
    let chord = [
        0.0,
        lerp(9.0, 22.0, size),
        lerp(6.0, 13.0, size),
        lerp(10.0, 20.0, size),
        lerp(10.0, 18.0, size),
        lerp(22.0, 40.0, size),
    ][kind];
    let amp = min(
        20.0,
        chord
            * [
                0.0,
                lerp(0.3, 0.55, tall),
                lerp(0.55, 1.1, tall),
                lerp(0.6, 1.3, tall),
                lerp(0.3, 0.5, tall),
                lerp(0.14, 0.26, tall),
            ][kind],
    );
    // everywhere, the crown only, or the lower body only
    CoatGenes {
        kind,
        chord,
        amp,
        reach: amp * 1.3,
        region: r.pick(1555, &[5.0, 2.0, 2.0]),
        lean: r.lin(1556, 0.1, 0.9),
    }
}

fn cumulative(pts: &[P]) -> Vec<f64> {
    let mut cum = vec![0.0];
    for i in 1..pts.len() {
        cum.push(cum[i - 1] + dist(pts[i - 1][0], pts[i - 1][1], pts[i][0], pts[i][1]));
    }
    cum
}

/// Point at arc length `d` along a chain with cumulative lengths.
fn chain_at(pts: &[P], cum: &[f64], d: f64) -> P {
    let (mut lo, mut hi) = (0usize, cum.len() - 1);
    while hi - lo > 1 {
        let mid = (lo + hi) >> 1;
        if cum[mid] <= d {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let span = or(cum[hi] - cum[lo], 1e-12);
    let t = clamp((d - cum[lo]) / span, 0.0, 1.0);
    [
        lerp(pts[lo][0], pts[hi][0], t),
        lerp(pts[lo][1], pts[hi][1], t),
    ]
}

/// The hand of the genome in the outline: two dozen genes push the skin out
/// or in by a unit or two along its length. The base, where the body rests,
/// is left flat. Works on the right half; the left mirrors it.
pub fn wobble_half(r: &Reader, half: &[P], bottom_y: f64) -> Vec<P> {
    let amp = r.lin(2432, 0.7, 2.4);
    let knots: Vec<f64> = (0..24).map(|i| r.sgn(2433 + i)).collect();
    let cum = cumulative(half);
    let total = or(cum[cum.len() - 1], 1.0);
    let n = half.len();
    half.iter()
        .enumerate()
        .map(|(i, &q)| {
            let a = half[i.saturating_sub(1)];
            let b = half[(i + 1).min(n - 1)];
            let t = unit(b[0] - a[0], b[1] - a[1]);
            let (nx, ny) = (t[1], -t[0]);
            let u = cum[i] / total * 23.0;
            let k = min(22.0, u.floor()) as usize;
            let f = smooth01(u - k as f64);
            let mut d = amp * lerp(knots[k], knots[k + 1], f);
            if ny > 0.75 && q[1] > bottom_y - 8.0 {
                d = 0.0;
            }
            // The ends stay on the axis: only their height moves.
            if i == 0 || i == n - 1 {
                [q[0], q[1] + ny * d]
            } else {
                [q[0] + nx * d, q[1] + ny * d]
            }
        })
        .collect()
}

/// Turns the right half of the outline (frame units, crown to base) into
/// path segments, growing the coat outward from it. `depth(x, y)` is how far
/// inside the body a point lies: the coat thins out where the body is thin
/// and stops where the body rests on the ground.
pub fn coat_half(
    r: &Reader,
    g: &CoatGenes,
    half: &[P],
    depth: &dyn Fn(f64, f64) -> f64,
    bottom_y: f64,
) -> Vec<Seg> {
    let kind = g.kind;
    let mut segs = Vec::new();
    if kind == 0 {
        for p in &half[1..] {
            segs.push(Seg::L(*p));
        }
        return segs;
    }
    let (chord, amp, region, lean) = (g.chord, g.amp, g.region, g.lean);
    let cum = cumulative(half);
    let total = cum[cum.len() - 1];
    let teeth = max(4.0, js_round(total / chord)) as usize;
    let step = total / teeth as f64;
    let top_y = half[0][1];
    let height = bottom_y - top_y;
    let mut at = 1usize;
    for k in 0..teeth {
        let d0 = k as f64 * step;
        let d1 = (k as f64 + 1.0) * step;
        let p0 = chain_at(half, &cum, d0);
        let p1 = if k == teeth - 1 {
            half[half.len() - 1]
        } else {
            chain_at(half, &cum, d1)
        };
        let pm = chain_at(half, &cum, (d0 + d1) / 2.0);
        let t = unit(p1[0] - p0[0], p1[1] - p0[1]);
        let n = [t[1], -t[0]];
        // How much coat this stretch carries.
        let probe = max(amp * 1.2, 5.0);
        let mut w = clamp(
            (depth(pm[0] - n[0] * probe, pm[1] - n[1] * probe) / probe - 0.45) / 0.35,
            0.0,
            1.0,
        );
        if n[1] > 0.8 && pm[1] > bottom_y - 7.0 {
            w = 0.0;
        }
        let v = (pm[1] - top_y) / height;
        if region == 1 {
            w *= 1.0 - smooth01((v - 0.3) / 0.2);
        }
        if region == 2 {
            w *= smooth01((v - 0.4) / 0.2);
        }
        let gene = r.u8(1568 + k % 256);
        let a = amp * w * lerp(0.55, 1.3, f64::from(gene) / 255.0);
        while at < half.len() && cum[at] <= d0 + 1e-9 {
            at += 1;
        }
        if w < 0.15 || (kind == 4 && !gene.is_multiple_of(3)) {
            // No coat here: keep the skin's own line.
            while at < half.len() && cum[at] < d1 - 1e-9 {
                segs.push(Seg::L(half[at]));
                at += 1;
            }
            segs.push(Seg::L(p1));
            continue;
        }
        if kind == 2 {
            // Fur: a lock that leans the way it hangs.
            let tip = [
                p0[0] + t[0] * step * (0.45 + lean * 0.7) + n[0] * a,
                p0[1] + t[1] * step * (0.45 + lean * 0.7) + n[1] * a,
            ];
            segs.push(Seg::C(
                [p0[0] + n[0] * a * 0.55, p0[1] + n[1] * a * 0.55],
                [tip[0] - t[0] * step * 0.25, tip[1] - t[1] * step * 0.25],
                tip,
            ));
            segs.push(Seg::L(p1));
        } else if kind == 3 {
            segs.push(Seg::L([
                pm[0] + n[0] * a + t[0] * step * (lean - 0.5) * 0.5,
                pm[1] + n[1] * a + t[1] * step * (lean - 0.5) * 0.5,
            ]));
            segs.push(Seg::L(p1));
        } else {
            // A round bump: wool, a wart, or one swell of a lumpy edge.
            let lift = a * 1.33;
            let slack = step * (if kind == 5 { 0.3 } else { 0.05 });
            segs.push(Seg::C(
                [
                    p0[0] + n[0] * lift + t[0] * slack,
                    p0[1] + n[1] * lift + t[1] * slack,
                ],
                [
                    p1[0] + n[0] * lift - t[0] * slack,
                    p1[1] + n[1] * lift - t[1] * slack,
                ],
                p1,
            ));
        }
    }
    segs
}

/// The closed body path from the right half's segments: down the right
/// side, then the mirror image back up the left, about `axis`.
pub fn close_mirrored(start: P, segs: &[Seg], axis: f64) -> Vec<Seg> {
    let mx = |p: P| [2.0 * axis - p[0], p[1]];
    let mut pts = vec![start];
    let mut out: Vec<Seg> = segs.to_vec();
    for s in segs {
        pts.push(s.end());
    }
    for i in (0..segs.len()).rev() {
        let to = mx(pts[i]);
        out.push(match segs[i] {
            Seg::L(_) => Seg::L(to),
            Seg::C(c1, c2, _) => Seg::C(mx(c2), mx(c1), to),
        });
    }
    out
}
