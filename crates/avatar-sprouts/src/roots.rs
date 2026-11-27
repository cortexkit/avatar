//! Roots: legs that are thick where they leave the body, taper, bend at a
//! knee and end in a foot. The feet stay where they are; when a mood lifts,
//! lowers or tips the body, the knees bend to follow.

use crate::body::{bottom_at, outline_at, Body};
use crate::geometry::{circle_d, limb_path, smooth_axis, Bx, PathD};
use crate::maths::{hyp, max, min, rot, V};
use crate::reader::{dna, Reader};

pub const ROOT_LAYOUTS: [&str; 6] = ["biped", "quad", "skirt", "stilt", "nubs", "tripod"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Toe {
    Foot,
    Toes,
    Curl,
    Nub,
}

#[derive(Clone, Debug)]
pub(crate) struct Leg {
    pub(crate) skirt: bool,
    pub(crate) hip: V,
    pub(crate) foot: f64,
    pub(crate) w0: f64,
    pub(crate) w1: f64,
    pub(crate) slack: f64,
    pub(crate) toe: Toe,
    pub(crate) toe_len: f64,
    pub(crate) pair: bool,
    pub(crate) back: bool,
    pub(crate) curl: f64,
    pub(crate) side: f64,
    /// The leg's length at rest.
    pub(crate) l: f64,
}

impl Leg {
    fn new(hip: V, foot: f64, w0: f64) -> Self {
        Self {
            skirt: false,
            hip,
            foot,
            w0,
            w1: 0.0,
            slack: 0.0,
            toe: Toe::Foot,
            toe_len: 0.0,
            pair: true,
            back: false,
            curl: 0.0,
            side: 1.0,
            l: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Roots {
    pub(crate) layout: usize,
    pub(crate) lift: f64,
    pub(crate) legs: Vec<Leg>,
}

pub(crate) fn build_roots(r: &Reader<'_>, b: &Body, least: f64) -> Roots {
    let o = dna::ROOTS;
    let layout = r.pick(o, &crate::body::spec(b.kind).roots);
    let mut legs = Vec::new();
    let low = b.hw(b.bottom - b.h * 0.14);
    let toe = [Toe::Foot, Toe::Toes, Toe::Curl, Toe::Nub][r.pick(o + 7, &[3, 3, 2, 1])];
    let toe_len = r.lin(o + 8, 8.0, 15.0);
    let hip_at = |x: f64| [x, bottom_at(b, x) - 5.0];
    let lift;
    match ROOT_LAYOUTS[layout] {
        "biped" => {
            lift = r.lin(o + 1, 11.0, 30.0);
            let hx = low * r.lin(o + 2, 0.34, 0.62);
            let w0 = r.lin(o + 4, 9.5, 16.0);
            let mut leg = Leg::new(hip_at(hx), hx + r.lin(o + 3, 0.0, 9.0), w0);
            leg.w1 = w0 * r.lin(o + 5, 0.58, 0.82);
            leg.slack = r.lin(o + 6, 0.03, 0.13);
            leg.toe = toe;
            leg.toe_len = toe_len;
            legs.push(leg);
        }
        "quad" => {
            lift = r.lin(o + 1, 9.0, 22.0);
            let hx = low * r.lin(o + 2, 0.26, 0.42);
            let w0 = r.lin(o + 4, 9.0, 14.0);
            let w1 = w0 * r.lin(o + 5, 0.58, 0.82);
            let slack = r.lin(o + 6, 0.03, 0.12);
            let bx = min(low * 0.92, hx + r.lin(o + 9, 13.0, 21.0));
            // The hind pair stands further out and is drawn behind, darker.
            let mut hind = Leg::new(hip_at(bx), bx + r.lin(o + 3, 3.0, 10.0), w0 * 0.9);
            hind.w1 = w1 * 0.9;
            hind.slack = slack;
            hind.toe = toe;
            hind.toe_len = toe_len;
            hind.back = true;
            legs.push(hind);
            let mut fore = Leg::new(hip_at(hx), hx + r.lin(o + 3, 0.0, 5.0), w0);
            fore.w1 = w1;
            fore.slack = slack;
            fore.toe = toe;
            fore.toe_len = toe_len;
            legs.push(fore);
        }
        "skirt" => {
            // Roots spread from under the body like the arms of an octopus,
            // each curling up at its tip.
            lift = r.lin(o + 1, 8.0, 16.0);
            let n = 4 + r.one(o + 2, 4) as usize;
            let w0 = r.lin(o + 4, 7.5, 12.0);
            let reach = r.lin(o + 3, 10.0, 22.0);
            let curl = r.lin(o + 5, 5.0, 12.0);
            for k in 0..n {
                let f = k as f64 / (n - 1) as f64 * 2.0 - 1.0;
                if f < -0.01 {
                    continue;
                }
                let mid = f < 0.01;
                let x = low * 0.74 * f;
                let mut leg = Leg::new(
                    hip_at(x),
                    x + if mid { 0.0 } else { reach * (0.45 + 0.55 * f) },
                    w0 * if mid { 0.9 } else { 1.0 },
                );
                leg.skirt = true;
                leg.curl = curl * r.lin(o + 10 + k, 0.7, 1.3);
                leg.side = if mid {
                    if r.chance(o + 9, 0.5) {
                        1.0
                    } else {
                        -1.0
                    }
                } else {
                    1.0
                };
                leg.pair = !mid;
                legs.push(leg);
            }
        }
        "stilt" => {
            // Prop roots: they leave the flanks and arch out to the ground.
            lift = r.lin(o + 1, 14.0, 27.0);
            let pairs = 1 + r.one(o + 2, 2) as usize;
            let w0 = r.lin(o + 4, 10.0, 14.5);
            let w1 = w0 * r.lin(o + 5, 0.55, 0.72);
            for k in (0..pairs).rev() {
                let at = outline_at(b, b.bottom - b.h * (0.2 + 0.1 * k as f64));
                let hip = [at.p[0] - at.n[0] * 5.0, at.p[1] - at.n[1] * 5.0];
                let mut leg = Leg::new(hip, hip[0] + r.lin(o + 3, 6.0, 18.0) + 7.0 * k as f64, w0);
                leg.w1 = w1;
                leg.slack = r.lin(o + 6, 0.12, 0.3);
                leg.toe = toe;
                leg.toe_len = toe_len;
                leg.back = k == 1;
                legs.push(leg);
            }
        }
        "nubs" => {
            lift = r.lin(o + 1, 3.5, 7.5);
            let hx = low * r.lin(o + 2, 0.36, 0.62);
            let w0 = r.lin(o + 4, 11.0, 16.0);
            let mut leg = Leg::new(hip_at(hx), hx + r.lin(o + 3, 0.0, 5.0), w0);
            leg.w1 = w0 * 0.8;
            leg.slack = 0.02;
            leg.toe = if r.chance(o + 7, 0.5) {
                Toe::Foot
            } else {
                Toe::Toes
            };
            leg.toe_len = r.lin(o + 8, 6.0, 10.0);
            legs.push(leg);
        }
        _ => {
            // A taproot keeps its own point as a third foot and props itself
            // on two side roots.
            lift = r.lin(o + 1, 2.0, 6.0);
            let at = outline_at(b, b.bottom - b.h * r.lin(o + 2, 0.24, 0.34));
            let hip = [at.p[0] - at.n[0] * 5.0, at.p[1] - at.n[1] * 5.0];
            let w0 = r.lin(o + 4, 8.0, 12.5);
            let mut leg = Leg::new(hip, max(hip[0] + 6.0, b.w * r.lin(o + 3, 0.62, 1.0)), w0);
            leg.w1 = w0 * r.lin(o + 5, 0.5, 0.7);
            leg.slack = r.lin(o + 6, 0.08, 0.24);
            leg.toe = toe;
            leg.toe_len = toe_len;
            legs.push(leg);
        }
    }
    // A body with growth hanging under it stands at least `least` high.
    let lift = max(lift, least);
    for leg in legs.iter_mut() {
        if leg.skirt {
            continue;
        }
        leg.l = hyp(
            leg.foot - leg.hip[0],
            -leg.w1 / 2.0 - (leg.hip[1] - b.bottom - lift),
        ) * (1.0 + leg.slack);
    }
    Roots { layout, lift, legs }
}

/// The shapes of one leg: `m` is +1 for the right-hand one of a pair and -1
/// for the left; `tf` moves a point of the body to where the mood put it.
pub(crate) fn leg_shapes(leg: &Leg, m: f64, tf: &dyn Fn(V) -> V) -> Vec<(PathD, Bx)> {
    let hip = tf([m * leg.hip[0], leg.hip[1]]);
    let side = if leg.pair { m } else { leg.side };
    let fx = m * leg.foot;
    let disc = |c: V, r: f64| {
        (
            circle_d(c[0], c[1], r),
            [c[0] - r, c[1] - r, c[0] + r, c[1] + r],
        )
    };
    if leg.skirt {
        let w = leg.w0;
        let g = [fx, -w * 0.36];
        let c = leg.curl;
        // Down to the ground in one stretch, then along it and up into a
        // curl in another: each bends gently on its own.
        return vec![
            limb_path(&smooth_axis(
                &[
                    [hip[0], hip[1], w * 1.1],
                    [
                        (hip[0] + g[0]) / 2.0 + side * w * 0.45,
                        (hip[1] + g[1]) / 2.0,
                        w * 0.95,
                    ],
                    [g[0], g[1], w * 0.72],
                ],
                4,
            )),
            disc(g, w * 0.36),
            limb_path(&smooth_axis(
                &[
                    [g[0], g[1], w * 0.72],
                    [g[0] + side * c * 0.75, g[1] - c * 0.12, w * 0.5],
                    [g[0] + side * c * 1.15, g[1] - c * 0.7, w * 0.32],
                ],
                4,
            )),
        ];
    }
    let w = leg.w1;
    let a = [fx, -w / 2.0];
    let dx = a[0] - hip[0];
    let dy = a[1] - hip[1];
    let d = {
        let d = hyp(dx, dy);
        if d == 0.0 {
            1e-6
        } else {
            d
        }
    };
    // The knee: where a leg of fixed length has to bend to fit between hip
    // and ankle. It bends outwards.
    let bulge = max(0.0, leg.l * leg.l / 4.0 - d * d / 4.0).sqrt();
    let mut nx = -dy / d;
    let mut ny = dx / d;
    if nx * side < 0.0 {
        nx = -nx;
        ny = -ny;
    }
    let knee = [
        (hip[0] + a[0]) / 2.0 + nx * bulge,
        (hip[1] + a[1]) / 2.0 + ny * bulge,
    ];
    let mut shapes = vec![limb_path(&smooth_axis(
        &[
            [hip[0], hip[1], leg.w0 * 1.12],
            [knee[0], knee[1], (leg.w0 + w) / 2.0],
            [a[0], a[1], w],
        ],
        4,
    ))];
    shapes.push(disc(a, w / 2.0));
    let tl = leg.toe_len;
    match leg.toe {
        Toe::Foot => shapes.push(limb_path(&[
            [a[0], a[1], w],
            [a[0] + side * tl * 0.55, a[1] + w * 0.04, w * 0.86],
            [a[0] + side * tl, a[1] + w * 0.12, w * 0.62],
        ])),
        Toe::Toes => {
            for [angle, k] in [[0.1, 1.0], [0.75, 0.7], [2.75, 0.5]] {
                let v = rot([side, 0.0], -side * angle);
                shapes.push(limb_path(&[
                    [a[0], a[1], w * 0.7],
                    [a[0] + v[0] * tl * k, a[1] + v[1] * tl * k, w * 0.4],
                ]));
            }
        }
        Toe::Curl => shapes.push(limb_path(&smooth_axis(
            &[
                [a[0], a[1], w],
                [a[0] + side * tl * 0.7, a[1], w * 0.8],
                [a[0] + side * tl, a[1] - tl * 0.45, w * 0.55],
                [a[0] + side * tl * 0.72, a[1] - tl * 0.85, w * 0.38],
            ],
            3,
        ))),
        Toe::Nub => {
            let r = w * 0.74;
            shapes.push(disc([a[0] + side * w * 0.16, -r], r));
        }
    }
    shapes
}
