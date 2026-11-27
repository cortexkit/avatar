//! The stuffed pieces (a one-piece body, a head, a body) and the tubes that
//! make ears, horns, limbs and tails.

use crate::decode::PieceGenes;
use crate::geom::{
    add, clamp, cos, crossings, flip_x, max, max_of, min_of, perp, scl, signed_area, sin, sq, sub,
    unit, Pt, PI, TAU,
};

/// A piece's radius at angle t from its crown (t = 0) round the right side
/// to its base (t = pi): an ellipse with a taper, made fuller or more
/// pointed halfway between its axes by its crown and base fullness, and
/// pinched at the middle for a bean's waist. The waist is a bell-shaped dip,
/// 1 / (1 + x^2 + x^4 / 2).
fn base_r(p: &PieceGenes, t: f64) -> f64 {
    let s = sin(t).abs();
    let c = cos(t);
    let up = c >= 0.0;
    let b = if up { p.b_t } else { p.b_b };
    let k = if up { p.k_t } else { p.k_b };
    let mut a = p.a * (1.0 + p.taper * -c);
    if p.pinch != 0.0 {
        let x2 = sq((t - PI / 2.0) / 0.38);
        a *= 1.0 - p.pinch * 0.3 / (1.0 + x2 + 0.5 * x2 * x2);
    }
    let u = sq(s / a);
    let v = sq(c / b);
    let w = u + v;
    (1.0 + k * 4.0 * u * v / (w * w)) / w.sqrt()
}

/// The deviation interpolated between the DNA spokes (Catmull-Rom round the
/// whole outline, the left side mirroring the right).
fn dev_at(p: &PieceGenes, t: f64) -> f64 {
    let s = &p.spokes;
    let mut full: Vec<(f64, f64)> = s.iter().map(|s| (s.t, s.dev)).collect();
    for k in (1..=s.len() - 2).rev() {
        full.push((TAU - s[k].t, s[k].dev));
    }
    let n = full.len() as i64;
    let tt = ((t % TAU) + TAU) % TAU;
    let mut i = n - 1;
    for k in 0..n - 1 {
        if tt >= full[k as usize].0 && tt < full[k as usize + 1].0 {
            i = k;
            break;
        }
    }
    let g = |k: i64| full[(((k % n) + n) % n) as usize].1;
    let ta = full[i as usize].0;
    let tb = if i + 1 < n {
        full[i as usize + 1].0
    } else {
        TAU
    };
    let mut span = tb - ta;
    if span == 0.0 {
        span = 1.0;
    }
    let u = clamp((tt - ta) / span, 0.0, 1.0);
    let p0 = g(i - 1);
    let p1 = g(i);
    let p2 = g(i + 1);
    let p3 = g(i + 2);
    0.5 * (2.0 * p1
        + (-p0 + p2) * u
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u * u * u)
}

pub fn piece_r(p: &PieceGenes, t: f64) -> f64 {
    base_r(p, t) * (1.0 + dev_at(p, t))
}

/// A stuffed piece in toy space.
#[derive(Clone, Debug)]
pub struct Piece {
    pub c: Pt,
    pub p: PieceGenes,
    pub outline: Vec<Pt>,
    pub y_top: f64,
    pub y_bot: f64,
    pub h: f64,
    pub w: f64,
}

impl Piece {
    /// The half-width of the outline at height y, from the piece's axis.
    pub fn half_w(&self, y: f64) -> f64 {
        let xs = crossings(&self.outline, y);
        if xs.is_empty() {
            0.0
        } else {
            max(
                (xs[0] - self.c[0]).abs(),
                (xs[xs.len() - 1] - self.c[0]).abs(),
            )
        }
    }

    /// The outline point at angle t and its outward normal, from the
    /// unflattened function (parts are sewn above the flattened base).
    pub fn at(&self, t: f64) -> (Pt, Pt) {
        let r = piece_r(&self.p, t);
        let e = 1e-3;
        let r2 = piece_r(&self.p, t + e);
        let c = self.c;
        let p = [c[0] + r * sin(t), c[1] - r * cos(t)];
        let q = [c[0] + r2 * sin(t + e), c[1] - r2 * cos(t + e)];
        let tg = unit(sub(q, p));
        (p, [tg[1], -tg[0]])
    }
}

pub fn make_piece(p: &PieceGenes, c: Pt, sits: bool) -> Piece {
    const N: usize = 72;
    let mut right: Vec<Pt> = (0..=N)
        .map(|i| {
            let t = i as f64 / N as f64 * PI;
            let r = piece_r(p, t);
            [r * sin(t), -r * cos(t)]
        })
        .collect();
    if sits {
        // A sitting piece rests on its base, pressed flat where it meets the
        // table.
        let y0 = min_of(right.iter().map(|p| p[1]));
        let y1 = max_of(right.iter().map(|p| p[1]));
        let yf = y1 - (y1 - y0) * 0.15;
        for q in &mut right {
            if q[1] > yf {
                q[1] = yf + (q[1] - yf) * 0.32;
            }
        }
    }
    let mut outline: Vec<Pt> = right.clone();
    outline.extend(right[1..N].iter().rev().map(|&q| flip_x(q)));
    let outline: Vec<Pt> = outline
        .into_iter()
        .map(|q| [q[0] + c[0], q[1] + c[1]])
        .collect();
    let y_top = min_of(outline.iter().map(|q| q[1]));
    let y_bot = max_of(outline.iter().map(|q| q[1]));
    let w = max_of(outline.iter().map(|q| (q[0] - c[0]).abs()));
    Piece {
        c,
        p: p.clone(),
        outline,
        y_top,
        y_bot,
        h: y_bot - y_top,
        w,
    }
}

/// A tube: a spine from its sewn-on base to its tip, and a half-width along
/// it.
#[derive(Clone, Debug)]
pub struct Tube {
    pub spine: Vec<Pt>,
    pub ws: Vec<f64>,
    pub nrm: Vec<Pt>,
    pub tan: Vec<Pt>,
    pub outline: Vec<Pt>,
    pub tip_w: f64,
}

/// A spine of length `l` from `base`, starting at angle `ang` and turning by
/// `curl` in all, in `n` steps (18 when `None`). With `late` (0 to 1) it runs
/// straight for that share of its length and does all its turning after it.
pub fn arc_spine(base: Pt, ang: f64, l: f64, curl: f64, n: Option<usize>, late: f64) -> Vec<Pt> {
    let n = n.unwrap_or(18);
    let nf = n as f64;
    let mut out = vec![base];
    let mut a = ang;
    let mut p = base;
    for i in 0..n {
        a += if late != 0.0 {
            if (i as f64 + 0.5) / nf > late {
                curl / (nf * (1.0 - late))
            } else {
                0.0
            }
        } else {
            curl / nf
        };
        p = [p[0] + cos(a) * l / nf, p[1] + sin(a) * l / nf];
        out.push(p);
    }
    out
}

pub fn make_tube(spine: Vec<Pt>, prof: &dyn Fn(f64) -> f64, w: f64, round_base: bool) -> Tube {
    let n = spine.len();
    let mut ws = Vec::with_capacity(n);
    let mut nrm = Vec::with_capacity(n);
    let mut tan = Vec::with_capacity(n);
    for i in 0..n {
        let a = spine[i.saturating_sub(1)];
        let b = spine[(i + 1).min(n - 1)];
        let t = unit(sub(b, a));
        tan.push(t);
        nrm.push(perp(t));
        ws.push(max(0.05, w * prof(i as f64 / (n - 1) as f64)));
    }
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        left.push(add(spine[i], scl(nrm[i], ws[i])));
        right.push(sub(spine[i], scl(nrm[i], ws[i])));
    }
    let tip_w = ws[n - 1];
    let mut cap = Vec::new();
    if tip_w > 0.6 {
        // A round tip: a half circle on the last spine point.
        let t = tan[n - 1];
        let nn = nrm[n - 1];
        for k in 1..12 {
            let a = PI * k as f64 / 12.0;
            cap.push(add(
                spine[n - 1],
                add(scl(nn, cos(a) * tip_w), scl(t, sin(a) * tip_w)),
            ));
        }
    } else {
        cap.push(add(spine[n - 1], scl(tan[n - 1], tip_w * 0.6)));
    }
    let mut outline = left;
    outline.extend(cap);
    outline.extend(right.into_iter().rev());
    if round_base {
        let t = tan[0];
        let nn = nrm[0];
        let w0 = ws[0];
        for k in 1..10 {
            let a = PI * k as f64 / 10.0;
            outline.push(add(
                spine[0],
                add(scl(nn, -cos(a) * w0), scl(t, -sin(a) * w0)),
            ));
        }
    }
    if signed_area(&outline) < 0.0 {
        outline.reverse();
    }
    Tube {
        spine,
        ws,
        nrm,
        tan,
        outline,
        tip_w,
    }
}

/// The tube mirrored to the toy's left side.
pub fn mirror_tube(tb: &Tube) -> Tube {
    let mut outline: Vec<Pt> = tb.outline.iter().map(|&p| flip_x(p)).collect();
    outline.reverse();
    Tube {
        spine: tb.spine.iter().map(|&p| flip_x(p)).collect(),
        ws: tb.ws.clone(),
        nrm: tb.nrm.iter().map(|&p| flip_x(p)).collect(),
        tan: tb.tan.iter().map(|&p| flip_x(p)).collect(),
        outline,
        tip_w: tb.tip_w,
    }
}
