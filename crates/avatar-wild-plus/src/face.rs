//! The face: where it sits on the head and what it is made of.

use crate::genes::{Reader, Wild};
use crate::geom::{bbox_of, clamp, dist, lerp, max, min, poly_sdf, span_through, thin, P};
use crate::parts::Part;

pub const EYE_PLANS: [&str; 7] = [
    "pair", "cyclops", "three", "four", "cluster", "stalks", "wide",
];
pub const EYE_STYLES: [&str; 13] = [
    "dot",
    "lidded",
    "arc",
    "mismatched",
    "half-lid",
    "googly",
    "iris",
    "slit pupil",
    "bar pupil",
    "ring",
    "sparkle",
    "bead",
    "lashed",
];
pub const MOUTH_PLANS: [&str; 8] = [
    "plain",
    "muzzle",
    "beak",
    "tusks",
    "buck teeth",
    "underbite",
    "maw",
    "whisker pads",
];
/// The outer ellipse of each eye style, in eye radii: its vertical offset
/// and its two radii. Lids and brows follow it.
pub const EYE_OUTER: [[f64; 3]; 13] = [
    [0.0, 1.0, 1.0],
    [0.0, 1.1, 1.25],
    [0.0, 1.0, 1.0],
    [0.0, 1.0, 1.0],
    [0.0, 1.1, 1.1],
    [0.0, 1.2, 1.2],
    [0.0, 1.15, 1.15],
    [0.0, 1.1, 1.1],
    [0.0, 1.1, 1.1],
    [0.0, 1.0, 1.0],
    [0.0, 1.12, 1.12],
    // A bead is a small dark eye, but under seven tenths of the eye radius
    // it shrinks to a dot that does not read as an eye at small sizes.
    [0.0, 0.7, 0.7],
    [0.0, 1.0, 1.0],
];
/// Room the mouth needs below the eye line, in eye radii: how far down, how
/// wide to each side, how deep below that.
const MOUTH_ROOM: [[f64; 3]; 8] = [
    [2.71, 0.8, 0.62],
    [2.3, 1.45, 1.1],
    [2.0, 0.6, 0.8],
    [2.71, 1.0, 0.7],
    [2.71, 0.8, 0.9],
    [2.71, 0.9, 0.62],
    [2.3, 1.35, 0.8],
    [2.4, 1.0, 0.85],
];

pub fn out_k(style: usize) -> f64 {
    max(EYE_OUTER[style][1], EYE_OUTER[style][2])
}

/// The share of a head the face is sized by: its height, counted taller
/// when the head is wide.
pub fn face_basis(hb: [f64; 4]) -> f64 {
    let h = hb[3] - hb[1];
    h * clamp(((hb[2] - hb[0]) / h).sqrt(), 1.0, 1.45)
}

/// The largest eye radius a head may carry.
fn eye_cap(hb: [f64; 4], gain: f64) -> f64 {
    let basis = face_basis(hb);
    max(basis * 0.085 * 1.15 * gain, min(11.0, basis * 0.16))
}

/// Where the face sits: the eye line, the eye spacing and size, and the
/// mouth line.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub score: f64,
    pub fx: f64,
    pub eye_y: f64,
    pub ipd: f64,
    pub r: f64,
    pub big: f64,
    pub mouth_y: f64,
    pub mouth_fits: bool,
    pub wide: Option<bool>,
    pub mid: Option<f64>,
}

/// One eye of the face.
#[derive(Clone, Debug)]
pub struct FaceEye {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub side: f64,
    pub main: bool,
    /// The eye stalk part that carries it, if any.
    pub part: Option<String>,
    pub derp: Option<[f64; 2]>,
}

/// Everything the face is made of.
pub struct Face {
    pub p: Place,
    pub eye_plan: usize,
    pub mouth_plan: usize,
    pub mw: f64,
    pub md: f64,
    pub sparse: Vec<P>,
    pub hb: [f64; 4],
    pub room: [f64; 3],
    pub low: bool,
    pub style: usize,
    pub eyes: Vec<FaceEye>,
    pub iris: &'static str,
    pub brow_w: f64,
    pub pupil_k: f64,
    pub brow_tilt: f64,
    pub marking: usize,
    pub mark_tone: &'static str,
    pub mark_size: f64,
    pub nose: usize,
    pub nose_tone: &'static str,
    pub kind: u32,
    pub tone: u32,
    pub size: f64,
    pub teeth: u32,
    pub smirk: f64,
    pub maw_half: f64,
    pub patch: &'static str,
    pub blush: usize,
    pub whiskers: u32,
    pub whisker_len: f64,
    pub whisker_droop: f64,
    pub freckles: Vec<[f64; 2]>,
    pub mole: f64,
}

impl Face {
    /// How far inside the head a point lies.
    pub fn inside(&self, x: f64, y: f64) -> f64 {
        -poly_sdf(&self.sparse, x, y)
    }
}

/// What the face search needs to know of the creature.
pub struct FaceInput<'a> {
    pub head: &'a [P],
    pub bbox: [f64; 4],
    pub frame: [f64; 4],
    pub mass_width: f64,
    pub parts: &'a [Part],
    pub eye_plan: usize,
    pub recipe: u32,
}

struct Opts {
    gain: f64,
    band: f64,
    ipd_bias: f64,
    room: [f64; 3],
    want: f64,
    floor: f64,
    outer: f64,
    top: f64,
    lowest: f64,
    ratios: &'static [f64],
}

fn better_face(a: &Place, b: &Place) -> bool {
    if a.wide != b.wide {
        a.wide == Some(true)
    } else {
        a.score > b.score
    }
}

/// A face belongs in the upper half of its body: where the best face sits
/// low, the best face above `o.lowest` is taken instead if the head has room
/// for one nearly as good.
fn raised_face(
    pick: Option<Place>,
    cands: &[Place],
    o: &Opts,
    better: fn(&Place, &Place) -> bool,
    size: fn(&Place) -> f64,
) -> Option<Place> {
    let pick = pick?;
    if pick.mid.is_some_and(|m| m <= o.lowest) {
        return Some(pick);
    }
    let mut up: Option<Place> = None;
    for c in cands {
        if c.mid.is_some_and(|m| m > o.lowest)
            || (pick.mouth_fits && !c.mouth_fits)
            || (pick.wide == Some(true) && c.wide != Some(true))
            || size(c) < 0.8 * size(&pick)
        {
            continue;
        }
        if up.as_ref().is_none_or(|u| better(c, u)) {
            up = Some(*c);
        }
    }
    Some(up.unwrap_or(pick))
}

fn place_face(head: &[P], inside: &dyn Fn(f64, f64) -> f64, o: &Opts) -> Option<Place> {
    let hb = bbox_of(head);
    let min_y = hb[1];
    let h = hb[3] - hb[1];
    let r_cap = eye_cap(hb, o.gain);
    let r_floor = h * 0.028;
    let mut cands = Vec::new();
    // A wide head sets its eyes wide, so the face spans it.
    let spread_bias = o.ipd_bias + 0.9 * clamp((hb[2] - hb[0]) / h - 1.05, 0.0, 0.6);
    let mut best: Option<Place> = None;
    let mut eyes_only: Option<Place> = None;
    for yi in 0..=18 {
        let y = min_y + h * (0.20 + 0.026 * f64::from(yi));
        let Some(span) = span_through(head, y, 120.0) else {
            continue;
        };
        let half_w = min(120.0 - span[0], span[1] - 120.0);
        for &ratio in o.ratios {
            let ex = half_w * ratio;
            let r = min(
                min(
                    min(inside(120.0 - ex, y), inside(120.0 + ex, y)) / 1.22,
                    r_cap,
                ),
                min(half_w * 0.69 * o.gain, ex / 1.45),
            );
            if r < r_floor {
                continue;
            }
            let span2 = 2.0 * (ex * 0.987 + r * 1.1);
            let score = r
                * (1.0 + spread_bias * ratio)
                * (1.0 - ((y - (min_y + h * o.band)) / h).abs() * 0.25)
                * (if span2 >= o.want {
                    1.0
                } else {
                    0.5 + 0.5 * span2 / o.want
                });
            let my = y + r * o.room[0];
            let fits = inside(120.0 - r * o.room[1], my) > 1.0
                && inside(120.0 + r * o.room[1], my) > 1.0
                && inside(120.0, my + r * o.room[2]) > 1.0;
            let cand = Place {
                score,
                fx: 120.0,
                eye_y: y,
                ipd: ex * 0.987,
                r,
                big: 0.0,
                mouth_y: y + r * 1.765,
                mouth_fits: fits,
                wide: Some(2.0 * (ex * 0.987 + r * o.outer) >= o.floor),
                mid: Some(y + r * (2.365 - o.top) / 2.0),
            };
            cands.push(cand);
            if eyes_only.as_ref().is_none_or(|e| better_face(&cand, e)) {
                eyes_only = Some(cand);
            }
            if fits && best.as_ref().is_none_or(|b| better_face(&cand, b)) {
                best = Some(cand);
            }
        }
    }
    raised_face(best.or(eyes_only), &cands, o, better_face, |c| c.r)
}

/// One eye on the axis: the largest disc that leaves room for the mouth.
fn place_cyclops(head: &[P], inside: &dyn Fn(f64, f64) -> f64, o: &Opts) -> Option<Place> {
    let hb = bbox_of(head);
    let min_y = hb[1];
    let h = hb[3] - hb[1];
    let mut cands = Vec::new();
    let mut best: Option<Place> = None;
    let mut eyes_only: Option<Place> = None;
    for yi in 0..=18 {
        let y = min_y + h * (0.22 + 0.024 * f64::from(yi));
        if span_through(head, y, 120.0).is_none() {
            continue;
        }
        let big = min(
            inside(120.0, y) / 1.25,
            max(eye_cap(hb, o.gain) * 2.0, o.want * 0.52),
        );
        if big < h * 0.05 {
            continue;
        }
        let r = big / 1.85;
        let score = big * (1.0 - ((y - (min_y + h * o.band)) / h).abs() * 0.5);
        let my = y + big + r * 0.8;
        let fits = inside(120.0 - r * o.room[1], my) > 1.0
            && inside(120.0 + r * o.room[1], my) > 1.0
            && inside(120.0, my + r * o.room[2]) > 1.0;
        let cand = Place {
            score,
            fx: 120.0,
            eye_y: y,
            ipd: 0.0,
            r,
            big,
            mouth_y: my,
            mouth_fits: fits,
            wide: None,
            mid: Some(y + r * 0.7),
        };
        cands.push(cand);
        if eyes_only.as_ref().is_none_or(|e| score > e.score) {
            eyes_only = Some(cand);
        }
        if fits && best.as_ref().is_none_or(|b| score > b.score) {
            best = Some(cand);
        }
    }
    raised_face(
        best.or(eyes_only),
        &cands,
        o,
        |a, b| a.score > b.score,
        |c| c.big,
    )
}

/// No eyes on the head (they are on stalks): the mouth takes the upper
/// middle, and is drawn larger, since it is the whole face there.
fn place_mouth_only(head: &[P], inside: &dyn Fn(f64, f64) -> f64, o: &Opts) -> Option<Place> {
    let hb = bbox_of(head);
    let min_y = hb[1];
    let h = hb[3] - hb[1];
    let mut best: Option<Place> = None;
    for yi in 0..=14 {
        let y = min_y + h * (0.34 + 0.02 * f64::from(yi));
        if span_through(head, y, 120.0).is_none() {
            continue;
        }
        let r = min(inside(120.0, y) / 2.0, eye_cap(hb, o.gain) * 1.3);
        if r < h * 0.03 {
            continue;
        }
        let score = r * (1.0 - ((y - (min_y + h * 0.47)) / h).abs() * 0.8);
        if best.as_ref().is_none_or(|b| score > b.score) {
            best = Some(Place {
                score,
                fx: 120.0,
                eye_y: y - r * 1.765,
                ipd: r * 1.7,
                r,
                big: 0.0,
                mouth_y: y,
                mouth_fits: true,
                wide: None,
                mid: None,
            });
        }
    }
    best
}

pub fn build_face(r: &Reader, w: &Wild, c: &FaceInput) -> Face {
    let reader = r;
    let head = c.head;
    let sparse = thin(head, 2);
    let inside = |x: f64, y: f64| -poly_sdf(&sparse, x, y);
    let hb = bbox_of(head);
    let gain = r.lin(2465, 0.8, 1.45);
    let band = r.lin(2466, 0.27, 0.48);
    let ipd_bias = r.lin(2467, -0.35, 0.6);
    let mut mouth_plan = r.pick(2528, &[8.0, 3.0, 3.0, 2.0, 1.0, 1.0, 2.0, 2.0]);
    let mut eye_plan = c.eye_plan;
    let scale_mouth = (w.mouth_w - 1.0).abs() > 0.02 || (w.mouth_d - 1.0).abs() > 0.02;
    let mw = if scale_mouth { w.mouth_w } else { 1.0 };
    let md = if scale_mouth { w.mouth_d } else { 1.0 };
    let room_of = |plan: usize| -> [f64; 3] {
        if plan == 0 {
            [2.71, 0.8 * w.mouth_w, 0.62 * w.mouth_d]
        } else {
            MOUTH_ROOM[plan]
        }
    };
    // ---- eye style ----
    let pick_style = r.pick(2476, &[9.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.0, 2.0]);
    let mut style = if pick_style == 0 {
        w.eyes as usize
    } else {
        4 + pick_style
    };
    if style == 3 && eye_plan != 0 && eye_plan != 6 {
        style = 0;
    }
    // An eye on a stalk has to read as a ball.
    if eye_plan == 5 && ![1, 5, 6, 7, 8, 10].contains(&style) {
        style = [5, 6, 1][(w.eyes % 3) as usize];
    }
    // A single eye has to read as an eye on its own: always a white eye.
    if eye_plan == 1 && (style == 2 || style == 11) {
        style = [5, 6, 1][(w.eyes % 3) as usize];
    }
    let outk = out_k(style);
    let lowest = (c.bbox[1] + c.bbox[3]) / 2.0 + 0.05 * (c.bbox[3] - c.bbox[1]);
    let face_top = [outk, outk, 3.0, 1.85, 2.9, outk, outk];
    let place = |plan: usize, g: f64, raise: bool, eye_plan: usize| -> Option<Place> {
        let mut o = Opts {
            gain: g,
            band,
            ipd_bias,
            room: room_of(plan),
            want: 0.3 * (c.frame[2] - c.frame[0]),
            floor: 0.3 * c.mass_width,
            outer: outk,
            top: face_top[eye_plan],
            lowest: if raise { lowest } else { f64::INFINITY },
            ratios: if eye_plan == 6 {
                &[0.66, 0.58, 0.5]
            } else {
                &[0.52, 0.44, 0.36, 0.28, 0.21]
            },
        };
        if eye_plan == 1 {
            return place_cyclops(head, &inside, &o);
        }
        if eye_plan == 5 {
            return place_mouth_only(head, &inside, &o);
        }
        if eye_plan == 6 {
            o.gain = g * 0.9;
        }
        place_face(head, &inside, &o)
    };
    // The planned mouth first; if the head has no room for it, a plain
    // mouth; then smaller eyes, which leave more room below them.
    let search = |raise: bool, eye_plan: usize| -> (Option<Place>, usize) {
        let mut p = place(mouth_plan, gain, raise, eye_plan);
        let mut plan = mouth_plan;
        if p.is_none_or(|p| !p.mouth_fits) {
            for (tried, g) in [(0, gain), (0, gain * 0.7), (0, gain * 0.5)] {
                let q = place(tried, g, raise, eye_plan);
                if q.is_some_and(|q| q.mouth_fits) {
                    p = q;
                    plan = tried;
                    break;
                }
                if p.is_none() && q.is_some() {
                    p = q;
                }
            }
        }
        (p, plan)
    };
    let settle = |raise: bool| -> (Option<Place>, usize, usize) {
        let mut ep = c.eye_plan;
        let mut found = search(raise, ep);
        // One eye is only as wide as the head is deep on its axis; under
        // 0.28 of the body's width the creature has a pair instead, if a
        // pair spreads wider across the head.
        if ep == 1 {
            if let Some(fp) = found.0 {
                if 2.0 * fp.big < 0.28 * c.mass_width {
                    ep = 0;
                    let pair = search(raise, ep);
                    match pair.0 {
                        Some(pp) if pp.ipd + pp.r * outk > fp.big => found = pair,
                        _ => ep = 1,
                    }
                }
            }
        }
        (found.0, found.1, ep)
    };
    let wonk = |right: bool| -> f64 {
        if style == 3 || right != (w.eye_wonk >= 0.0) {
            1.0
        } else {
            clamp(1.0 - w.eye_wonk.abs(), 0.55, 1.0)
        }
    };
    // ---- the eyes of a face placed at `p` ----
    let eyes_at = |p: &Place, eye_plan: usize| -> Vec<FaceEye> {
        let fx = p.fx;
        let rr0 = p.r;
        let mut eyes: Vec<FaceEye> = Vec::new();
        let fits = |eyes: &[FaceEye], x: f64, y: f64, rr: f64| {
            inside(x, y) >= rr * outk * 1.12
                && eyes
                    .iter()
                    .all(|e| dist(e.x, e.y, x, y) >= (e.r + rr) * outk * 1.04)
        };
        let eye = |x: f64, y: f64, r: f64, side: f64, main: bool| FaceEye {
            x,
            y,
            r,
            side,
            main,
            part: None,
            derp: None,
        };
        let add_pair = |eyes: &mut Vec<FaceEye>, dx: f64, dy: f64, mut rr: f64| -> bool {
            for _ in 0..4 {
                if rr >= rr0 * 0.3
                    && dx >= rr * outk * 1.04
                    && fits(eyes, fx + dx, p.eye_y + dy, rr)
                    && fits(eyes, fx - dx, p.eye_y + dy, rr)
                {
                    eyes.push(eye(fx - dx, p.eye_y + dy, rr, -1.0, false));
                    eyes.push(eye(fx + dx, p.eye_y + dy, rr, 1.0, false));
                    return true;
                }
                rr *= 0.82;
            }
            false
        };
        let add_mid = |eyes: &mut Vec<FaceEye>, dy: f64, mut rr: f64| -> bool {
            for _ in 0..4 {
                if rr >= rr0 * 0.3 && fits(eyes, fx, p.eye_y + dy, rr) {
                    eyes.push(eye(fx, p.eye_y + dy, rr, 0.0, false));
                    return true;
                }
                rr *= 0.82;
            }
            false
        };
        if eye_plan == 1 {
            eyes.push(eye(fx, p.eye_y, p.big / outk, 0.0, true));
        } else if eye_plan == 5 {
            for part in c.parts {
                if let Some(e) = part.eye {
                    let mut fe = eye(
                        e.x,
                        e.y,
                        e.r / outk,
                        if e.x < 120.0 { -1.0 } else { 1.0 },
                        true,
                    );
                    fe.part = Some(part.name.clone());
                    eyes.push(fe);
                }
            }
            eyes.sort_by(|a, b| a.x.total_cmp(&b.x));
        } else {
            let r = rr0;
            if style == 3 {
                eyes.push(eye(fx - p.ipd, p.eye_y, r * 1.25, -1.0, true));
                eyes.push(eye(fx + p.ipd, p.eye_y + r * 0.15, r * 0.62, 1.0, true));
            } else {
                eyes.push(eye(fx - p.ipd, p.eye_y, r * wonk(false), -1.0, true));
                eyes.push(eye(fx + p.ipd, p.eye_y, r * wonk(true), 1.0, true));
            }
            if eye_plan == 2 {
                if !add_mid(
                    &mut eyes,
                    -r * reader.lin(2478, 1.9, 2.6),
                    r * reader.lin(2479, 0.6, 0.95),
                ) {
                    add_mid(&mut eyes, -r * 1.3, r * 0.45);
                }
            } else if eye_plan == 3 {
                if !add_pair(
                    &mut eyes,
                    p.ipd * reader.lin(2478, 1.5, 1.95),
                    -r * reader.lin(2479, 0.8, 1.7),
                    r * reader.lin(2480, 0.45, 0.72),
                ) {
                    add_pair(&mut eyes, p.ipd * 0.45, -r * 1.95, r * 0.5);
                }
            } else if eye_plan == 4 {
                add_pair(
                    &mut eyes,
                    p.ipd * reader.lin(2478, 1.5, 1.9),
                    -r * reader.lin(2479, 0.7, 1.5),
                    r * reader.lin(2480, 0.4, 0.62),
                );
                add_pair(
                    &mut eyes,
                    p.ipd * reader.lin(2481, 0.4, 0.75),
                    -r * reader.lin(2482, 1.6, 2.2),
                    r * reader.lin(2483, 0.35, 0.55),
                );
                if eyes.len() < 6 {
                    add_mid(&mut eyes, -r * 2.4, r * 0.5);
                }
            }
        }
        eyes
    };
    let mut got = settle(true);
    // A face with extra eyes above its pair is raised only if all of them
    // still fit above it there.
    if got.0.is_some() && (2..=4).contains(&got.2) {
        let flat = settle(false);
        if let (Some(fp), Some(gp)) = (flat.0, got.0) {
            if eyes_at(&fp, flat.2).len() > eyes_at(&gp, flat.2).len() {
                got = flat;
            }
        }
    }
    eye_plan = got.2;
    mouth_plan = got.1;
    let p = match got.0 {
        Some(p) => p,
        None => {
            // A head too broken up for the search: a small face at its
            // deepest point.
            let mut by = (hb[1] + hb[3]) / 2.0;
            let mut bd = f64::NEG_INFINITY;
            for i in 2..=18 {
                let y = lerp(hb[1], hb[3], f64::from(i) / 20.0);
                let d = inside(120.0, y);
                if d > bd {
                    bd = d;
                    by = y;
                }
            }
            let rr = max(2.5, bd * 0.3);
            mouth_plan = 0;
            Place {
                score: 0.0,
                fx: 120.0,
                eye_y: by - rr * 0.6,
                ipd: max(rr * 1.45, bd * 0.45),
                r: rr,
                big: rr * 1.85,
                mouth_y: by + rr * 1.2,
                mouth_fits: bd > rr * 2.4,
                wide: None,
                mid: None,
            }
        }
    };
    let low = eye_plan != 5 && p.mid.is_some_and(|m| m > lowest);
    let (fx, rr) = (p.fx, p.r);
    let mut eyes = eyes_at(&p, eye_plan);
    // Googly eyes each look their own way.
    if style == 5 {
        for (i, e) in eyes.iter_mut().enumerate() {
            e.derp = Some([
                r.sgn(2488 + 2 * (i % 4)) * 0.7,
                r.sgn(2489 + 2 * (i % 4)) * 0.7,
            ]);
        }
    }
    // The iris of a slit or bar eye is always a light colour.
    let iris = if style == 7 || style == 8 {
        ["#f0c64f", "#a9d868", "#f2b65a", "irisPale"][r.one(2486, 4) as usize]
    } else if style == 9 {
        ["irisPale", "white", "#f0c64f"][r.mod_(2486, 3) as usize]
    } else {
        "iris"
    };
    let brow_w = r.lin(2487, 2.6, 4.6);
    // One eye a little higher than the other.
    if eye_plan == 0 || eye_plan == 6 {
        let lift = r.sgn(2540) * 0.12 * rr;
        for e in eyes.iter_mut() {
            if e.main {
                e.y += e.side * lift;
            }
        }
    }
    // No eye on the head may reach past the head's edge.
    for e in eyes.iter_mut() {
        if e.part.is_some() {
            continue;
        }
        let room = inside(e.x, e.y) / (outk * 1.08);
        if e.r > room {
            e.r = max(room, e.r * 0.5);
        }
    }
    let pupil_k = r.lin(2541, 0.84, 1.16);
    let brow_tilt = r.sgn(2542) * 0.14;

    // ---- markings around the eyes ----
    // A patch round a single eye reads as a mask or a visor, and a streak
    // under it has no twin, so a one-eyed face has neither.
    let mut marking = if eye_plan == 5 || eye_plan == 1 {
        0
    } else {
        [0, 1, 4][r.pick(2416, &[12.0, 4.0, 2.0])]
    };
    let mark_tone = [
        if c.recipe == 3 { "lift" } else { "toneDown" },
        "markPale",
        "markPale",
    ][r.mod_(2417, 3) as usize];
    let mut mark_size = r.lin(2419, 1.5, 2.1);
    // Patches stay apart: body colour must show between the eyes.
    if marking == 1 && eyes.len() > 1 {
        let mut biggest = f64::NEG_INFINITY;
        for e in eyes.iter().filter(|e| e.main) {
            biggest = max(biggest, e.r);
        }
        biggest *= outk;
        mark_size = min(mark_size, 0.9 * p.ipd / biggest);
        if mark_size < 1.3 {
            marking = 0;
        }
    }

    // ---- nose, mouth, cheeks ----
    let nose = if mouth_plan == 2 || mouth_plan == 6 {
        0
    } else if mouth_plan == 1 {
        1 + r.pick(2530, &[2.0, 3.0, 3.0, 1.0, 2.0])
    } else if mouth_plan == 7 {
        [2, 3][r.one(2530, 2) as usize]
    } else {
        r.pick(2530, &[6.0, 2.0, 2.0, 1.0, 2.0, 1.0])
    };
    let nose_tone = ["ink", "ink", "accent", "blush"][r.one(2537, 4) as usize];
    let kind = r.one(2531, 4);
    let tone = r.one(2532, 4);
    let size = r.unit(2533);
    let teeth = 2 + r.one(2536, 4);
    let smirk = r.sgn(2539) * 0.13;
    // A maw is as wide as the head allows at the mouth line.
    let mut maw_half = lerp(1.2, 1.85, size);
    while maw_half > 0.8 && inside(fx + maw_half * rr * mw, p.mouth_y) < 2.5 {
        maw_half *= 0.92;
    }
    let patch = ["belly", "light", "markPale", "part2"][r.one(2538, 4) as usize];
    let blush = r.pick(2620, &[7.0, 2.0, 2.0]);
    let whiskers = if mouth_plan == 7 || r.chance(2592, 0.2) {
        2 + r.one(2593, 2)
    } else {
        0
    };
    let (whisker_len, whisker_droop) = if whiskers != 0 {
        (r.lin(2594, 1.6, 3.0), r.lin(2595, -0.25, 0.35))
    } else {
        (0.0, 0.0)
    };
    let mut freckles = Vec::new();
    if r.chance(2600, 0.25) {
        let count = 3 + r.mod_(2601, 3) as usize;
        for i in 0..count {
            freckles.push([
                r.lin(2602 + 2 * i, -0.9, 0.9),
                r.lin(2603 + 2 * i, -0.55, 0.55),
            ]);
        }
    }
    let mole = if r.chance(2621, 0.12) {
        if r.u8(2621) & 1 == 1 {
            1.0
        } else {
            -1.0
        }
    } else {
        0.0
    };
    Face {
        p,
        eye_plan,
        mouth_plan,
        mw,
        md,
        sparse: sparse.clone(),
        hb,
        room: room_of(mouth_plan),
        low,
        style,
        eyes,
        iris,
        brow_w,
        pupil_k,
        brow_tilt,
        marking,
        mark_tone,
        mark_size,
        nose,
        nose_tone,
        kind,
        tone,
        size,
        teeth,
        smirk,
        maw_half,
        patch,
        blush,
        whiskers,
        whisker_len,
        whisker_droop,
        freckles,
        mole,
    }
}
