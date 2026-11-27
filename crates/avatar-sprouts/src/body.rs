//! Bodies: twelve kinds of plant organ, each a mirror-symmetric outline
//! through control points the genome moves.
//!
//! An outline is given for its right-hand half, from the top of the axis to
//! the bottom; its lowest point is y = 0. Every kind also says where a face
//! may sit and where shoots may grow.

use crate::geometry::{box_of, path_of, pt, ptt, sample_of, Bx, PathD, Pt, W};
use crate::maths::{cos, hyp, js_round, lerp, max, min, sin, unit, PI, V};
use crate::reader::{dna, Reader};

/// The twelve body kinds, in the reference's order.
pub const BODY_KINDS: [&str; 12] = [
    "bulb", "seed", "tuber", "gourd", "pumpkin", "cone", "column", "taproot", "pod", "rosette",
    "berry", "bush",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Bulb,
    Seed,
    Tuber,
    Gourd,
    Pumpkin,
    Cone,
    Column,
    Taproot,
    Pod,
    Rosette,
    Berry,
    Bush,
}

impl Kind {
    pub(crate) fn name(self) -> &'static str {
        BODY_KINDS[self as usize]
    }
}

/// The body gene picks one of these slots in proportion to its weight.
/// Taproot and gourd each hold a second, smaller slot.
const BODY_SLOTS: [Kind; 14] = [
    Kind::Bulb,
    Kind::Seed,
    Kind::Tuber,
    Kind::Gourd,
    Kind::Pumpkin,
    Kind::Cone,
    Kind::Column,
    Kind::Taproot,
    Kind::Pod,
    Kind::Taproot,
    Kind::Gourd,
    Kind::Rosette,
    Kind::Berry,
    Kind::Bush,
];
const BODY_WEIGHTS: [u32; 14] = [3, 4, 3, 3, 2, 2, 3, 3, 2, 1, 1, 2, 2, 2];

/// How the shoot on top of a body grows: as the body's own continuation, or
/// as the stalk a fruit hung from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApexStyle {
    None,
    Trunk,
    Stalk,
}

/// What a body kind fixes: where the face and shoots go, as heights from the
/// top (0) to the bottom (1), and the weights of its roots, skins and coats.
pub(crate) struct KindSpec {
    pub(crate) eye: Option<[f64; 3]>,
    pub(crate) ear: Option<[f64; 2]>,
    pub(crate) arm: Option<[f64; 2]>,
    pub(crate) apex: f64,
    pub(crate) apex_style: ApexStyle,
    pub(crate) roots: [u32; 6],
    pub(crate) skins: &'static [(&'static str, u32)],
    pub(crate) coats: &'static [(&'static str, u32)],
}

#[allow(clippy::type_complexity)]
pub(crate) fn spec(kind: Kind) -> KindSpec {
    use ApexStyle as A;
    let (eye, ear, arm, apex, apex_style, roots, skins, coats): (
        Option<[f64; 3]>,
        Option<[f64; 2]>,
        Option<[f64; 2]>,
        f64,
        ApexStyle,
        [u32; 6],
        &'static [(&'static str, u32)],
        &'static [(&'static str, u32)],
    ) = match kind {
        Kind::Bulb => (
            None,
            None,
            None,
            1.0,
            A::Trunk,
            [2, 1, 5, 0, 1, 0],
            &[("veins", 5), ("stripes", 2), ("plain", 1), ("speckles", 1)],
            &[("none", 5), ("fuzz", 1), ("thorns", 1)],
        ),
        Kind::Seed => (
            Some([0.38, 0.58, 0.47]),
            Some([0.07, 0.2]),
            Some([0.38, 0.6]),
            0.3,
            A::Trunk,
            [4, 2, 1, 1, 3, 0],
            &[("plain", 2), ("stripes", 3), ("speckles", 3), ("veins", 1)],
            &[("none", 4), ("fuzz", 2), ("thorns", 1), ("spines", 1)],
        ),
        Kind::Tuber => (
            Some([0.36, 0.56, 0.45]),
            Some([0.06, 0.22]),
            Some([0.32, 0.6]),
            0.0,
            A::None,
            [2, 3, 3, 0, 2, 0],
            &[("dimples", 4), ("speckles", 3), ("plain", 1)],
            &[("none", 4), ("fuzz", 2)],
        ),
        Kind::Gourd => (
            None,
            None,
            None,
            0.75,
            A::Stalk,
            [4, 3, 0, 0, 2, 0],
            &[("stripes", 3), ("veins", 3), ("speckles", 2), ("plain", 1)],
            &[("none", 4), ("fuzz", 1), ("thorns", 1)],
        ),
        Kind::Pumpkin => (
            Some([0.38, 0.56, 0.46]),
            Some([0.07, 0.17]),
            Some([0.36, 0.6]),
            0.85,
            A::Stalk,
            [2, 4, 1, 0, 3, 0],
            &[("veins", 5), ("stripes", 2)],
            &[("none", 5), ("fuzz", 1)],
        ),
        Kind::Cone => (
            Some([0.48, 0.66, 0.56]),
            Some([0.14, 0.28]),
            Some([0.46, 0.66]),
            0.0,
            A::None,
            [3, 2, 0, 3, 1, 0],
            &[("scales", 1)],
            &[("bracts", 1)],
        ),
        Kind::Column => (
            Some([0.2, 0.4, 0.28]),
            None,
            Some([0.44, 0.66]),
            0.0,
            A::None,
            [3, 2, 2, 2, 0, 0],
            &[("ribs", 3), ("rings", 2), ("areoles", 2)],
            &[("spines", 4), ("thorns", 1), ("none", 2), ("fuzz", 1)],
        ),
        Kind::Taproot => (
            Some([0.2, 0.36, 0.27]),
            Some([0.03, 0.09]),
            Some([0.36, 0.5]),
            0.6,
            A::Trunk,
            [0, 0, 0, 2, 0, 5],
            &[("rings", 4), ("plain", 1), ("speckles", 1)],
            &[("none", 3), ("fuzz", 3)],
        ),
        Kind::Pod => (
            None,
            None,
            None,
            0.0,
            A::None,
            [4, 0, 1, 1, 1, 0],
            &[("seam", 3), ("plain", 2), ("speckles", 1)],
            &[("none", 3), ("fuzz", 2)],
        ),
        Kind::Rosette => (
            Some([0.42, 0.54, 0.47]),
            None,
            Some([0.64, 0.8]),
            0.0,
            A::None,
            [4, 0, 0, 2, 1, 0],
            &[("florets", 2), ("plain", 1)],
            &[("none", 1)],
        ),
        Kind::Berry => (
            Some([0.38, 0.56, 0.46]),
            Some([0.05, 0.16]),
            Some([0.4, 0.6]),
            0.6,
            A::Stalk,
            [3, 2, 0, 1, 2, 0],
            &[("drupelets", 1)],
            &[("none", 3), ("fuzz", 1)],
        ),
        Kind::Bush => (
            Some([0.46, 0.64, 0.55]),
            None,
            Some([0.5, 0.68]),
            0.0,
            A::None,
            [4, 2, 0, 1, 1, 0],
            &[("tufts", 1)],
            &[("none", 1)],
        ),
    };
    KindSpec {
        eye,
        ear,
        arm,
        apex,
        apex_style,
        roots,
        skins,
        coats,
    }
}

/// Something welded to the body: a disc, or a limb along an axis.
#[derive(Clone, Debug)]
pub(crate) enum Weld {
    Disc([f64; 3]),
    Axis(Vec<W>),
}

/// A built body.
#[derive(Clone, Debug)]
pub(crate) struct Body {
    pub(crate) kind: Kind,
    pub(crate) half: Vec<Pt>,
    pub(crate) poly: Vec<V>,
    pub(crate) d: PathD,
    pub(crate) bx: Bx,
    pub(crate) top: f64,
    pub(crate) bottom: f64,
    pub(crate) h: f64,
    pub(crate) w: f64,
    hw_y0: f64,
    hw_tab: Vec<f64>,
    pub(crate) eye: [f64; 3],
    pub(crate) ear: Option<[f64; 2]>,
    pub(crate) arm: Option<[f64; 2]>,
    pub(crate) apex_w: f64,
    pub(crate) apex_chance: f64,
    pub(crate) arm_scale: Option<f64>,
    pub(crate) welds: Vec<Weld>,
    pub(crate) seeds: usize,
    pub(crate) seed_h: f64,
    pub(crate) drupelet: f64,
    pub(crate) lobes: Vec<[f64; 3]>,
}

impl Body {
    /// The half-width at a height, from a table kept at every whole unit.
    pub(crate) fn hw(&self, y: f64) -> f64 {
        let i = js_round(y - self.hw_y0);
        if i < 0.0 || i >= self.hw_tab.len() as f64 {
            0.0
        } else {
            self.hw_tab[i as usize]
        }
    }
}

/// The right half of an egg: `n` + 1 control points.
fn egg_half(w: f64, h: f64, skew: f64, square: f64, n: usize) -> Vec<Pt> {
    (0..=n)
        .map(|k| {
            let a = -PI / 2.0 + PI * k as f64 / n as f64;
            let c = cos(a);
            let s = sin(a);
            let q = 1.0 + square * 4.0 * c * c * s * s;
            pt(
                if k == 0 || k == n {
                    0.0
                } else {
                    w * min(1.0, c * q) * (1.0 + skew * s)
                },
                -h / 2.0 + h / 2.0 * s,
            )
        })
        .collect()
}

/// Shape parts a kind sets while drawing its outline.
#[derive(Default)]
struct Extras {
    eye: Option<[f64; 3]>,
    ear: Option<Option<[f64; 2]>>,
    arm: Option<Option<[f64; 2]>>,
    apex_w: f64,
    apex_boost: f64,
    arm_scale: Option<f64>,
    seeds: usize,
    seed_h: f64,
}

fn outline_half(kind: Kind, r: &Reader<'_>, o: usize, x: &mut Extras) -> Vec<Pt> {
    match kind {
        Kind::Bulb => {
            let w = r.lin(o, 40.0, 56.0);
            let hb = r.lin(o + 1, 76.0, 102.0);
            let low = r.unit(o + 2);
            let nl = r.lin(o + 4, 8.0, 20.0);
            let nw = w * r.lin(o + 5, 0.24, 0.38);
            let h = hb + nl;
            let c = -hb / 2.0;
            x.eye = Some([-hb * 0.6, -hb * 0.38, -hb * 0.5]);
            x.ear = Some(None);
            x.arm = Some(Some([-hb * 0.72, -hb * 0.42]));
            x.apex_w = nw * 1.8;
            vec![
                pt(0.0, -h),
                pt(nw * 0.9, -h + nw * 0.5),
                pt(nw, -hb - nl * 0.25),
                pt(w * 0.68, c - hb * 0.36),
                pt(w * 0.95, c - hb * (0.1 - low * 0.06)),
                pt(w, c + hb * (0.1 + low * 0.1)),
                pt(w * 0.84, c + hb * 0.36),
                pt(w * 0.46, c + hb * 0.49),
                pt(0.0, 0.0),
            ]
        }
        Kind::Seed => {
            let rr = r.lin(o, 44.0, 54.0);
            let a = r.lin(o + 1, 0.78, 1.45);
            let w = rr / a.sqrt();
            let h = 2.0 * rr * a.sqrt();
            let mut half = egg_half(w, h, r.lin(o + 2, -0.22, 0.3), r.lin(o + 3, 0.0, 0.16), 8);
            if r.chance(o + 4, 0.35) {
                half[0].t = Some(0.3);
                half[1].x *= 0.66;
                x.apex_boost = 0.3;
            }
            x.apex_w = w * 0.4;
            half
        }
        Kind::Tuber => {
            let rr = r.lin(o, 44.0, 54.0);
            let a = r.lin(o + 1, 0.7, 1.2);
            let w = rr / a.sqrt();
            let h = 2.0 * rr * a.sqrt();
            let mut half = egg_half(w, h, r.lin(o + 2, -0.1, 0.2), r.lin(o + 3, 0.04, 0.2), 7);
            let last = half.len() - 1;
            for (i, p) in half.iter_mut().enumerate().take(last).skip(1) {
                p.x *= 1.0 + r.sgn(dna::BODY + 16 + i) * 0.15;
            }
            half
        }
        Kind::Gourd => {
            let big = r.lin(o, 42.0, 56.0);
            let small = big * r.lin(o + 1, 0.52, 0.86);
            let top_heavy = r.chance(o + 2, 0.3);
            let r1 = if top_heavy { big } else { small };
            let r2 = if top_heavy { small } else { big };
            let h1 = r1
                * if top_heavy {
                    r.lin(o + 3, 1.5, 1.85)
                } else {
                    r.lin(o + 4, 1.4, 2.3)
                };
            let h2 = r2
                * if top_heavy {
                    r.lin(o + 4, 1.3, 1.9)
                } else {
                    r.lin(o + 3, 1.5, 1.85)
                };
            let waist = min(r1, r2) * r.lin(o + 5, 0.62, 0.92);
            let h = h1 + h2;
            if top_heavy {
                x.eye = Some([-h + h1 * 0.4, -h + h1 * 0.64, -h + h1 * 0.52]);
                x.ear = Some(Some([-h + h1 * 0.1, -h + h1 * 0.22]));
                x.arm = Some(Some([-h2 * 0.8, -h2 * 0.45]));
                // The arms grow from the small lobe under the head, and they
                // and their leaves are in proportion to it.
                x.arm_scale = Some(0.8);
            } else {
                x.eye = Some([-h2 * 0.7, -h2 * 0.44, -h2 * 0.56]);
                x.ear = Some(None);
                x.arm = Some(Some([-h2 * 0.98, -h2 * 0.75]));
            }
            x.apex_w = r1 * 0.4;
            vec![
                pt(0.0, -h),
                pt(r1 * 0.7, -h + h1 * 0.1),
                pt(r1, -h + h1 * 0.45),
                pt(r1 * 0.9, -h + h1 * 0.8),
                pt(waist, -h2),
                pt(r2 * 0.93, -h2 * 0.72),
                pt(r2, -h2 * 0.42),
                pt(r2 * 0.8, -h2 * 0.12),
                pt(r2 * 0.42, -h2 * 0.005),
                pt(0.0, 0.0),
            ]
        }
        Kind::Pumpkin => {
            let w = r.lin(o, 56.0, 70.0);
            let h = r.lin(o + 1, 72.0, 92.0);
            let dent = r.lin(o + 4, 3.0, 8.0);
            let mut half = egg_half(w, h, r.lin(o + 2, -0.08, 0.1), r.lin(o + 3, 0.1, 0.24), 8);
            half[0].y += dent;
            half[1].y -= dent * 0.2;
            x.apex_w = w * 0.24;
            half
        }
        Kind::Cone => {
            let w = r.lin(o, 42.0, 54.0);
            let h = r.lin(o + 1, 98.0, 126.0);
            let mut half = egg_half(w, h, r.lin(o + 2, 0.05, 0.26), r.lin(o + 3, 0.0, 0.08), 8);
            half[0].t = Some(0.35);
            half[1].x *= 0.7;
            half[2].x *= 0.92;
            x.apex_w = w * 0.3;
            half
        }
        Kind::Column => {
            let w = r.lin(o, 31.0, 43.0);
            let h = r.lin(o + 1, 106.0, 146.0);
            egg_half(w, h, r.lin(o + 2, -0.1, 0.12), r.lin(o + 3, 0.26, 0.42), 8)
        }
        Kind::Taproot => {
            let w = r.lin(o, 44.0, 58.0);
            let h = r.lin(o + 1, 98.0, 134.0);
            let fat = r.unit(o + 2);
            let crown = r.lin(o + 3, 0.0, 5.0);
            x.apex_w = w * 0.5;
            vec![
                pt(0.0, -h + crown),
                pt(w * 0.5, -h + 0.5),
                pt(w * 0.92, -h * 0.87),
                pt(w, -h * 0.7),
                pt(w * (0.8 + 0.16 * fat), -h * 0.5),
                pt(w * (0.48 + 0.34 * fat), -h * 0.3),
                pt(w * (0.2 + 0.14 * fat), -h * 0.13),
                pt(w * 0.07, -h * 0.035),
                ptt(0.0, 0.0, 0.2),
            ]
        }
        Kind::Pod => {
            let n = if r.chance(o + 4, 0.45) { 4 } else { 3 };
            let w = r.lin(o, 39.0, 49.0);
            let sh = r.lin(o + 1, 30.0, 38.0);
            let pinch = r.lin(o + 2, 0.78, 0.92);
            let h = n as f64 * sh + 12.0;
            let mut half = vec![ptt(0.0, -h, 0.6), pt(w * 0.46, -h + 5.0)];
            for k in 0..n {
                let y = -h + 8.0 + sh * (k as f64 + 0.5);
                let width = w * if k == 0 {
                    0.9
                } else if k == n - 1 {
                    0.93
                } else {
                    1.0
                };
                half.push(pt(width, y));
                if k < n - 1 {
                    half.push(pt(w * pinch, y + sh / 2.0));
                }
            }
            half.push(pt(w * 0.5, -3.0));
            half.push(pt(0.0, 0.0));
            // The face sits two fifths of the way down, across a pinch if
            // need be: a pod is one body, not a stack of heads.
            let fy = -h * 0.6;
            x.seeds = n;
            x.seed_h = sh;
            x.eye = Some([fy - 6.0, fy + 8.0, fy]);
            x.ear = Some(None);
            x.arm = Some(Some([fy + sh * 0.5, fy + sh * 1.1]));
            half
        }
        Kind::Rosette => {
            let w = r.lin(o, 38.0, 46.0);
            egg_half(
                w,
                2.0 * w * r.lin(o + 1, 0.92, 1.06),
                0.0,
                r.lin(o + 3, 0.0, 0.06),
                8,
            )
        }
        Kind::Berry => {
            let w = r.lin(o, 44.0, 54.0);
            x.apex_w = w * 0.22;
            egg_half(
                w,
                2.0 * w * r.lin(o + 1, 0.95, 1.22),
                r.lin(o + 2, -0.26, 0.1),
                0.1,
                8,
            )
        }
        Kind::Bush => {
            let w = r.lin(o, 44.0, 54.0);
            egg_half(
                w,
                2.0 * w * r.lin(o + 1, 0.82, 1.0),
                r.lin(o + 2, 0.0, 0.14),
                0.14,
                8,
            )
        }
    }
}

/// A point of the right-hand outline at a height, with its outward normal.
pub(crate) struct OutlinePoint {
    pub(crate) p: V,
    pub(crate) n: V,
}

pub(crate) fn outline_at(b: &Body, y: f64) -> OutlinePoint {
    let poly = &b.poly;
    let n = poly.len();
    let mut best: Option<(f64, usize)> = None;
    for i in 0..n {
        let a = poly[i];
        let c = poly[(i + 1) % n];
        if a[1] == c[1] || (a[1] - y) * (c[1] - y) > 0.0 {
            continue;
        }
        let x = a[0] + (c[0] - a[0]) * (y - a[1]) / (c[1] - a[1]);
        if x >= 0.0 && best.is_none_or(|(bx, _)| x > bx) {
            best = Some((x, i));
        }
    }
    let Some((x, i)) = best else {
        return OutlinePoint {
            p: [0.0, y],
            n: [1.0, 0.0],
        };
    };
    // The outline runs clockwise, so down the right-hand side; its outward
    // normal is the direction of travel turned a quarter turn to the left.
    let a = poly[(i + n - 2) % n];
    let c = poly[(i + 3) % n];
    OutlinePoint {
        p: [x, y],
        n: unit(c[1] - a[1], -(c[0] - a[0])),
    }
}

/// The lowest point of the outline above `x`.
pub(crate) fn bottom_at(b: &Body, x: f64) -> f64 {
    let poly = &b.poly;
    let n = poly.len();
    let mut best = b.top;
    for i in 0..n {
        let a = poly[i];
        let c = poly[(i + 1) % n];
        if a[0] == c[0] || (a[0] - x) * (c[0] - x) > 0.0 {
            continue;
        }
        let y = a[1] + (c[1] - a[1]) * (x - a[0]) / (c[0] - a[0]);
        if y > best {
            best = y;
        }
    }
    best
}

/// Points a fixed distance apart down the right-hand side of the outline,
/// between two heights, each with its outward normal.
pub(crate) fn walk_outline(b: &Body, gap: f64, y0: f64, y1: f64) -> Vec<OutlinePoint> {
    let mut out = Vec::new();
    let poly = &b.poly;
    let count = (b.half.len() - 1) * 6;
    let mut acc = gap * 0.5;
    for i in 1..=count {
        let a = poly[i - 1];
        let c = poly[i % poly.len()];
        acc += hyp(c[0] - a[0], c[1] - a[1]);
        if acc < gap || c[1] < y0 || c[1] > y1 {
            continue;
        }
        acc = 0.0;
        let p = poly[i.saturating_sub(2)];
        let q = poly[(i + 2).min(count) % poly.len()];
        out.push(OutlinePoint {
            p: c,
            n: unit(q[1] - p[1], -(q[0] - p[0])),
        });
    }
    out
}

pub(crate) fn build_body(r: &Reader<'_>) -> Body {
    let kind = BODY_SLOTS[r.pick(dna::BODY, &BODY_WEIGHTS)];
    let k = spec(kind);
    let mut extras = Extras::default();
    let mut half = outline_half(kind, r, dna::BODY + 2, &mut extras);
    // No two bodies of a kind have quite the same outline. A tuber's own
    // lumps are larger and come from the same bytes.
    if kind != Kind::Tuber {
        let last = half.len() - 1;
        for (i, p) in half.iter_mut().enumerate().take(last).skip(1) {
            p.x *= 1.0 + r.sgn(dna::BODY + 16 + i) * 0.06;
        }
    }
    let mut ctrl = half.clone();
    for i in (1..half.len() - 1).rev() {
        let p = half[i];
        ctrl.push(Pt { x: -p.x, ..p });
    }
    let poly = sample_of(&ctrl, true, 6);
    let d = path_of(&ctrl, true);
    let bx = box_of(poly.iter().copied());
    let top = bx[1];
    let bottom = bx[3];
    let h = bottom - top;
    // Half-width at each whole unit of height.
    let y0 = top.floor();
    let rows = (bottom.ceil() - y0 + 1.0) as usize;
    let mut tab = vec![0.0; rows];
    for i in 0..poly.len() {
        let a = poly[i];
        let c = poly[(i + 1) % poly.len()];
        if a[0] < 0.0 && c[0] < 0.0 {
            continue;
        }
        let mut y = min(a[1], c[1]).ceil();
        while y <= max(a[1], c[1]).floor() {
            let x = if a[1] == c[1] {
                max(a[0], c[0])
            } else {
                a[0] + (c[0] - a[0]) * (y - a[1]) / (c[1] - a[1])
            };
            let row = (y - y0) as usize;
            if x > tab[row] {
                tab[row] = x;
            }
            y += 1.0;
        }
    }
    let w = tab.iter().fold(0.0, |m, &v| if v > m { v } else { m });
    let at = |t: f64| top + t * h;
    let band2 = |b: Option<[f64; 2]>| b.map(|b| [at(b[0]), at(b[1])]);
    let eye = extras.eye.unwrap_or_else(|| {
        k.eye
            .map(|b| [at(b[0]), at(b[1]), at(b[2])])
            .expect("every kind places the eyes")
    });
    let ear = extras.ear.unwrap_or_else(|| band2(k.ear));
    let arm = extras.arm.unwrap_or_else(|| band2(k.arm));
    let mut body = Body {
        kind,
        half,
        poly,
        d,
        bx,
        top,
        bottom,
        h,
        w,
        hw_y0: y0,
        hw_tab: tab,
        eye,
        ear,
        arm,
        apex_w: extras.apex_w,
        apex_chance: min(1.0, k.apex + extras.apex_boost),
        arm_scale: extras.arm_scale,
        welds: Vec::new(),
        seeds: extras.seeds,
        seed_h: extras.seed_h,
        drupelet: 0.0,
        lobes: Vec::new(),
    };
    after(&mut body, r);
    body
}

/// What a kind adds once its outline is known: knobs, curled tips,
/// drupelets and the lobes of a shrub, all welded to the body.
fn after(b: &mut Body, r: &Reader<'_>) {
    match b.kind {
        Kind::Tuber => {
            // Knobs welded to the flanks, where a tuber's eyes swell.
            let pairs = r.one(dna::BODY + 24, 3) as usize;
            for i in 0..pairs {
                let at = outline_at(b, b.top + b.h * r.lin(dna::BODY + 25 + 2 * i, 0.16, 0.8));
                let rr = r.lin(dna::BODY + 26 + 2 * i, 8.0, 13.0);
                let c = [at.p[0] - at.n[0] * rr * 0.3, at.p[1] - at.n[1] * rr * 0.3];
                b.welds.push(Weld::Disc([c[0], c[1], rr]));
                b.welds.push(Weld::Disc([-c[0], c[1], rr]));
            }
        }
        Kind::Taproot => {
            let side = if r.chance(dna::EXTRA, 0.5) { 1.0 } else { -1.0 };
            let len = r.lin(dna::EXTRA + 1, 0.7, 1.3);
            b.welds.push(Weld::Axis(vec![
                [0.0, -6.0, 7.5],
                [side * 5.0 * len, -1.5, 4.6],
                [side * 12.0 * len, -3.5, 2.8],
                [side * 16.0 * len, -9.0 * len, 1.8],
            ]));
        }
        Kind::Pod => {
            let s = if r.chance(dna::EXTRA, 0.5) { 1.0 } else { -1.0 };
            let k = r.lin(dna::EXTRA + 1, 0.7, 1.25);
            let t = b.top;
            b.welds.push(Weld::Axis(vec![
                [0.0, t + 5.0, 9.0],
                [s * 2.0 * k, t - 4.0 * k, 6.0],
                [s * 8.0 * k, t - 9.0 * k, 4.0],
                [s * 13.0 * k, t - 6.0 * k, 2.8],
                [s * 13.0 * k, t - 1.0 * k, 2.0],
            ]));
        }
        Kind::Berry => {
            // Drupelets along the rim, welded on, so the edge is bumpy.
            let rr = r.lin(dna::EXTRA, 8.0, 13.0);
            b.drupelet = rr;
            for s in walk_outline(b, rr * 1.5, b.top + rr * 0.4, -rr * 0.4) {
                let c = [s.p[0] - s.n[0] * rr * 0.3, s.p[1] - s.n[1] * rr * 0.3];
                if c[0] < rr * 0.7 {
                    continue;
                }
                b.welds.push(Weld::Disc([c[0], c[1], rr]));
                b.welds.push(Weld::Disc([-c[0], c[1], rr]));
            }
            b.welds.push(Weld::Disc([0.0, b.top + rr * 0.3, rr]));
            b.welds.push(Weld::Disc([0.0, -rr * 0.3, rr]));
        }
        Kind::Bush => {
            let n = 2 + r.one(dna::EXTRA, 3) as usize;
            let r0 = r.lin(dna::EXTRA + 1, 15.0, 23.0);
            let mut lobes = Vec::new();
            for i in 0..n {
                let t = lerp(
                    0.1,
                    0.64,
                    if n == 1 {
                        0.5
                    } else {
                        i as f64 / (n - 1) as f64
                    },
                ) + r.sgn(dna::EXTRA + 2 + i) * 0.05;
                let rr = r0 * r.lin(dna::EXTRA + 6 + i, 0.8, 1.2);
                let at = outline_at(b, b.top + b.h * t);
                let c = [at.p[0] - at.n[0] * rr * 0.45, at.p[1] - at.n[1] * rr * 0.45];
                if c[0] < rr * 0.5 {
                    continue;
                }
                lobes.push([c[0], c[1], rr]);
                lobes.push([-c[0], c[1], rr]);
            }
            let rt = r0 * r.lin(dna::EXTRA + 10, 0.9, 1.25);
            lobes.push([0.0, b.top + rt * 0.5, rt]);
            for l in &lobes {
                b.welds.push(Weld::Disc(*l));
            }
            b.lobes = lobes;
        }
        _ => {}
    }
}
