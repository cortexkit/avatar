//! Anatomy on the body: limbs, eye stalks, crest, arms, tail and feet.

use avatar_svg::wild::math::{cos, sin};

use crate::body::Body;
use crate::genes::{Ext, Reader, Wild};
use crate::geom::{
    bbox_of, circle_shape, clamp, dist, ellipse_poly, field, fluff_poly, js_round, lerp, max, min,
    or, poly_sdf, poly_shape, ray_hit, rot, span_through, spline_open, thin, unit, WShape, P,
};
use crate::parts::{
    build_limb, curl_cap, limb_shape, mirror_part, part_points, reach_scale, scale_part, Eye,
    Layer, LimbOpts, Part, Reach, Shape, Weld, LIMB_KINDS, LIMB_WEIGHTS,
};

pub const CRESTS: [&str; 8] = [
    "none", "ridge", "horn", "lure", "sprout", "comb", "sail", "peak",
];
pub const TAILS: [&str; 5] = ["curled", "tapering", "bushy", "spade", "puff"];

/// Limb kinds with a one-sided quirk.
fn quirky(kind: usize) -> bool {
    matches!(kind, 1 | 2 | 3 | 5 | 10)
}
/// Limb kinds made of blades round one point.
fn bladed(kind: usize) -> bool {
    matches!(kind, 6 | 7 | 9 | 11 | 12)
}
/// The kinds whose foot a bladed limb may not share.
fn ear_or_horn(kind: usize) -> bool {
    matches!(kind, 1 | 3 | 4 | 5 | 10)
}
/// Fills that are not a shade of the body.
fn not_skin(fill: &str) -> bool {
    matches!(fill, "part" | "part2" | "accent" | "bone")
}

fn skin_of(fill: u32) -> &'static str {
    match fill {
        0 => "toneUp",
        1 => "toneDown",
        _ => "body",
    }
}

/// The share of a head the face is sized by: its height, counted taller
/// when the head is wide.
pub fn face_basis(hb: [f64; 4]) -> f64 {
    let h = hb[3] - hb[1];
    h * clamp(((hb[2] - hb[0]) / h).sqrt(), 1.0, 1.45)
}

/// One wild limb slot while its pose is being found.
struct Pose {
    i: usize,
    at: usize,
    e: Ext,
    kind: usize,
    fill: &'static str,
    d: P,
    root: P,
    right: Part,
    turned: bool,
    near: bool,
    drawn: bool,
    settled: bool,
    own: f64,
    fan: bool,
    clear: f64,
}

struct Ctx<'a> {
    r: &'a Reader<'a>,
    head: &'a [P],
    sparse_head: Vec<P>,
    u: f64,
    ct: f64,
    reach: Reach,
    near_reach: Reach,
}

impl Ctx<'_> {
    fn fitted(&self, part: &Part, near: bool) -> Part {
        let f = reach_scale(part, if near { self.near_reach } else { self.reach });
        if f < 0.999 {
            scale_part(part, f)
        } else {
            part.clone()
        }
    }

    fn build(&self, pose: &mut Pose) {
        let v = pose.d;
        let rr = ray_hit(self.head, 0.0, 0.0, v[0], v[1]).unwrap_or(self.u);
        pose.root = [v[0] * rr * 0.85, v[1] * rr * 0.85];
        pose.right = build_limb(
            self.r, pose.at, pose.kind, &pose.e, pose.root, pose.d, self.u, pose.fill, false,
        );
    }

    fn crowded(&self, pose: &Pose) -> bool {
        let own = pose.own;
        part_points(&pose.right).iter().any(|q| {
            poly_sdf(&self.sparse_head, q[0], q[1]) > 0.0
                && (own * q[0] < pose.clear
                    || (pose.fan
                        && q[1] < pose.root[1]
                        && own * (q[0] - pose.root[0]) < 0.36 * (pose.root[1] - q[1])))
        })
    }

    fn turn(&self, pose: &mut Pose, angle: f64) {
        pose.d = rot(pose.d, angle);
        self.build(pose);
    }

    fn aim(&self, pose: &mut Pose, dir: P, genes: Ext) {
        pose.d = dir;
        pose.e = genes;
        self.build(pose);
    }

    fn rise_of(&self, pose: &Pose) -> f64 {
        let mut lowest = f64::INFINITY;
        for q in part_points(&self.fitted(&pose.right, pose.near)) {
            lowest = min(lowest, q[1]);
        }
        self.ct - lowest
    }

    /// The balls a limb carries, each as its centre and radius, measured on
    /// the limb as it will be drawn.
    fn balls_of(&self, part: &Part) -> Vec<[f64; 3]> {
        self.fitted(part, false)
            .layers
            .iter()
            .filter(|l| l.cls == "bulb" || l.cls == "pom")
            .map(|l| {
                let b = match &l.shape {
                    Shape::Disc { x, y, r } => [x - r, y - r, x + r, y + r],
                    Shape::Poly(pts) => bbox_of(pts),
                    _ => unreachable!("a bulb or pom is a disc or a polygon"),
                };
                [
                    (b[0] + b[2]) / 2.0,
                    (b[1] + b[3]) / 2.0,
                    max(b[2] - b[0], b[3] - b[1]) / 2.0,
                ]
            })
            .collect()
    }

    /// Whether one of a limb's balls does not stand clear above the head.
    fn ball_beside(&self, part: &Part) -> bool {
        self.balls_of(part)
            .iter()
            .any(|c| c[1] + 1.33 * c[2] > self.ct)
    }

    /// Whether a part shows outside the head only as a low bump.
    fn shows_as_bump(&self, part: &Part) -> bool {
        let out: Vec<P> = part_points(part)
            .into_iter()
            .filter(|q| poly_sdf(&self.sparse_head, q[0], q[1]) > 0.0)
            .collect();
        if out.len() < 3 {
            return false;
        }
        let mut far = out[0];
        let mut far_away = 0.0;
        for q in &out {
            let away = dist(q[0], q[1], part.root[0], part.root[1]);
            if away > far_away {
                far_away = away;
                far = *q;
            }
        }
        let ax = (far[0] - part.root[0]) / far_away;
        let ay = (far[1] - part.root[1]) / far_away;
        let (mut a0, mut a1, mut c0, mut c1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for q in &out {
            let x = q[0] - part.root[0];
            let y = q[1] - part.root[1];
            let along = x * ax + y * ay;
            let across = y * ax - x * ay;
            a0 = min(a0, along);
            a1 = max(a1, along);
            c0 = min(c0, across);
            c1 = max(c1, across);
        }
        a1 - a0 < 0.8 * (c1 - c0)
    }
}

/// Whether a ball lies over a part: over one of its discs or solid shapes,
/// or over the eyeball an eye stalk carries. Balls are `[x, y, r, clear]`.
fn ball_over(balls: &[[f64; 4]], part: &Part, side: f64) -> bool {
    let sides: Vec<f64> = if side != 0.0 {
        vec![side]
    } else {
        vec![1.0, -1.0]
    };
    if let Some(eye) = part.eye {
        if balls
            .iter()
            .any(|c| dist(eye.x * side, eye.y, c[0], c[1]) < eye.r * 1.25 + c[3])
        {
            return true;
        }
    }
    part.layers.iter().any(|l| {
        if let Shape::Disc { x, y, r } = l.shape {
            return sides
                .iter()
                .any(|&m| balls.iter().any(|c| dist(m * x, y, c[0], c[1]) < r + c[3]));
        }
        let polys: Vec<&Vec<P>> = match &l.shape {
            Shape::Poly(pts) => vec![pts],
            Shape::Set(polys) => polys.iter().collect(),
            _ => Vec::new(),
        };
        polys.iter().any(|poly| {
            sides.iter().any(|&m| {
                let own: Vec<P> = poly.iter().map(|q| [m * q[0], q[1]]).collect();
                balls.iter().any(|c| poly_sdf(&own, c[0], c[1]) < c[3])
            })
        })
    })
}

/// A limb slot settled on its pose, before it joins the drawing.
struct Slot {
    right: Part,
    left: Part,
    weld_it: bool,
    kind: usize,
    near: bool,
    skin: &'static str,
}

pub fn compose_anatomy(r: &Reader, w: &Wild, b: &mut Body, eye_plan: usize, spread: bool) {
    let head = b.head.clone();
    let (u, cw, ch, plan) = (b.u, b.cw, b.ch, b.plan);
    let welded = w.topology == 0;
    let limb_fill_of = |e: &Ext| -> &'static str {
        if w.topology == 2 {
            "part"
        } else {
            skin_of(e.fill)
        }
    };
    let weld_shape = |wd: &Weld, k: f64| -> WShape {
        match wd {
            Weld::Circle(c) => circle_shape(c[0], c[1], c[2], k, true),
            Weld::Poly(pts) => poly_shape(pts.clone(), k, true),
        }
    };
    // A body that is already wide gets little room to spread further.
    let wideness = 2.0 * b.mass[2] / (b.mass[3] - b.mass[1]);
    let reach = Reach {
        x: b.mass[2] + cw * clamp(0.5 - 0.45 * (wideness - 1.0), 0.1, 0.5),
        top: b.mass[1] - 0.62 * ch,
        bottom: max(b.ground, b.mass[3]) + 0.12 * ch,
    };
    let near_reach = Reach {
        x: min(reach.x, b.mass[2] + 0.16 * cw),
        top: reach.top,
        bottom: reach.bottom,
    };
    let cx = Ctx {
        r,
        head: &head,
        sparse_head: thin(&head, 2),
        u,
        ct: b.ct,
        reach,
        near_reach,
    };
    let add_limb = |b: &mut Body,
                    right: Part,
                    left: Part,
                    weld_it: bool,
                    k: f64,
                    near: bool,
                    skin: Option<&'static str>| {
        let mut right = cx.fitted(&right, near);
        let mut left = cx.fitted(&left, near);
        if let Some(skin) = skin {
            if !weld_it && cx.shows_as_bump(&right) {
                for p in [&mut right, &mut left] {
                    for l in p.layers.iter_mut() {
                        if not_skin(l.fill) {
                            l.fill = skin;
                        }
                    }
                }
            }
        }
        if weld_it {
            b.limbs_in_skin = true;
            b.shapes
                .push(weld_shape(right.weld.as_ref().expect("a welded limb"), k));
            for p in [&mut right, &mut left] {
                p.layers.retain(|l| !l.base);
                p.over = true;
                p.sway = 0.0;
            }
        }
        if !right.layers.is_empty() {
            b.parts.push(right);
            b.parts.push(left);
        }
    };

    // ---- eyes on stalks (the eyeballs themselves are drawn with the face) ----
    let mut eye_stalk: Option<Part> = None;
    if eye_plan == 5 {
        let up = unit(cw * r.lin(2470, 0.3, 0.8), b.ct);
        let a = clamp(up[0], 0.2, 0.8);
        let hit = or(
            ray_hit(&head, 0.0, 0.0, up[0], up[1]).unwrap_or(0.0),
            ch * 0.4,
        );
        let root = [up[0] * hit * 0.93, up[1] * hit * 0.93];
        let re = face_basis(bbox_of(&head)) * r.lin(2474, 0.15, 0.2);
        let mw = re * r.lin(2472, 0.34, 0.6);
        let curl = r.lin(2473, -0.5, 0.6);
        let mut len = hit * 0.07 + re * r.lin(2471, 0.7, 2.0);
        let mut stalk: Option<Part> = None;
        // A stalk that reaches too far is shortened; the eye keeps its size.
        for _ in 0..3 {
            let one = |_: f64| 1.0;
            let mut s = limb_shape(&LimbOpts::new(
                root,
                rot(up, -a * 0.45),
                len,
                curl,
                0.0,
                &one,
            ));
            let mut eye = Eye {
                x: s.tip[0] + s.tip_dir[0] * re * 0.55,
                y: s.tip[1] + s.tip_dir[1] * re * 0.55,
                r: re,
            };
            // The two eyeballs never touch: a stalk that ends too near the
            // axis is leaned outward.
            let short = re * 1.6 - eye.x;
            if short > 0.0 {
                let n = s.spine.len();
                for (i, q) in s.spine.iter_mut().enumerate() {
                    q[0] += short * i as f64 / (n as f64 - 1.0);
                }
                eye.x += short;
            }
            let mut part = Part::new(
                "eyestalk-r",
                "eye stalk",
                1.0,
                root,
                0.9,
                vec![Layer::tube(s.spine, mw, "body", "stalk")],
            );
            part.eye = Some(eye);
            let f = reach_scale(&part, reach);
            stalk = Some(part);
            if f > 0.999 {
                break;
            }
            len *= max(f, 0.5);
        }
        eye_stalk = stalk;
    }

    // ---- crest: something on the crown of the head ----
    let crest = r.pick(1424, &[9.0, 3.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
    let top_hit = or(ray_hit(&head, 0.0, 0.0, 0.0, -1.0).unwrap_or(0.0), ch * 0.5);
    let top_y = -top_hit;
    let mut crest_part: Option<Part> = None;
    let mut crest_weld: Option<WShape> = None;
    if crest != 0 {
        let cnt = u32::from(r.u8(1425));
        let spr = r.unit(1426);
        let tall = r.unit(1427);
        let tint = r.u8(1436);
        let hard = ["bone", "dark", "bone", "toneDown"][usize::from(tint % 4)];
        let lean = r.sgn(1437);
        let wide = r.unit(1438);
        let mut layers = Vec::new();
        let mut sway = 0.5;
        let mut root = [0.0, top_y + ch * 0.06];
        // A crest takes a shade of the body.
        let shade = ["toneDown", "dark", "toneDown", "toneUp"][usize::from(tint % 4)];
        if crest == 1 {
            // ridge: plates in single file along the midline.
            let plates = 3 + (cnt % 3) as usize;
            let shared = lerp(0.35, 0.5, wide);
            let reach_x = cw * lerp(0.24, 0.44, spr);
            let bb = reach_x / (1.0 + (1.0 - shared) * (plates as f64 - 1.0));
            let hh = min(bb * lerp(1.7, 2.8, tall), u * lerp(0.2, 0.46, tall));
            let peak = lean * 0.6;
            let mut polys = Vec::with_capacity(plates);
            for j in 0..plates {
                let uu = j as f64 / (plates as f64 - 1.0) * 2.0 - 1.0;
                let d = rot([0.0, -1.0], uu * (reach_x - bb) / top_hit);
                let hit = or(ray_hit(&head, 0.0, 0.0, d[0], d[1]).unwrap_or(0.0), top_hit);
                let o = [d[0] * hit * 0.93, d[1] * hit * 0.93];
                // All plates tilt the same way a little, and splay half as
                // much as the crown curves under them.
                let up = rot(
                    [0.0, -1.0],
                    lean * 0.14 + uu * (reach_x - bb) / top_hit * 0.5,
                );
                let off = (uu - peak) / (1.0 + peak.abs());
                let h = hh * (1.0 - 0.62 * off * off) * r.lin(1428 + j, 0.6, 1.1);
                let at = |along: f64, across: f64| -> P {
                    [
                        o[0] + up[0] * h * along - up[1] * bb * across,
                        o[1] + up[1] * h * along + up[0] * bb * across,
                    ]
                };
                polys.push(vec![
                    at(0.0, -1.0),
                    at(0.55, -0.7),
                    at(0.9, -0.25),
                    at(1.0, 0.0),
                    at(0.9, 0.25),
                    at(0.55, 0.7),
                    at(0.0, 1.0),
                ]);
            }
            layers.push(Layer::set(polys, shade, "plates"));
            sway = 0.2;
        } else if crest == 2 {
            // one horn on the axis, with spiral ridges
            let l = u * lerp(0.5, 1.15, tall);
            let bw = u * lerp(0.08, 0.17, wide);
            let prof = |t: f64| (1.0 - t) * (1.0 + 0.4 * t);
            let s = limb_shape(&LimbOpts::new(root, [0.0, -1.0], l, 0.0, bw, &prof).cap(bw * 0.1));
            layers.push(Layer::poly(s.outline.clone(), hard, true, "horn"));
            let mut i = 2;
            while i < 11 {
                layers.push(Layer::curve(
                    vec![s.left[i], s.right[i + 1]],
                    1.1,
                    Some(0.55),
                    "ridge",
                ));
                i += 3;
            }
            sway = 0.15;
        } else if crest == 3 {
            // lure: one stalk that leans, with a glowing bulb
            let l = u * lerp(0.5, 1.2, tall);
            let rr = u * lerp(0.08, 0.16, wide);
            let one = |_: f64| 1.0;
            let s = limb_shape(&LimbOpts::new(
                root,
                rot([0.0, -1.0], lean * 0.25),
                l,
                lean * lerp(0.4, 1.7, spr),
                0.0,
                &one,
            ));
            layers.push(Layer::tube(s.spine.clone(), u * 0.045, shade, "stalk"));
            layers.push(Layer::glow(s.tip[0], s.tip[1], rr * 2.6, "accent", "glow"));
            layers.push(Layer::disc(s.tip[0], s.tip[1], rr, "accent", true, "bulb"));
            layers.push(
                Layer::disc(
                    s.tip[0] - rr * 0.3,
                    s.tip[1] - rr * 0.3,
                    rr * 0.28,
                    "#ffffff",
                    false,
                    "shine",
                )
                .op(0.7),
            );
            sway = 1.4;
        } else if crest == 4 {
            // sprout: a stem and two leaves
            let stem = u * lerp(0.15, 0.45, tall);
            let l = u * lerp(0.35, 0.75, spr);
            let bw = u * lerp(0.1, 0.2, wide);
            let tip = [0.0, root[1] - stem - ch * 0.06];
            layers.push(Layer::tube(vec![root, tip], u * 0.045, shade, "stem"));
            let open = lerp(0.55, 1.2, f64::from(cnt % 16) / 15.0);
            for sgn in [1.0, -1.0] {
                let prof = |t: f64| (0.12 * (1.0 - t) + 3.6 * t * (1.0 - t)) / 0.96;
                let s = limb_shape(
                    &LimbOpts::new(
                        tip,
                        rot([0.0, -1.0], sgn * open),
                        l * (if sgn > 0.0 {
                            1.0
                        } else {
                            lerp(0.6, 1.0, lean.abs())
                        }),
                        sgn * 0.7,
                        bw,
                        &prof,
                    )
                    .cap(bw * 0.2)
                    .steps(10),
                );
                layers.push(Layer::poly(s.outline.clone(), shade, true, "leaf"));
                layers.push(Layer::curve(s.spine[1..9].to_vec(), 1.1, Some(0.5), "rib"));
            }
            sway = 1.0;
        } else if crest == 5 {
            // comb: round lobes along the crown, as on a rooster. Pale, low
            // lobes across the crown read as a shower cap, so a comb is a
            // deeper shade and its lobes stand taller than they are wide.
            let lobes = 3 + (cnt % 3) as usize;
            let half = lerp(0.25, 0.6, spr);
            let comb_shade = if shade == "toneUp" { "toneDown" } else { shade };
            let mut polys = Vec::with_capacity(lobes);
            for j in 0..lobes {
                let uu = j as f64 / (lobes as f64 - 1.0) * 2.0 - 1.0;
                let rank = (j as f64 - (lobes as f64 - 1.0) / 2.0).abs();
                let d = rot([0.0, -1.0], uu * half);
                let hit = or(ray_hit(&head, 0.0, 0.0, d[0], d[1]).unwrap_or(0.0), top_hit);
                let rr = u
                    * lerp(0.1, 0.19, wide)
                    * (1.0 - 0.3 * uu * uu)
                    * r.lin(1428 + js_round(rank) as usize, 0.75, 1.15);
                polys.push(ellipse_poly(
                    d[0] * (hit + rr * 0.35),
                    d[1] * (hit + rr * 0.35),
                    rr,
                    rr * lerp(1.3, 1.8, tall),
                    [d[1], -d[0]],
                    16,
                ));
            }
            layers.push(Layer::set(polys, comb_shade, "comb"));
            sway = 0.5;
        } else if crest == 6 {
            // sail: a fin standing on the crown
            let hh = u * lerp(0.35, 0.85, tall);
            let bw = cw * lerp(0.2, 0.5, spr);
            let tip_x = lean * bw * 0.9;
            let span = span_through(&head, top_y + ch * 0.12, 0.0);
            let bb = match span {
                Some(s) => min(bw, s[1] * 0.95),
                None => bw,
            };
            let base = top_y + ch * 0.1;
            let rays = 3 + (cnt % 3) as usize;
            let mut edge = Vec::with_capacity(rays + 1);
            for j in 0..=rays {
                let uu = j as f64 / rays as f64;
                let x = lerp(-bb, bb, uu);
                let bump = 4.0 * uu * (1.0 - uu);
                let k = if j % 2 == 1 || j == rays || j == 0 {
                    1.0
                } else {
                    0.86
                };
                edge.push([
                    lerp(x, tip_x, bump * 0.55),
                    base - hh * (0.15 + 0.85 * bump) * k,
                ]);
            }
            let mut pts = vec![[-bb, base]];
            pts.extend(spline_open(&edge, 4));
            pts.push([bb, base]);
            layers.push(Layer::poly(pts, shade, true, "membrane"));
            for (j, &tip) in edge.iter().enumerate().take(rays).skip(1) {
                layers.push(Layer::curve(
                    vec![[lerp(-bb, bb, j as f64 / rays as f64) * 0.6, base], tip],
                    1.2,
                    Some(0.55),
                    "ray",
                ));
            }
            root = [0.0, base];
            sway = 0.4;
        } else {
            // peak: the crown drawn up into a point
            let l = u * lerp(0.3, 0.8, tall);
            let bw = u * lerp(0.13, 0.28, wide);
            let prof = |t: f64| (1.0 - t) * (1.0 + 0.9 * t);
            let s = limb_shape(
                &LimbOpts::new([0.0, top_y + ch * 0.1], [0.0, -1.0], l, 0.0, bw, &prof)
                    .cap(bw * 0.1),
            );
            crest_weld = Some(poly_shape(s.outline, u * lerp(0.08, 0.25, spr), false));
        }
        if !layers.is_empty() {
            let part = Part::new("crest", CRESTS[crest], 0.0, root, sway, layers);
            crest_part = Some(cx.fitted(&part, false));
        }
    }

    // ---- the wild limb slots, each drawn as a kind ----
    let count = w.exts.len();
    let mut seen: Vec<(&'static str, usize)> = Vec::new();
    let mut slots: Vec<Slot> = Vec::new();
    if count != 0 {
        // Each slot its own kind, one kind for all, or two kinds alternating.
        let family = r.pick(1288, &[3.0, 4.0, 3.0]);
        let mut poses: Vec<Pose> = Vec::with_capacity(count);
        for (i, &e) in w.exts.iter().enumerate() {
            let at = 1296 + 32 * i;
            let slot = match family {
                1 => 0,
                2 => i % 2,
                _ => i,
            };
            let kind = r.pick(1296 + 32 * slot, &LIMB_WEIGHTS);
            let fill = limb_fill_of(&e);
            let d = [cos(e.theta), sin(e.theta)];
            let own = if d[0] >= 0.0 { 1.0 } else { -1.0 };
            let fan = kind == 6 || kind == 11;
            // A limb keeps clear of the axis, so the two of a pair never
            // meet there: two limbs that meet on the crown read as a bow.
            let clear = if fan {
                max(u * 0.04, cw * 0.3)
            } else {
                u * 0.1
            };
            let mut pose = Pose {
                i,
                at,
                e,
                kind,
                fill,
                d,
                root: [0.0, 0.0],
                right: Part::new("", "", 1.0, [0.0, 0.0], 0.0, Vec::new()),
                turned: false,
                near: false,
                drawn: true,
                settled: false,
                own,
                fan,
                clear,
            };
            cx.build(&mut pose);
            let mut k = 0;
            while k < (if fan { 18 } else { 13 }) && cx.crowded(&pose) {
                pose.turned = true;
                cx.turn(&mut pose, own * 0.09);
                k += 1;
            }
            poses.push(pose);
        }
        // Of the limbs that stand up from the top of the head, only the pair
        // that rises highest stays there; every other pair is turned outward
        // until it rises little above the head. A stalk with a ball is not
        // counted.
        let spike = |p: &Pose| p.kind != 2 && p.kind != 12;
        let mut tallest: Option<usize> = None;
        let mut most = 0.0;
        for (pi, pose) in poses.iter().enumerate() {
            if !spike(pose) {
                continue;
            }
            let rise = cx.rise_of(pose);
            if rise > most {
                most = rise;
                tallest = Some(pi);
            }
        }
        if let Some(t) = tallest {
            for (pi, pose) in poses.iter_mut().enumerate() {
                if pi == t || !spike(pose) {
                    continue;
                }
                let limit = max(most * 0.34, ch * 0.1);
                let mut k = 0;
                while k < 14 && cx.rise_of(pose) > limit {
                    pose.near = true;
                    let own = pose.own;
                    cx.turn(pose, own * 0.09);
                    k += 1;
                }
            }
        }
        b.limbs_on_top = poses.iter().any(|p| spike(p) && cx.rise_of(p) > ch * 0.02);
        if spread {
            for pose in poses.iter_mut() {
                if !spike(pose) {
                    continue;
                }
                let mut k = 0;
                while k < 18 && cx.rise_of(pose) > ch * 0.02 {
                    pose.near = true;
                    let own = pose.own;
                    cx.turn(pose, own * 0.09);
                    k += 1;
                }
            }
        }
        // A ball on a stalk stands above the head; a slot whose ball would
        // not clear the top of the head is turned toward the crown until it
        // does and lies over no other part, the stalk drawn longer if need
        // be; failing that the limb is not drawn.
        let over_limb = |poses: &[Pose], pi: usize| -> bool {
            let pose = &poses[pi];
            let balls: Vec<[f64; 4]> = cx
                .balls_of(&pose.right)
                .iter()
                .map(|c| {
                    [
                        pose.own * c[0],
                        c[1],
                        c[2],
                        c[2] * 1.1 + 0.1 * dist(c[0], c[1], pose.root[0], pose.root[1]),
                    ]
                })
                .collect();
            crest_part
                .as_ref()
                .is_some_and(|c| ball_over(&balls, c, 0.0))
                || eye_stalk
                    .as_ref()
                    .is_some_and(|s| ball_over(&balls, s, 1.0))
                || poses.iter().enumerate().any(|(oi, o)| {
                    oi != pi
                        && o.drawn
                        && o.settled
                        && ball_over(&balls, &cx.fitted(&o.right, o.near), o.own)
                })
        };
        for pose in poses.iter_mut() {
            pose.settled = !cx.ball_beside(&pose.right);
        }
        for pi in 0..poses.len() {
            if poses[pi].settled {
                continue;
            }
            let from = poses[pi].d;
            let genes = poses[pi].e;
            poses[pi].drawn = false;
            for grow in [1.0, 1.2, 1.45, 1.7, 2.0] {
                let grown = Ext {
                    len: genes.len * grow,
                    ..genes
                };
                cx.aim(&mut poses[pi], from, grown);
                while !cx.crowded(&poses[pi]) {
                    if !cx.ball_beside(&poses[pi].right)
                        && !over_limb(&poses, pi)
                        && (grow == 1.0 || cx.rise_of(&poses[pi]) <= 0.5 * ch)
                    {
                        poses[pi].drawn = true;
                        break;
                    }
                    let own = poses[pi].own;
                    let next = rot(poses[pi].d, -own * 0.09);
                    if own * next[0] < 0.05 {
                        break;
                    }
                    let e = poses[pi].e;
                    cx.aim(&mut poses[pi], next, e);
                }
                if poses[pi].drawn {
                    break;
                }
            }
            poses[pi].settled = true;
        }
        for pose in &poses {
            if !pose.drawn {
                continue;
            }
            let (i, at, kind, fill, d, root) =
                (pose.i, pose.at, pose.kind, pose.fill, pose.d, pose.root);
            let mut right = pose.right.clone();
            let weld_it = welded && right.weld.is_some();
            let mut left = mirror_part(&right);
            if !weld_it && quirky(kind) && r.chance(at + 7, 0.24) {
                let mut odd = build_limb(r, at, kind, &pose.e, root, d, u, fill, true);
                // An antenna that bends its own way still has to hold its
                // ball above the head; one that would not is drawn like its
                // twin.
                if !cx.ball_beside(&odd) {
                    if r.u8(at + 7) & 1 == 1 {
                        left = mirror_part(&odd);
                    } else {
                        odd.weld = right.weld.clone();
                        right = odd;
                    }
                    left.odd = true;
                    right.odd = true;
                }
            }
            right.name = format!("limb-{i}-r");
            left.name = format!("limb-{i}-l");
            // Each side is cocked a few degrees its own way.
            if !weld_it {
                right.cock = r.sgn(at + 22) * 0.1;
                left.cock = r.sgn(at + 23) * 0.1;
            }
            let skin = skin_of(pose.e.fill);
            // A leaf whose blade lies beside the head and not above it is
            // skin, as an ear is.
            if kind == 9 {
                let fitted = cx.fitted(&pose.right, pose.near);
                let blade = fitted
                    .layers
                    .iter()
                    .find(|l| l.cls == "blade")
                    .map(|l| match &l.shape {
                        Shape::Poly(pts) => bbox_of(pts),
                        _ => unreachable!("a blade is a polygon"),
                    })
                    .expect("a leaf has a blade");
                if (blade[1] + blade[3]) / 2.0 > b.ct {
                    for part in [&mut right, &mut left] {
                        for l in part.layers.iter_mut() {
                            if not_skin(l.fill) {
                                l.fill = skin;
                            }
                        }
                    }
                }
            }
            slots.push(Slot {
                right,
                left,
                weld_it,
                kind,
                near: pose.near || (pose.fan && pose.turned),
                skin,
            });
        }
        // A fan, a leaf or a pom rooted where an ear, a horn or an antler is
        // rooted reads as a flower pinned there, so it is not drawn.
        let at_root_of = |a: &Slot, o: &Slot| {
            dist(
                a.right.root[0].abs(),
                a.right.root[1],
                o.right.root[0].abs(),
                o.right.root[1],
            ) < 0.15 * cw
        };
        // Ears come in one pair: more round ears round the head read as
        // curlers or a pair of earmuffs, so only the highest pair is drawn,
        // and only if it is rooted in the upper 35% of the head's height;
        // lower down, beside the eyes, round ears read as earmuffs.
        let top_ear = slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == 4)
            .min_by(|(_, a), (_, b)| a.right.root[1].total_cmp(&b.right.root[1]))
            .filter(|(_, s)| s.right.root[1] < b.ct + ch * 0.35)
            .map(|(i, _)| i);
        let skip: Vec<bool> = slots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (s.kind == 4 && Some(i) != top_ear)
                    || (bladed(s.kind)
                        && slots
                            .iter()
                            .any(|o| ear_or_horn(o.kind) && !o.weld_it && at_root_of(s, o)))
            })
            .collect();
        for (s, skipped) in slots.into_iter().zip(skip) {
            if skipped {
                continue;
            }
            // A fan is several blades whatever its length, never one patch.
            let skin = if matches!(s.kind, 6 | 7 | 11) {
                None
            } else {
                Some(s.skin)
            };
            add_limb(b, s.right, s.left, s.weld_it, u * 0.16, s.near, skin);
            let name = LIMB_KINDS[s.kind];
            match seen.iter_mut().find(|(k, _)| *k == name) {
                Some(entry) => entry.1 += 1,
                None => seen.push((name, 1)),
            }
        }
        if !seen.is_empty() {
            b.notes.push(
                seen.iter()
                    .map(|(k, n)| {
                        if *n > 1 {
                            format!("{n} pairs of {k}")
                        } else {
                            (*k).to_owned()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
    }

    if let Some(stalk) = eye_stalk {
        let mut left = mirror_part(&stalk);
        left.name = "eyestalk-l".to_owned();
        b.parts.push(stalk);
        b.parts.push(left);
    }
    if let Some(weld) = crest_weld {
        b.shapes.push(weld);
    }
    if let Some(part) = crest_part {
        b.parts.push(part);
    }
    if crest != 0 {
        b.notes.push(format!("{} crest", CRESTS[crest]));
    }

    // ---- arms on the torso or the first segment ----
    if let Some((poly, cy)) = b.arm.clone() {
        let chance = [0.22, 0.75, 0.4, 0.22].get(plan).copied().unwrap_or(0.0);
        if r.chance(1520, chance) {
            let ang = r.lin(1521, -0.25, 0.45);
            let d = [cos(ang), sin(ang)];
            let hit = or(ray_hit(&poly, 0.0, cy, d[0], d[1]).unwrap_or(0.0), cw * 0.6);
            let root = [d[0] * hit * 0.82, cy + d[1] * hit * 0.82];
            let e = Ext {
                theta: 0.0,
                len: cw / u * r.lin(1523, 0.35, 0.85),
                width: cw / u * r.lin(1524, 0.09, 0.2),
                curl: r.lin(1525, -0.5, 1.0),
                taper: r.lin(1526, 0.45, 0.9),
                fill: 2,
            };
            let kind = [0, 13, 14][r.pick(1527, &[5.0, 2.0, 2.0])];
            let mut right = build_limb(
                r,
                1530,
                kind,
                &e,
                root,
                rot(d, r.lin(1522, 0.2, 1.0)),
                u,
                if w.topology == 2 { "part" } else { "torso" },
                false,
            );
            // An arm that carries rings is the body's colour, as its rings
            // are a shade of the body.
            if right.layers.iter().any(|l| l.cls == "band") {
                for l in right.layers.iter_mut() {
                    if l.base {
                        l.fill = "torso";
                    }
                }
            }
            let mut left = mirror_part(&right);
            right.name = "arm-r".to_owned();
            left.name = "arm-l".to_owned();
            let weld_it = welded && right.weld.is_some();
            if !weld_it && r.chance(1528, 0.45) {
                right.front = true;
                left.front = true;
            }
            add_limb(b, right, left, weld_it, u * 0.1, false, None);
            b.notes.push(if kind == 0 {
                "arms".to_owned()
            } else {
                format!("{} arms", LIMB_KINDS[kind])
            });
        }
    }

    // ---- a tail, to one side ----
    if plan != 5 && r.chance(1488, 0.32) {
        let side = if r.u8(1488) & 1 == 1 { 1.0 } else { -1.0 };
        let kind = r.mod_(1489, 5) as usize;
        let low = b.low.clone();
        let root_y = lerp(low.cy, low.bottom, 0.55);
        let edge = match span_through(&low.poly, root_y, 0.0) {
            Some(s) => min(-s[0], s[1]),
            None => low.half * 0.5,
        };
        let root = [side * edge * 0.6, root_y];
        let len = u * r.lin(1490, 0.8, 1.7);
        let curl = -side * r.lin(1491, 0.5, 2.3);
        let tw = u * r.lin(1492, 0.07, 0.2);
        let dir = unit(side * 0.9, -r.lin(1493, 0.0, 0.6));
        let tint = r.u8(1494);
        let fill = ["body", "part", "toneDown", "toneUp"][usize::from(tint % 4)];
        let tip_fill = ["light", "accent", "dark", "bone"][usize::from((tint >> 2) % 4)];
        // The end of a bushy tail is pale, dark, bone or the belly's colour.
        let brush_fill = if tip_fill == "accent" {
            "belly"
        } else {
            tip_fill
        };
        let mut layers = Vec::new();
        let one = |_: f64| 1.0;
        if kind == 0 {
            let curl_at = |t: f64| 0.3 + 1.7 * t;
            let curled = |ease: f64| {
                limb_shape(
                    &LimbOpts::new(root, dir, len * 1.2, curl * 1.4 * ease, 0.0, &one)
                        .curl_at(&curl_at)
                        .steps(18),
                )
            };
            // A curl whose end runs back behind the body joins the body at
            // both ends and reads as the handle of a mug, so the curl is
            // eased until the end stands clear of the body.
            let mut s = curled(1.0);
            for ease in [0.75, 0.55, 0.4] {
                if field(&b.shapes, s.tip[0], s.tip[1]) > tw {
                    break;
                }
                s = curled(ease);
            }
            layers.push(Layer::tube(s.spine, tw * 0.55, fill, "tail"));
        } else if kind == 1 {
            let prof = |t: f64| 1.0 - 0.85 * t;
            let s = limb_shape(
                &LimbOpts::new(
                    root,
                    dir,
                    len,
                    curl,
                    min(tw * 1.3, curl_cap(len, curl)),
                    &prof,
                )
                .round(),
            );
            layers.push(Layer::poly(s.outline, fill, true, "tail"));
        } else if kind == 2 {
            let prof = |t: f64| 0.35 + 2.3 * t * (1.0 - t) + 0.25 * t;
            let s = limb_shape(
                &LimbOpts::new(
                    root,
                    dir,
                    len,
                    curl * 0.7,
                    min(tw * 2.2, curl_cap(len, curl * 0.7) * 0.9),
                    &prof,
                )
                .round(),
            );
            layers.push(Layer::poly(s.outline.clone(), fill, true, "tail"));
            let mut tip: Vec<P> = s.left[10..].to_vec();
            tip.extend_from_slice(&s.outline[15..s.outline.len() - 15]);
            tip.extend(s.right[10..].iter().rev());
            layers.push(Layer::poly(tip, brush_fill, true, "tail-tip"));
        } else if kind == 3 {
            let s = limb_shape(&LimbOpts::new(root, dir, len, curl, 0.0, &one));
            let a = tw * 1.3;
            let (t, p) = (s.tip_dir, s.tip);
            layers.push(Layer::tube(s.spine.clone(), tw * 0.5, fill, "tail"));
            layers.push(Layer::poly(
                vec![
                    [p[0] - t[1] * a, p[1] + t[0] * a],
                    [p[0] + t[0] * a * 1.9, p[1] + t[1] * a * 1.9],
                    [p[0] + t[1] * a, p[1] - t[0] * a],
                    [p[0] + t[0] * a * 0.35, p[1] + t[1] * a * 0.35],
                ],
                tip_fill,
                true,
                "tail-tip",
            ));
        } else {
            let rr = u * r.lin(1495, 0.17, 0.32);
            layers.push(Layer::poly(
                fluff_poly(
                    side * (edge + rr * 0.3),
                    root_y,
                    rr,
                    7 + usize::from(tint % 3),
                    0.22,
                ),
                if tip_fill == "dark" {
                    "light"
                } else {
                    tip_fill
                },
                true,
                "tail",
            ));
        }
        let mut tail = Part::new("tail", "tail", side, root, 0.7, layers);
        tail.back = true;
        let f = reach_scale(
            &tail,
            Reach {
                x: reach.x + 0.3 * cw,
                top: reach.top,
                bottom: reach.bottom,
            },
        );
        b.parts.push(if f < 0.999 {
            scale_part(&tail, f)
        } else {
            tail
        });
        b.notes.push(format!("{} tail", TAILS[kind]));
    }

    // ---- feet, for the plans that sit on the ground ----
    if plan <= 4 {
        let kind = if plan == 1 || plan == 4 {
            1 + r.pick(1280, &[3.0, 2.0])
        } else {
            r.pick(1280, &[5.0, 4.0, 2.0])
        };
        if kind != 0 {
            let low = b.low.clone();
            let span = span_through(&low.poly, low.bottom - ch * 0.06, 0.0)
                .unwrap_or([-low.half * 0.6, low.half * 0.6]);
            let fx = min(-span[0], span[1]) * r.lin(1281, 0.45, 1.0);
            let frx = cw * r.lin(1282, 0.16, 0.32);
            let fry = frx * r.lin(1283, 0.45, 0.75);
            let tint = r.u8(1284);
            // A plain oval foot much paler or darker than the body reads as a
            // slipper or a shoe, so it is the body's colour or its shade;
            // toes or claws make a foot of any shade read as a foot.
            let fill = if kind == 1 {
                ["toneDown", "toneDown", "body", "body"][usize::from(tint % 4)]
            } else {
                ["toneDown", "dark", "body", "toneUp"][usize::from(tint % 4)]
            };
            let gy = b.ground;
            for side in [1.0, -1.0] {
                let mut polys = vec![ellipse_poly(
                    side * fx,
                    gy - fry * 0.2,
                    frx,
                    fry,
                    [1.0, 0.0],
                    18,
                )];
                if kind == 2 {
                    for j in -1..=1 {
                        let tx = side * fx + f64::from(j) * frx * 0.6;
                        if tint & 16 != 0 {
                            polys.push(vec![
                                [tx - frx * 0.16, gy + fry * 0.45],
                                [tx, gy + fry * 1.25],
                                [tx + frx * 0.16, gy + fry * 0.45],
                            ]);
                        } else {
                            polys.push(ellipse_poly(
                                tx,
                                gy + fry * 0.62,
                                frx * 0.24,
                                frx * 0.24,
                                [1.0, 0.0],
                                10,
                            ));
                        }
                    }
                }
                let mut foot = Part::new(
                    if side > 0.0 { "foot-r" } else { "foot-l" },
                    "foot",
                    side,
                    [side * fx, gy],
                    0.0,
                    vec![Layer::set(polys, fill, "foot")],
                );
                foot.front = true;
                b.parts.push(foot);
            }
            b.ground = gy + fry * (if kind == 2 { 1.1 } else { 0.8 });
            b.notes.push(
                if kind == 2 {
                    if tint & 16 != 0 {
                        "clawed feet"
                    } else {
                        "toed feet"
                    }
                } else {
                    "feet"
                }
                .to_owned(),
            );
        }
    }

    // A head whose eyes stand on stalks has no eyes of its own, and the
    // inner discs of welded round ears on it read as a pair of eyes, so
    // there they are not drawn.
    if eye_plan == 5 {
        for part in b.parts.iter_mut().filter(|p| p.over) {
            part.layers
                .retain(|l| !(l.cls == "inner" && matches!(l.shape, Shape::Disc { .. })));
        }
    }

    // A limb that points up turns toward the axis when it perks. Each limb
    // that carries a ball gets the largest perk, in steps of 0.03 radians,
    // that still keeps what is on its own side of the axis there.
    for part in b.parts.iter_mut() {
        if part.sway == 0.0
            || part.root[0].abs() < u * 0.01
            || !part
                .layers
                .iter()
                .any(|l| l.cls == "bulb" || l.cls == "pom")
        {
            continue;
        }
        let side = if part.root[0] > 0.0 { 1.0 } else { -1.0 };
        let o = part.root;
        let pts: Vec<P> = part_points(part)
            .into_iter()
            .filter(|q| side * q[0] > 0.0)
            .collect();
        let cock = part.cock;
        let crosses = |a: f64| {
            let turn = -side * a + cock;
            let (c, sn) = (cos(turn), sin(turn));
            pts.iter()
                .any(|q| side * (o[0] + (q[0] - o[0]) * c - (q[1] - o[1]) * sn) < 0.0)
        };
        let mut cap = 0.0;
        while cap < 0.36 && !crosses(cap + 0.03) {
            cap += 0.03;
        }
        if cap < 0.36 {
            part.perk_max = Some(cap);
        }
    }
}
