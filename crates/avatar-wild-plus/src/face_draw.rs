//! Drawing the face in one mood: markings, eyes, lids, brows, cheeks, mouth,
//! nose, whiskers and a tear.

use avatar_svg::wild::math::{cos, sin};
use avatar_svg::{Mood, Primitive, Rgb, StrokeCap, StrokeJoin};
use std::f64::consts::PI;

use crate::creature::Built;
use crate::draw::{
    circle, ellipse, filled, lined, local_shape, n2, path_of, plain_stroke, prim, q1, q2,
    round_stroke, solid, stroke, Cmd, PathB,
};
use crate::face::{FaceEye, EYE_OUTER};
use crate::geom::{clamp, dist, ellipse_poly, max, min, span_through, unit, P};
use crate::mood::{spec, MoodSpec};
use crate::palette::{hsl, Palette};

/// The face for one mood, in layers.
#[derive(Default)]
pub struct FaceOut {
    /// On the skin, inside the body's clip.
    pub skin: Vec<Primitive>,
    pub eyes: Vec<Primitive>,
    pub lids: Vec<Primitive>,
    pub brows: Vec<Primitive>,
    pub nose: Vec<Primitive>,
    pub mouth: Vec<Primitive>,
    /// Unclipped, on top of everything.
    pub extra: Vec<Primitive>,
    /// For eyes on stalks: what each stalk part carries.
    pub stalk: Vec<(String, Vec<Primitive>)>,
}

/// Brow shapes by kind, in eye radii: how far the inner and outer ends sit
/// below the brow line, and how far the whole brow is lifted.
fn brows(kind: usize) -> [f64; 3] {
    match kind {
        1 => [0.1, 0.0, 0.0],
        2 => [-0.15, -0.05, -0.5],
        3 => [0.55, -0.15, 0.1],
        _ => [-0.55, 0.15, 0.0],
    }
}

/// Cubic commands tracing the unit circle through the given unit vectors,
/// each step shorter than a half turn, from square roots only.
fn arc_through(us: &[P]) -> Vec<Cmd> {
    let mut out = Vec::new();
    for i in 0..us.len() - 1 {
        let (a, b) = (us[i], us[i + 1]);
        let c = clamp(a[0] * b[0] + a[1] * b[1], -1.0, 1.0);
        let ch = ((1.0 + c) / 2.0).sqrt();
        let sh = ((1.0 - c) / 2.0).sqrt();
        let k = 4.0 / 3.0 * sh / (1.0 + ch);
        let turn = if (-a[1]) * (b[0] - a[0]) + a[0] * (b[1] - a[1]) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        out.push(Cmd::C(
            a[0] - a[1] * k * turn,
            a[1] + a[0] * k * turn,
            b[0] + b[1] * k * turn,
            b[1] - b[0] * k * turn,
            b[0],
            b[1],
        ));
    }
    out
}

/// The part of the unit disc an upper eyelid covers, and its edge.
fn lid_shape(lid: f64, deg: f64) -> (Vec<Cmd>, Vec<Cmd>) {
    let c = clamp(-1.0 + 2.0 * lid, -0.97, 0.97);
    let s = (1.0 - c * c).sqrt();
    let a = deg * PI / 180.0;
    let n = [-sin(a), cos(a)];
    let t = [cos(a), sin(a)];
    let pa = [c * n[0] - s * t[0], c * n[1] - s * t[1]];
    let pb = [c * n[0] + s * t[0], c * n[1] + s * t[1]];
    let top = [-n[0], -n[1]];
    let ma = unit(pa[0] + top[0], pa[1] + top[1]);
    let mb = unit(top[0] + pb[0], top[1] + pb[1]);
    let mut fill = vec![Cmd::M(pa[0], pa[1])];
    fill.extend(arc_through(&[pa, ma, top, mb, pb]));
    fill.push(Cmd::Z);
    let edge = vec![
        Cmd::M(c * n[0] - s * t[0] * 1.12, c * n[1] - s * t[1] * 1.12),
        Cmd::L(c * n[0] + s * t[0] * 1.12, c * n[1] + s * t[1] * 1.12),
    ];
    (fill, edge)
}

/// The part of the unit disc a squint covers, and its edge.
fn squint_shape(sq: f64) -> (Vec<Cmd>, Vec<Cmd>) {
    let c = clamp(1.0 - 2.0 * sq, -0.9, 0.95);
    let s = (1.0 - c * c).sqrt();
    let pa = [-s, c];
    let pb = [s, c];
    let bottom = [0.0, 1.0];
    let ma = unit(pa[0], pa[1] + 1.0);
    let mb = unit(pb[0], pb[1] + 1.0);
    let mut fill = vec![Cmd::M(pa[0], pa[1]), Cmd::Q(0.0, c - 0.55, pb[0], pb[1])];
    fill.extend(arc_through(&[pb, mb, bottom, ma, pa]));
    fill.push(Cmd::Z);
    let edge = vec![
        Cmd::M(pa[0] * 1.08, pa[1]),
        Cmd::Q(0.0, c - 0.55, pb[0] * 1.08, pb[1]),
    ];
    (fill, edge)
}

fn luma(c: Rgb) -> f64 {
    (0.2126 * f64::from(c.r) + 0.7152 * f64::from(c.g) + 0.0722 * f64::from(c.b)) / 255.0
}

const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};

/// A stroke-only path with round caps and joins.
fn stroked(
    shape: avatar_svg::Shape,
    color: Rgb,
    w: f64,
    label: &str,
    op: Option<f64>,
) -> Primitive {
    lined(shape, round_stroke(color, q2(w)), op, label)
}

/// A brow over a main eye in a mood.
struct Brow {
    resting: bool,
    on_stalk: bool,
    pts: Vec<P>,
    w: f64,
    level: f64,
    over: P,
}

pub fn face_prims(c: &Built, mood: Mood) -> FaceOut {
    let f = &c.face;
    let p = &f.p;
    let pal: &Palette = &c.p;
    let w = &c.w;
    let sp: MoodSpec = spec(mood);
    let r = p.r;
    let fx = p.fx;
    let style = f.style;
    let outer = EYE_OUTER[style];
    let mut out = FaceOut::default();
    let asp = w.eye_aspect;
    let mut sx = (if asp >= 1.0 { 1.0 / asp } else { 1.0 }) * sp.es;
    let mut sy = (if asp >= 1.0 { 1.0 } else { asp }) * sp.es;
    if !((sx - 1.0).abs() > 0.005 || (sy - 1.0).abs() > 0.005) {
        sx = 1.0;
        sy = 1.0;
    }
    let mains: Vec<&FaceEye> = f.eyes.iter().filter(|e| e.main).collect();

    // ---- markings under the eyes (they also set the eyelid colour) ----
    let skin_of = |e: &FaceEye| -> &'static str {
        if !e.main {
            return "body";
        }
        if f.marking == 1 {
            f.mark_tone
        } else {
            "body"
        }
    };
    let big_r = |e: &FaceEye| e.r * max(outer[1], outer[2]);
    if f.marking == 1 {
        for e in &mains {
            let s = if e.side == 0.0 { 1.0 } else { e.side };
            let r1 = big_r(e) * f.mark_size;
            out.skin.push(filled(
                path_of(
                    &ellipse_poly(
                        e.x + s * r1 * 0.12,
                        e.y + r1 * 0.14,
                        r1 * 0.95,
                        r1 * 1.15,
                        unit(1.0, -s * 0.5),
                        24,
                    ),
                    true,
                ),
                pal.ink(f.mark_tone),
                None,
                "wp-eye-patch",
            ));
        }
    } else if f.marking == 4 {
        for e in &mains {
            let s = if e.side == 0.0 { 1.0 } else { e.side };
            let y0 = e.y + big_r(e) * 0.8;
            let x0 = e.x - s * big_r(e) * 0.45;
            let mut d = PathB::new();
            d.m2(x0, y0);
            d.quad_to(
                x0,
                y0,
                x0 - s * r * 0.1,
                (y0 + p.mouth_y) / 2.0,
                fx + s * r * 0.95,
                p.mouth_y + r * 0.35,
            );
            out.skin.push(stroked(
                d.shape(),
                pal.mark_deep,
                r * 0.26,
                "wp-eye-streak",
                Some(0.8),
            ));
        }
    }

    // ---- eyes ----
    let closed = sp.lid >= 0.95;
    let pk = f.pupil_k;
    let brow_of = |e: &FaceEye, on_rim: bool| -> Option<Brow> {
        let on_stalk = e.part.is_some();
        let bk = if closed {
            0
        } else if sp.brows != 0 {
            sp.brows
        } else if on_stalk {
            0
        } else {
            w.brow as usize
        };
        if !e.main || bk == 0 {
            return None;
        }
        let cx = if on_stalk { e.x } else { fx + (e.x - fx) * sx };
        let cy = if on_stalk {
            e.y
        } else {
            p.eye_y + (e.y - p.eye_y) * sy
        };
        let ux = e.r * sx;
        let uy = e.r * sy;
        let resting = sp.brows == 0;
        let cfg = brows(bk);
        let bw = (if resting {
            0.8
        } else {
            0.95 * max(1.0, outer[1])
        }) * ux;
        let brim = cy - (outer[2] - outer[0]) * uy;
        let top = if on_stalk {
            brim + 0.12 * e.r - cfg[2] * r
        } else if on_rim {
            brim + 0.1 * r - cfg[2] * r
        } else {
            brim - 0.7 * r * sy
        };
        let yi = top + (cfg[0] + cfg[2] + f.brow_tilt) * r;
        let yo = top + (cfg[1] + cfg[2] - f.brow_tilt) * r;
        let pts = if e.side == 0.0 {
            vec![[cx - bw, yo], [cx, yi], [cx + bw, yo]]
        } else {
            vec![[cx - e.side * bw, yi], [cx + e.side * bw, yo]]
        };
        Some(Brow {
            resting,
            on_stalk,
            pts,
            w: bw,
            level: (yi + yo) / 2.0,
            over: [cx, top + cfg[2] * r],
        })
    };
    // A brow that would land outside the head has no room above its eye.
    let room_over = |e: &FaceEye| match brow_of(e, false) {
        None => true,
        Some(b) => b.on_stalk || f.inside(b.over[0], b.over[1]) >= 2.0,
    };
    // On a face with extra eyes no brow may pass an extra eye closer than
    // half an eye radius, nor touch the head's outline.
    let extras: Vec<&FaceEye> = f
        .eyes
        .iter()
        .filter(|e| !e.main && e.part.is_none())
        .collect();
    let crowded = !extras.is_empty()
        && mains.iter().any(|e| {
            let Some(b) = brow_of(e, false) else {
                return false;
            };
            if b.on_stalk {
                return false;
            }
            let touch = f.brow_w / 2.0 + 1.0;
            let (a, z) = (b.pts[0], b.pts[b.pts.len() - 1]);
            if f.inside(a[0], a[1]) < touch || f.inside(z[0], z[1]) < touch {
                return true;
            }
            extras.iter().any(|x| {
                let mut near = f64::INFINITY;
                for i in 0..b.pts.len() - 1 {
                    let (u, v) = (b.pts[i], b.pts[i + 1]);
                    let (dx, dy) = (v[0] - u[0], v[1] - u[1]);
                    let t = clamp(
                        ((x.x - u[0]) * dx + (x.y - u[1]) * dy) / (dx * dx + dy * dy),
                        0.0,
                        1.0,
                    );
                    near = min(near, dist(x.x, x.y, u[0] + dx * t, u[1] + dy * t));
                }
                near < big_r(x) + 0.5 * r
            })
        });
    for e in &f.eyes {
        let on_stalk = e.part.is_some();
        let cx = if on_stalk { e.x } else { fx + (e.x - fx) * sx };
        let cy = if on_stalk {
            e.y
        } else {
            p.eye_y + (e.y - p.eye_y) * sy
        };
        let ux = e.r * sx;
        let uy = e.r * sy;
        let side = e.side;
        let xf = |v: f64| cx + v * ux;
        let yf = |v: f64| cy + v * uy;
        let el = |dx: f64, dy: f64, a: f64, b: f64| {
            ellipse(q2(xf(dx)), q2(yf(dy)), q2(a * ux), q2(b * uy))
        };
        let skin = pal.ink(skin_of(e));
        let dark_skin = luma(skin) < 0.36;
        // The rim of a white eye is thin and soft.
        let rim = if pal.dark { pal.outline } else { pal.tone_rim };
        let derp = e.derp.unwrap_or([0.0, 0.0]);
        let g = [
            clamp(sp.gaze[0] * c.temper.side + derp[0], -1.0, 1.0),
            clamp(sp.gaze[1] + derp[1], -1.0, 1.0),
        ];
        let mut eye: Vec<Primitive> = Vec::new();
        let mut lid: Vec<Primitive> = Vec::new();
        let mut brow: Vec<Primitive> = Vec::new();
        let shut_line = || {
            stroked(
                local_shape(
                    &[Cmd::M(-1.0, -0.1), Cmd::Q(0.0, 0.75, 1.0, -0.1)],
                    &xf,
                    &yf,
                    None,
                ),
                if dark_skin { pal.white } else { pal.pupil },
                3.0,
                "wp-lid-line",
                None,
            )
        };
        let sclera = |a: f64, b: f64| {
            prim(
                el(0.0, 0.0, a, b),
                solid(pal.white),
                Some(plain_stroke(rim, 120)),
                None,
                "wp-sclera",
            )
        };
        let shine = |dx: f64, dy: f64, rr: f64, op: f64| {
            filled(el(dx, dy, rr, rr), WHITE, Some(op), "wp-shine")
        };
        if style == 2 {
            // Arc eyes have no lids: each mood draws its own stroke.
            let pen = if dark_skin { pal.white } else { pal.ink };
            let inner = if side == 0.0 { 0.0 } else { -side };
            let cmds = match mood {
                Mood::Happy => vec![Cmd::M(-1.05, 0.3), Cmd::Q(0.0, -1.25, 1.05, 0.3)],
                Mood::Working => vec![Cmd::M(-1.0, 0.05), Cmd::Q(0.0, -0.45, 1.0, 0.05)],
                Mood::Blocked => vec![
                    Cmd::M(-1.0, -0.05 - 0.3 * inner),
                    Cmd::L(1.0, -0.05 + 0.3 * inner),
                ],
                Mood::Sad => vec![
                    Cmd::M(-1.0, 0.05 + 0.3 * inner),
                    Cmd::L(1.0, 0.05 - 0.3 * inner),
                ],
                Mood::Sleepy => vec![Cmd::M(-1.0, -0.1), Cmd::Q(0.0, 0.75, 1.0, -0.1)],
                _ => vec![Cmd::M(-1.0, 0.2), Cmd::Q(0.0, -0.9, 1.0, 0.2)],
            };
            if mood == Mood::Waiting {
                eye.push(filled(
                    el(0.0, 0.0, 0.66, 0.66),
                    if dark_skin { pal.white } else { pal.pupil },
                    None,
                    "wp-pupil",
                ));
                eye.push(filled(
                    el(0.2, -0.22, 0.2, 0.2),
                    if dark_skin { pal.pupil } else { WHITE },
                    Some(0.9),
                    "wp-shine",
                ));
            } else {
                eye.push(stroked(
                    local_shape(&cmds, &xf, &yf, None),
                    pen,
                    3.4,
                    "wp-eye-arc",
                    None,
                ));
            }
        } else {
            // A dark eye gets a white backing on dark skin.
            let backing =
                (pal.sclera.is_some() || dark_skin) && matches!(style, 0 | 3 | 10 | 11 | 12);
            let k = if style == 11 { 0.5 } else { 1.0 };
            if backing {
                eye.push(filled(
                    el(0.0, 0.0, 1.2 * k + 0.02, 1.2 * k + 0.02),
                    pal.white,
                    None,
                    "wp-sclera",
                ));
            }
            match style {
                0 | 3 | 11 | 12 => {
                    eye.push(filled(el(0.0, 0.0, k, k), pal.pupil, None, "wp-pupil"));
                    if style != 11 {
                        eye.push(shine(0.32, -0.35, 0.3, 0.9));
                    }
                    if style == 12 {
                        let o = if side == 0.0 { 1.0 } else { side };
                        for (lx, ly) in [(0.5, -0.87), (0.8, -0.6), (0.97, -0.26)] {
                            eye.push(stroked(
                                local_shape(
                                    &[Cmd::M(o * lx, ly), Cmd::L(o * lx * 1.5, ly * 1.5)],
                                    &xf,
                                    &yf,
                                    None,
                                ),
                                if dark_skin { pal.white } else { pal.pupil },
                                1.6,
                                "wp-lash",
                                None,
                            ));
                        }
                    }
                }
                1 => {
                    eye.push(sclera(1.1, 1.25));
                    eye.push(filled(
                        el(g[0] * 0.25, 0.1 + g[1] * 0.3, 0.78 * pk, 0.78 * pk),
                        pal.pupil,
                        None,
                        "wp-pupil",
                    ));
                    eye.push(shine(0.28 + g[0] * 0.25, -0.2 + g[1] * 0.3, 0.22, 0.9));
                    let m = 0.5_f64.sqrt();
                    let mut cmds = vec![Cmd::M(-1.1, -0.14)];
                    for cmd in
                        arc_through(&[[-1.0, 0.0], [-m, -m], [0.0, -1.0], [m, -m], [1.0, 0.0]])
                    {
                        if let Cmd::C(a1, b1, a2, b2, a, b) = cmd {
                            cmds.push(Cmd::C(
                                a1 * 1.1,
                                b1 * 1.25 - 0.14,
                                a2 * 1.1,
                                b2 * 1.25 - 0.14,
                                a * 1.1,
                                b * 1.25 - 0.14,
                            ));
                        }
                    }
                    eye.push(stroked(
                        local_shape(&cmds, &xf, &yf, None),
                        pal.outline,
                        1.6,
                        "wp-crease",
                        Some(0.5),
                    ));
                }
                4 => {
                    eye.push(sclera(1.1, 1.1));
                    eye.push(filled(
                        el(g[0] * 0.25, 0.2 + g[1] * 0.2, 0.62 * pk, 0.62 * pk),
                        pal.pupil,
                        None,
                        "wp-pupil",
                    ));
                    eye.push(shine(0.2 + g[0] * 0.25, 0.02 + g[1] * 0.2, 0.18, 0.9));
                }
                5 => {
                    eye.push(sclera(1.2, 1.2));
                    eye.push(filled(
                        el(g[0] * 0.62, g[1] * 0.62, 0.48 * pk, 0.48 * pk),
                        pal.pupil,
                        None,
                        "wp-pupil",
                    ));
                }
                6 => {
                    eye.push(sclera(1.15, 1.15));
                    eye.push(filled(
                        el(g[0] * 0.3, g[1] * 0.3, 0.8, 0.8),
                        pal.ink(f.iris),
                        None,
                        "wp-iris",
                    ));
                    eye.push(filled(
                        el(g[0] * 0.3, g[1] * 0.3, 0.42 * pk, 0.42 * pk),
                        pal.pupil,
                        None,
                        "wp-pupil",
                    ));
                    eye.push(shine(0.3 + g[0] * 0.3, -0.32 + g[1] * 0.3, 0.22, 0.9));
                }
                7 | 8 => {
                    // A white eye, a light iris inside it, and the slit or
                    // bar inside that.
                    eye.push(sclera(1.1, 1.1));
                    eye.push(filled(
                        el(g[0] * 0.25, g[1] * 0.25, 0.74, 0.74),
                        pal.ink(f.iris),
                        None,
                        "wp-iris",
                    ));
                    let pupil = if style == 7 {
                        el(g[0] * 0.25, g[1] * 0.25, 0.2 * pk, 0.66)
                    } else {
                        el(g[0] * 0.25, g[1] * 0.25, 0.62, 0.22 * pk)
                    };
                    eye.push(filled(pupil, pal.pupil, None, "wp-pupil"));
                    eye.push(shine(0.3 + g[0] * 0.25, -0.34 + g[1] * 0.25, 0.17, 0.85));
                }
                9 => {
                    // An eye all of iris colour inside a thin rim; an iris
                    // too close to the skin's lightness is white instead.
                    let iris = pal.ink(f.iris);
                    let fill = if (luma(iris) - luma(skin)).abs() < 0.2 {
                        pal.white
                    } else {
                        iris
                    };
                    eye.push(prim(
                        el(0.0, 0.0, 1.0, 1.0),
                        solid(fill),
                        Some(plain_stroke(pal.outline, 160)),
                        None,
                        "wp-iris",
                    ));
                    eye.push(filled(
                        el(g[0] * 0.2, g[1] * 0.2, 0.4 * pk, 0.4 * pk),
                        pal.pupil,
                        None,
                        "wp-pupil",
                    ));
                }
                10 => {
                    eye.push(filled(
                        el(0.0, 0.0, 1.12, 1.12),
                        pal.pupil,
                        None,
                        "wp-pupil",
                    ));
                    eye.push(filled(
                        el(0.0, 0.5, 0.72, 0.42),
                        pal.ink(f.iris),
                        Some(0.85),
                        "wp-iris",
                    ));
                    eye.push(shine(0.36, -0.4, 0.34, 0.95));
                    eye.push(shine(-0.36, 0.18, 0.15, 0.9));
                }
                _ => {}
            }
            // Lids follow the style's outer ellipse, a little oversized.
            let oy = outer[0];
            let oa = outer[1] * (if backing { 1.22 } else { 1.0 }) * 1.06;
            let ob = outer[2] * (if backing { 1.22 } else { 1.0 }) * 1.06;
            let lx = |v: f64| cx + v * oa * ux;
            let ly = |v: f64| cy + (oy + v * ob) * uy;
            let ball = style == 1 || (4..=8).contains(&style);
            let lid_tone = skin;
            // The half-lid eye is never more than six tenths open.
            let resting = if style == 4 { 0.4 } else { 0.0 };
            let lid_now = max(resting, sp.lid);
            if closed {
                eye.clear();
                if ball {
                    eye.push(prim(
                        ellipse(q2(cx), q2(yf(oy)), q2(outer[1] * ux), q2(outer[2] * uy)),
                        solid(lid_tone),
                        Some(plain_stroke(rim, 120)),
                        None,
                        "wp-lid",
                    ));
                }
                lid.push(shut_line());
            } else {
                if lid_now > 0.01 {
                    let (fill, edge) = lid_shape(lid_now, sp.ang * side);
                    lid.push(filled(
                        local_shape(&fill, &lx, &ly, None),
                        lid_tone,
                        None,
                        "wp-lid",
                    ));
                    lid.push(stroked(
                        local_shape(&edge, &lx, &ly, None),
                        if style == 4 { pal.pupil } else { pal.outline },
                        if style == 4 { 2.6 } else { 2.2 },
                        "wp-lid-line",
                        None,
                    ));
                }
                if sp.sq > 0.01 && style != 4 {
                    let (fill, edge) = squint_shape(sp.sq);
                    lid.push(filled(
                        local_shape(&fill, &lx, &ly, None),
                        lid_tone,
                        None,
                        "wp-squint",
                    ));
                    lid.push(stroked(
                        local_shape(&edge, &lx, &ly, None),
                        pal.outline,
                        2.2,
                        "wp-lid-line",
                        None,
                    ));
                }
            }
        }
        // Brows over the main eyes: the mood's, or the creature's resting
        // brow. A brow without room lies on its eye's rim in a mood and is
        // left out at rest.
        let brow_room = room_over(e) && !crowded;
        if let Some(b) = brow_of(e, !brow_room) {
            let pen = if dark_skin { pal.white } else { pal.ink };
            let weight = if b.resting {
                min(f.brow_w, 3.3)
            } else {
                f.brow_w
            };
            let across = if b.resting && side == 0.0 {
                span_through(&c.head, b.level, cx)
            } else {
                None
            };
            let hidden = b.resting
                && (!brow_room || across.is_some_and(|a| 2.0 * b.w > 0.8 * (a[1] - a[0])));
            if !hidden {
                let mut d = PathB::new();
                for (i, q) in b.pts.iter().enumerate() {
                    if i == 0 {
                        d.m2(q[0], q[1]);
                    } else {
                        d.l2(q[0], q[1]);
                    }
                }
                brow.push(stroked(d.shape(), pen, weight, "wp-brow", None));
            }
        }
        if let Some(name) = &e.part {
            let mut all = eye;
            all.extend(lid);
            all.extend(brow);
            out.stalk.push((name.clone(), all));
        } else {
            out.eyes.extend(eye);
            out.lids.extend(lid);
            out.brows.extend(brow);
        }
    }

    // ---- cheeks ----
    let cheek_x = if f.eye_plan == 1 {
        p.big + r * 0.9
    } else {
        p.ipd * 1.28
    };
    let cheek_y = if f.eye_plan == 1 {
        p.eye_y + p.big * 0.75
    } else {
        p.eye_y + r * 1.3
    };
    let glow = if mood == Mood::Happy { 1.2 } else { 1.0 };
    for s in [-1.0, 1.0] {
        let bx = fx + s * cheek_x;
        if f.blush == 0 {
            out.skin.push(filled(
                ellipse(q1(bx), q1(cheek_y), q2(r * 0.82 * glow), q2(r * 0.5 * glow)),
                pal.blush,
                Some(n2(min(1.0, pal.blush_alpha * glow))),
                "wp-blush",
            ));
        } else if f.blush == 2 {
            for i in -1..=1 {
                let i = f64::from(i);
                let mut d = PathB::new();
                d.m2(bx + i * r * 0.36 - r * 0.12, cheek_y + r * 0.3);
                d.l2(bx + i * r * 0.36 + r * 0.12, cheek_y - r * 0.3);
                out.skin
                    .push(stroked(d.shape(), pal.blush, 1.5, "wp-blush", None));
            }
        }
        for fr in &f.freckles {
            out.skin.push(filled(
                circle(
                    q2(bx + s * fr[0] * r * 0.9),
                    q2(cheek_y + fr[1] * r),
                    q2(max(0.7, r * 0.09)),
                ),
                if pal.dark { pal.light } else { pal.outline },
                Some(0.6),
                "wp-freckle",
            ));
        }
    }

    // ---- mouth ----
    if p.mouth_fits {
        mouth(c, &sp, &mut out);
    }

    // ---- whiskers reach past the cheeks, over everything ----
    if f.whiskers != 0 {
        let pen = if pal.dark { pal.ink } else { pal.outline };
        for s in [-1.0, 1.0] {
            for i in 0..f.whiskers {
                let v = f64::from(i) - (f64::from(f.whiskers) - 1.0) / 2.0;
                let x0 = fx + s * max(p.ipd * 0.85, r * 0.95);
                let y0 = p.mouth_y - r * 0.35 + v * r * 0.3;
                let mut d = PathB::new();
                d.m2(x0, y0);
                d.l2(
                    x0 + s * r * f.whisker_len,
                    y0 + r * f.whisker_len * (v * 0.22 + f.whisker_droop),
                );
                out.extra
                    .push(stroked(d.shape(), pen, 1.1, "wp-whisker", Some(0.75)));
            }
        }
    }
    // A tear under one eye when sad.
    if sp.adj == "tear" && !mains.is_empty() {
        let e = mains[mains.len() - 1];
        let x = (if e.part.is_some() {
            e.x
        } else {
            fx + (e.x - fx) * sx
        }) + e.r * 0.2;
        let y0 = e.y + big_r(e) + r * 0.2;
        let h = r * 0.95;
        let tw = r * 0.36;
        let mut d = PathB::new();
        d.m2(x, y0);
        d.quad_to(x, y0, x + tw, y0 + h * 0.8, x, y0 + h);
        d.quad_to(x, y0 + h, x - tw, y0 + h * 0.8, x, y0);
        d.z();
        let tear = filled(
            d.shape(),
            crate::palette::hex(0x7fb8e8),
            Some(0.9),
            "wp-tear",
        );
        match &e.part {
            Some(name) => {
                if let Some(entry) = out.stalk.iter_mut().find(|(n, _)| n == name) {
                    entry.1.push(tear);
                }
            }
            None => out.extra.push(tear),
        }
    }
    out
}

/// The mouth, the muzzle or whisker pads under it, the nose and a mole.
fn mouth(c: &Built, sp: &MoodSpec, out: &mut FaceOut) {
    let f = &c.face;
    let p = &f.p;
    let pal = &c.p;
    let w = &c.w;
    let r = p.r;
    let fx = p.fx;
    let (mw, md, plan) = (f.mw, f.md, f.mouth_plan);
    let mx = |v: f64| fx + v * r * mw;
    let my = |v: f64| p.mouth_y + r * (0.94 + (v - 0.94) * md);
    let pen = pal.mouth;
    let kind = sp.mouth.unwrap_or(w.mouth);
    // A beak is rigid; every other mouth carries the creature's smirk.
    let smirk = if plan == 2 { None } else { Some((fx, f.smirk)) };
    let line = |cmds: &[Cmd], wd: f64| {
        stroked(
            local_shape(cmds, &mx, &my, smirk),
            pen,
            wd,
            "wp-mouth-line",
            None,
        )
    };
    let shape = |cmds: &[Cmd]| local_shape(cmds, &mx, &my, smirk);
    let poly = |pts: &[P]| -> avatar_svg::Shape {
        let mapped: Vec<P> = pts
            .iter()
            .map(|q| {
                [
                    mx(q[0]),
                    my(q[1])
                        + (if smirk.is_some() {
                            f.smirk * (mx(q[0]) - fx)
                        } else {
                            0.0
                        }),
                ]
            })
            .collect();
        path_of(&mapped, true)
    };
    let tooth = |pts: &[P], cls: &str| {
        prim(
            poly(pts),
            solid(WHITE),
            Some(stroke(pen, 110, StrokeCap::Butt, StrokeJoin::Round)),
            None,
            cls,
        )
    };
    let tongue = |cmds: &[Cmd]| {
        prim(
            shape(cmds),
            solid(pal.tongue),
            Some(stroke(pen, 120, StrokeCap::Butt, StrokeJoin::Round)),
            None,
            "wp-tongue",
        )
    };
    let mut m: Vec<Primitive> = Vec::new();
    // Beak fill, kept off a body of nearly the same colour.
    let warm_clash = (((w.hue - 32.0 + 540.0) % 360.0) - 180.0).abs() < 38.0 && !pal.dark;
    let beak_fill = match f.tone {
        0 => {
            if warm_clash {
                pal.bone
            } else {
                hsl(32.0, 90.0, 58.0)
            }
        }
        1 => pal.bone,
        2 => {
            if warm_clash {
                pal.ink("dark")
            } else {
                hsl(46.0, 92.0, 60.0)
            }
        }
        _ => pal.accent,
    };
    if plan == 2 {
        let open = if kind == 2 {
            1.0
        } else if kind == 6 {
            0.6
        } else {
            0.0
        };
        let k = 0.7 + 0.6 * f.size;
        let scaled =
            |pts: &[P]| -> Vec<P> { pts.iter().map(|q| [q[0] * k, q[1] * k - 0.45]).collect() };
        let beak_stroke = Some(stroke(pal.outline, 150, StrokeCap::Butt, StrokeJoin::Round));
        let piece = |pts: &[P]| {
            prim(
                poly(&scaled(pts)),
                solid(beak_fill),
                beak_stroke,
                None,
                "wp-beak",
            )
        };
        let inside = |pts: &[P]| {
            filled(
                poly(&scaled(pts)),
                pal.mouth_inside,
                None,
                "wp-mouth-inside",
            )
        };
        let ty = f.kind;
        if ty == 1 {
            // A duck bill: wide, flat, round.
            let bill = |cy0: f64, a: f64, b: f64| ellipse_poly(0.0, cy0, a, b, [1.0, 0.0], 20);
            if open != 0.0 {
                m.push(piece(&bill(0.1 + 0.4 * open, 0.75, 0.32)));
                m.push(inside(&bill(0.08 + 0.16 * open, 0.62, 0.26)));
            }
            m.push(piece(&bill(-0.12 - 0.08 * open, 0.95, 0.4)));
            for s in [-1.0, 1.0] {
                m.push(filled(
                    circle(
                        q2(mx(s * 0.2 * k)),
                        q2(my((-0.3 - 0.08 * open) * k - 0.45)),
                        q2(max(0.6, r * 0.06)),
                    ),
                    pal.outline,
                    None,
                    "wp-nostril",
                ));
            }
        } else {
            let tip = if ty == 3 {
                1.3
            } else if ty == 2 {
                0.7
            } else {
                0.5
            };
            let half = if ty == 3 { 0.36 } else { 0.6 };
            if open != 0.0 {
                m.push(inside(&[
                    [-half * 0.8, -0.3],
                    [half * 0.8, -0.3],
                    [0.0, tip * 0.7 + 0.6 * open],
                ]));
                m.push(piece(&[
                    [-half * 0.66, -0.05 + 0.25 * open],
                    [half * 0.66, -0.05 + 0.25 * open],
                    [0.0, tip * 0.9 + 0.6 * open],
                ]));
            }
            if ty == 2 {
                // A hooked beak: the upper half curves down over the lower.
                m.push(prim(
                    shape(&[
                        Cmd::M(-0.55 * k, -0.5 * k - 0.45),
                        Cmd::Q(0.0, -0.75 * k - 0.45, 0.55 * k, -0.5 * k - 0.45),
                        Cmd::Q(0.42 * k, 0.2 * k - 0.45, 0.0, (tip - 0.3 * open) * k - 0.45),
                        Cmd::Q(-0.42 * k, 0.2 * k - 0.45, -0.55 * k, -0.5 * k - 0.45),
                        Cmd::Z,
                    ]),
                    solid(beak_fill),
                    beak_stroke,
                    None,
                    "wp-beak",
                ));
            } else {
                m.push(piece(&[
                    [-half, -0.5],
                    [half, -0.5],
                    [0.0, tip - 0.35 * open],
                ]));
            }
            if open == 0.0 {
                m.push(stroked(
                    local_shape(
                        &[
                            Cmd::M(-half * 0.6 * k, -0.02 * k - 0.45),
                            Cmd::Q(0.0, 0.16 * k - 0.45, half * 0.6 * k, -0.02 * k - 0.45),
                        ],
                        &mx,
                        &my,
                        None,
                    ),
                    pal.outline,
                    1.3,
                    "wp-mouth-line",
                    Some(0.7),
                ));
            }
        }
        // The corners of the gape carry the mood where the beak cannot.
        let turn = if kind == 2 {
            -0.2
        } else if kind == 7 || kind == 9 {
            0.22
        } else {
            0.0
        };
        if turn != 0.0 {
            for s in [-1.0, 1.0] {
                m.push(stroked(
                    local_shape(
                        &[
                            Cmd::M(s * 0.62 * k, -0.5 * k - 0.45),
                            Cmd::L(s * 0.9 * k, (-0.5 + turn) * k - 0.45),
                        ],
                        &mx,
                        &my,
                        None,
                    ),
                    pen,
                    1.8,
                    "wp-mouth-line",
                    None,
                ));
            }
        }
    } else if plan == 6 {
        // A maw: a mouth as wide as the face allows, with teeth.
        let hw = f.maw_half;
        let n = f.teeth;
        let fang = |x: f64, y: f64, up: bool| {
            tooth(
                &[
                    [x - 0.15, y],
                    [x + 0.15, y],
                    [x, y + (if up { -0.42 } else { 0.42 })],
                ],
                "wp-tooth",
            )
        };
        let inside_stroke = Some(stroke(pen, 200, StrokeCap::Butt, StrokeJoin::Round));
        if kind == 2 {
            m.push(prim(
                shape(&[Cmd::M(-hw, -0.1), Cmd::Q(0.0, 1.55, hw, -0.1), Cmd::Z]),
                solid(pal.mouth_inside),
                inside_stroke,
                None,
                "wp-mouth-inside",
            ));
            m.push(tongue(&[
                Cmd::M(-hw * 0.4, 0.45),
                Cmd::Q(0.0, 0.95, hw * 0.4, 0.45),
                Cmd::Q(0.0, 0.25, -hw * 0.4, 0.45),
                Cmd::Z,
            ]));
            for i in 0..=n {
                m.push(fang(
                    hw * (-0.72 + 1.44 * f64::from(i) / f64::from(n)),
                    -0.08,
                    false,
                ));
            }
        } else if kind == 6 || kind == 10 {
            let big = if kind == 6 { 1.0 } else { 0.55 };
            m.push(prim(
                ellipse(
                    q2(mx(0.0)),
                    q2(my(0.25)),
                    q2(hw * 0.5 * big * r * mw),
                    q2(0.55 * big * r * md),
                ),
                solid(pal.mouth_inside),
                Some(plain_stroke(pen, 190)),
                None,
                "wp-mouth-inside",
            ));
            if kind == 6 {
                for s in [-1.0, 1.0] {
                    m.push(fang(s * hw * 0.2, -0.22, false));
                }
            }
        } else if kind == 9 {
            let (x0, x1) = (-hw * 0.85, hw * 0.85);
            m.push(prim(
                shape(&[
                    Cmd::M(x0, -0.12),
                    Cmd::Q(x0 - 0.2, 0.2, x0, 0.55),
                    Cmd::L(x1, 0.55),
                    Cmd::Q(x1 + 0.2, 0.2, x1, -0.12),
                    Cmd::Z,
                ]),
                solid(WHITE),
                inside_stroke,
                None,
                "wp-teeth",
            ));
            m.push(line(
                &[Cmd::M(x0 - 0.08, 0.21), Cmd::L(x1 + 0.08, 0.21)],
                1.3,
            ));
            for i in 1..n + 2 {
                let x = x0 + (x1 - x0) * (f64::from(i) / f64::from(n + 2));
                m.push(line(&[Cmd::M(x, -0.1), Cmd::L(x, 0.53)], 1.3));
            }
        } else {
            let sag = if kind == 7 {
                -0.6
            } else if kind == 8 || kind == 4 {
                0.0
            } else {
                0.5
            };
            let base = if kind == 7 { 0.45 } else { 0.02 };
            let y_at = |x: f64| base + sag * 0.5 * (1.0 - (x / hw) * (x / hw));
            for i in 0..n {
                let x = hw
                    * (if n == 1 {
                        0.0
                    } else {
                        -0.62 + 1.24 * f64::from(i) / f64::from(n - 1)
                    });
                m.push(fang(x, y_at(x) + 0.03, i % 2 == 0));
            }
            m.push(line(
                &[Cmd::M(-hw, base), Cmd::Q(0.0, base + sag, hw, base)],
                2.6,
            ));
            if kind == 8 {
                m.push(tongue(&[
                    Cmd::M(0.05, 0.05),
                    Cmd::Q(0.08, 0.8, 0.3, 0.8),
                    Cmd::Q(0.52, 0.8, 0.55, 0.05),
                    Cmd::Z,
                ]));
            }
        }
    } else {
        // The wild mouths, the mood mouths, and what the plan adds to them.
        let own = w.mouth;
        let inside_stroke = Some(stroke(pen, 200, StrokeCap::Butt, StrokeJoin::Round));
        match kind {
            0 => m.push(line(&[Cmd::M(-0.5, 0.0), Cmd::Q(0.0, 0.45, 0.5, 0.0)], 2.4)),
            1 => m.push(line(
                &[
                    Cmd::M(-0.8, 0.0),
                    Cmd::Q(-0.4, 0.55, 0.0, 0.0),
                    Cmd::Q(0.4, 0.55, 0.8, 0.0),
                ],
                2.4,
            )),
            2 => {
                m.push(prim(
                    shape(&[Cmd::M(-0.7, -0.1), Cmd::Q(0.0, 1.1, 0.7, -0.1), Cmd::Z]),
                    solid(pal.mouth_inside),
                    inside_stroke,
                    None,
                    "wp-mouth-inside",
                ));
                m.push(filled(
                    shape(&[Cmd::M(-0.35, 0.3), Cmd::Q(0.0, 0.62, 0.35, 0.3), Cmd::Z]),
                    pal.tongue,
                    None,
                    "wp-tongue",
                ));
            }
            3 => {
                m.push(line(
                    &[Cmd::M(-0.75, 0.15), Cmd::Q(0.0, 0.5, 0.7, -0.1)],
                    2.4,
                ));
                m.push(tooth(
                    &[[0.13, 0.23], [0.42, 0.1], [0.33, 0.55]],
                    "wp-tooth",
                ));
            }
            4 => m.push(line(&[Cmd::M(-0.55, 0.0), Cmd::L(0.55, 0.0)], 2.6)),
            5 => {
                m.push(line(&[Cmd::M(-0.6, 0.0), Cmd::Q(0.0, 0.5, 0.6, 0.0)], 2.4));
                m.push(tooth(
                    &[[-0.45, 0.12], [-0.15, 0.14], [-0.3, 0.62]],
                    "wp-fang",
                ));
                m.push(tooth(&[[0.15, 0.14], [0.45, 0.12], [0.3, 0.62]], "wp-fang"));
            }
            6 | 10 => {
                let big = if kind == 6 { 1.0 } else { 0.68 };
                m.push(prim(
                    ellipse(
                        q2(mx(0.0)),
                        q2(my(0.2)),
                        q2(0.3 * big * r * mw),
                        q2(0.38 * big * r * md),
                    ),
                    solid(pal.mouth_inside),
                    Some(plain_stroke(pen, 180)),
                    None,
                    "wp-mouth-inside",
                ));
            }
            7 => m.push(line(
                &[Cmd::M(-0.55, 0.35), Cmd::Q(0.0, -0.25, 0.55, 0.35)],
                2.4,
            )),
            8 => {
                m.push(line(&[Cmd::M(-0.5, 0.05), Cmd::L(0.5, 0.05)], 2.6));
                m.push(tongue(&[
                    Cmd::M(0.02, 0.05),
                    Cmd::Q(0.05, 0.72, 0.24, 0.72),
                    Cmd::Q(0.44, 0.72, 0.46, 0.05),
                    Cmd::Z,
                ]));
            }
            _ => m.push(line(
                &[
                    Cmd::M(-0.66, 0.1),
                    Cmd::Q(-0.44, -0.28, -0.22, 0.1),
                    Cmd::Q(0.0, 0.48, 0.22, 0.1),
                    Cmd::Q(0.44, -0.28, 0.66, 0.1),
                ],
                2.4,
            )),
        }
        // A creature born with a fang or a snaggle tooth keeps it in every
        // mood.
        if sp.mouth.is_some() && kind != 6 && kind != 10 {
            let dy = if kind == 7 {
                0.14
            } else if kind == 2 {
                -0.04
            } else {
                0.02
            };
            if own == 5 {
                m.push(tooth(
                    &[[-0.42, dy], [-0.18, dy + 0.02], [-0.3, dy + 0.46]],
                    "wp-fang",
                ));
                m.push(tooth(
                    &[[0.18, dy + 0.02], [0.42, dy], [0.3, dy + 0.46]],
                    "wp-fang",
                ));
            }
            if own == 3 {
                m.push(tooth(
                    &[[0.1, dy], [0.38, dy], [0.26, dy + 0.42]],
                    "wp-tooth",
                ));
            }
        }
        let z = f.size;
        let tusk = |pts: &[P]| {
            prim(
                poly(pts),
                solid(pal.bone),
                Some(stroke(pen, 120, StrokeCap::Butt, StrokeJoin::Round)),
                None,
                "wp-tusk",
            )
        };
        if plan == 3 {
            for s in [-1.0, 1.0] {
                if f.kind & 1 == 1 {
                    m.push(tusk(&[
                        [s * 0.28, 0.08],
                        [s * 0.58, 0.08],
                        [s * 0.46, 0.75 + 0.6 * z],
                    ]));
                } else {
                    m.push(tusk(&[
                        [s * 0.5, 0.2],
                        [s * 0.84, 0.1],
                        [s * (0.92 + 0.2 * z), -0.45 - 0.6 * z],
                    ]));
                }
            }
        } else if plan == 4 {
            let y0 = if kind == 7 { 0.12 } else { 0.06 };
            let y1 = y0 + 0.42 + 0.25 * z;
            m.push(tooth(
                &[[-0.27, y0], [0.27, y0], [0.25, y1], [-0.25, y1]],
                "wp-tooth",
            ));
            m.push(line(&[Cmd::M(0.0, y0), Cmd::L(0.0, y1)], 1.1));
        } else if plan == 5 && kind != 2 && kind != 6 && kind != 10 {
            for s in [-1.0, 1.0] {
                m.push(tooth(
                    &[
                        [s * 0.18, 0.2],
                        [s * 0.42, 0.16],
                        [s * 0.31, -0.2 - 0.2 * z],
                    ],
                    "wp-tooth",
                ));
            }
        }
    }
    out.mouth = m;

    // Muzzle or whisker pads sit under the mouth, on the skin.
    if plan == 1 {
        out.skin.push(filled(
            ellipse(q1(fx), q2(p.mouth_y - r * 0.12), q2(r * 1.5), q2(r * 1.12)),
            pal.ink(f.patch),
            None,
            "wp-muzzle",
        ));
    }
    if plan == 7 {
        for s in [-1.0, 1.0] {
            out.skin.push(filled(
                circle(q2(fx + s * r * 0.52), q2(p.mouth_y - r * 0.1), q2(r * 0.56)),
                pal.ink(f.patch),
                None,
                "wp-pad",
            ));
        }
    }

    // ---- nose ----
    if f.nose != 0 {
        let ny = p.mouth_y - r * 0.72;
        let room = if f.eye_plan == 1 || f.eye_plan == 5 {
            r * 0.5
        } else {
            max(
                r * 0.16,
                p.ipd - r * EYE_OUTER[f.style][1] * eye_sx(c, sp) - r * 0.12,
            )
        };
        let tone = pal.ink(f.nose_tone);
        let nx = |v: f64| fx + v;
        let nyf = |v: f64| ny + v;
        let mut n: Vec<Primitive> = Vec::new();
        match f.nose {
            1 => n.push(filled(
                ellipse(q1(fx), q2(ny), q2(min(r * 0.2, room)), q2(r * 0.14)),
                pal.ink,
                None,
                "wp-nose",
            )),
            2 => {
                let wd = min(r * 0.36, room);
                let h = r * 0.24;
                n.push(prim(
                    local_shape(
                        &[
                            Cmd::M(-wd, -h),
                            Cmd::Q(0.0, -h * 1.5, wd, -h),
                            Cmd::Q(wd * 0.5, h * 0.7, 0.0, h),
                            Cmd::Q(-wd * 0.5, h * 0.7, -wd, -h),
                            Cmd::Z,
                        ],
                        &nx,
                        &nyf,
                        None,
                    ),
                    solid(tone),
                    Some(stroke(pal.outline, 100, StrokeCap::Butt, StrokeJoin::Round)),
                    None,
                    "wp-nose",
                ));
            }
            3 => {
                let wd = min(r * 0.44, room);
                n.push(filled(
                    ellipse(q1(fx), q2(ny), q2(wd), q2(r * 0.28)),
                    tone,
                    None,
                    "wp-nose",
                ));
                n.push(filled(
                    ellipse(
                        q2(fx - wd * 0.3),
                        q2(ny - r * 0.1),
                        q2(wd * 0.3),
                        q2(r * 0.07),
                    ),
                    WHITE,
                    Some(0.6),
                    "wp-nose",
                ));
            }
            4 => {
                for s in [-1.0, 1.0] {
                    n.push(filled(
                        ellipse(
                            q2(fx + s * min(r * 0.18, room * 0.7)),
                            q2(ny),
                            q2(max(0.6, r * 0.07)),
                            q2(max(0.8, r * 0.1)),
                        ),
                        pal.ink,
                        None,
                        "wp-nose",
                    ));
                }
            }
            _ => {
                let wd = min(r * 0.42, room);
                let fill = pal.ink(if f.nose_tone == "ink" {
                    "accent"
                } else {
                    f.nose_tone
                });
                n.push(prim(
                    circle(q1(fx), q2(ny), q2(wd)),
                    solid(fill),
                    Some(plain_stroke(pal.outline, 130)),
                    None,
                    "wp-nose",
                ));
                n.push(filled(
                    circle(q2(fx - wd * 0.3), q2(ny - wd * 0.3), q2(wd * 0.25)),
                    WHITE,
                    Some(0.7),
                    "wp-nose",
                ));
            }
        }
        let shown = sp.mouth.unwrap_or(w.mouth);
        if (f.nose == 2 || f.nose == 3)
            && [0, 1, 3, 4, 5, 7, 8, 9].contains(&shown)
            && (plan == 0 || plan == 1 || plan == 7)
        {
            let mut d = PathB::new();
            d.mq(q1(fx), q2(ny + r * 0.24));
            d.lq(q1(fx), q2(my(0.02)));
            n.push(stroked(d.shape(), pal.mouth, 1.5, "wp-philtrum", None));
        }
        out.nose = n;
    }
    if f.mole != 0.0 {
        out.skin.push(filled(
            circle(
                q2(fx + f.mole * r * 1.05 * mw),
                q2(p.mouth_y + r * 0.3),
                q2(max(0.8, r * 0.1)),
            ),
            pal.ink,
            Some(0.85),
            "wp-mole",
        ));
    }
}

/// The horizontal eye scale in a mood.
fn eye_sx(c: &Built, sp: &MoodSpec) -> f64 {
    let asp = c.w.eye_aspect;
    let sx = (if asp >= 1.0 { 1.0 / asp } else { 1.0 }) * sp.es;
    let sy = (if asp >= 1.0 { 1.0 } else { asp }) * sp.es;
    if !((sx - 1.0).abs() > 0.005 || (sy - 1.0).abs() > 0.005) {
        1.0
    } else {
        sx
    }
}
