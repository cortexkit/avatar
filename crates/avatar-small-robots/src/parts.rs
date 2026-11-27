//! What is plugged into the head: antennae and what else stands on top,
//! lens pods on stalks, pieces bolted to its sides, and a cable or a pipe
//! behind.

use crate::draw::{
    bone, disc, hose, lamp, plate, plate_k, rays, rod, seam, Ctx, El, Out, NO_DOT, RAYS,
};
use crate::face::Eye;
use crate::face_draw::lens_eye;
use crate::geom::{apply_m, cp, cpp, disc_pts, line_path, place, rrect, unit_of, up_dir, Path};
use crate::kit::{blend, clamp, hex, max, min, Reader, P2};
use crate::mood::MoodSpec;
use crate::robot::{Frame, Layer, Part, PartKind, Pose, Robot, Trim};

const TOP: usize = 1064;
const SIDE: usize = 1070;
const BACK: usize = 1112;

/// What stands on top of the head, in the order its gene picks them.
pub const TOP_NAMES: [&str; 11] = [
    "none", "rod", "twin", "zigzag", "coil", "loop", "dish", "bulb", "beacon", "triple", "ears",
];
const TOP_WEIGHTS: [u32; 11] = [8, 10, 9, 5, 5, 4, 5, 6, 3, 4, 5];
const TIP_NAMES: [&str; 4] = ["ball", "fork", "arrow", "bare"];

/// What is bolted to the sides of the head.
pub const SIDE_NAMES: [&str; 7] = ["none", "bolts", "rods", "cones", "vents", "knobs", "fins"];
const SIDE_WEIGHTS: [u32; 7] = [10, 7, 4, 4, 3, 2, 2];

/// What trails behind.
pub const BACK_NAMES: [&str; 3] = ["none", "cable", "pipe"];

/// One mast: where it stands on the head, how it leans and how long it is,
/// which side it belongs to (0 for the middle), what it is, and whether
/// its upper half is bent and which way.
#[derive(Clone, Debug)]
pub(crate) struct Mast {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) lean: f64,
    pub(crate) len: f64,
    pub(crate) side: f64,
    pub(crate) kind: &'static str,
    pub(crate) kink: f64,
}

/// Where a mast stands and points in one mood.
pub(crate) struct MastGeom {
    pub(crate) b: P2,
    pub(crate) cp: P2,
    pub(crate) t: P2,
    pub(crate) u1: P2,
    pub(crate) u2: P2,
    pub(crate) len: f64,
    pub(crate) straight: bool,
    pub(crate) kinked: bool,
}

/// A mast is two halves: perked, both stand straighter and it is a little
/// longer; drooping, the lower half leans and the upper half falls over. A
/// stiff thing on a stem only leans.
pub(crate) fn mast_geom(c: &Robot, m: &Mast, md: &MoodSpec) -> MastGeom {
    let a = md.ant;
    let s = if m.side != 0.0 { m.side } else { c.ant_side };
    let k = m.kind;
    let stiff = matches!(k, "loop" | "dish" | "bulb" | "beacon");
    let mut t1 = m.lean;
    let mut t2 = m.lean;
    let mut len = m.len;
    if a > 0.0 {
        t1 = m.lean * (1.0 - 0.6 * a);
        t2 = t1;
        len *= 1.0 + 0.07 * a;
    } else if a < 0.0 && stiff {
        t1 = m.lean + s * 30.0 * -a;
        t2 = t1;
    } else if a < 0.0 && (k == "zigzag" || k == "coil") {
        t1 = m.lean + s * 64.0 * -a;
        t2 = t1;
    } else if a < 0.0 && k == "stalk" {
        t1 = m.lean + s * 12.0 * -a;
        t2 = m.lean + s * 48.0 * -a;
    } else if a < 0.0 {
        t1 = m.lean + s * 22.0 * -a;
        t2 = m.lean + s * 118.0 * -a;
    }
    t1 += md.ant_tilt;
    t2 += md.ant_tilt + m.kink * 42.0;
    let b = [m.x, m.y];
    let u1 = up_dir(t1);
    let u2 = up_dir(t2);
    let cp = [b[0] + u1[0] * len * 0.5, b[1] + u1[1] * len * 0.5];
    let t = [cp[0] + u2[0] * len * 0.5, cp[1] + u2[1] * len * 0.5];
    MastGeom {
        b,
        cp,
        t,
        u1,
        u2,
        len,
        straight: t1 == t2,
        kinked: m.kink != 0.0,
    }
}

fn tip(out: &mut Out, ctx: &Ctx, c: &Robot, t: P2, u: P2, md: &MoodSpec) {
    let p = &c.p;
    let col = p.status(md.status);
    match c.tip {
        "ball" => {
            lamp(out, ctx, t, c.tip_r, col, &p.metal);
            if md.rays {
                rays(out, ctx, t, c.tip_r, col, &RAYS);
            }
        }
        "fork" => {
            bone(out, ctx, t, place(t, u, -4.6, 7.5), 2.4, &p.metal);
            bone(out, ctx, t, place(t, u, 4.6, 7.5), 2.4, &p.metal);
        }
        "arrow" => out.put(
            El::path(&line_path(
                &[
                    place(t, u, -5.0, -1.5),
                    place(t, u, 0.0, 8.5),
                    place(t, u, 5.0, -1.5),
                ],
                true,
            ))
            .fill(col)
            .stroke(p.metal.line, min(ctx.line_w * 0.7, 2.6))
            .round_join(),
        ),
        _ => {}
    }
}

pub(crate) fn mast_draw(out: &mut Out, ctx: &Ctx, c: &Robot, m: &Mast, md: &MoodSpec) {
    let p = &c.p;
    let g = mast_geom(c, m, md);
    let k = m.kind;
    let met = &p.metal;
    let w = 3.2;
    let col = p.status(md.status);
    if k == "rod" {
        if g.kinked {
            rod(out, ctx, &line_path(&[g.b, g.cp, g.t], false), w, met, true);
        } else if g.straight {
            bone(out, ctx, g.b, g.t, w, met);
        } else {
            hose(out, ctx, g.b, g.cp, g.t, w, met, 0);
        }
        tip(out, ctx, c, g.t, g.u2, md);
        return;
    }
    let u = g.u1;
    match k {
        "zigzag" | "coil" => {
            let coil = k == "coil";
            let n = if coil { 9 } else { 4 };
            let amp = if coil { 4.2 } else { 5.4 };
            let end = place(g.b, u, 0.0, g.len);
            let mut pts = vec![g.b];
            for i in 1..n {
                let across = if i % 2 == 1 { amp } else { -amp };
                pts.push(place(g.b, u, across, g.len * f64::from(i) / f64::from(n)));
            }
            pts.push(end);
            rod(
                out,
                ctx,
                &line_path(&pts, false),
                if coil { 2.6 } else { w },
                met,
                true,
            );
            tip(out, ctx, c, end, u, md);
        }
        "loop" => {
            let e = place(g.b, u, 0.0, g.len * 0.46);
            let r = g.len * 0.27;
            let ctr = place(e, u, 0.0, r);
            bone(out, ctx, g.b, e, w, met);
            if w * ctx.px < 1.7 {
                out.put(El::circle(ctr, r).stroke(
                    blend(met.line, met.base, 0.3),
                    max(w, ctx.thin) + ctx.line_w * 0.5,
                ));
            } else {
                out.put(El::circle(ctr, r).stroke(met.line, w + 1.6 * ctx.line_w));
                out.put(El::circle(ctr, r).stroke(met.base, w));
            }
        }
        "dish" => {
            // The bowl looks up and to one side; perked it looks straight up,
            // drooping it tips over.
            let e = place(g.b, u, 0.0, g.len * 0.5);
            let r = g.len * 0.36;
            let face = up_dir(
                m.lean + c.ant_side * (34.0 * (1.0 - max(0.0, md.ant)) + 44.0 * max(0.0, -md.ant)),
            );
            let a = place(e, face, -r, 0.0);
            let b = place(e, face, r, 0.0);
            let back = place(e, face, 0.0, -r * 1.15);
            let f = place(e, face, 0.0, r * 0.75);
            bone(out, ctx, g.b, e, w, met);
            let mut d = Path::new();
            d.move_to(a);
            d.quad_to(a, back, b);
            d.close();
            out.put(
                El::path(&d)
                    .fill(met.base)
                    .stroke(met.line, ctx.line_w * 0.8)
                    .round_join(),
            );
            bone(out, ctx, place(e, face, 0.0, -r * 0.3), f, 2.0, met);
            lamp(out, ctx, f, 2.6, col, met);
        }
        "bulb" => {
            let r = clamp(g.len * 0.3, 7.5, 11.0);
            let ctr = place(g.b, u, 0.0, 5.0 + r * 0.9);
            out.put(
                El::path(&line_path(
                    &[
                        place(g.b, u, -4.6, -1.0),
                        place(g.b, u, -4.6, 6.5),
                        place(g.b, u, 4.6, 6.5),
                        place(g.b, u, 4.6, -1.0),
                    ],
                    true,
                ))
                .fill(met.base)
                .stroke(met.line, ctx.line_w * 0.8)
                .round_join(),
            );
            out.put(
                El::circle(ctr, r)
                    .class("rb-light")
                    .fill(if md.lit {
                        col
                    } else {
                        blend(hex(0xf1f3f5), col, 0.22)
                    })
                    .stroke(met.line, ctx.line_w * 0.8),
            );
            if ctx.fine {
                out.put(
                    El::path(&line_path(
                        &[
                            place(ctr, u, -r * 0.34, -r * 0.5),
                            place(ctr, u, -r * 0.17, r * 0.12),
                            place(ctr, u, 0.0, -r * 0.3),
                            place(ctr, u, r * 0.17, r * 0.12),
                            place(ctr, u, r * 0.34, -r * 0.5),
                        ],
                        false,
                    ))
                    .stroke(if md.lit { hex(0xffffff) } else { met.dark }, 1.2)
                    .round_join(),
                );
            }
            if md.rays || md.lit {
                rays(out, ctx, ctr, r, col, &RAYS);
            }
        }
        _ => {
            // A beacon: a low dome on a base plate.
            let a = place(g.b, u, -8.5, 3.5);
            let b = place(g.b, u, 8.5, 3.5);
            let mut d = Path::new();
            d.move_to(a);
            d.cubic_to(place(g.b, u, -8.5, 16.0), place(g.b, u, 8.5, 16.0), b);
            d.close();
            out.put(
                El::path(&d)
                    .class("rb-light")
                    .fill(if md.rays {
                        col
                    } else {
                        blend(p.glass, col, 0.7)
                    })
                    .stroke(met.line, ctx.line_w * 0.8)
                    .round_join(),
            );
            out.put(
                El::path(&line_path(
                    &[
                        place(g.b, u, -10.5, -1.0),
                        place(g.b, u, -10.5, 4.0),
                        place(g.b, u, 10.5, 4.0),
                        place(g.b, u, 10.5, -1.0),
                    ],
                    true,
                ))
                .fill(met.base)
                .stroke(met.line, ctx.line_w * 0.8)
                .round_join(),
            );
            if md.rays {
                rays(out, ctx, place(g.b, u, 0.0, 8.0), 8.0, col, &RAYS);
            }
        }
    }
}

pub(crate) fn mast_pts(c: &Robot, m: &Mast, md: &MoodSpec) -> Vec<P2> {
    let g = mast_geom(c, m, md);
    let u = g.u1;
    let with = |p: P2, r: f64| {
        let mut v = vec![g.b];
        v.extend(disc_pts(p[0], p[1], r));
        v
    };
    match m.kind {
        "loop" => with(place(g.b, u, 0.0, g.len * 0.73), g.len * 0.27 + 3.5),
        "dish" => with(place(g.b, u, 0.0, g.len * 0.5), g.len * 0.44 + 3.0),
        "bulb" => {
            let r = clamp(g.len * 0.3, 7.5, 11.0);
            let extra = if md.rays || md.lit { 9.0 } else { 0.0 };
            with(place(g.b, u, 0.0, 5.0 + r * 0.9), r + 2.5 + extra)
        }
        "beacon" => with(
            place(g.b, u, 0.0, 7.0),
            11.5 + if md.rays { 8.0 } else { 0.0 },
        ),
        "zigzag" | "coil" => {
            let mut v = vec![
                g.b,
                place(g.b, u, 6.0, g.len / 2.0),
                place(g.b, u, -6.0, g.len / 2.0),
            ];
            let e = place(g.b, u, 0.0, g.len);
            v.extend(disc_pts(
                e[0],
                e[1],
                c.tip_r + 2.0 + if md.rays { 9.0 } else { 0.0 },
            ));
            v
        }
        _ => {
            let ball = c.tip == "ball";
            let r =
                (if ball { c.tip_r } else { 7.0 }) + 2.0 + if md.rays && ball { 9.0 } else { 0.0 };
            let mut v = vec![g.b, g.cp];
            v.extend(disc_pts(g.t[0], g.t[1], r));
            v
        }
    }
}

pub(crate) fn plug_top(r: &Reader, c: &mut Robot) {
    let o = TOP;
    let hull = &c.head;
    let w = hull.w;
    let h = hull.h;
    let weights: &[u32] = if c.f.screen.is_some() {
        &[0, 12, 11, 6, 6, 5, 6, 7, 3, 4, 4]
    } else {
        &TOP_WEIGHTS
    };
    let mut kind = TOP_NAMES[r.pick(o, 0, weights, 0)];
    // Eyes on stalks take the top of the head: only one mast fits between them.
    if c.f.stalk.is_some() && matches!(kind, "twin" | "triple" | "ears") {
        kind = "rod";
    }
    c.top = kind;
    c.masts = Vec::new();
    if kind == "none" {
        return;
    }
    let l = max(14.0, h * r.lin(o + 1, 0, 3, 0.2, 0.42)) * c.trim_top;
    // Where there are several masts, no two are quite the same length.
    let several = matches!(kind, "twin" | "triple" | "ears");
    let mut count = 0;
    let mut mast = |x: f64, lean: f64, len: f64, side: f64, k: &'static str| {
        let own = if several {
            let v = [0.87, 0.96, 1.04, 1.13][r.raw(o + 3, 2 * count, 2) as usize];
            count += 1;
            v
        } else {
            1.0
        };
        Mast {
            x,
            y: hull.top_at(x) + 2.5,
            lean,
            len: len * own,
            side,
            kind: k,
            kink: 0.0,
        }
    };
    let mut masts = match kind {
        "twin" => {
            let x = w * r.lin(o + 1, 3, 3, 0.3, 0.58);
            let lean = r.lin(o + 1, 6, 2, 6.0, 24.0);
            vec![
                mast(-x, -lean, l, -1.0, "rod"),
                mast(x, lean, l, 1.0, "rod"),
            ]
        }
        "triple" => vec![
            mast(-w * 0.44, -14.0, l * 0.62, -1.0, "rod"),
            mast(0.0, 0.0, l, 0.0, "rod"),
            mast(w * 0.44, 14.0, l * 0.62, 1.0, "rod"),
        ],
        "ears" => {
            let x = w * r.lin(o + 1, 3, 3, 0.7, 0.88);
            let lean = r.lin(o + 1, 6, 2, 36.0, 58.0);
            vec![
                mast(-x, -lean, l * 0.8, -1.0, "rod"),
                mast(x, lean, l * 0.8, 1.0, "rod"),
            ]
        }
        _ => {
            // In the middle, or to one side on a head whose top is flat enough.
            let mut x = 0.0;
            if matches!(kind, "rod" | "zigzag" | "coil") {
                let wh = r.pick(o + 1, 3, &[2, 1, 1], 0);
                if wh != 0
                    && c.f.stalk.is_none()
                    && hull.top_at(w * 0.36) - hull.top_at(0.0) < h * 0.1
                {
                    x = (if wh == 1 { -1.0 } else { 1.0 }) * w * 0.36;
                }
            }
            vec![mast(x, 0.0, l, 0.0, kind)]
        }
    };
    if !matches!(kind, "loop" | "dish" | "bulb" | "beacon") {
        c.tip = TIP_NAMES[r.pick(o + 2, 0, &[5, 1, 1, 1], 0)];
        if c.tip == "ball" {
            c.tip_r = r.lin(o + 2, 3, 2, 4.4, 6.6);
        }
        // One antenna may be bent: the left one, the right one, or none.
        let bent = if kind == "zigzag" || kind == "coil" {
            1
        } else {
            r.raw(o + 2, 5, 2)
        };
        if bent == 0 || bent == 3 {
            let n = masts.len();
            let i = if n > 1 {
                if bent == 0 {
                    0
                } else {
                    n - 1
                }
            } else {
                0
            };
            masts[i].kink = if bent == 0 { -1.0 } else { 1.0 };
        }
    }
    if c.tip_r == 0.0 {
        c.tip_r = 5.0;
    }
    // A mast in the middle falls to one side when it droops: which one.
    if masts.iter().any(|m| m.side == 0.0) {
        c.ant_side = if r.flag(o + 2, 7) { 1.0 } else { -1.0 };
    }
    for (i, m) in masts.iter().enumerate() {
        let class = if m.side < 0.0 {
            "rb-antenna rb-antenna-l"
        } else if m.side > 0.0 {
            "rb-antenna rb-antenna-r"
        } else {
            "rb-antenna rb-antenna-c"
        };
        c.parts.push(Part {
            kind: PartKind::Mast(i),
            class,
            layer: Layer::Top,
            frame: Frame::Head,
            trim: Some(Trim::Top),
        });
    }
    c.masts = std::mem::take(&mut masts);
}

/// The mast a stalk is, on one side.
fn stalk_mast(c: &Robot, side: f64) -> Mast {
    let st = c.f.stalk.as_ref().expect("a stalk part has a stalk");
    Mast {
        x: side * st.bx,
        y: c.head.top_at(side * st.bx) + 2.5,
        lean: side * c.stalk_lean,
        len: st.len * c.trim_top,
        side,
        kind: "stalk",
        kink: 0.0,
    }
}

fn stalk_at(c: &Robot, side: f64, md: &MoodSpec) -> (MastGeom, P2) {
    let st = c.f.stalk.as_ref().expect("a stalk part has a stalk");
    let g = mast_geom(c, &stalk_mast(c, side), md);
    let ctr = place(g.t, g.u2, 0.0, st.r * 0.7);
    (g, ctr)
}

fn pod_eye(c: &Robot, side: f64) -> Eye {
    let st = c.f.stalk.as_ref().expect("a stalk part has a stalk");
    Eye {
        side,
        rx: st.r * st.shape[0],
        ry: st.r * st.shape[1],
        qx: st.r * st.shape[2],
        qy: st.r * st.shape[3],
        pod: true,
        ..Eye::default()
    }
}

/// Lens pods on stalks. Each stalk is a mast that carries an eye, so the
/// eyes rise and sink with the mood.
pub(crate) fn plug_stalks(r: &Reader, c: &mut Robot) {
    c.stalk_lean = r.lin(1048 + 4, 3, 2, 3.0, 15.0);
    for (side, class) in [(-1.0, "rb-stalk rb-stalk-l"), (1.0, "rb-stalk rb-stalk-r")] {
        c.parts.push(Part {
            kind: PartKind::Stalk(side),
            class,
            layer: Layer::Top,
            frame: Frame::Head,
            trim: Some(Trim::Top),
        });
    }
}

pub(crate) fn stalk_draw(out: &mut Out, ctx: &Ctx, c: &Robot, side: f64, md: &MoodSpec) {
    let (g, ctr) = stalk_at(c, side, md);
    if g.straight {
        bone(out, ctx, g.b, g.t, 5.6, &c.p.metal);
    } else {
        hose(out, ctx, g.b, g.cp, g.t, 5.6, &c.p.metal, 0);
    }
    let e = Eye {
        x: ctr[0],
        y: ctr[1],
        ..pod_eye(c, side)
    };
    lens_eye(out, ctx, &c.f, &c.p, &e, &md.eye_for(side), md);
}

pub(crate) fn stalk_pts(c: &Robot, side: f64, md: &MoodSpec) -> Vec<P2> {
    let st = c.f.stalk.as_ref().expect("a stalk part has a stalk");
    let (g, ctr) = stalk_at(c, side, md);
    let mut v = vec![g.b];
    v.extend(disc_pts(ctr[0], ctr[1], st.r * 1.1 + 2.0));
    v
}

/// Pieces bolted to both sides of the head: what they are, where, and how large.
#[derive(Clone, Debug)]
pub(crate) struct Sides {
    kind: &'static str,
    x: f64,
    y: f64,
    k: f64,
    reach: f64,
}

pub(crate) fn plug_sides(r: &Reader, c: &mut Robot) {
    let o = SIDE;
    let weights: &[u32] = if c.f.screen.is_some() {
        &[0, 11, 7, 7, 0, 3, 4]
    } else {
        &SIDE_WEIGHTS
    };
    let kind = SIDE_NAMES[r.pick(o, 0, weights, 0)];
    c.side = kind;
    if kind == "none" {
        return;
    }
    // Above where a pod's arms are mounted, below where the top rounds in.
    let y = -c.head.h * r.lin(o, 5, 2, 0.5, 0.66);
    let x = c.head.hw_at(y);
    let k = r.lin(o + 1, 0, 2, 0.85, 1.25);
    if kind == "vents" {
        c.side_vents = Some([x, y, k]);
        return;
    }
    let reach = match kind {
        "bolts" => 10.5,
        "rods" => 19.0,
        "cones" => 14.0,
        "knobs" => 6.5,
        _ => 13.0,
    } * k;
    c.sides = Some(Sides {
        kind,
        x,
        y,
        k,
        reach,
    });
    for (side, class) in [(-1.0, "rb-ear rb-ear-l"), (1.0, "rb-ear rb-ear-r")] {
        c.parts.push(Part {
            kind: PartKind::Ear(side),
            class,
            layer: Layer::Side,
            frame: Frame::Head,
            trim: None,
        });
    }
}

pub(crate) fn ear_pts(c: &Robot, side: f64) -> Vec<P2> {
    let s = c.sides.as_ref().expect("an ear part has sides");
    vec![
        [side * (s.x - 3.0), s.y - 11.0 * s.k],
        [side * (s.x + s.reach + 2.0), s.y + 11.0 * s.k],
    ]
}

pub(crate) fn ear_draw(out: &mut Out, ctx: &Ctx, c: &Robot, side: f64) {
    let s = c.sides.as_ref().expect("an ear part has sides");
    let p = &c.p;
    let (x, y, k) = (s.x, s.y, s.k);
    let xs = |v: f64| side * v;
    match s.kind {
        "bolts" => {
            plate(
                out,
                ctx,
                &rrect(
                    min(xs(x - 3.0), xs(x + 7.0 * k)),
                    y - 4.2 * k,
                    max(xs(x - 3.0), xs(x + 7.0 * k)),
                    y + 4.2 * k,
                    1.5,
                ),
                &p.metal,
            );
            plate(
                out,
                ctx,
                &rrect(
                    min(xs(x + 6.0 * k), xs(x + 10.5 * k)),
                    y - 6.6 * k,
                    max(xs(x + 6.0 * k), xs(x + 10.5 * k)),
                    y + 6.6 * k,
                    1.8,
                ),
                &p.metal,
            );
        }
        "rods" => {
            bone(
                out,
                ctx,
                [xs(x - 2.0), y],
                [xs(x + 15.0 * k), y - 5.0 * k],
                3.0,
                &p.metal,
            );
            disc(
                out,
                ctx,
                [xs(x + 15.0 * k), y - 5.0 * k],
                4.2 * k,
                &p.accent,
                NO_DOT,
            );
        }
        "cones" => {
            plate(
                out,
                ctx,
                &[
                    cp(xs(x - 2.0), y + 4.0 * k, 1.0),
                    cp(xs(x - 2.0), y - 4.0 * k, 1.0),
                    cp(xs(x + 13.0 * k), y - 10.0 * k, 2.0),
                    cp(xs(x + 13.0 * k), y + 10.0 * k, 2.0),
                ],
                &p.metal,
            );
            if ctx.fine {
                out.put(El::ellipse(xs(x + 13.0 * k), y, 2.4, 8.6 * k).fill(p.glass));
            }
        }
        "knobs" => {
            plate(
                out,
                ctx,
                &rrect(
                    min(xs(x - 2.0), xs(x + 6.0 * k)),
                    y - 7.0 * k,
                    max(xs(x - 2.0), xs(x + 6.0 * k)),
                    y + 7.0 * k,
                    2.0,
                ),
                &p.metal,
            );
            let mut d = Path::new();
            for dy in [-3.5 * k, 0.0, 3.5 * k] {
                d.move_to([xs(x + 1.0), y + dy]);
                d.line_to([xs(x + 5.0 * k), y + dy]);
            }
            seam(out, ctx, &d, p.metal.line, 1.1, None);
        }
        _ => plate(
            out,
            ctx,
            &[
                cp(xs(x - 3.0), y + 9.0 * k, 2.0),
                cp(xs(x - 3.0), y - 11.0 * k, 2.0),
                cp(xs(x + 13.0 * k), y - 3.0 * k, 3.5),
            ],
            &p.paint,
        ),
    }
}

/// A power cable that trails from the lowest module and ends in a plug.
#[derive(Clone, Debug)]
pub(crate) struct Cable {
    side: f64,
    len: f64,
    ax: f64,
    ay: f64,
}

/// An exhaust pipe at one upper corner of the head.
#[derive(Clone, Debug)]
pub(crate) struct Pipe {
    x: f64,
    sd: f64,
    b: P2,
    k: f64,
    end: P2,
}

pub(crate) fn plug_back(r: &Reader, c: &mut Robot) {
    let o = BACK;
    let kind = BACK_NAMES[r.pick(o, 0, &[10, 4, 2], 0)];
    c.back = kind;
    if kind == "none" {
        return;
    }
    let side = if r.flag(o, 4) { 1.0 } else { -1.0 };
    if kind == "cable" {
        let len = r.lin(o, 5, 2, 22.0, 40.0) * c.scale;
        let low = c.low();
        let ay = -low.h * 0.32;
        let ax = side * low.hw_at(ay) * 0.7;
        c.cable = Some(Cable { side, len, ax, ay });
        c.parts.push(Part {
            kind: PartKind::Cable,
            class: "rb-cable",
            layer: Layer::Back,
            frame: Frame::Ground,
            trim: None,
        });
        return;
    }
    // The pipe stands at one upper corner of the head, clear of whatever else stands there.
    let hull = &c.head;
    let mut x = side * hull.w * 0.6;
    let near = |v: f64| {
        c.masts.iter().any(|m| (m.x - v).abs() < 17.0)
            || c.f
                .stalk
                .as_ref()
                .is_some_and(|st| (v.abs() - st.bx).abs() < st.r + 9.0)
    };
    if near(x) {
        x = -x;
    }
    if near(x) {
        c.back = "none";
        return;
    }
    let sd = if x < 0.0 { -1.0 } else { 1.0 };
    let b = [x, hull.top_at(x) + 3.0];
    let k = c.scale;
    let end = [x + sd * 9.0 * k, b[1] - 16.0 * k];
    c.pipe = Some(Pipe { x, sd, b, k, end });
    c.parts.push(Part {
        kind: PartKind::Pipe,
        class: "rb-pipe",
        layer: Layer::Top,
        frame: Frame::Head,
        trim: None,
    });
}

struct CableGeom {
    a: P2,
    ctl: P2,
    tip: P2,
    u: P2,
}

/// The cable swings up when the robot is pleased and lies on the ground when it is not.
fn cable_geom(c: &Robot, md: &MoodSpec, pose: &Pose) -> CableGeom {
    let cb = c.cable.as_ref().expect("a cable part has a cable");
    let (side, len) = (cb.side, cb.len);
    let a = apply_m(pose.u, [cb.ax, cb.ay]);
    let t = md.tail;
    let up = max(0.0, t);
    let down = max(0.0, -t);
    let tip = [
        a[0] + side * len * (1.0 - 0.15 * up - 0.3 * down),
        min(-3.5, a[1] + len * (0.35 - 1.05 * up) + down * 40.0),
    ];
    let ctl = [
        a[0] + side * len * (0.55 + 0.2 * up),
        min(-2.0, a[1] + len * (0.5 - 0.35 * up) + down * 30.0),
    ];
    CableGeom {
        a,
        ctl,
        tip,
        u: unit_of(ctl, tip),
    }
}

pub(crate) fn cable_pts(c: &Robot, md: &MoodSpec, pose: &Pose) -> Vec<P2> {
    let g = cable_geom(c, md, pose);
    let mut v = vec![g.a, g.ctl];
    v.extend(disc_pts(
        g.tip[0] + g.u[0] * 6.0,
        g.tip[1] + g.u[1] * 6.0,
        9.0,
    ));
    v
}

pub(crate) fn cable_draw(out: &mut Out, ctx: &Ctx, c: &Robot, md: &MoodSpec, pose: &Pose) {
    let g = cable_geom(c, md, pose);
    let p = |ac: f64, al: f64| place(g.tip, g.u, ac, al);
    let pal = &c.p;
    hose(out, ctx, g.a, g.ctl, g.tip, 3.4, &pal.rubber, 0);
    let mut prongs = line_path(&[p(-2.2, 6.0), p(-2.2, 11.5)], false);
    prongs.append(line_path(&[p(2.2, 6.0), p(2.2, 11.5)], false));
    rod(out, ctx, &prongs, 1.8, &pal.metal, true);
    plate_k(
        out,
        ctx,
        &[
            cpp(p(-4.4, -1.0), 1.2),
            cpp(p(-4.4, 7.0), 1.2),
            cpp(p(4.4, 7.0), 1.2),
            cpp(p(4.4, -1.0), 1.2),
        ],
        &pal.accent,
        0.8,
    );
}

pub(crate) fn pipe_pts(c: &Robot, md: &MoodSpec) -> Vec<P2> {
    let pp = c.pipe.as_ref().expect("a pipe part has a pipe");
    let mut v = vec![pp.b, pp.end];
    if md.puff != 0.0 {
        v.extend(disc_pts(
            pp.end[0] + pp.sd * 8.0,
            pp.end[1] - 14.0,
            12.0 + md.puff * 4.0,
        ));
    } else {
        v.extend(disc_pts(pp.end[0], pp.end[1], 6.0));
    }
    v
}

pub(crate) fn pipe_draw(out: &mut Out, ctx: &Ctx, c: &Robot, md: &MoodSpec) {
    let pp = c.pipe.as_ref().expect("a pipe part has a pipe");
    let (x, b, k, end, sd) = (pp.x, pp.b, pp.k, pp.end, pp.sd);
    let u = unit_of([x, b[1] - 9.0 * k], end);
    rod(
        out,
        ctx,
        &line_path(&[b, [x, b[1] - 9.0 * k], end], false),
        6.6 * k,
        &c.p.metal,
        true,
    );
    rod(
        out,
        ctx,
        &line_path(
            &[place(end, u, -5.0 * k, 0.0), place(end, u, 5.0 * k, 0.0)],
            false,
        ),
        2.6 * k,
        &c.p.metal,
        true,
    );
    if md.puff != 0.0 && ctx.size >= 48.0 {
        puff(
            out,
            c,
            end[0] + sd * 6.0,
            end[1] - 10.0,
            0.75 + 0.2 * md.puff,
            sd,
        );
        if md.puff > 1.0 {
            puff(out, c, end[0] + sd * 15.0, end[1] - 22.0, 0.7, sd);
        }
    }
}

/// A puff of steam: three discs that share one outline.
pub(crate) fn puff(out: &mut Out, c: &Robot, x: f64, y: f64, k: f64, sd: f64) {
    let p = &c.p;
    let discs = [
        [x, y, 5.0 * k],
        [x + sd * 5.5 * k, y - 3.0 * k, 6.2 * k],
        [x + sd * 11.0 * k, y, 4.6 * k],
    ];
    out.group("rb-steam", &[], |out| {
        for q in &discs {
            out.put(El::circle([q[0], q[1]], q[2]).fill(p.steam_edge));
        }
        for q in &discs {
            out.put(El::circle([q[0], q[1]], q[2] - 1.3).fill(p.steam));
        }
    });
}
