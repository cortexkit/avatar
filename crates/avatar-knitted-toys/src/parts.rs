//! The sewn-on parts: ears and what else grows at the head's sides, the
//! crown's horns and crests, arms, legs, the tail, the snout and the patches.

use crate::build::{ang_delta, ArmSpec, Belly, EarSpec, Part, Seam, Toy};
use crate::dna::{hash_str, Stream};
use crate::geom::{
    add, atan2, clamp, dir_of, dist, ellipse_pts, flip_x, hyp, lerp, max, max_of, min, min_of,
    mixp, perp, pw16, rot, scl, sdist, sin, smooth, spline, sq, sub, unit, Pt, PI,
};
use crate::palette::hsl;
use crate::piece::{arc_spine, make_tube, mirror_tube, Tube};

/// The fixed key for the handwork's small irregularities.
pub const HAND: u32 = 0x6b6e_6974;

/// How strongly each kind of ear follows a mood's ear angle: long ears droop
/// the most; hanging ears and tentacles swing in when others droop out.
fn ear_rot(kind: &str) -> f64 {
    match kind {
        "bunny" => 1.0,
        "cat" => 0.6,
        "round" => 0.35,
        "floppy" => -0.45,
        "leaf" => 0.8,
        "gills" => 0.5,
        "tentacles" => -0.4,
        "frill" => 0.45,
        "ram" => 0.2,
        _ => 0.45,
    }
}

/// Kinds that frame the face from the side instead of standing on the head.
pub fn ear_frames(kind: &str) -> bool {
    matches!(kind, "floppy" | "gills" | "tentacles" | "ram")
}

type Prof = Box<dyn Fn(f64) -> f64>;

/// One tube of an ear, a gill, a frill fin, a ram's horn or a tuft's loop.
struct EarSpecT {
    l: f64,
    w: f64,
    prof: Prof,
    curl: f64,
    d_ang: f64,
    n: Option<usize>,
    late: f64,
    make: &'static str,
    horn: bool,
}

fn spec(l: f64, w: f64, prof: Prof, curl: f64) -> EarSpecT {
    EarSpecT {
        l,
        w,
        prof,
        curl,
        d_ang: 0.0,
        n: None,
        late: 0.0,
        make: "",
        horn: false,
    }
}

fn round_prof(at: f64) -> Prof {
    Box::new(move |t| {
        if t < at {
            1.0
        } else {
            max(0.0, 1.0 - sq((t - at) / (1.0 - at))).sqrt()
        }
    })
}

fn tube_max_y(tb: &Tube) -> f64 {
    max_of(tb.outline.iter().map(|q| q[1]))
}

fn tube_max_x(tb: &Tube) -> f64 {
    max_of(tb.outline.iter().map(|q| q[0]))
}

/// What grows at the sides of the head: ears, or something stranger.
pub fn build_ears(g: &mut Toy) {
    let x = g.t.ears.clone();
    if x.kind == "none" {
        return;
    }
    let p = g.face_piece().clone();
    let base = min(p.w, p.h * 0.55);
    let f = g.face().clone();
    let mut range: [f64; 2] = match x.kind {
        "bunny" => [0.07, 0.2],
        "cat" => [0.12, 0.27],
        "round" => [0.14, 0.31],
        "floppy" => [0.2, 0.33],
        "gills" => [0.42, 0.54],
        "leaf" => [0.1, 0.25],
        "tentacles" => [0.5, 0.6],
        "frill" => [0.18, 0.3],
        "ram" => [0.2, 0.3],
        _ => [0.15, 0.3],
    };
    // Eyes on stalks or bumps hold the top of the head, so ears start below.
    if let Some(ms) = g.mount_spec {
        let lo = ms.th / PI + if ms.kind == "stalks" { 0.12 } else { 0.17 };
        range = [max(range[0], lo), max(range[1], lo + 0.06)];
    }
    let bend = x.bend.unwrap_or(0.0) * 0.55;
    let specs_at = |k: f64| -> Vec<EarSpecT> {
        let sz = base * x.size * k;
        let wd = base * x.width * k;
        let mut specs = Vec::new();
        match x.kind {
            "round" => specs.push(spec(sz * 0.58, wd * 0.3, round_prof(0.42), 0.0)),
            "cat" => specs.push(spec(
                sz * 0.52,
                wd * 0.27,
                Box::new(|t| pw16(1.0 - t, 14) * 0.94 + 0.06),
                bend * 0.5,
            )),
            "bunny" => specs.push(spec(
                sz * 1.0,
                wd * 0.165,
                Box::new(|t| 0.74 + 0.34 * sin(PI * pw16(t, 14)) - 0.16 * t),
                bend,
            )),
            "floppy" => specs.push(spec(
                sz * 0.82,
                wd * 0.2,
                Box::new(|t| 0.6 + 0.45 * sin(PI * t * 0.92)),
                0.35 + bend * 0.6,
            )),
            "leaf" => specs.push(spec(
                sz * 0.62,
                wd * 0.2,
                Box::new(|t| pw16(sin(PI * (0.14 + 0.86 * t)), 13) * 0.96 + 0.04),
                bend,
            )),
            "gills" => {
                for d in [-0.5, 0.0, 0.5] {
                    specs.push(EarSpecT {
                        d_ang: d,
                        ..spec(
                            sz * if d == 0.0 { 0.36 } else { 0.29 },
                            wd * 0.075,
                            Box::new(|t| pw16(1.0 - t, 11) * 0.9 + 0.1),
                            d * 0.5,
                        )
                    });
                }
            }
            // Two tentacles a side hang from the widest part of the head and
            // curl out at their tips.
            "tentacles" => {
                for d in [0.0, 1.0] {
                    specs.push(EarSpecT {
                        d_ang: -0.3 * d,
                        n: Some(24),
                        late: 0.45,
                        ..spec(
                            sz * (1.05 - 0.28 * d),
                            wd * 0.085,
                            Box::new(|t| pw16(1.0 - t, 10) * 0.8 + 0.2),
                            -(1.5 + 0.3 * d + bend * 0.4),
                        )
                    });
                }
            }
            // A fan of three felt fins a side.
            "frill" => {
                for d in [-0.62, 0.0, 0.62] {
                    specs.push(EarSpecT {
                        d_ang: d,
                        make: "felt",
                        ..spec(
                            sz * if d == 0.0 { 0.38 } else { 0.3 },
                            wd * 0.115,
                            Box::new(|t| pw16(sin(PI * (0.1 + 0.9 * t)), 11) * 0.96 + 0.04),
                            d * 0.3,
                        )
                    });
                }
            }
            // A ram's horn: one long tube that curls round on itself.
            "ram" => specs.push(EarSpecT {
                n: Some(34),
                horn: true,
                ..spec(
                    sz * 1.02,
                    wd * 0.14,
                    Box::new(|t| pw16(1.0 - t, 12) * 0.84 + 0.16),
                    3.5 + bend * 0.7,
                )
            }),
            // A tuft of yarn loops a side.
            _ => {
                for d in [-0.5, 0.0, 0.5] {
                    specs.push(EarSpecT {
                        d_ang: d,
                        make: "loop",
                        ..spec(
                            sz * if d == 0.0 { 0.4 } else { 0.32 },
                            wd * 0.07,
                            Box::new(|_| 1.0),
                            0.0,
                        )
                    });
                }
            }
        }
        specs
    };
    let blend = match x.kind {
        "bunny" => 0.62,
        "cat" => 0.45,
        "round" => 0.25,
        "floppy" | "gills" | "tentacles" => 0.0,
        "leaf" => 0.42,
        "frill" | "ram" => 0.2,
        _ => 0.4,
    };
    let hangs = x.kind == "floppy" || x.kind == "tentacles";
    let ang_at = |n: Pt| -> f64 {
        let n_ang = atan2(n[1], n[0]);
        match x.kind {
            "floppy" => n_ang + 1.25 + x.tilt * 0.2,
            "tentacles" => PI / 2.0 - 0.22 + x.tilt * 0.1,
            "ram" => n_ang - 0.75 + x.tilt * 0.15,
            _ => n_ang + ang_delta(n_ang, -PI / 2.0) * blend + x.tilt * 0.3,
        }
    };
    let tubes_at = |specs: &[EarSpecT], p: Pt, ang: f64| -> Vec<Tube> {
        specs
            .iter()
            .map(|sp| {
                let a0 = ang + sp.d_ang;
                make_tube(
                    arc_spine(
                        sub(p, scl(dir_of(a0), sp.w * 0.55)),
                        a0,
                        sp.l,
                        sp.curl,
                        sp.n,
                        sp.late,
                    ),
                    &*sp.prof,
                    sp.w,
                    false,
                )
            })
            .collect()
    };
    // How far the ears keep from the eyes, in eye radii past the eye's edge.
    let dark = g.pal.dark && g.t.eyes.kind != "felt";
    let er = |r: f64| r * if dark { 1.42 } else { 1.0 };
    // Parts sewn on behind the head can't cover an eye.
    let back = x.kind == "tentacles" && g.head.is_some();
    let low = x.kind == "tentacles";
    let clearance =
        |tbs: &[Tube]| -> f64 {
            if low {
                return f64::INFINITY;
            }
            min_of(f.eyes.iter().map(|e| {
                min_of(tbs.iter().map(|tb| sdist([e.x.abs(), e.y], &tb.outline))) / er(e.r)
            }))
        };
    // Frills and tufts are fans: they are lifted only until they clear the
    // tops of the eyes, and stand no nearer the middle than 0.45 of the
    // face's half-width.
    let fan = x.kind == "frill" || x.kind == "tufts";
    let eye_top = min_of(f.eyes.iter().map(|e| e.y - e.r));
    let lift_to = if fan { eye_top } else { f.brow_y - f.r * 0.35 };
    let apart_x = if fan { f.fw * 0.45 } else { 0.0 };
    let mut specs = Vec::new();
    let mut th = 0.0;
    let mut pt = [0.0, 0.0];
    let mut ang = 0.0;
    let mut tbs: Vec<Tube> = Vec::new();
    let mut k = 1.0;
    for _ in 0..12 {
        specs = specs_at(k);
        let wb = specs[0].w;
        let step = 0.008 * PI;
        th = lerp(range[0], range[1], x.pos) * PI;
        if !ear_frames(x.kind) && g.mount_spec.is_none() {
            let mut it = 0;
            while it < 40
                && th > 0.05 * PI
                && p.at(th).0[1] + wb * 0.6 > lift_to
                && p.at(th - step).0[0] > max(wb * 1.15, apart_x)
            {
                th -= step;
                it += 1;
            }
            let mut it = 0;
            while it < 40 && p.at(th).0[0] < max(wb * 1.1, apart_x) {
                th += step;
                it += 1;
            }
        }
        let (q, n) = p.at(th);
        pt = q;
        ang = ang_at(n);
        for _ in 0..12 {
            tbs = tubes_at(&specs, pt, ang);
            let inward = min_of(
                tbs[0]
                    .outline
                    .iter()
                    .filter(|q| dist(**q, pt) > specs[0].w * 1.3)
                    .map(|q| q[0]),
            );
            if inward > specs[0].w * 0.3 && clearance(&tbs) > 1.3 {
                break;
            }
            ang += if hangs { -0.06 } else { 0.06 };
        }
        // Hanging parts must end above the ground too.
        if clearance(&tbs) > 1.3 && max_of(tbs.iter().map(tube_max_y)) <= g.ground_y - 1.5 {
            break;
        }
        k *= 0.92;
    }
    // A head too narrow above its eyes for two fans standing apart has none.
    if fan && pt[1] > eye_top {
        return;
    }
    let horn_gold = g.pal.main.y > 0.72;
    let horn_yarn = if horn_gold {
        g.pal.horn("gold").clone()
    } else {
        g.pal.horn("cream").clone()
    };
    let ringed = if horn_gold {
        g.pal.horn("cream").clone()
    } else {
        g.pal.horn("gold").clone()
    };
    let parent = g.head_rig();
    let n_specs = specs.len();
    for side in [1.0, -1.0] {
        let rname = if side > 0.0 { "earR" } else { "earL" };
        let sfx = if side > 0.0 { "r" } else { "l" };
        let root = if side > 0.0 { pt } else { flip_x(pt) };
        g.add_rig(rname, Some(parent), root);
        let mut loops = Vec::new();
        for (i, sp) in specs.iter().enumerate() {
            let a0 = ang + sp.d_ang;
            let start = sub(pt, scl(dir_of(a0), sp.w * 0.55));
            let tb = if side < 0.0 {
                mirror_tube(&tbs[i])
            } else {
                tbs[i].clone()
            };
            let id = format!(
                "ear-{sfx}{}",
                if n_specs > 1 {
                    i.to_string()
                } else {
                    String::new()
                }
            );
            if sp.make == "felt" {
                g.parts.push(Part {
                    id,
                    cls: "ear frill",
                    group: "ears",
                    kind: "felt",
                    rig: rname,
                    layer: 70,
                    outline: tb.outline.clone(),
                    tube: Some(tb),
                    felt: Some(g.pal.crest.hex),
                    felt_edge: Some(g.pal.crest.deep),
                    vein: true,
                    ..Part::default()
                });
                continue;
            }
            if sp.make == "loop" {
                // One loop of yarn: out along one side of the spine, round
                // its tip and back along the other.
                let tip = tb.spine[tb.spine.len() - 1];
                let b0 = tb.spine[0];
                let mid = mixp(b0, tip, 0.55);
                let nn = unit(perp(sub(tip, b0)));
                let w = sp.l * 0.3;
                loops.push(spline(
                    &[
                        add(b0, scl(nn, -w * 0.2)),
                        add(mid, scl(nn, -w)),
                        tip,
                        add(mid, scl(nn, w)),
                        add(b0, scl(nn, w * 0.2)),
                    ],
                    false,
                    6,
                ));
                continue;
            }
            let tip_rows = if x.tip == Some("tipped") { 0.3 } else { 0.0 };
            if sp.horn {
                g.parts.push(Part {
                    id: id.clone(),
                    cls: "ear ram-horn",
                    group: "ears",
                    kind: "tube",
                    rig: rname,
                    layer: 70,
                    outline: tb.outline.clone(),
                    tube: Some(tb),
                    yarn: Some(horn_yarn.clone()),
                    ringed: Some(ringed.clone()),
                    seam: Seam::Tube,
                    ..Part::default()
                });
            } else {
                g.parts.push(Part {
                    id: id.clone(),
                    cls: if low { "ear head-tentacle" } else { "ear" },
                    group: "ears",
                    kind: "tube",
                    rig: rname,
                    layer: if back {
                        26 - i as i32
                    } else if low {
                        45 - i as i32
                    } else {
                        70 - i as i32
                    },
                    outline: tb.outline.clone(),
                    tube: Some(tb),
                    yarn: Some(g.pal.main.clone()),
                    tip_yarn: (tip_rows != 0.0).then_some("second-or-accent"),
                    tip_rows,
                    seam: if back { Seam::None } else { Seam::Tube },
                    ..Part::default()
                });
            }
            // The inner ear: felt, or a smaller piece in the accent yarn.
            if let Some(inner) = x.inner {
                if inner != "none" && i == 0 {
                    let floppy = x.kind == "floppy";
                    let prof = &sp.prof;
                    let ip = |t: f64| prof(lerp(0.1, 0.92, t)) * if floppy { 0.58 } else { 0.55 };
                    let sp2 = arc_spine(
                        add(start, scl(dir_of(a0), sp.l * 0.14)),
                        a0,
                        sp.l * 0.74,
                        sp.curl * 0.74,
                        None,
                        0.0,
                    );
                    let mut it = make_tube(sp2, &ip, sp.w, false);
                    if side < 0.0 {
                        it = mirror_tube(&it);
                    }
                    g.parts.push(Part {
                        id: format!("{id}-inner"),
                        cls: "inner-ear patches",
                        group: "ears",
                        kind: if inner == "felt" { "felt" } else { "tube" },
                        rig: rname,
                        layer: 71,
                        outline: it.outline.clone(),
                        tube: Some(it),
                        yarn: Some(g.pal.accent.clone()),
                        felt: Some(g.pal.blush),
                        felt_edge: Some(g.pal.blush_edge),
                        ..Part::default()
                    });
                }
            }
        }
        // Tufts are the body's own yarn.
        if !loops.is_empty() {
            let outline = loops.concat();
            g.parts.push(Part {
                id: format!("ear-{sfx}"),
                cls: "ear tuft",
                group: "ears",
                kind: "loops",
                rig: rname,
                layer: 70,
                loops,
                width: max(2.0, base * x.width * 0.05),
                outline,
                yarn: Some(g.pal.main.clone()),
                ..Part::default()
            });
        }
    }
    g.ear_spec = Some(EarSpec {
        kind: x.kind,
        th,
        rot_k: ear_rot(x.kind),
    });
}

/// What grows on the crown.
pub fn build_top(g: &mut Toy) {
    let hd = g.t.top.clone();
    if hd.kind == "none" {
        return;
    }
    let p = g.face_piece().clone();
    let base = min(p.w, p.h * 0.55);
    let sz = base * hd.size;
    let rig_name = g.head_rig();
    let yarn_name = hd.yarn.unwrap_or("cream");
    let horn_yarn = g.pal.horn(yarn_name).clone();
    // Keep paired top parts clear of the ears.
    let ear_th = g.ear_spec.map_or(PI, |e| e.th);
    let spread = hd.spread.unwrap_or(0.0);
    let curl = hd.curl.unwrap_or(0.0);
    let pair_at = |lo: f64, hi: f64| min(lerp(lo, hi, spread) * PI, ear_th - 0.11 * PI);
    let up_ang = |n: Pt, k: f64| {
        let a = atan2(n[1], n[0]);
        a + ang_delta(a, -PI / 2.0) * k
    };
    let pair_pts = |th: f64| p.at(max(0.03, th));
    let mut parts: Vec<Part> = Vec::new();
    let horn_part = |id: String, tb: Tube, layer: i32, cls: &'static str, seam: Seam| Part {
        id,
        cls,
        group: "horns",
        kind: "tube",
        rig: rig_name,
        layer,
        outline: tb.outline.clone(),
        tube: Some(tb),
        yarn: Some(horn_yarn.clone()),
        seam,
        ..Part::default()
    };
    match hd.kind {
        "nubs" | "horns" => {
            let nubs = hd.kind == "nubs";
            let (pt, n) = pair_pts(if nubs {
                pair_at(0.05, 0.16)
            } else {
                pair_at(0.07, 0.2)
            });
            for side in [1.0, -1.0] {
                let a = up_ang(n, if nubs { 0.4 } else { 0.55 });
                let w = sz * if nubs { 0.1 } else { 0.105 };
                let mut tb = if nubs {
                    make_tube(
                        arc_spine(
                            sub(pt, scl(dir_of(a), w * 0.6)),
                            a,
                            sz * 0.2,
                            0.0,
                            None,
                            0.0,
                        ),
                        &|t| pw16(1.0 - t, 12) * 0.85 + 0.15,
                        w,
                        false,
                    )
                } else {
                    make_tube(
                        arc_spine(
                            sub(pt, scl(dir_of(a), w * 0.6)),
                            a,
                            sz * 0.5,
                            0.5 + curl * 0.9,
                            None,
                            0.0,
                        ),
                        &|t| pw16(1.0 - t, 14) * 0.93 + 0.07,
                        w,
                        false,
                    )
                };
                if side < 0.0 {
                    tb = mirror_tube(&tb);
                }
                let id = if side > 0.0 { "horn-r" } else { "horn-l" };
                parts.push(horn_part(id.into(), tb, 72, "horn", Seam::Tube));
            }
        }
        "unicorn" => {
            let (pt, _) = p.at(0.0);
            let w = sz * 0.1;
            let a = -PI / 2.0;
            let tb = make_tube(
                arc_spine(
                    sub(pt, scl(dir_of(a), w * 0.7)),
                    a,
                    sz * 0.55,
                    0.0,
                    None,
                    0.0,
                ),
                &|t| pw16(1.0 - t, 14) * 0.94 + 0.06,
                w,
                false,
            );
            let ringed = if yarn_name == "cream" {
                g.pal.horn("gold").clone()
            } else {
                g.pal.horn("cream").clone()
            };
            let mut part = horn_part("horn".into(), tb, 72, "horn", Seam::Tube);
            part.ringed = Some(ringed);
            parts.push(part);
        }
        "antennae" => {
            let (pt, n) = pair_pts(pair_at(0.03, 0.13));
            for side in [1.0, -1.0] {
                let a = up_ang(n, 0.7);
                let w = max(1.3, sz * 0.03);
                let mut tb = make_tube(
                    arc_spine(
                        sub(pt, scl(dir_of(a), 2.0)),
                        a,
                        sz * 0.5,
                        0.7 * curl + 0.25,
                        None,
                        0.0,
                    ),
                    &|_| 1.0,
                    w,
                    false,
                );
                if side < 0.0 {
                    tb = mirror_tube(&tb);
                }
                let tip = tb.spine[tb.spine.len() - 1];
                parts.push(Part {
                    id: (if side > 0.0 { "antenna-r" } else { "antenna-l" }).into(),
                    cls: "antenna",
                    group: "antennae",
                    kind: "stalk",
                    rig: rig_name,
                    layer: 72,
                    outline: tb.outline.clone(),
                    tube: Some(tb),
                    yarn: Some(g.pal.main.clone()),
                    ..Part::default()
                });
                let ball_yarn = if g.pal.accent.y > 0.82 {
                    g.pal.horn("gold").clone()
                } else {
                    g.pal.accent.clone()
                };
                parts.push(Part {
                    id: (if side > 0.0 {
                        "antenna-ball-r"
                    } else {
                        "antenna-ball-l"
                    })
                    .into(),
                    cls: "antenna-ball",
                    group: "antennae",
                    kind: "pompom",
                    rig: rig_name,
                    layer: 73,
                    c: tip,
                    r: sz * 0.11,
                    outline: ellipse_pts(tip, sz * 0.11, sz * 0.11, 24, 0.0),
                    yarn: Some(ball_yarn),
                    ..Part::default()
                });
            }
        }
        // A tuft: three loose loops of the body's own yarn standing on the
        // crown, the middle one the tallest.
        "tuft" => {
            let (pt, _) = p.at(0.0);
            let mut pts = Vec::new();
            for f in [-1.0_f64, 0.0, 1.0] {
                let a = -PI / 2.0 + f * 0.62 + curl * 0.3;
                let n2 = perp(dir_of(a));
                let lp = sz * (0.44 - 0.12 * f.abs());
                let w = lp * 0.22;
                let tip = add(pt, scl(dir_of(a), lp));
                pts.push(spline(
                    &[
                        add(pt, scl(n2, -w * 0.2)),
                        add(mixp(pt, tip, 0.55), scl(n2, -w)),
                        tip,
                        add(mixp(pt, tip, 0.55), scl(n2, w)),
                        add(pt, scl(n2, w * 0.2)),
                    ],
                    false,
                    6,
                ));
            }
            let outline = pts.concat();
            parts.push(Part {
                id: "tuft".into(),
                cls: "tuft",
                group: "tuft",
                kind: "loops",
                rig: rig_name,
                layer: 73,
                loops: pts,
                width: max(1.6, sz * 0.04),
                outline,
                yarn: Some(g.pal.main.clone()),
                ..Part::default()
            });
        }
        "sprout" => {
            let (pt, _) = p.at(0.0);
            let a = -PI / 2.0 + curl * 0.3;
            let stalk = make_tube(
                arc_spine(sub(pt, [0.0, -2.0]), a, sz * 0.3, curl * 0.6, None, 0.0),
                &|_| 1.0,
                max(1.4, sz * 0.035),
                false,
            );
            let tip = stalk.spine[stalk.spine.len() - 1];
            let ta = a + curl * 0.6;
            parts.push(Part {
                id: "sprout-stalk".into(),
                cls: "sprout",
                group: "sprout",
                kind: "stalk",
                rig: rig_name,
                layer: 72,
                outline: stalk.outline.clone(),
                tube: Some(stalk),
                yarn: Some(g.pal.stalk.clone()),
                ..Part::default()
            });
            for side in [1.0, -1.0] {
                let la = ta + side * 1.0;
                let lb = make_tube(
                    arc_spine(tip, la, sz * 0.34, -side * 0.5, None, 0.0),
                    &|t| pw16(sin(PI * (0.06 + 0.94 * t)), 11),
                    sz * 0.11,
                    false,
                );
                parts.push(Part {
                    id: (if side > 0.0 { "leaf-r" } else { "leaf-l" }).into(),
                    cls: "sprout leaf",
                    group: "sprout",
                    kind: "felt",
                    rig: rig_name,
                    layer: 73,
                    outline: lb.outline.clone(),
                    tube: Some(lb),
                    felt: Some(g.pal.felt.leaf),
                    felt_edge: Some(g.pal.felt.leaf_edge),
                    vein: true,
                    ..Part::default()
                });
            }
        }
        // Antlers: a knitted beam a side with two or three short tines sewn
        // to it, each a small tube of its own.
        "antlers" => {
            let (pt, n) = pair_pts(pair_at(0.08, 0.2));
            for side in [1.0, -1.0] {
                let a = up_ang(n, 0.6);
                let w = sz * 0.062;
                let sfx = if side > 0.0 { "r" } else { "l" };
                let beam = make_tube(
                    arc_spine(
                        sub(pt, scl(dir_of(a), w * 0.6)),
                        a,
                        sz * 0.62,
                        0.35 + curl * 0.3,
                        Some(20),
                        0.0,
                    ),
                    &|t| 1.0 - 0.3 * t,
                    w,
                    false,
                );
                let at: &[f64] = if hd.tines == Some(3) {
                    &[0.3, 0.55, 0.78]
                } else {
                    &[0.38, 0.7]
                };
                for (i, &f) in at.iter().enumerate() {
                    let j = crate::geom::js_round(f * 20.0) as usize;
                    let q = beam.spine[j];
                    let tg = beam.tan[j];
                    // Tines branch to the outside and a little forward.
                    let odd = i % 2 == 1;
                    let ta = atan2(tg[1], tg[0]) + if odd { -1.0 } else { 1.0 } * 0.95;
                    let mut tine = make_tube(
                        arc_spine(
                            q,
                            ta,
                            sz * (0.24 - 0.03 * i as f64),
                            if odd { 1.0 } else { -1.0 } * 0.3,
                            Some(8),
                            0.0,
                        ),
                        &|t| 1.0 - 0.35 * t,
                        w * 0.8,
                        false,
                    );
                    if side < 0.0 {
                        tine = mirror_tube(&tine);
                    }
                    parts.push(horn_part(
                        format!("tine-{sfx}{i}"),
                        tine,
                        71,
                        "horn antler tine",
                        Seam::None,
                    ));
                }
                let b2 = if side < 0.0 { mirror_tube(&beam) } else { beam };
                parts.push(horn_part(
                    format!("antler-{sfx}"),
                    b2,
                    72,
                    "horn antler",
                    Seam::Tube,
                ));
            }
        }
        // Spines: a crest of three or five knitted cones over the crown.
        // Plates: the same crest cut from felt and sewn on behind the head.
        // The middle one is tall, each further one about half the one
        // before.
        _ => {
            let felt = hd.kind == "plates";
            let count = hd.count.unwrap_or(3) as i32;
            let half = (count - 1) / 2;
            let crest = g.pal.crest.clone();
            let tall = [1.0, 0.55, 0.28];
            let width_of =
                |j: usize| sz * if felt { 0.13 } else { 0.095 } * (0.55 + 0.45 * tall[j]);
            // Where each one stands, as an angle from the crown: far enough
            // from the one before for their bases to meet.
            let mut ths = vec![0.0];
            for j in 1..=half as usize {
                let prev = p.at(ths[j - 1]).0;
                let want = (width_of(j - 1) + width_of(j)) * 0.9;
                let mut th = ths[j - 1];
                let mut it = 0;
                while it < 60 && dist(p.at(th).0, prev) < want {
                    th += 0.004 * PI;
                    it += 1;
                }
                ths.push(th);
            }
            for i in -half..=half {
                let j = i.unsigned_abs() as usize;
                if ths[j] > ear_th - 0.1 * PI {
                    continue;
                }
                let (q, n) = p.at(ths[j]);
                let pp = if i < 0 { flip_x(q) } else { q };
                let nn = if i < 0 { flip_x(n) } else { n };
                let a0 = atan2(nn[1], nn[0]);
                let a = a0 + ang_delta(a0, -PI / 2.0) * 0.3;
                let l = sz * if felt { 0.4 } else { 0.36 } * tall[j];
                let w = width_of(j);
                let spine = arc_spine(
                    sub(pp, scl(dir_of(a), if felt { l * 0.22 } else { w * 0.6 })),
                    a,
                    l * if felt { 1.22 } else { 1.0 },
                    0.0,
                    Some(10),
                    0.0,
                );
                let tb = if felt {
                    make_tube(spine, &|t| pw16(sin(PI * (0.3 + 0.7 * t)), 12), w, false)
                } else {
                    make_tube(spine, &|t| pw16(1.0 - t, 14) * 0.92 + 0.08, w, false)
                };
                let id = format!("{}{}", if felt { "plate-" } else { "spine-" }, i + half);
                if felt {
                    parts.push(Part {
                        id,
                        cls: "plate",
                        group: "spines",
                        kind: "felt",
                        rig: rig_name,
                        layer: 59 - j as i32,
                        outline: tb.outline.clone(),
                        tube: Some(tb),
                        felt: Some(crest.hex),
                        felt_edge: Some(crest.deep),
                        stitched: true,
                        ..Part::default()
                    });
                } else {
                    parts.push(Part {
                        id,
                        cls: "spine",
                        group: "spines",
                        kind: "tube",
                        rig: rig_name,
                        layer: 72 - j as i32,
                        outline: tb.outline.clone(),
                        tube: Some(tb),
                        yarn: Some(crest.clone()),
                        seam: Seam::Tube,
                        ..Part::default()
                    });
                }
            }
        }
    }
    g.parts.extend(parts);
}

struct ArmSpecT {
    l: f64,
    w: f64,
    ang: f64,
    prof: Prof,
    curl: f64,
    n: Option<usize>,
    late: f64,
}

pub fn build_arms(g: &mut Toy) {
    let a = g.t.arms.clone();
    if a.kind == "none" {
        return;
    }
    let b = g.body.clone();
    let mut th = if g.head.is_some() {
        lerp(0.3, 0.5, a.pos) * PI
    } else {
        lerp(0.44, 0.62, a.pos) * PI
    };
    // A stacked toy's shoulders move down the body until they clear the head.
    if let Some(head) = &g.head {
        let mut it = 0;
        while it < 30 && sdist(b.at(th).0, &head.outline) < 3.0 {
            th += 0.012 * PI;
            it += 1;
        }
    }
    // On a one-piece toy, gills, floppy ears and hanging tentacles sit where
    // shoulders would; the arms start below them.
    if g.head.is_none() {
        if let Some(es) = g.ear_spec {
            if ear_frames(es.kind) && es.kind != "ram" {
                th = clamp(es.th + 0.14 * PI, th, 0.7 * PI);
            }
        }
    }
    // Arms are sized from the body's half-width, held to a share of its
    // height.
    let rf = clamp(b.w, 28.0, max(28.0, b.h * 0.52));
    let h = b.h;
    let four = a.kind == "four";
    if four {
        th = min(th, 0.56 * PI) - 0.04 * PI;
    }
    // How far past the body's side a limb should reach.
    let reach = match a.kind {
        "nub" => 0.42,
        "long" => 0.36,
        "wing" => 0.55,
        "four" => 0.4,
        _ => 0.42,
    } * rf;
    let pairs: Vec<(f64, f64, f64)> = if four {
        vec![
            (th, 0.86, -0.1),
            (min(th + 0.15 * PI, 0.73 * PI), 0.74, 0.35),
        ]
    } else {
        vec![(th, 1.0, 0.0)]
    };
    for (idx, &(pth, k0, down)) in pairs.iter().enumerate() {
        let (pt, n) = b.at(pth);
        let mut k = k0;
        let make_spec = |k: f64| -> ArmSpecT {
            match a.kind {
                "nub" | "four" => ArmSpecT {
                    l: rf * 0.4 * a.len * k,
                    w: rf * 0.19 * a.width * k0,
                    ang: lerp(0.15, 1.0, a.angle) + down,
                    prof: Box::new(|t| 0.82 + 0.18 * sin(PI * t)),
                    curl: 0.2,
                    n: None,
                    late: 0.0,
                },
                "long" => ArmSpecT {
                    l: max(h * 0.56, rf * 0.6) * a.len * k,
                    w: rf * 0.125 * a.width,
                    ang: PI / 2.0 - lerp(0.06, 0.42, a.angle),
                    prof: Box::new(|t| 0.82 + 0.24 * t * t),
                    curl: -0.12,
                    n: None,
                    late: 0.0,
                },
                // A tentacle arm hangs down and curls out and up at its tip.
                "tentacle" => ArmSpecT {
                    l: max(h * 0.62, rf * 0.7) * a.len * k,
                    w: rf * 0.1 * a.width,
                    ang: PI / 2.0 - lerp(0.1, 0.4, a.angle),
                    prof: Box::new(|t| pw16(1.0 - t, 10) * 0.78 + 0.22),
                    curl: -1.7,
                    n: Some(24),
                    late: 0.4,
                },
                _ => ArmSpecT {
                    l: rf * 0.62 * a.len * k,
                    w: rf * 0.27 * a.width,
                    ang: lerp(0.45, 1.1, a.angle),
                    prof: Box::new(|t| pw16(sin(PI * (0.16 + 0.84 * t)), 10) * 0.95 + 0.05),
                    curl: 0.35,
                    n: None,
                    late: 0.0,
                },
            }
        };
        let mut sp = make_spec(k);
        let mut tb = make_tube(
            arc_spine(
                sub(pt, scl(dir_of(sp.ang), sp.w * 0.7)),
                sp.ang,
                sp.l,
                sp.curl,
                sp.n,
                sp.late,
            ),
            &*sp.prof,
            sp.w,
            false,
        );
        for it in 0..6 {
            if it > 0 {
                sp = make_spec(k);
                tb = make_tube(
                    arc_spine(
                        sub(pt, scl(dir_of(sp.ang), sp.w * 0.7)),
                        sp.ang,
                        sp.l,
                        sp.curl,
                        sp.n,
                        sp.late,
                    ),
                    &*sp.prof,
                    sp.w,
                    false,
                );
            }
            let over = tube_max_x(&tb) - (b.w + reach);
            // An arm that hangs must end above the ground.
            let under = tube_max_y(&tb) - (g.ground_y - 1.5);
            if over <= 0.0 && under <= 0.0 {
                break;
            }
            k *= max(0.5, 1.0 - max(over, under) / sp.l);
        }
        let shoulder = add(pt, scl(n, -sp.w * 0.35));
        for side in [1.0, -1.0] {
            let rname = match (idx, side > 0.0) {
                (0, true) => "armR",
                (0, false) => "armL",
                (_, true) => "arm2R",
                (_, false) => "arm2L",
            };
            g.add_rig(
                rname,
                Some("root"),
                if side > 0.0 {
                    shoulder
                } else {
                    flip_x(shoulder)
                },
            );
            let t2 = if side < 0.0 {
                mirror_tube(&tb)
            } else {
                tb.clone()
            };
            let paw = a.paw == Some("paw");
            g.parts.push(Part {
                id: format!(
                    "{}{}",
                    if idx > 0 { "arm2-" } else { "arm-" },
                    if side > 0.0 { "r" } else { "l" }
                ),
                cls: match a.kind {
                    "wing" => "wing limb",
                    "tentacle" => "arm tentacle limb",
                    _ => "arm limb",
                },
                group: "arms",
                kind: "tube",
                rig: rname,
                layer: 50 - idx as i32,
                outline: t2.outline.clone(),
                tube: Some(t2),
                yarn: Some(g.pal.main.clone()),
                tip_yarn: paw.then_some("accent"),
                tip_rows: if paw { 0.26 } else { 0.0 },
                seam: Seam::Tube,
                ..Part::default()
            });
        }
        if idx == 0 {
            g.arm_spec = Some(ArmSpec {
                kind: a.kind,
                shoulder,
            });
        }
    }
}

pub fn build_legs(g: &mut Toy) {
    let lg = g.t.legs.clone();
    if lg.kind == "none" {
        return;
    }
    let b = g.body.clone();
    let rf = max(b.w, 28.0);
    let yb = b.y_bot;
    let span_at = |y: f64| b.half_w(y);
    let main = g.pal.main.clone();
    if lg.kind == "feet" {
        let fa = rf * 0.24 * lg.width * lerp(0.9, 1.1, (lg.len - 0.78) / 0.5);
        let fb = fa * 0.78;
        let fy = yb - fb * 0.42;
        let fx = max(
            fa * 0.95,
            min(
                span_at(yb - fb) - fa * 0.35,
                lerp(0.3, 0.62, lg.spread) * span_at(yb - fb * 1.2),
            ),
        );
        let pads = lg.pads.unwrap_or("none");
        for side in [1.0, -1.0] {
            let c = [side * fx, fy];
            g.parts.push(Part {
                id: (if side > 0.0 { "foot-r" } else { "foot-l" }).into(),
                cls: "foot limb",
                group: "legs",
                kind: "ring",
                rig: "root",
                layer: 40,
                c,
                rx: fa,
                ry: fb,
                outline: ellipse_pts(c, fa, fb, 40, 0.0),
                yarn: Some(main.clone()),
                seam: Seam::Ring,
                ..Part::default()
            });
            if pads != "none" {
                let pc = [c[0], c[1] + fb * 0.08];
                let pad = |id: String, cls: &'static str, outline: Vec<Pt>| Part {
                    id,
                    cls,
                    group: "legs",
                    kind: "felt",
                    rig: "root",
                    layer: 41,
                    outline,
                    felt: Some(g.pal.felt.pad),
                    felt_edge: Some(g.pal.blush_edge),
                    ..Part::default()
                };
                let pid = if side > 0.0 { "pad-r" } else { "pad-l" };
                if pads == "pad" {
                    g.parts.push(pad(
                        pid.into(),
                        "foot-pad patches",
                        ellipse_pts(pc, fa * 0.55, fb * 0.5, 28, 0.0),
                    ));
                } else {
                    let mut new = vec![pad(
                        pid.into(),
                        "foot-pad patches",
                        ellipse_pts([pc[0], pc[1] + fb * 0.18], fa * 0.42, fb * 0.34, 24, 0.0),
                    )];
                    for j in [-1.0_f64, 0.0, 1.0] {
                        let bc = [
                            pc[0] + j * fa * 0.38,
                            pc[1] - fb * 0.38 + j.abs() * fb * 0.1,
                        ];
                        new.push(pad(
                            format!(
                                "{}{}",
                                if side > 0.0 { "bean-r" } else { "bean-l" },
                                j as i32 + 1
                            ),
                            "toe-bean patches",
                            ellipse_pts(bc, fa * 0.15, fb * 0.15, 16, 0.0),
                        ));
                    }
                    g.parts.extend(new);
                }
            }
        }
        return;
    }
    if lg.kind == "legs" {
        // Legs are sized from the body's half-width, held to a share of its
        // height.
        let lref = clamp(b.w, 28.0, max(28.0, b.h * 0.52));
        let w = lref * 0.15 * lg.width;
        let l = b.h * 0.3 * lg.len;
        let x = min(
            span_at(yb - w) - w * 1.1,
            max(w * 1.2, lerp(0.25, 0.55, lg.spread) * span_at(yb - w * 2.0)),
        );
        for side in [1.0, -1.0] {
            let ang = PI / 2.0 - lerp(0.0, 0.18, lg.spread);
            let mut tb = make_tube(
                arc_spine([x, yb - w * 1.6], ang, l + w * 1.6, 0.0, None, 0.0),
                &|t| 1.0 + 0.22 * smooth(clamp((t - 0.72) / 0.28, 0.0, 1.0)),
                w,
                false,
            );
            if side < 0.0 {
                tb = mirror_tube(&tb);
            }
            g.parts.push(Part {
                id: (if side > 0.0 { "leg-r" } else { "leg-l" }).into(),
                cls: "leg limb",
                group: "legs",
                kind: "tube",
                rig: "root",
                layer: 20,
                outline: tb.outline.clone(),
                tube: Some(tb),
                yarn: Some(main.clone()),
                ..Part::default()
            });
        }
        return;
    }
    // Tentacles: four or six, fanning out under the body, tips curling out.
    let cnt = lg.count.unwrap_or(4);
    let w = rf * 0.1 * lg.width;
    let l = b.h * 0.42 * lg.len;
    let half = cnt / 2;
    for i in 0..half {
        let f = (i as f64 + 0.5) / half as f64;
        let x = lerp(0.12, 0.72, f) * span_at(yb - w * 2.0);
        let ang = PI / 2.0 - lerp(0.1, 0.9, f) * lerp(0.6, 1.1, lg.spread);
        for side in [1.0, -1.0] {
            let mut tb = make_tube(
                arc_spine([x, yb - w * 2.2], ang, l, -(1.1 + 0.6 * f), None, 0.0),
                &|t| pw16(1.0 - t, 10) * 0.82 + 0.18,
                w,
                false,
            );
            if side < 0.0 {
                tb = mirror_tube(&tb);
            }
            g.parts.push(Part {
                id: format!("tentacle-{}{i}", if side > 0.0 { "r" } else { "l" }),
                cls: "tentacle limb",
                group: "legs",
                kind: "tube",
                rig: "root",
                layer: 20 - i as i32,
                outline: tb.outline.clone(),
                tube: Some(tb),
                yarn: Some(main.clone()),
                ..Part::default()
            });
        }
    }
}

/// The tail is sewn on behind the body, low on one side.
pub fn build_tail(g: &mut Toy) {
    let tl = g.t.tail.clone();
    if tl.kind == "none" {
        return;
    }
    let b = g.body.clone();
    let rf = max(b.w, 28.0);
    let side = tl.side;
    let sz = tl.size;
    let y = b.y_bot - b.h * 0.24;
    let x = b.half_w(y);
    let pivot = if tl.kind == "pompom" {
        [x * 0.7, y]
    } else {
        [x * 0.8, y]
    };
    g.add_rig(
        "tail",
        Some("root"),
        if side > 0.0 { pivot } else { flip_x(pivot) },
    );
    if tl.kind == "pompom" {
        let r = rf * 0.24 * sz;
        let c = [x + r * 0.15, y + r * 0.2];
        let cc = if side > 0.0 { c } else { flip_x(c) };
        let yarn = if g.pal.accent.y > 0.75 {
            g.pal.accent.clone()
        } else {
            g.pal.main.clone()
        };
        g.parts.push(Part {
            id: "tail".into(),
            cls: "tail pompom",
            group: "tail",
            kind: "pompom",
            rig: "tail",
            layer: 10,
            c: cc,
            r,
            outline: ellipse_pts(cc, r, r, 30, 0.0),
            yarn: Some(yarn),
            ..Part::default()
        });
        return;
    }
    let tipped = tl.tip == Some("tipped") && (tl.kind == "long" || tl.kind == "bushy");
    let curl = tl.curl.unwrap_or(0.0);
    let make = |k: f64, up: f64| -> Tube {
        match tl.kind {
            "curly" => {
                let w = rf * 0.075 * sz;
                make_tube(
                    arc_spine(
                        [x - w * 1.5, y - up],
                        -0.25,
                        rf * 0.62 * sz * k,
                        -(3.6 + 1.2 * curl),
                        Some(30),
                        0.0,
                    ),
                    &|t| 1.0 - 0.3 * t,
                    w,
                    false,
                )
            }
            "long" => {
                let w = rf * 0.07 * sz;
                make_tube(
                    arc_spine(
                        [x - w * 2.0, y + b.h * 0.1 - up],
                        0.25,
                        b.h * 1.0 * sz * k,
                        -(1.9 + 0.8 * curl),
                        Some(26),
                        0.0,
                    ),
                    &|t| 1.0 - 0.25 * t,
                    w,
                    false,
                )
            }
            "bushy" => {
                let w = rf * 0.32 * sz * k.sqrt();
                make_tube(
                    arc_spine(
                        [x - w * 0.8, y + b.h * 0.12 - up],
                        -0.75,
                        b.h * 0.95 * sz * k,
                        -(0.4 + 0.5 * curl),
                        Some(22),
                        0.0,
                    ),
                    &|t| pw16(sin(PI * (0.12 + 0.8 * t)), 10) * 0.9 + 0.1,
                    w,
                    false,
                )
            }
            _ => {
                let w = rf * 0.2 * sz;
                make_tube(
                    arc_spine(
                        [x - w * 1.2, b.y_bot - w * 1.1 - up],
                        0.22,
                        (rf * 0.75 * sz + w) * k,
                        -0.15,
                        None,
                        0.0,
                    ),
                    &|t| pw16(1.0 - t, 14) * 0.94 + 0.06,
                    w,
                    false,
                )
            }
        }
    };
    let mut k = 1.0;
    let mut tb = make(k, 0.0);
    for _ in 0..5 {
        let over = tube_max_x(&tb) - (b.w + rf * 0.6);
        if over <= 0.0 {
            break;
        }
        k *= max(0.5, 1.0 - over / (rf * 1.2));
        tb = make(k, 0.0);
    }
    // A tail that would dip below the ground is sewn on that much higher.
    let under = tube_max_y(&tb) - (g.ground_y - 1.0);
    if under > 0.0 {
        tb = make(k, under);
    }
    if side < 0.0 {
        tb = mirror_tube(&tb);
    }
    g.parts.push(Part {
        id: "tail".into(),
        cls: "tail",
        group: "tail",
        kind: "tube",
        rig: "tail",
        layer: 10,
        outline: tb.outline.clone(),
        tube: Some(tb),
        yarn: Some(g.pal.main.clone()),
        tip_yarn: tipped.then_some("second-or-accent"),
        tip_rows: if tipped {
            if tl.kind == "bushy" {
                0.3
            } else {
                0.22
            }
        } else {
            0.0
        },
        ..Part::default()
    });
}

pub fn build_snout(g: &mut Toy) {
    let Some(s) = g.face().snout else {
        return;
    };
    let rig_name = g.head_rig();
    if s.kind == "beak" {
        let (x, y, c) = (s.rx, s.ry, s.c);
        let pts = spline(
            &[
                [c[0] - x, c[1] - y * 0.62],
                [c[0], c[1] - y * 0.8],
                [c[0] + x, c[1] - y * 0.62],
                [c[0] + x * 0.5, c[1] + y * 0.3],
                [c[0], c[1] + y],
                [c[0] - x * 0.5, c[1] + y * 0.3],
            ],
            true,
            5,
        );
        g.parts.push(Part {
            id: "snout".into(),
            cls: "snout beak",
            group: "face",
            kind: "felt",
            rig: rig_name,
            layer: 82,
            outline: pts,
            felt: Some(g.pal.beak.hex),
            felt_edge: Some(g.pal.beak.deep),
            stitched: true,
            ..Part::default()
        });
        return;
    }
    let yarn = if s.kind == "bill" {
        g.pal.beak.clone()
    } else {
        match g.t.snout.yarn {
            Some("accent") => g.pal.light.clone(),
            Some("tint") => g.pal.tint.clone(),
            _ => g.pal.main.clone(),
        }
    };
    g.parts.push(Part {
        id: "snout".into(),
        cls: "snout",
        group: "face",
        kind: "ring",
        rig: rig_name,
        layer: 82,
        c: s.c,
        rx: s.rx,
        ry: s.ry,
        outline: ellipse_pts(s.c, s.rx, s.ry, 44, 0.0),
        yarn: Some(yarn),
        seam: Seam::Ring,
        ..Part::default()
    });
}

/// The control points of a heart of half-width about s round c.
pub fn heart_pts(c: Pt, s: f64) -> Vec<Pt> {
    vec![
        [c[0], c[1] - s * 0.35],
        [c[0] + s * 0.55, c[1] - s * 0.8],
        [c[0] + s * 1.05, c[1] - s * 0.3],
        [c[0] + s * 0.5, c[1] + s * 0.35],
        [c[0], c[1] + s * 0.75],
        [c[0] - s * 0.5, c[1] + s * 0.35],
        [c[0] - s * 1.05, c[1] - s * 0.3],
        [c[0] - s * 0.55, c[1] - s * 0.8],
    ]
}

pub fn build_patches(g: &mut Toy) {
    let pt = g.t.patches.clone();
    let b = g.body.clone();
    let f = g.face().clone();
    let head = g.head.clone();
    if pt.belly != "none" {
        // On a stacked toy the belly sits on the body below the head; on a
        // one-piece toy, below the mouth.
        let top = match &head {
            Some(h) => max(h.y_bot - h.h * 0.02, b.y_top + b.h * 0.25),
            None => f.mouth.y + f.mouth.w * 0.6 + 6.0,
        };
        let bottom = b.y_bot - b.h * 0.07;
        if bottom - top > b.h * 0.18 {
            let cy = (top + bottom) / 2.0;
            let mut ry = (bottom - top) / 2.0 * (0.75 + 0.25 * pt.belly_size);
            let mut rx = min(b.half_w(cy) * pt.belly_size * 1.1, ry * 1.25);
            let mut it = 0;
            while it < 30
                && !(sdist([rx, cy], &b.outline) < -3.0 && sdist([0.0, cy + ry], &b.outline) < -2.0)
            {
                rx *= 0.96;
                ry *= 0.97;
                it += 1;
            }
            let out = ellipse_pts([0.0, cy], rx, ry, 44, 0.0);
            let light = g.pal.light.clone();
            if pt.belly == "knit" {
                g.parts.push(Part {
                    id: "belly".into(),
                    cls: "belly patches",
                    group: "patches",
                    kind: "ring",
                    rig: "root",
                    layer: 32,
                    c: [0.0, cy],
                    rx,
                    ry,
                    outline: out,
                    yarn: Some(light),
                    seam: Seam::Ring,
                    ..Part::default()
                });
            } else {
                g.parts.push(Part {
                    id: "belly".into(),
                    cls: "belly patches",
                    group: "patches",
                    kind: "felt",
                    rig: "root",
                    layer: 32,
                    outline: out,
                    felt: Some(light.hex),
                    felt_edge: Some(light.deep),
                    stitched: true,
                    ..Part::default()
                });
            }
            g.belly = Some(Belly {
                c: [0.0, cy],
                rx,
                ry,
            });
        }
    }
    if pt.spots == "spots" {
        // Mirrored pairs of felt spots on the body's flanks, at one of eight
        // layouts.
        let l8 = pt.spot_layout;
        let mut hs = Stream::new(hash_str("spots", HAND ^ l8));
        let cnt = 2 + (l8 & 3);
        let pal = &g.pal;
        let col = if pal.accent.y > 0.7 || (pal.accent.y - pal.main.y).abs() < 0.1 {
            pal.main.deep
        } else {
            pal.accent.hex
        };
        let face_line = if head.is_some() {
            f64::NEG_INFINITY
        } else {
            f.mouth.y + f.mouth.w * 0.9
        };
        let main_deep = pal.main.deep;
        let mut new = Vec::new();
        for i in 0..cnt {
            let th = lerp(
                0.3,
                0.85,
                (f64::from(i) + 0.3 + 0.4 * hs.draw()) / f64::from(cnt),
            ) * PI;
            let rr = max(4.0, b.w * lerp(0.09, 0.16, hs.draw()));
            let (q, n) = b.at(th);
            let c = sub(
                q,
                scl(
                    n,
                    rr * (1.6 + hs.draw() * 1.2) + b.w * 0.06 * f64::from(l8 >> 2),
                ),
            );
            if sdist(c, &b.outline) > -rr * 1.1 {
                continue;
            }
            if c[1] - rr < face_line + 3.0 {
                continue;
            }
            if let Some(h) = &head {
                if sdist(c, &h.outline) < rr + 2.0 {
                    continue;
                }
            }
            if let Some(be) = g.belly {
                if hyp(c[0] / (be.rx + rr), (c[1] - be.c[1]) / (be.ry + rr)) < 1.0 {
                    continue;
                }
            }
            for side in [1.0, -1.0] {
                let tilt = hs.draw() - 0.5;
                new.push(Part {
                    id: format!("spot-{}{i}", if side > 0.0 { "r" } else { "l" }),
                    cls: "spot patches",
                    group: "patches",
                    kind: "felt",
                    rig: "root",
                    layer: 33,
                    outline: ellipse_pts(
                        if side > 0.0 { c } else { flip_x(c) },
                        rr,
                        rr * 0.9,
                        20,
                        tilt,
                    ),
                    felt: Some(col),
                    felt_edge: Some(main_deep),
                    ..Part::default()
                });
            }
        }
        g.parts.extend(new);
    }
    // Joint buttons mark the shoulders, except where a one-piece toy's arms
    // start beside its face.
    let pal = g.pal.clone();
    if pt.mend == "joints"
        && g.arm_spec
            .is_some_and(|a| head.is_some() || a.shoulder[1] > f.mouth.y)
    {
        let sh = g.arm_spec.expect("checked above").shoulder;
        let br = clamp(b.w * 0.07, 3.0, 5.5);
        g.joint_buttons = vec![(sh, br), (flip_x(sh), br)];
    } else if pt.mend == "mend" {
        let sd = pt.mend_side;
        let y = match &head {
            Some(h) => h.y_bot + (b.y_bot - h.y_bot) * 0.45,
            None => b.y_top + b.h * 0.68,
        };
        let x = sd * b.half_w(y) * 0.5;
        let half = clamp(b.w * 0.2, 6.0, 14.0);
        let pts: Vec<Pt> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
            .iter()
            .map(|q: &[f64; 2]| {
                add(
                    [x, y],
                    rot([q[0] * half, q[1] * half * 0.9], sd * 0.18, None),
                )
            })
            .collect();
        // On a one-piece toy the patch keeps below the face.
        let clear = head.is_some()
            || y - half * 1.1
                > max(
                    f.mouth.y + f.mouth.w * 0.6,
                    f.cheeks.map_or(0.0, |c| c.y + c.ry),
                ) + 3.0;
        if clear && pts.iter().all(|q| sdist(*q, &b.outline) < -2.0) {
            g.parts.push(Part {
                id: "mend".into(),
                cls: "mend patches",
                group: "patches",
                kind: "felt",
                rig: "root",
                layer: 34,
                outline: pts,
                felt: Some(if pal.accent.y > 0.75 {
                    hsl(pal.main.h + 160.0, 45.0, 74.0)
                } else {
                    pal.accent.hex
                }),
                felt_edge: Some(pal.main.deep),
                cross_stitch: true,
                ..Part::default()
            });
        }
    } else if pt.mend == "heart" {
        // A small felt heart sewn on the chest, a little to one side.
        let sd = pt.mend_side;
        let y = match &head {
            Some(h) => h.y_bot + (b.y_bot - h.y_bot) * 0.38,
            None => b.y_top + b.h * 0.72,
        };
        let s = clamp(b.w * 0.13, 4.5, 9.0);
        let x = if g.belly.is_some() {
            0.0
        } else {
            sd * b.half_w(y) * 0.42
        };
        let c = [x, g.belly.map_or(y, |be| be.c[1])];
        let turn = if g.belly.is_some() { 0.0 } else { sd * 0.2 };
        let pts: Vec<Pt> = spline(&heart_pts(c, s), true, 5)
            .into_iter()
            .map(|q| rot(q, turn, Some(c)))
            .collect();
        let clear = head.is_some() || c[1] - s > f.mouth.y + f.mouth.w * 0.6 + 3.0;
        if clear && pts.iter().all(|q| sdist(*q, &b.outline) < -2.0) {
            g.parts.push(Part {
                id: "heart".into(),
                cls: "heart patches",
                group: "patches",
                kind: "felt",
                rig: "root",
                layer: 34,
                outline: pts,
                felt: Some(pal.blush),
                felt_edge: Some(pal.blush_edge),
                stitched: true,
                thread_on: Some(pal.blush_deep),
                ..Part::default()
            });
        }
    }
}
