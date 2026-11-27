//! Body plans: the wild body is always the head, and the plan gene decides
//! what else the creature is made of.

use avatar_svg::wild::math::{cos, sin};

use crate::genes::{Reader, Wild};
use crate::geom::{
    bbox_of, blob_poly, capsule_shape, centroid, circle_shape, clamp, clip_below, ellipse_poly,
    flattened, lerp, max, min, or, poly_area, poly_sdf, poly_shape, ray_hit, ray_hit_far, rot,
    span_through, spline_open, thin, traced_loop, wild_body, WShape, P,
};
use crate::parts::{curl_cap, limb_shape, mirror_part, Layer, LimbOpts, Part};

pub const PLANS: [&str; 7] = [
    "blob",
    "torso",
    "segments",
    "lobed",
    "long neck",
    "tentacled",
    "legged",
];
const PLAN_WEIGHTS: [f64; 7] = [3.0, 3.0, 2.0, 2.0, 2.0, 2.0, 2.0];
/// A stack of segments reaches no further below the head than this many
/// head heights, so the head stays the creature's largest mass.
const UNDER_HEAD: f64 = 1.1;

/// A mass of the body: its polygon, centre height, half-width and bottom.
#[derive(Clone)]
pub struct Mass {
    pub poly: Vec<P>,
    pub cy: f64,
    pub half: f64,
    pub bottom: f64,
}

/// A mass that sits in front of the mass below it.
#[derive(Clone)]
pub struct Zone {
    pub poly: Vec<P>,
    pub cy: f64,
    pub role: &'static str,
}

/// The body as composed in model space.
pub struct Body {
    pub plan: usize,
    pub head: Vec<P>,
    pub cw: f64,
    pub ct: f64,
    pub ch: f64,
    pub u: f64,
    pub shapes: Vec<WShape>,
    pub parts: Vec<Part>,
    pub notes: Vec<String>,
    pub low: Mass,
    /// The mass arms grow from, and the height they leave at.
    pub arm: Option<(Vec<P>, f64)>,
    pub ground: f64,
    pub zones: Vec<Zone>,
    pub mass: [f64; 4],
    pub limbs_in_skin: bool,
    pub limbs_on_top: bool,
}

/// A wild body can be notched so deeply on its axis that no face fits
/// there; such a head is filled out round its middle until a disc of a
/// useful size fits. `floor_y` keeps the disc above a tentacled body's cut.
fn with_face_room(head: Vec<P>, floor_y: Option<f64>, high: bool) -> Vec<P> {
    let hb = bbox_of(&head);
    let sparse = thin(&head, 2);
    let bottom = floor_y.unwrap_or(hb[3]);
    let need = 0.42 * poly_area(&head).sqrt();
    let cy = if high {
        lerp(hb[1], bottom, 0.45)
    } else {
        centroid(&head)[1]
    };
    let mut best: Option<(f64, f64, f64)> = None;
    let (from, to) = if high { (7, 15) } else { (1, 24) };
    for i in from..to {
        let y = lerp(hb[1], bottom, f64::from(i) / 24.0);
        let d = -poly_sdf(&sparse, 0.0, y);
        let score = min(d, need) - 0.35 * (y - cy).abs();
        if best.is_none_or(|b| score > b.2) {
            best = Some((y, d, score));
        }
    }
    let (by, bd, _) = best.expect("at least one row");
    if bd >= need {
        return head;
    }
    let y = clamp(
        by,
        hb[1] + need * 0.9,
        max(hb[1] + need * 0.9, bottom - need * 0.9),
    );
    traced_loop(
        &[
            poly_shape(sparse, 0.0, false),
            circle_shape(0.0, y, need, need * 0.6, false),
        ],
        96.0,
        110,
    )
    .unwrap_or(head)
}

pub fn compose_body(r: &Reader, w: &Wild, eye_plan: usize) -> Body {
    let plan = r.pick(1024, &PLAN_WEIGHTS);
    let stalked = eye_plan == 5;
    // How far the lumps of the wild body are smoothed away.
    let dom = if plan == 5 || stalked {
        r.lin(1025, 0.32, 0.5)
    } else {
        r.lin(1025, 0.16, 0.48)
    };
    let grounded = plan == 0 || plan == 3;
    let nb = bbox_of(&wild_body(&w.spokes, dom, 1.0, 1.0, grounded));
    let natural = 2.0 * max(-nb[0], nb[2]) / (nb[3] - nb[1]);
    let wanted = natural
        * (1.0
            + 0.45
                * (if plan == 5 {
                    r.sgn(1026).abs()
                } else {
                    r.sgn(1026)
                }));
    let narrowest = max(
        if plan == 5 {
            0.75
        } else if plan == 4 {
            0.95
        } else {
            0.7
        },
        if stalked { 1.0 } else { 0.0 },
    );
    let ratio = clamp(wanted, narrowest, if plan == 5 { 1.05 } else { 1.5 });
    let sx = (ratio / natural).sqrt();
    let sy = 1.0 / sx;
    let raw_head = wild_body(&w.spokes, dom, sx, sy, grounded);
    let traced =
        traced_loop(&[poly_shape(thin(&raw_head, 2), 0.0, false)], 96.0, 110).unwrap_or(raw_head);
    let mut head = with_face_room(traced, None, stalked);
    let hb = bbox_of(&head);
    let cw = max(-hb[0], hb[2]);
    let (ct, cb) = (hb[1], hb[3]);
    let ch = cb - ct;
    let mut sum = 0.0;
    for s in &w.spokes {
        sum += s.r;
    }
    let u = sum / w.spokes.len() as f64;
    let mut shapes: Vec<WShape> = Vec::new();
    let mut parts: Vec<Part> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut limbs_in_skin = false;
    let head_shape = |head: &[P]| poly_shape(thin(head, 2), 0.0, false);
    let mut low = Mass {
        poly: head.clone(),
        cy: (ct + cb) / 2.0,
        half: cw,
        bottom: cb,
    };
    let mut arm = Some((head.clone(), (ct + cb) / 2.0 + ch * 0.12));
    let mut ground = cb;
    let mut zones = vec![Zone {
        poly: head.clone(),
        cy: (ct + cb) / 2.0,
        role: "head",
    }];

    if plan == 1 {
        let th = ch * r.lin(1056, 0.55, 1.05);
        let tw = cw * r.lin(1057, 0.7, 1.4);
        let overlap = r.lin(1058, 0.16, 0.42);
        let neck = if r.chance(1059, 0.4) {
            ch * r.lin(1060, 0.1, 0.4)
        } else {
            0.0
        };
        // Shoulders, chest, belly, hips, and how far the base is drawn up
        // between the legs.
        let factors = [
            1.0,
            r.lin(1061, 0.55, 1.05),
            r.lin(1062, 0.8, 1.2),
            r.lin(1063, 0.75, 1.25),
            r.lin(1064, 0.85, 1.2),
            r.lin(1068, 0.68, 0.92),
        ];
        let weld = cw * r.lin(1065, 0.04, 0.3);
        let top = cb - overlap * ch + neck;
        let torso = flattened(
            blob_poly(top + th / 2.0, tw, th / 2.0, &factors, 6),
            0.14,
            0.4,
        );
        shapes.push(head_shape(&head));
        shapes.push(poly_shape(torso.clone(), weld, false));
        if neck > 0.0 {
            let nw = cw * r.lin(1066, 0.16, 0.42);
            shapes.push(capsule_shape(
                0.0,
                cb - 0.3 * ch,
                0.0,
                top + 0.25 * th,
                nw,
                nw * r.lin(1067, 0.9, 1.5),
                weld,
                false,
            ));
        }
        let tb = bbox_of(&torso);
        zones.push(Zone {
            poly: torso.clone(),
            cy: tb[3],
            role: "torso",
        });
        low = Mass {
            poly: torso.clone(),
            cy: (tb[1] + tb[3]) / 2.0,
            half: tb[2],
            bottom: tb[3],
        };
        arm = Some((torso, tb[1] + (tb[3] - tb[1]) * 0.36));
        ground = tb[3];
        notes.push(if neck > 0.0 { "necked torso" } else { "torso" }.to_owned());
    } else if plan == 2 {
        let count = (1 + r.pick(1088, &[3.0, 4.0, 2.0])).min(if stalked { 2 } else { 3 });
        let taper = r.lin(1089, 0.8, 1.14);
        let squash = r.lin(1091, 0.5, 0.8);
        let overlap = r.lin(1092, 0.3, 0.5);
        let weld = cw * r.lin(1093, 0.03, 0.2);
        let legs = r.chance(1094, 0.5);
        let leg_r = r.lin(1095, 0.12, 0.24);
        // The stack under the head, every segment `f` times its gene size.
        let stack = |f: f64| {
            let mut segs: Vec<(Vec<P>, f64, f64, f64)> = Vec::new();
            let mut rr = cw * r.lin(1090, 0.7, 1.05) * f;
            let mut bottom = cb;
            for i in 0..count {
                let rx = rr * r.lin(1096 + i, 0.88, 1.14);
                let ry = rx * squash;
                let cy = bottom + ry * (1.0 - 2.0 * overlap * (if i == 0 { 1.1 } else { 1.0 }));
                let mut poly = blob_poly(
                    cy,
                    rx,
                    ry,
                    &[1.0, 1.0, r.lin(1100 + i, 0.92, 1.1), 1.0, 1.0],
                    5,
                );
                if i == count - 1 {
                    poly = flattened(poly, 0.16, 0.4);
                }
                segs.push((poly, cy, rx, ry));
                bottom = cy + ry;
                rr *= taper;
            }
            segs
        };
        let mut segs = stack(1.0);
        let depth = bbox_of(&segs[count - 1].0)[3] - cb;
        if depth > UNDER_HEAD * ch {
            segs = stack(UNDER_HEAD * ch / depth);
        }
        shapes.push(head_shape(&head));
        let mut first: Option<(Vec<P>, f64)> = None;
        let mut last: Option<Mass> = None;
        for (poly, cy, rx, ry) in segs {
            shapes.push(poly_shape(poly.clone(), weld, false));
            if legs {
                shapes.push(circle_shape(
                    rx * 0.9,
                    cy + ry * 0.3,
                    rx * leg_r,
                    weld * 0.5,
                    true,
                ));
            }
            zones.push(Zone {
                poly: poly.clone(),
                cy,
                role: "segment",
            });
            let sb = bbox_of(&poly);
            if first.is_none() {
                first = Some((poly.clone(), cy));
            }
            last = Some(Mass {
                poly,
                cy,
                half: sb[2],
                bottom: sb[3],
            });
        }
        low = last.expect("a segment");
        arm = first;
        ground = low.bottom;
        notes.push(format!(
            "{count} segments{}",
            if legs { " with knobs" } else { "" }
        ));
    } else if plan == 3 {
        let pairs = 1 + r.mod_(1120, 3) as usize;
        let weld = cw * r.lin(1121, 0.12, 0.4);
        shapes.push(head_shape(&head));
        for i in 0..pairs {
            let at = 1124 + 4 * i;
            let ang = r.lin(at, -0.6, 1.25);
            let d = [cos(ang), sin(ang)];
            let hit = or(ray_hit(&head, 0.0, 0.0, d[0], d[1]).unwrap_or(0.0), cw);
            let rr = cw * r.lin(at + 1, 0.3, 0.62);
            let off = hit * r.lin(at + 2, 0.7, 1.05);
            // A lobe may reach a little below the base, as a haunch does.
            let cy = min(d[1] * off, cb + 0.1 * ch - rr);
            shapes.push(circle_shape(d[0] * off, cy, rr, weld, true));
            for side in [1.0, -1.0] {
                zones.push(Zone {
                    poly: ellipse_poly(side * d[0] * off, cy, rr, rr, [1.0, 0.0], 28),
                    cy: f64::NEG_INFINITY,
                    role: "lobe",
                });
            }
            ground = max(ground, cy + rr);
            low.half = max(low.half, d[0] * off + rr);
        }
        low.bottom = ground;
        notes.push(format!(
            "{pairs}{}",
            if pairs > 1 {
                " lobe pairs"
            } else {
                " lobe pair"
            }
        ));
        limbs_in_skin = true;
    } else if plan == 4 {
        let l = ch * r.lin(1152, 0.22, if stalked { 0.38 } else { 0.6 });
        let wt = cw * r.lin(1153, 0.26, 0.42);
        let wb = wt * r.lin(1154, 1.05, 1.5);
        let rx = cw * r.lin(1155, 0.8, 1.15);
        let ry = ch * r.lin(1156, 0.24, 0.38);
        let weld = cw * r.lin(1157, 0.1, 0.3);
        let y1 = cb + l;
        let trunk = flattened(
            blob_poly(
                y1 + ry * 0.7,
                rx,
                ry,
                &[
                    1.0,
                    r.lin(1158, 0.8, 1.1),
                    r.lin(1159, 0.9, 1.2),
                    r.lin(1160, 0.9, 1.2),
                    r.lin(1161, 0.7, 0.95),
                ],
                6,
            ),
            0.16,
            0.4,
        );
        shapes.push(head_shape(&head));
        shapes.push(capsule_shape(
            0.0,
            cb - 0.35 * ch,
            0.0,
            y1 + ry * 0.55,
            wt,
            wb,
            weld,
            false,
        ));
        shapes.push(poly_shape(trunk.clone(), weld, false));
        let bb = bbox_of(&trunk);
        zones.push(Zone {
            poly: trunk.clone(),
            cy: bb[3],
            role: "trunk",
        });
        low = Mass {
            poly: trunk,
            cy: (bb[1] + bb[3]) / 2.0,
            half: bb[2],
            bottom: bb[3],
        };
        arm = None;
        ground = bb[3];
        notes.push("long neck".to_owned());
    } else if plan == 5 {
        let cut_y = lerp(ct, cb, r.lin(1184, 0.74, 0.9));
        head = clip_below(
            &with_face_room(clip_below(&head, cut_y), Some(cut_y), stalked),
            cut_y,
        );
        let hem = span_through(&head, cut_y - ch * 0.03, 0.0).unwrap_or([-cw * 0.6, cw * 0.6]);
        let hem_half = min(-hem[0], hem[1]);
        shapes.push(head_shape(&head));
        let pairs = 2 + r.one(1186, 2) as usize;
        let base_len = ch * r.lin(1187, 0.3, 0.55);
        let thick = hem_half / pairs as f64 * r.lin(1188, 0.42, 0.6);
        let curl = r.lin(1189, 0.9, 2.2);
        let flare = r.lin(1190, 0.15, 0.6);
        let suckers = r.chance(1191, 0.5);
        ground = cut_y;
        for i in 0..pairs {
            let uu = (i as f64 + 0.5) / pairs as f64;
            let len = base_len * r.lin(1192 + i, 0.75, 1.15) * (0.8 + 0.3 * uu);
            let root = [uu * hem_half * 0.92, cut_y - ch * 0.08];
            let turn = curl * (0.5 + uu);
            let curl_at = |t: f64| 0.25 + 1.5 * t;
            let prof = |t: f64| 1.0 - 0.68 * t;
            let s = limb_shape(
                &LimbOpts::new(
                    root,
                    rot([0.0, 1.0], -uu * flare),
                    len,
                    -turn,
                    min(thick, curl_cap(len, turn * 1.75) * 1.6),
                    &prof,
                )
                .curl_at(&curl_at)
                .round()
                .steps(16),
            );
            shapes.push(poly_shape(s.outline.clone(), cw * 0.05, true));
            for q in &s.outline {
                if q[1] > ground {
                    ground = q[1];
                }
            }
            if suckers {
                let mut layers = Vec::new();
                let mut k = 3;
                while k < 14 {
                    layers.push(Layer::disc(
                        s.spine[k][0] - s.tang[k][1] * s.widths[k] * 0.3,
                        s.spine[k][1] + s.tang[k][0] * s.widths[k] * 0.3,
                        s.widths[k] * 0.34,
                        "light",
                        false,
                        "sucker",
                    ));
                    k += 3;
                }
                let mut part = Part::new(&format!("arm-{i}-r"), "tentacle", 1.0, root, 0.0, layers);
                part.over = true;
                let mut left = mirror_part(&part);
                left.name = format!("arm-{i}-l");
                parts.push(part);
                parts.push(left);
            }
        }
        low = Mass {
            poly: head.clone(),
            cy: (ct + cut_y) / 2.0,
            half: hem_half,
            bottom: ground,
        };
        arm = None;
        notes.push(format!("{} tentacles", pairs * 2));
        limbs_in_skin = true;
    } else if plan == 6 {
        let style = r.pick(1248, &[3.0, 2.0, 3.0, 3.0]);
        // Legs are always shorter than the body is tall.
        let lift = ch * r.lin(1249, 0.16, 0.36);
        ground = cb + lift;
        shapes.push(head_shape(&head));
        let mid_y = (ct + cb) / 2.0;
        // Where a leg leaves the body: the underside at the hip's x.
        let mut hx = cw * r.lin(1250, 0.22, 0.6);
        let under = match ray_hit_far(&head, hx, mid_y, 0.0, 1.0) {
            Some(v) => v,
            None => {
                hx = cw * 0.15;
                or(
                    ray_hit_far(&head, hx, mid_y, 0.0, 1.0).unwrap_or(0.0),
                    ch * 0.3,
                )
            }
        };
        let hip_y = mid_y + under;
        // Legs and feet take the body's colour or a shade of it.
        let tone = ["toneDown", "dark", "body", "toneUp"][r.one(1253, 4) as usize];
        if style == 0 {
            let rad = cw * r.lin(1251, 0.1, 0.2);
            let splay = cw * r.lin(1252, -0.08, 0.25);
            shapes.push(capsule_shape(
                hx,
                hip_y - rad,
                hx + splay,
                ground - rad,
                rad,
                rad * r.lin(1254, 0.85, 1.35),
                rad * 0.5,
                true,
            ));
            let frx = rad * r.lin(1255, 1.3, 2.2);
            let fry = rad * r.lin(1256, 0.7, 1.1);
            for side in [1.0, -1.0] {
                let mut foot = Part::new(
                    if side > 0.0 { "foot-r" } else { "foot-l" },
                    "foot",
                    side,
                    [side * (hx + splay), ground],
                    0.0,
                    vec![Layer::poly(
                        ellipse_poly(
                            side * (hx + splay + frx * 0.25),
                            ground - fry * 0.3,
                            frx,
                            fry,
                            [1.0, 0.0],
                            18,
                        ),
                        tone,
                        true,
                        "foot",
                    )],
                );
                foot.front = true;
                parts.push(foot);
            }
            ground += fry * 0.7;
            notes.push("two thick legs".to_owned());
            limbs_in_skin = true;
        } else if style == 1 {
            let knee_side = if r.u8(1251) & 1 == 1 { 1.0 } else { -1.0 };
            let knee = knee_side * r.lin(1251, 0.12, 0.3) * cw;
            let mw = cw * r.lin(1252, 0.05, 0.085);
            let toe = cw * r.lin(1254, 0.14, 0.26);
            let fx = hx + cw * r.lin(1255, -0.1, 0.2);
            let leg = spline_open(
                &[
                    [hx, hip_y - ch * 0.06],
                    [hx + knee, lerp(hip_y, ground, 0.5)],
                    [fx, ground],
                ],
                6,
            );
            let right = Part::new(
                "leg-r",
                "bird leg",
                1.0,
                [hx, hip_y],
                0.0,
                vec![
                    Layer::tube(
                        vec![
                            [fx - toe, ground],
                            [fx, ground - toe * 0.15],
                            [fx + toe, ground],
                        ],
                        mw,
                        tone,
                        "toes",
                    ),
                    Layer::tube(
                        vec![
                            [fx, ground - toe * 0.15],
                            [fx + toe * 0.1, ground + toe * 0.35],
                        ],
                        mw,
                        tone,
                        "toes",
                    ),
                    Layer::tube(leg, mw, tone, "shank"),
                ],
            );
            let mut left = mirror_part(&right);
            left.name = "leg-l".to_owned();
            parts.push(right);
            parts.push(left);
            ground += toe * 0.35 + mw;
            notes.push("bird legs".to_owned());
        } else if style == 2 {
            // Arched legs, as an insect's or a crab's.
            let pairs = 2 + r.one(1251, 2) as usize;
            let thick = cw * r.lin(1252, 0.075, 0.115);
            for i in (0..pairs).rev() {
                let at = 1258 + 4 * i;
                let ang = lerp(0.35, 1.0, (i as f64 + 0.5) / pairs as f64) + r.lin(at, -0.08, 0.08);
                let d = [cos(ang), sin(ang)];
                let hit = or(
                    ray_hit(&head, 0.0, mid_y, d[0], d[1]).unwrap_or(0.0),
                    cw * 0.5,
                );
                let hip = [d[0] * hit * 0.86, mid_y + d[1] * hit * 0.86];
                let start = lerp(
                    0.45,
                    0.95,
                    if pairs > 1 {
                        i as f64 / (pairs as f64 - 1.0)
                    } else {
                        0.5
                    },
                ) + r.lin(at + 1, -0.08, 0.1);
                let dir0 = [cos(start), sin(start)];
                let bend = std::f64::consts::PI / 2.0 - start + r.lin(at + 2, 0.3, 0.75);
                // The whole leg is one arc, a little tighter toward the foot.
                let arc = |t: f64| 0.7 + 0.6 * t;
                let one = |_: f64| 1.0;
                let probe = limb_shape(
                    &LimbOpts::new([0.0, 0.0], dir0, 1.0, bend, 0.0, &one).curl_at(&arc),
                );
                let len = (ground - hip[1]) / max(probe.tip[1], 0.35);
                let prof = |t: f64| 1.0 - 0.68 * t;
                let leg = limb_shape(
                    &LimbOpts::new(
                        hip,
                        dir0,
                        len,
                        bend,
                        min(thick * 1.15, curl_cap(len, bend * 1.3)),
                        &prof,
                    )
                    .curl_at(&arc)
                    .cap(0.0),
                );
                let pad = ellipse_poly(
                    leg.tip[0] + thick * 0.2,
                    ground - thick * 0.2,
                    thick * 0.95,
                    thick * 0.5,
                    [1.0, 0.0],
                    14,
                );
                let right = Part::new(
                    &format!("leg-{i}-r"),
                    "arched leg",
                    1.0,
                    hip,
                    0.0,
                    vec![Layer::set(vec![leg.outline, pad], tone, "leg")],
                );
                let mut left = mirror_part(&right);
                left.name = format!("leg-{i}-l");
                parts.push(right);
                parts.push(left);
            }
            ground += thick * 0.3;
            notes.push(format!("{} arched legs", pairs * 2));
        } else {
            // Four stumps, as a four-legged animal seen from the front.
            hx = min(hx, cw * 0.45);
            let rad = cw * r.lin(1251, 0.1, 0.17);
            let frx = rad * r.lin(1255, 1.25, 1.8);
            let fry = rad * r.lin(1256, 0.6, 0.9);
            let ankle = rad * r.lin(1254, 0.6, 0.78);
            let fore_x = hx + cw * r.lin(1258, -0.08, 0.12);
            shapes.push(capsule_shape(
                hx,
                hip_y - rad,
                fore_x,
                ground - ankle,
                rad * 1.2,
                ankle,
                rad * 0.5,
                true,
            ));
            let bx = hx + rad * r.lin(1252, 1.6, 2.6);
            let top = mid_y
                + or(
                    ray_hit_far(&head, bx, mid_y, 0.0, 1.0).unwrap_or(0.0),
                    ch * 0.2,
                )
                - rad;
            let back = ground - ch * 0.03;
            // A hind leg leaves the hip heading down and outward, then bows
            // back in to its paw.
            let out = r.lin(1259, 0.3, 0.6);
            let bow = out + r.lin(1260, 0.25, 0.6);
            let dir0 = rot([0.0, 1.0], -out);
            let one = |_: f64| 1.0;
            let probe = limb_shape(&LimbOpts::new([0.0, 0.0], dir0, 1.0, bow, 0.0, &one));
            let hind_len = max(back - ankle - top, rad) / max(probe.tip[1], 0.5);
            let prof = |t: f64| (0.7 + 1.6 * t * (1.0 - t)) * (1.0 - 0.5 * t);
            let hind = limb_shape(
                &LimbOpts::new(
                    [bx, top],
                    dir0,
                    hind_len,
                    bow,
                    min(rad * 1.5, curl_cap(hind_len, bow)),
                    &prof,
                )
                .round(),
            );
            let paw = ellipse_poly(
                hind.tip[0] + frx * 0.2,
                back - fry * 0.3,
                frx * 0.9,
                fry * 0.9,
                [1.0, 0.0],
                16,
            );
            let hind_leg = Part::new(
                "hindleg-r",
                "hind leg",
                1.0,
                [bx, top],
                0.0,
                vec![Layer::set(vec![hind.outline, paw], "toneDown", "leg")],
            );
            let mut left = mirror_part(&hind_leg);
            left.name = "hindleg-l".to_owned();
            parts.push(hind_leg);
            parts.push(left);
            for side in [1.0, -1.0] {
                let mut foot = Part::new(
                    if side > 0.0 { "foot-r" } else { "foot-l" },
                    "foot",
                    side,
                    [side * fore_x, ground],
                    0.0,
                    vec![Layer::poly(
                        ellipse_poly(
                            side * (fore_x + frx * 0.2),
                            ground - fry * 0.3,
                            frx,
                            fry,
                            [1.0, 0.0],
                            18,
                        ),
                        tone,
                        true,
                        "foot",
                    )],
                );
                foot.front = true;
                parts.push(foot);
            }
            ground += fry * 0.7;
            notes.push("four legs".to_owned());
            limbs_in_skin = true;
        }
        low = Mass {
            poly: head.clone(),
            cy: mid_y,
            half: cw,
            bottom: cb,
        };
        arm = None;
    } else {
        shapes.push(head_shape(&head));
    }
    // The masses' own extent, before any limb.
    let mut mass = [0.0, f64::INFINITY, 0.0, f64::NEG_INFINITY];
    for sh in &shapes {
        mass[2] = max(max(mass[2], sh.bx[2]), -sh.bx[0]);
        mass[1] = min(mass[1], sh.bx[1]);
        mass[3] = max(mass[3], sh.bx[3]);
    }
    mass[0] = -mass[2];
    Body {
        plan,
        head,
        cw,
        ct,
        ch,
        u,
        shapes,
        parts,
        notes,
        low,
        arm,
        ground,
        zones,
        mass,
        limbs_in_skin,
        limbs_on_top: false,
    }
}
