//! The creature: genes, body, anatomy, coat, palette, fit to the frame,
//! face and surface, built in the reference's order.

use avatar_svg::wild::math::{cos, sin};
use std::f64::consts::PI;

use crate::anatomy::compose_anatomy;
use crate::body::{compose_body, Body, Zone};
use crate::coat::{close_mirrored, coat_genes, coat_half, wobble_half, Seg};
use crate::face::{build_face, out_k, Face, FaceInput};
use crate::genes::{decode_wild, Reader, Wild};
use crate::geom::{
    bbox_of, chain_length, clamp, dist, field, grow_box, js_round, max, min, mirror_half,
    resample_open, smooth_half, trace_half, P,
};
use crate::mood::{mood_turn, spec, way_of, Temper, MOODS};
use crate::palette::{build_palette, Palette};
use crate::parts::{map_part, part_points, Part, Shape};
use crate::surface::{build_surface, Surface};

/// One creature built from its genome, in frame units.
pub struct Built {
    pub w: Wild,
    pub b: Body,
    pub p: Palette,
    /// The outline width.
    pub ow: f64,
    /// Model units to frame units.
    pub s: f64,
    pub dx: f64,
    pub head: Vec<P>,
    pub parts: Vec<Part>,
    pub bbox: [f64; 4],
    pub ground_y: f64,
    pub coat_kind: usize,
    /// The closed body path: its start and its segments.
    pub body_start: P,
    pub body_segs: Vec<Seg>,
    pub face: Face,
    pub temper: Temper,
    pub surface: Surface,
}

/// A face that sits low on its body is raised where the head has room.
/// Where it has none and limbs stand above the head, the creature is built
/// again with those limbs turned outward, and that build is kept unless it
/// is drawn more than a twentieth smaller.
pub fn creature(bytes: &[u8]) -> Built {
    let c = build(bytes, false);
    if !(c.face.low && c.b.limbs_on_top) {
        return c;
    }
    let spread = build(bytes, true);
    if spread.s >= 0.95 * c.s {
        spread
    } else {
        c
    }
}

/// Every point a part draws, with the radius drawn round it.
fn reach_of(part: &Part) -> Vec<[f64; 3]> {
    let mut pts = Vec::new();
    for l in &part.layers {
        match &l.shape {
            Shape::Tube { pts: p, mw } => {
                for q in p {
                    pts.push([q[0], q[1], mw / 2.0]);
                }
            }
            Shape::Poly(p) | Shape::Curve { pts: p, .. } => {
                for q in p {
                    pts.push([q[0], q[1], 0.0]);
                }
            }
            Shape::Set(polys) => {
                for poly in polys {
                    for q in poly {
                        pts.push([q[0], q[1], 0.0]);
                    }
                }
            }
            Shape::Disc { x, y, r } => pts.push([*x, *y, *r]),
            Shape::Glow { x, y, r } => pts.push([*x, *y, r * 0.67]),
        }
    }
    if let Some(e) = part.eye {
        pts.push([e.x, e.y, e.r * 1.25]);
    }
    pts
}

fn build(bytes: &[u8], spread: bool) -> Built {
    let r = Reader::new(bytes);
    let w = decode_wild(&r);
    // two eyes, one, three, four, a cluster, eyes on stalks, eyes set wide
    let eye_plan = r.pick(2464, &[10.0, 2.0, 2.0, 2.0, 1.0, 2.0, 2.0]);
    let mut b = compose_body(&r, &w, eye_plan);
    compose_anatomy(&r, &w, &mut b, eye_plan, spread);
    let g = coat_genes(&r);
    let mut p = build_palette(&r, &w);
    let ow = r.lin(1830, 1.4, 2.5);
    let coat_kind = g.kind;

    // Weld every mass into one silhouette (model space).
    let (mut x1, mut y0, mut y1, mut k_max) = (0.0, f64::INFINITY, f64::NEG_INFINITY, 0.0);
    for sh in &b.shapes {
        x1 = max(max(x1, sh.bx[2]), -sh.bx[0]);
        y0 = min(y0, sh.bx[1]);
        y1 = max(y1, sh.bx[3]);
        k_max = max(k_max, sh.k);
    }
    let pad = k_max + max(x1, y1 - y0) * 0.05;
    let cell = max(x1 + pad, y1 - y0 + 2.0 * pad) / 112.0;
    let half = match trace_half(&b.shapes, [0.0, y0 - pad, x1 + pad, y1 + pad], 112.0) {
        Some(h) => h,
        None => {
            // Never expected: the head always straddles the axis. A plain
            // oval keeps the genome drawable all the same.
            let mut h: Vec<P> = (0..=40)
                .map(|i| {
                    let a = -PI / 2.0 + PI * f64::from(i) / 40.0;
                    [cos(a) * x1, (y0 + y1) / 2.0 + sin(a) * (y1 - y0) / 2.0]
                })
                .collect();
            h[0][0] = 0.0;
            h[40][0] = 0.0;
            h
        }
    };
    let count = clamp(js_round(chain_length(&half) / (cell * 1.25)), 90.0, 280.0) as usize;
    let half = smooth_half(resample_open(&half, count), 2);
    let outline_m = mirror_half(&half);

    // Fit everything into the frame, leaving room for the coat.
    let mut bx = bbox_of(&outline_m);
    for part in &b.parts {
        grow_box(&mut bx, &part_points(part));
        if let Some(e) = part.eye {
            grow_box(
                &mut bx,
                &[
                    [e.x - e.r * 1.25, e.y - e.r * 1.25],
                    [e.x + e.r * 1.25, e.y + e.r * 1.25],
                ],
            );
        }
    }
    bx[3] = max(bx[3], b.ground);
    // A tail makes the creature lopsided. The drawing is then centred on
    // its whole extent, by at most 20 units.
    let hgt = bx[3] - bx[1];
    let w_box = 176.0 - 1.7 * g.reach;
    let h_box = 198.0 - 1.2 * g.reach;
    let fit = |mid: f64| min(w_box / 2.0 / max(bx[2] - mid, mid - bx[0]), h_box / hgt);
    let mut mid = (bx[0] + bx[2]) / 2.0;
    for _ in 0..5 {
        let s = fit(mid);
        mid = clamp((bx[0] + bx[2]) / 2.0, -20.0 / s, 20.0 / s);
    }
    let mut s = fit(mid);
    let my = (bx[1] + bx[3]) / 2.0;
    let temper = Temper {
        side: if r.u8(2760) & 1 == 1 { 1.0 } else { -1.0 },
        tilt: r.lin(2761, 0.7, 1.4),
        droop: r.lin(2762, 0.7, 1.3),
        squash: r.lin(2763, 0.6, 1.4),
    };
    // Where a part would come within 3 units of the frame's edge in any
    // mood, the whole drawing is made a little smaller until none does.
    let reaches: Vec<Vec<[f64; 3]>> = b.parts.iter().map(reach_of).collect();
    let leaves_frame = |k: f64| -> bool {
        let gy = 120.0 + (b.ground - my) * k;
        let dx = -mid * k;
        for mood in MOODS {
            let sp = spec(mood);
            let tilt = sp.tilt * temper.tilt * temper.side * PI / 180.0;
            let (tc, ts) = (cos(tilt), sin(tilt));
            let kx = 1.0 + (sp.squash[0] - 1.0) * temper.squash;
            let ky = 1.0 + (sp.squash[1] - 1.0) * temper.squash;
            for (pi, part) in b.parts.iter().enumerate() {
                let rx = 120.0 + part.root[0] * k;
                let ry = 120.0 + (part.root[1] - my) * k;
                let turn = mood_turn(part, sp.droop, &temper, way_of(rx));
                let (c, sn) = (cos(turn), sin(turn));
                for q in &reaches[pi] {
                    // The limb's turn, then the stretch about the ground,
                    // the tilt about the same point, the lift and the shift.
                    let ax = 120.0 + q[0] * k - rx;
                    let ay = 120.0 + (q[1] - my) * k - ry;
                    let bxx = (rx + ax * c - ay * sn - 120.0) * kx;
                    let byy = (ry + ax * sn + ay * c - gy) * ky;
                    let x = 120.0 + bxx * tc - byy * ts + dx;
                    let y = gy + bxx * ts + byy * tc - sp.lift * temper.squash;
                    let pad = q[2] * k * max(kx, ky);
                    if x - pad < 3.0 || x + pad > 237.0 || y - pad < 3.0 || y + pad > 237.0 {
                        return true;
                    }
                }
            }
        }
        false
    };
    let mut pass = 0;
    while pass < 12 && leaves_frame(s) {
        s *= 0.97;
        pass += 1;
    }
    let t = |q: P| -> P { [120.0 + q[0] * s, 120.0 + (q[1] - my) * s] };

    let dx = -mid * s;
    let mass_width = 2.0 * b.mass[2] * s;
    let framed: Vec<P> = half.iter().map(|&q| t(q)).collect();
    let half_f = wobble_half(&r, &framed, t([0.0, bbox_of(&outline_m)[3]])[1]);
    let mut outline = half_f.clone();
    for q in half_f[1..half_f.len() - 1].iter().rev() {
        outline.push([240.0 - q[0], q[1]]);
    }
    let head: Vec<P> = b.head.iter().map(|&q| t(q)).collect();
    let mut parts: Vec<Part> = b.parts.iter().map(|part| map_part(part, &t, s)).collect();
    let bbox = bbox_of(&outline);
    let frame = [
        120.0 + bx[0] * s,
        t([0.0, bx[1]])[1],
        120.0 + bx[2] * s,
        t([0.0, bx[3]])[1],
    ];
    let ground_y = t([0.0, b.ground])[1];
    let zones: Vec<Zone> = b
        .zones
        .iter()
        .map(|z| Zone {
            poly: z.poly.iter().map(|&q| t(q)).collect(),
            cy: t([0.0, max(-1e6, z.cy)])[1],
            role: z.role,
        })
        .collect();
    let depth =
        |x: f64, y: f64| -field(&b.shapes, ((x - 120.0) / s).abs(), (y - 120.0) / s + my) * s;
    let coat_segs = coat_half(&r, &g, &half_f, &depth, bbox[3]);
    let body_segs = close_mirrored(half_f[0], &coat_segs, 120.0);
    let face = build_face(
        &r,
        &w,
        &FaceInput {
            head: &head,
            bbox,
            frame,
            mass_width,
            parts: &parts,
            eye_plan,
            recipe: p.recipe,
        },
    );
    settle_front(&mut parts, &face, &temper, p.recipe);
    let surface = build_surface(
        &r,
        &w,
        &mut p,
        &face,
        &SurfaceInput {
            outline: &outline,
            plan: b.plan,
            bbox,
            zones: &zones,
            head: &head,
            limbs_in_skin: b.limbs_in_skin,
            depth: &depth,
        },
    );
    Built {
        w,
        p,
        ow,
        s,
        dx,
        head,
        parts,
        bbox,
        ground_y,
        coat_kind,
        body_start: half_f[0],
        body_segs,
        face,
        temper,
        surface,
        b,
    }
}

/// What the surface needs to know of the creature.
pub struct SurfaceInput<'a> {
    pub outline: &'a [P],
    pub plan: usize,
    pub bbox: [f64; 4],
    pub zones: &'a [Zone],
    pub head: &'a [P],
    pub limbs_in_skin: bool,
    pub depth: &'a dyn Fn(f64, f64) -> f64,
}

/// Whether a point lies on the face: on an eye on the head, or on the box
/// under the eyes that the nose and mouth take.
pub fn on_face(f: &Face, q: P) -> bool {
    let p = &f.p;
    let r = p.r;
    let reach = out_k(f.style) * 1.1;
    let mouth_bottom = p.mouth_y + r * max(f.room[0] - 1.765 + f.room[2], 1.5);
    let mouth_half = f.room[1] * r;
    f.eyes
        .iter()
        .filter(|e| e.part.is_none())
        .any(|e| dist(q[0], q[1], e.x, e.y) < e.r * reach)
        || ((q[0] - 120.0).abs() < mouth_half && q[1] > p.mouth_y - r * 0.5 && q[1] < mouth_bottom)
}

/// Settles the parts drawn in front of the body once the face is placed:
/// none may lie over the face, and on the dark recipe the two darker shades
/// give way to lighter ones.
fn settle_front(parts: &mut [Part], f: &Face, temper: &Temper, recipe: u32) {
    let droops = [
        0.0,
        spec(avatar_svg::Mood::Happy).droop,
        spec(avatar_svg::Mood::Sleepy).droop,
    ];
    for part in parts.iter_mut() {
        if !part.front {
            continue;
        }
        // A mood turns a limb about its root, so the limb is tried at rest
        // and at both ends of that turn.
        let pts = part_points(part);
        let way = way_of(part.root[0]);
        let covers = droops.iter().any(|&droop| {
            let turn = mood_turn(part, droop, temper, way);
            let (c, s) = (cos(turn), sin(turn));
            pts.iter().any(|q| {
                let x = q[0] - part.root[0];
                let y = q[1] - part.root[1];
                on_face(
                    f,
                    [part.root[0] + x * c - y * s, part.root[1] + x * s + y * c],
                )
            })
        });
        // A part that would lie over the face goes behind the body instead.
        if covers {
            part.front = false;
            continue;
        }
        if recipe == 3 {
            for l in part.layers.iter_mut() {
                l.fill = match l.fill {
                    "toneDown" => "lift",
                    "dark" => "liftPale",
                    other => other,
                };
            }
        }
    }
}
