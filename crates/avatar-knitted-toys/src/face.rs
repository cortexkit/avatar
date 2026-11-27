//! Drawing the face and the mood marks: eyes and lids, brows, nose, mouth,
//! cheeks, and the felt drops, stitched letters and French knots a mood
//! adds.

use avatar_svg::{Rgb, StrokeCap, StrokeJoin};

use crate::build::{EyeSpot, Part, Seam};
use crate::draw::{drop_shadow, part_svg, running_stitch, Ctx};
use crate::emit::{circle, ellipse, eye_gloss, poly, seg, stitches, Style, WHITE};
use crate::geom::{
    add, bbox_of, clamp, cos, crossings, dir_of, dist, ellipse_pts, flip_x, max, min, mixp, perp,
    poly_len, quad, rot, scl, sin, spline, sq, sub, unit, Pt, PI, TAU,
};
use crate::palette::{hsl, rgb, Palette};
use crate::pose::FRAME;

/// Points at arc lengths `ss` along a polyline.
fn points_at(p: &[Pt], ss: &[f64]) -> Vec<Pt> {
    let mut cum = vec![0.0];
    for i in 1..p.len() {
        cum.push(cum[i - 1] + dist(p[i - 1], p[i]));
    }
    ss.iter()
        .map(|&s| {
            let mut i = 0;
            while i + 2 < p.len() && cum[i + 1] < s {
                i += 1;
            }
            let mut den = cum[i + 1] - cum[i];
            if den == 0.0 {
                den = 1.0;
            }
            let u = clamp((s - cum[i]) / den, 0.0, 1.0);
            mixp(p[i], p[i + 1], u)
        })
        .collect()
}

/// Embroidery: yarn sewn through the fabric. At full size a line is a
/// backstitch, straight stitches between the points where the needle went
/// in, with small gaps; in icons it is one line at least about a pixel wide.
pub fn thread(c: &mut Ctx, pts: &[Pt], w: f64, colour: Rgb, label: &str) {
    let g = c.g;
    let width = max(w, c.pxu * if c.detail == 0 { 1.2 } else { 0.9 });
    let style = Style::line(colour, width, StrokeCap::Round, StrokeJoin::Round);
    if c.detail < 2 {
        c.out.push(poly(pts, false), style, None, label);
        return;
    }
    let total = poly_len(pts, false);
    let l = max(width * 1.6, 3.4 / g.fit.s);
    let gap = min(width * 0.3, l * 0.15);
    let n = max(1.0, crate::geom::js_round(total / l)) as usize;
    let mut ss = Vec::with_capacity(2 * n);
    for i in 0..n {
        let a = total * i as f64 / n as f64;
        let b = total * (i + 1) as f64 / n as f64;
        ss.push(a + if i > 0 { gap / 2.0 } else { 0.0 });
        ss.push(b - if i < n - 1 { gap / 2.0 } else { 0.0 });
    }
    let qs = points_at(pts, &ss);
    let ds = qs.chunks(2).map(|pair| seg(pair[0], pair[1])).collect();
    if let Some(shape) = stitches(ds) {
        c.out.push(shape, style, None, label);
    }
}

/// A felt shape cut to `pts`, edged, with a running stitch at full size
/// when `stitched`.
pub fn felt_shape(c: &mut Ctx, pts: &[Pt], fill: Rgb, edge: Rgb, label: &str, stitched: bool) {
    let g = c.g;
    c.out.push(
        poly(pts, true),
        Style::fill(fill).with_stroke(edge, max(0.45 / g.fit.s, c.pxu * 0.45)),
        None,
        label,
    );
    if stitched && c.detail == 2 {
        running_stitch(c, pts, edge, 1.3 / g.fit.s);
    }
}

/// Keeps the part of polygon `p` on the side of the line through `p0`
/// where dot(q - p0, n) <= 0.
fn clip_half(p: &[Pt], p0: Pt, n: Pt) -> Vec<Pt> {
    let f = |q: Pt| (q[0] - p0[0]) * n[0] + (q[1] - p0[1]) * n[1];
    let mut out = Vec::new();
    for i in 0..p.len() {
        let a = p[i];
        let b = p[(i + 1) % p.len()];
        let fa = f(a);
        let fb = f(b);
        if fa <= 0.0 {
            out.push(a);
        }
        if (fa <= 0.0) != (fb <= 0.0) {
            out.push(mixp(a, b, fa / (fa - fb)));
        }
    }
    out
}

/// How far the upper lid covers the eye, and how it slants, per mood. The
/// moods with no entry use the toy's resting lid.
fn lids(mood: &str) -> Option<[f64; 2]> {
    match mood {
        "working" => Some([0.46, -0.06]),
        "waiting" => Some([0.12, 0.0]),
        "blocked" => Some([0.42, -0.44]),
        "sad" => Some([0.36, 0.42]),
        _ => None,
    }
}

/// A felt lid: the part of the disc (centre c, radius R) above the lid's
/// cut. The cut runs along `dir` through p0 and bulges down by `bulge` in
/// its middle. The region above it is convex, so it is the disc clipped by
/// the cut's tangent lines. Returns the lid polygon and the cut's points
/// inside the disc.
fn felt_lid(c: Pt, r: f64, p0: Pt, dir: Pt, bulge: f64) -> (Vec<Pt>, Vec<Pt>) {
    let n = [-dir[1], dir[0]];
    let g = |u: f64| bulge * (1.0 - (u / r) * (u / r));
    let dg = |u: f64| -2.0 * bulge * u / (r * r);
    let mut lid = ellipse_pts(c, r, r, 40, 0.0);
    let mut k = 0;
    while k <= 8 && lid.len() > 2 {
        let u = -r + 2.0 * r * k as f64 / 8.0;
        let nn = sub(n, scl(dir, dg(u)));
        let off = g(u) - dg(u) * u;
        lid = clip_half(
            &lid,
            add(p0, scl(nn, off / (nn[0] * nn[0] + nn[1] * nn[1]))),
            nn,
        );
        k += 1;
    }
    let mut cut = Vec::new();
    for k in 0..=16 {
        let u = -r + 2.0 * r * k as f64 / 16.0;
        let qp = add(p0, add(scl(dir, u), scl(n, g(u))));
        if dist(qp, c) < r * 0.97 {
            cut.push(qp);
        }
    }
    (lid, cut)
}

/// Closed eyes are embroidered: arches for a laugh, hanging curves with
/// lashes for sleep.
fn closed_eye(c: &mut Ctx, centre: Pt, r: f64, mood: &str) {
    let g = c.g;
    let col = g.pal.thread_dark;
    let w = max(r * 0.3, 1.6 / g.fit.s);
    let [x, y] = centre;
    if mood == "happy" {
        let arch = quad(
            [x - r * 0.95, y + r * 0.22],
            [x, y - r * 1.05],
            [x + r * 0.95, y + r * 0.22],
            12,
        );
        thread(c, &arch, w, col, "eye-closed");
        return;
    }
    let arc = quad(
        [x - r * 0.95, y - r * 0.05],
        [x, y + r * 0.8],
        [x + r * 0.95, y - r * 0.05],
        12,
    );
    thread(c, &arc, w, col, "eye-closed");
    if c.detail >= 1 {
        for t in [0.22, 0.5, 0.78] {
            let i = crate::geom::js_round(t * 12.0) as usize;
            let p = arc[i];
            let q = arc[(i + 1).min(12)];
            let n = unit(perp(sub(q, p)));
            let dn = if n[1] < 0.0 { scl(n, -1.0) } else { n };
            thread(c, &[p, add(p, scl(dn, r * 0.38))], w * 0.6, col, "lashes");
        }
    }
}

/// The eyes on the face. Eyes on stalks or bumps are drawn with their
/// mounts instead.
fn draw_eyes(c: &mut Ctx) {
    let eyes = c.g.face().eyes.clone();
    for e in eyes.iter().filter(|e| e.rig.is_none()) {
        draw_eye(c, e);
    }
}

pub fn draw_eye(c: &mut Ctx, e: &EyeSpot) {
    let g = c.g;
    let f = g.face();
    let t = &g.t;
    let ek = &t.eyes;
    let pal = &g.pal;
    let mood = c.mood_name();
    // A happy toy shuts its eyes in arches, or keeps them open above a
    // raised lower lid, by its own gene. Sleep shuts them in hanging curves.
    let beaming = mood == "happy" && t.temper.happy == "open";
    let closed = (mood == "happy" && !beaming) || mood == "sleepy";
    let rest = match ek.lid {
        "half" => 0.3,
        "heavy" => 0.44,
        _ => 0.0,
    };
    let lid_spec = if beaming {
        [0.0, 0.0]
    } else if let Some(l) = lids(mood) {
        [
            max(l[0], if mood == "waiting" { rest * 0.5 } else { rest }),
            l[1],
        ]
    } else {
        [rest, 0.0]
    };
    let side = if e.side != 0.0 { e.side } else { t.mood_side };
    // In an icon an eye keeps a least size on screen, and a pair moves apart
    // if that makes the two touch.
    let mut r = e.r;
    let mut ctr = [e.x, e.y];
    if c.detail == 0 {
        r = max(e.r, c.pxu * if c.px >= 40.0 { 2.1 } else { 1.7 });
        if e.side != 0.0 && e.rig.is_none() {
            ctr = [
                e.side * max(e.x.abs(), r * if f.eyes.len() == 3 { 2.7 } else { 1.2 }),
                e.y,
            ];
        }
    }
    let lid_c = ctr;
    let mut lid_r = r * 1.14;
    let mut dome = ctr;
    let mut dome_r = r;
    // A dark toy's eyes sit on light felt.
    let backed = pal.dark && ek.kind != "felt";
    if backed {
        let disc = ellipse_pts(ctr, r * 1.42, r * 1.42, 30, 0.0);
        felt_shape(
            c,
            &disc,
            pal.felt.white,
            pal.felt.white_edge,
            "eye-white",
            true,
        );
        lid_r = r * 1.5;
    }
    if ek.kind == "felt" {
        let wr = r * 1.12;
        let shape = match ek.felt {
            Some("oval") => ellipse_pts(ctr, wr * 0.88, wr * 1.12, 32, 0.0),
            Some("drop") => ellipse_pts(ctr, wr * 0.86, wr * 1.14, 32, side * 0.38),
            _ => ellipse_pts(ctr, wr, wr, 32, 0.0),
        };
        felt_shape(
            c,
            &shape,
            pal.felt.white,
            pal.felt.white_edge,
            "eye-white",
            true,
        );
        lid_r = wr * 1.16;
        dome_r = r * 0.6;
        // A waiting toy with felt eyes looks up and aside.
        let off = if mood == "waiting" {
            [-t.mood_side * 0.36, -0.3]
        } else {
            match ek.look {
                Some("out") => [side * 0.38, 0.16],
                Some("cross") => [-side * 0.38, 0.16],
                _ => [0.0, 0.22],
            }
        };
        dome = [ctr[0] + off[0] * r, ctr[1] + off[1] * r];
    }
    if closed {
        closed_eye(c, ctr, r, mood);
        return;
    }
    // A dark eye keeps its white highlight in view under a lid: where the
    // lid would cover it, the highlight slides down round the eye until it
    // is clear of the lid's cut.
    let [cov, slant] = lid_spec;
    let lid_a = side * slant;
    let cut_y = |x: f64| {
        if cov <= 0.02 {
            return f64::NEG_INFINITY;
        }
        let u = (x - lid_c[0]) / cos(lid_a);
        lid_c[1] - lid_r
            + 2.0 * lid_r * cov
            + sin(lid_a) * u
            + cos(lid_a) * lid_r * 0.2 * (0.4 - sq(u / lid_r))
    };
    let glint = |about: Pt, from: Pt, hy: f64| {
        let mut p = from;
        let mut k = 0;
        while k < 9 && p[1] - hy * 1.25 < cut_y(p[0]) {
            p = rot(from, -0.16 * (k + 1) as f64, Some(about));
            k += 1;
        }
        p
    };
    let white = WHITE;
    match ek.kind {
        "stitched" => {
            c.out.push(
                ellipse(ctr, r * 0.48, r * 0.68),
                Style::fill(pal.thread_dark),
                None,
                "eye-stitched",
            );
            if c.detail == 2 {
                let mut ds = Vec::new();
                for k in -1..=1 {
                    let kf = f64::from(k);
                    let x = ctr[0] + kf * r * 0.2;
                    let hh = r * 0.68 * max(0.0, 1.0 - sq(kf * 0.2 / 0.48)).sqrt() * 0.88;
                    ds.push(seg(
                        [x - r * 0.04, ctr[1] - hh],
                        [x + r * 0.04, ctr[1] + hh],
                    ));
                }
                if let Some(shape) = stitches(ds) {
                    c.out.push(
                        shape,
                        Style::line(rgb(0x5a4c48), r * 0.07, StrokeCap::Butt, StrokeJoin::Miter)
                            .with_opacity(0.8),
                        None,
                        "satin",
                    );
                }
            }
            // A satin-stitched eye is narrow, so its highlight moves
            // straight down.
            let sx = ctr[0] - r * 0.15;
            c.out.push(
                ellipse(
                    [
                        sx,
                        clamp(cut_y(sx) + r * 0.23, ctr[1] - r * 0.3, ctr[1] + r * 0.3),
                    ],
                    r * 0.14,
                    r * 0.18,
                ),
                Style::fill(white).with_opacity(0.92),
                None,
                "shine",
            );
        }
        "button" => {
            let bc = rgb(0x3d2f2c);
            let be = rgb(0x1f1614);
            if !backed {
                c.out.push(
                    circle(ctr, r * 1.12),
                    Style::fill(pal.main.deep).with_opacity(0.35),
                    None,
                    "socket",
                );
            }
            c.out.push(
                circle(ctr, r * 1.02),
                Style::fill(bc).with_stroke(be, max(r * 0.1, c.pxu * 0.5)),
                None,
                "button",
            );
            if c.detail >= 1 {
                c.out.push(
                    circle(ctr, r * 0.74),
                    Style::line(be, r * 0.06, StrokeCap::Butt, StrokeJoin::Miter),
                    None,
                    "button-rim",
                );
                for qd in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
                    let h = [ctr[0] + qd[0] * r * 0.27, ctr[1] + qd[1] * r * 0.27];
                    c.out
                        .push(circle(h, r * 0.1), Style::fill(be), None, "button");
                }
            }
            let at = glint(ctr, [ctr[0] - r * 0.45, ctr[1] - r * 0.5], r * 0.11);
            c.out.push(
                ellipse(at, r * 0.16, r * 0.11),
                Style::fill(white).with_opacity(0.6),
                None,
                "shine",
            );
        }
        _ => {
            if ek.kind != "felt" && !backed {
                c.out.push(
                    circle(ctr, r * 1.14),
                    Style::fill(pal.main.deep).with_opacity(0.38),
                    None,
                    "socket",
                );
            }
            if ek.kind == "iris" {
                c.out.push(
                    circle(dome, dome_r),
                    Style::fill(Palette::eye_iris(ek.iris.unwrap_or("amber"))),
                    None,
                    "iris",
                );
                c.out.push(
                    circle(dome, dome_r),
                    Style::paint(eye_gloss()).with_opacity(0.45),
                    None,
                    "gloss",
                );
                let pupil = rgb(0x0c0b0e);
                if ek.pupil == Some("slit") {
                    c.out.push(
                        ellipse(dome, dome_r * 0.17, dome_r * 0.72),
                        Style::fill(pupil),
                        None,
                        "pupil",
                    );
                } else {
                    c.out.push(
                        circle(dome, dome_r * 0.52),
                        Style::fill(pupil),
                        None,
                        "pupil",
                    );
                }
            } else {
                c.out.push(
                    circle(dome, dome_r),
                    Style::paint(eye_gloss()),
                    None,
                    "eye-dome",
                );
            }
            let shine_ry = dome_r * max(0.22, if c.detail == 0 { 0.32 } else { 0.0 });
            let shine_at = [dome[0] - dome_r * 0.32, dome[1] - dome_r * 0.34];
            let at = if ek.kind == "felt" {
                shine_at
            } else {
                glint(dome, shine_at, shine_ry)
            };
            c.out.push(
                ellipse(
                    at,
                    dome_r * max(0.27, if c.detail == 0 { 0.36 } else { 0.0 }),
                    shine_ry,
                ),
                Style::fill(white).with_opacity(0.93),
                None,
                "shine",
            );
            if c.detail >= 1 {
                c.out.push(
                    circle(
                        [dome[0] + dome_r * 0.3, dome[1] + dome_r * 0.32],
                        dome_r * 0.09,
                    ),
                    Style::fill(white).with_opacity(0.55),
                    None,
                    "shine",
                );
            }
        }
    }
    // A felt lid over the top of the eye, cut along a curve that slants with
    // the mood, edged with an embroidered line, in the body's own yarn.
    let ly = pal.main.clone();
    if cov > 0.02 {
        let a = side * slant;
        let dir = [cos(a), sin(a)];
        let n = [-dir[1], dir[0]];
        let bulge = lid_r * 0.2;
        let p0 = sub(
            [lid_c[0], lid_c[1] - lid_r + 2.0 * lid_r * cov],
            scl(n, bulge * 0.6),
        );
        let (lid, cut) = felt_lid(lid_c, lid_r, p0, dir, bulge);
        if lid.len() > 2 {
            c.out
                .push(poly(&lid, true), Style::fill(ly.mid), None, "lid felt");
            if cut.len() > 1 {
                thread(c, &cut, r * 0.13, ly.gap, "lid-line");
            }
        }
    }
    // The raised lower lid of an open happy eye: the upper lid's shape,
    // turned upside down, covering the lowest third of the eye.
    if beaming {
        let up = |q: Pt| [q[0], 2.0 * lid_c[1] - q[1]];
        let bulge = lid_r * 0.34;
        let (lid, cut) = felt_lid(
            lid_c,
            lid_r,
            [
                lid_c[0],
                lid_c[1] - lid_r + 2.0 * lid_r * 0.36 - bulge * 0.6,
            ],
            [1.0, 0.0],
            bulge,
        );
        if lid.len() > 2 {
            let lower: Vec<Pt> = lid.iter().map(|&q| up(q)).collect();
            c.out.push(
                poly(&lower, true),
                Style::fill(ly.mid),
                None,
                "lid lower-lid felt",
            );
            if cut.len() > 1 {
                let cut: Vec<Pt> = cut.iter().map(|&q| up(q)).collect();
                thread(c, &cut, r * 0.13, ly.gap, "lid-line");
            }
        }
    }
}

/// Brows, per mood: lift (in eye radii, down positive), slant (radians, the
/// inner end lower when positive), arch.
fn brows(mood: &str) -> [f64; 3] {
    match mood {
        "working" => [0.3, 0.24, 0.04],
        "happy" => [-0.3, -0.06, 0.45],
        "blocked" => [0.3, 0.54, -0.06],
        "sad" => [-0.12, -0.46, 0.12],
        "sleepy" => [0.2, -0.06, 0.14],
        _ => [0.0, 0.04, 0.2],
    }
}

fn draw_brows(c: &mut Ctx) {
    let g = c.g;
    let f = g.face();
    let b = &g.t.brows;
    let mood = c.mood_name();
    let pal = &g.pal;
    let has = b.rest != "none";
    // Eyes on stalks or bumps have no brow over them.
    if f.off_face {
        return;
    }
    if !has && matches!(mood, "neutral" | "happy" | "sleepy") {
        return;
    }
    let felt = b.rest == "felt";
    let col = if pal.dark {
        pal.thread
    } else {
        pal.thread_dark
    };
    let w = max(f.r * 0.22 * b.weight, 1.5 / g.fit.s) * if felt { 1.7 } else { 1.0 };
    let one = f.eyes.len() == 1;
    for side in [-1.0, 1.0] {
        let p = if mood == "waiting" {
            if side == g.t.mood_side {
                [-0.48, -0.22, 0.34]
            } else {
                [0.04, 0.1, 0.06]
            }
        } else {
            brows(mood)
        };
        let [dy, ang, arch] = p;
        let hl = f.brow_len * 0.5 * if one { 1.15 } else { 1.0 };
        let cx = if one { side * hl * 0.95 } else { side * f.ex };
        let cy = f.brow_y + dy * f.r;
        let inner = [cx - side * hl * cos(ang), cy + hl * sin(ang)];
        let outer = [cx + side * hl * cos(ang), cy - hl * sin(ang)];
        let mid = [cx, cy - arch * hl * 0.6];
        let pts = quad(
            inner,
            [
                2.0 * mid[0] - (inner[0] + outer[0]) / 2.0,
                2.0 * mid[1] - (inner[1] + outer[1]) / 2.0,
            ],
            outer,
            10,
        );
        let label = if side > 0.0 {
            "brow brow-r"
        } else {
            "brow brow-l"
        };
        if felt {
            let ww = max(w, c.pxu * 1.4) / 2.0;
            let mut l = Vec::new();
            let mut r = Vec::new();
            let last = pts.len() - 1;
            for (i, &p) in pts.iter().enumerate() {
                let q = pts[(i + 1).min(last)];
                let o = pts[i.saturating_sub(1)];
                let n = unit(perp(sub(q, o)));
                let k = ww * (0.55 + 0.45 * sin(PI * i as f64 / last as f64));
                l.push(add(p, scl(n, k)));
                r.push(sub(p, scl(n, k)));
            }
            r.reverse();
            l.extend(r);
            let shape = spline(&l, true, 2);
            felt_shape(
                c,
                &shape,
                hsl(pal.main.h, 18.0, if pal.dark { 84.0 } else { 24.0 }),
                hsl(pal.main.h, 18.0, if pal.dark { 66.0 } else { 14.0 }),
                label,
                false,
            );
        } else {
            thread(c, &pts, w, col, label);
        }
    }
}

fn draw_nose(c: &mut Ctx) {
    let g = c.g;
    let Some(n) = g.face().nose else {
        return;
    };
    let pal = &g.pal;
    let (ctr, s) = (n.c, n.s);
    let [x, y] = ctr;
    let shape = match n.shape {
        "oval" => ellipse_pts(ctr, s, s * 0.7, 28, 0.0),
        "heart" => spline(&crate::parts::heart_pts(ctr, s), true, 5),
        _ => spline(
            &[
                [x, y + s * 0.72],
                [x - s * 0.7, y - s * 0.05],
                [x - s * 0.92, y - s * 0.55],
                [x, y - s * 0.68],
                [x + s * 0.92, y - s * 0.55],
                [x + s * 0.7, y - s * 0.05],
            ],
            true,
            5,
        ),
    };
    match n.kind {
        "felt" => felt_shape(c, &shape, pal.blush, pal.blush_edge, "nose-felt", true),
        "stitched" => {
            let col = if pal.dark {
                pal.thread
            } else {
                pal.thread_dark
            };
            c.out
                .push(poly(&shape, true), Style::fill(col), None, "nose-stitched");
            if c.detail == 2 {
                let mut ds = Vec::new();
                for k in -2..=2 {
                    let yy = y + f64::from(k) * s * 0.24;
                    let xs = crossings(&shape, yy);
                    if xs.len() >= 2 {
                        ds.push(seg(
                            [xs[0] + s * 0.08, yy + s * 0.05],
                            [xs[xs.len() - 1] - s * 0.08, yy - s * 0.05],
                        ));
                    }
                }
                if let Some(sh) = stitches(ds) {
                    c.out.push(
                        sh,
                        Style::line(
                            rgb(if pal.dark { 0xc9beb6 } else { 0x5a4c48 }),
                            s * 0.09,
                            StrokeCap::Butt,
                            StrokeJoin::Miter,
                        )
                        .with_opacity(0.8),
                        None,
                        "satin",
                    );
                }
            }
        }
        _ => {
            c.out.push(
                poly(&shape, true),
                Style::paint(eye_gloss()),
                None,
                "nose-safety",
            );
            c.out.push(
                ellipse([x - s * 0.3, y - s * 0.3], s * 0.24, s * 0.14),
                Style::fill(WHITE).with_opacity(0.75),
                None,
                "shine",
            );
        }
    }
}

fn mirrored(l: &[Pt]) -> Vec<Pt> {
    let mut out = l.to_vec();
    out.extend(l[..l.len() - 1].iter().rev().map(|&p| flip_x(p)));
    out
}

fn draw_mouth(c: &mut Ctx) {
    let g = c.g;
    let f = g.face();
    let mo = f.mouth;
    let t = &g.t;
    let pal = &g.pal;
    let mood = c.mood_name();
    let (y, w) = (mo.y, mo.w);
    let side = t.mood_side;
    let col = if pal.dark {
        pal.thread
    } else {
        pal.thread_dark
    };
    let tw = max(max(f.r * 0.2, w * 0.13), 1.5 / g.fit.s);
    // The line from a muzzle's nose down to the mouth.
    if let (Some(nose), Some(snout)) = (f.nose, f.snout) {
        if (mood == "neutral" || mood == "waiting")
            && matches!(mo.kind, "w" | "smile" | "v")
            && snout.kind == "muzzle"
        {
            thread(
                c,
                &[[0.0, nose.c[1] + nose.s * 0.6], [0.0, y]],
                tw * 0.85,
                col,
                "philtrum",
            );
        }
    }
    let white = pal.felt.white;
    let white_edge = pal.felt.white_edge;
    match mood {
        "neutral" => {
            let k0 = mo.curve;
            match mo.kind {
                "w" => {
                    let l = quad(
                        [-w, y - 0.18 * w * k0],
                        [-w * 0.5, y + 0.62 * w * k0],
                        [0.0, y],
                        8,
                    );
                    thread(c, &mirrored(&l), tw, col, "mouth-line");
                }
                "v" => thread(
                    c,
                    &[
                        [-w * 0.55, y - w * 0.12],
                        [0.0, y + w * 0.28],
                        [w * 0.55, y - w * 0.12],
                    ],
                    tw,
                    col,
                    "mouth-line",
                ),
                "line" => thread(c, &[[-w * 0.62, y], [w * 0.62, y]], tw, col, "mouth-line"),
                _ => {
                    let k = match mo.kind {
                        "smile" => k0,
                        "fangs" => k0 * 0.6,
                        _ => 0.3,
                    };
                    let arc = quad(
                        [-w, y - w * 0.08],
                        [0.0, y + w * 0.8 * k],
                        [w, y - w * 0.08],
                        12,
                    );
                    thread(c, &arc, tw, col, "mouth-line");
                    if mo.kind == "fangs" {
                        for sx in [-1.0_f64, 1.0] {
                            let x = sx * w * 0.42;
                            let yy = y - w * 0.08
                                + w * 0.8 * k * 2.0 * 0.5 * (1.0 - sq(sx * 0.42)) * 0.5;
                            felt_shape(
                                c,
                                &[[x - w * 0.14, yy], [x + w * 0.14, yy], [x, yy + w * 0.34]],
                                white,
                                white_edge,
                                "fang",
                                false,
                            );
                        }
                    }
                    if mo.kind == "teeth" {
                        for sx in [-1.0, 1.0] {
                            let x0 = if sx > 0.0 { w * 0.02 } else { -w * 0.3 };
                            let yy = y + w * 0.8 * k * 0.5 - w * 0.04;
                            felt_shape(
                                c,
                                &[
                                    [x0, yy],
                                    [x0 + w * 0.28, yy],
                                    [x0 + w * 0.28, yy + w * 0.3],
                                    [x0 + w * 0.14, yy + w * 0.36],
                                    [x0, yy + w * 0.3],
                                ],
                                white,
                                white_edge,
                                "tooth",
                                false,
                            );
                        }
                    }
                }
            }
        }
        "working" => {
            let o = side * w * 0.22;
            if t.temper.working == "tongue" {
                let tongue = spline(
                    &[
                        [o + side * w * 0.22, y],
                        [o + side * w * 0.5, y + w * 0.06],
                        [o + side * w * 0.52, y + w * 0.34],
                        [o + side * w * 0.34, y + w * 0.42],
                        [o + side * w * 0.2, y + w * 0.2],
                    ],
                    true,
                    4,
                );
                felt_shape(c, &tongue, pal.felt.tongue, pal.blush_edge, "tongue", false);
                thread(
                    c,
                    &[[o - w * 0.42, y + w * 0.02], [o + w * 0.42, y - w * 0.02]],
                    tw,
                    col,
                    "mouth-line",
                );
            } else {
                // Lips pressed together: a short line with both ends turned
                // down.
                thread(
                    c,
                    &[
                        [-w * 0.5, y + w * 0.16],
                        [-w * 0.34, y],
                        [w * 0.34, y],
                        [w * 0.5, y + w * 0.16],
                    ],
                    tw,
                    col,
                    "mouth-line",
                );
            }
        }
        "happy" => {
            let top = quad(
                [-w * 0.92, y - w * 0.1],
                [0.0, y + w * 0.06],
                [w * 0.92, y - w * 0.1],
                10,
            );
            let bot = quad(
                [w * 0.92, y - w * 0.1],
                [0.0, y + w * 1.4],
                [-w * 0.92, y - w * 0.1],
                14,
            );
            let mut shape = top;
            shape.extend_from_slice(&bot[1..bot.len() - 1]);
            c.out.push(
                poly(&shape, true),
                Style::fill(pal.felt.mouth),
                None,
                "mouth-open felt",
            );
            c.out.push(
                ellipse([0.0, y + w * 0.4], w * 0.42, w * 0.17),
                Style::fill(pal.felt.tongue),
                None,
                "tongue felt",
            );
            let mut ring = shape.clone();
            ring.push(shape[0]);
            thread(c, &ring, tw * 0.8, col, "mouth-line");
        }
        "waiting" => thread(
            c,
            &[
                [side * w * 0.2 - w * 0.4, y + w * 0.03],
                [side * w * 0.2 + w * 0.4, y - w * 0.05],
            ],
            tw,
            col,
            "mouth-line",
        ),
        "blocked" => {
            if t.temper.blocked == "teeth" {
                let hw = w * 0.8;
                let hh = w * 0.27;
                let bx = spline(
                    &[
                        [-hw, y - hh],
                        [hw, y - hh],
                        [hw * 1.04, y + hh],
                        [-hw * 1.04, y + hh],
                    ],
                    true,
                    4,
                );
                felt_shape(c, &bx, white, white_edge, "teeth", false);
                let mut ds = vec![[[-hw * 0.98, y], [hw * 0.98, y]]];
                for k in [-0.5, 0.0, 0.5] {
                    ds.push([[k * hw, y - hh * 0.92], [k * hw, y + hh * 0.92]]);
                }
                for d in ds {
                    thread(c, &d, tw * 0.55, col, "teeth-line");
                }
                let mut ring = bx.clone();
                ring.push(bx[0]);
                thread(c, &ring, tw * 0.8, col, "mouth-line");
            } else {
                // A mouth clamped shut, embroidered as a zigzag.
                let pts: Vec<Pt> = (0..=6)
                    .map(|i| {
                        [
                            -w * 0.84 + f64::from(i) * w * 0.28,
                            y + if i % 2 == 1 { -1.0 } else { 1.0 } * w * 0.17,
                        ]
                    })
                    .collect();
                thread(c, &pts, tw * 1.05, col, "mouth-line");
            }
        }
        "sad" => {
            if t.temper.sad == "frown" {
                let arc = quad(
                    [-w * 0.75, y + w * 0.26],
                    [0.0, y - w * 0.5],
                    [w * 0.75, y + w * 0.26],
                    12,
                );
                thread(c, &arc, tw, col, "mouth-line");
            } else {
                // A trembling lip: a frown that dips again in the middle.
                let l = quad(
                    [-w * 0.8, y + w * 0.24],
                    [-w * 0.42, y - w * 0.42],
                    [0.0, y + w * 0.02],
                    7,
                );
                thread(c, &mirrored(&l), tw, col, "mouth-line");
            }
        }
        _ => {
            c.out.push(
                ellipse([0.0, y + w * 0.08], w * 0.21, w * 0.27),
                Style::fill(pal.felt.mouth),
                None,
                "mouth-open felt",
            );
            let mut ring = ellipse_pts([0.0, y + w * 0.08], w * 0.21, w * 0.27, 16, 0.0);
            ring.push([w * 0.21, y + w * 0.08]);
            thread(c, &ring, tw * 0.7, col, "mouth-line");
        }
    }
}

fn draw_snout_marks(c: &mut Ctx) {
    let g = c.g;
    let Some(s) = g.face().snout else {
        return;
    };
    if s.kind != "pig" {
        return;
    }
    let col = if g.pal.dark {
        g.pal.thread
    } else {
        g.pal.thread_dark
    };
    for sx in [-1.0, 1.0] {
        c.out.push(
            ellipse(
                [s.c[0] + sx * s.rx * 0.34, s.c[1]],
                s.rx * 0.12,
                s.ry * 0.26,
            ),
            Style::fill(col),
            None,
            "nostril",
        );
    }
}

fn draw_cheeks(c: &mut Ctx) {
    let g = c.g;
    let f = g.face();
    let pal = &g.pal;
    let mood = c.mood_name();
    let Some(ck) = f.cheeks else {
        return;
    };
    let kind = g.t.cheeks.kind;
    let show = kind != "none" || mood == "happy" || mood == "blocked" || f.off_face;
    if !show || mood == "sad" {
        return;
    }
    let k = match mood {
        "happy" => 1.15,
        "blocked" => 1.08,
        _ => 1.0,
    };
    let (fill, edge) = if mood == "blocked" {
        (pal.felt.anger, pal.felt.anger_edge)
    } else {
        (pal.blush, pal.blush_edge)
    };
    for sx in [-1.0, 1.0] {
        let ctr = [sx * ck.x, ck.y];
        if kind == "stitched" && mood != "blocked" && c.detail >= 1 {
            for j in [-1.0, 0.0, 1.0] {
                let d = [
                    [
                        ctr[0] + j * ck.rx * 0.5 - ck.rx * 0.18,
                        ctr[1] + ck.ry * 0.45,
                    ],
                    [
                        ctr[0] + j * ck.rx * 0.5 + ck.rx * 0.18,
                        ctr[1] - ck.ry * 0.45,
                    ],
                ];
                thread(c, &d, ck.ry * 0.32, pal.blush_deep, "blush-stitch");
            }
        } else {
            let shape = ellipse_pts(ctr, ck.rx * k, ck.ry * k, 26, 0.0);
            felt_shape(c, &shape, fill, edge, "blush", true);
        }
    }
}

/// The face: the parts sewn on it, then the embroidered and felt features.
pub fn draw_face(c: &mut Ctx, parts: &[&Part]) {
    let face_parts: Vec<&Part> = parts
        .iter()
        .copied()
        .filter(|p| p.group == "face")
        .collect();
    for p in face_parts {
        if p.seam == Seam::Ring && c.detail > 0 {
            drop_shadow(c, p);
        }
        part_svg(c, p);
    }
    draw_snout_marks(c);
    draw_cheeks(c);
    draw_eyes(c);
    draw_nose(c);
    draw_mouth(c);
    draw_brows(c);
}

/// A felt drop hanging from a point.
fn drop_pts(ctr: Pt, rr: f64) -> Vec<Pt> {
    let mut pts = vec![[ctr[0], ctr[1] - rr * 1.8]];
    for i in 0..=16 {
        let a = -0.3 + f64::from(i) / 16.0 * (PI + 0.6);
        pts.push([ctr[0] + cos(a) * rr, ctr[1] + sin(a) * rr]);
    }
    pts
}

/// Mood marks, made the way a maker would: felt drops, stitched letters and
/// French knots. They are sized for icons too.
pub fn draw_marks(c: &mut Ctx) {
    let g = c.g;
    let mood = c.mood_name();
    if mood == "neutral" {
        return;
    }
    let p = g.face_piece();
    let f = g.face();
    let side = g.t.mood_side;
    let pal = &g.pal;
    let [bx0, by0, bx1, by1] = bbox_of([p.outline.as_slice()]);
    let w = bx1 - bx0;
    let h = by1 - by0;
    let sz = max(
        max(w, h) * 0.14,
        c.pxu * if c.detail == 0 { 7.5 } else { 5.0 },
    );
    let fit = g.fit;
    let mg = 4.0;
    let fx0 = (mg - fit.ox) / fit.s;
    let fx1 = (FRAME - mg - fit.ox) / fit.s;
    let fy0 = (mg - fit.oy) / fit.s;
    let keep =
        |ctr: Pt, rx: f64, ry: f64| [clamp(ctr[0], fx0 + rx, fx1 - rx), max(ctr[1], fy0 + ry)];
    let ink = pal.thread_dark;
    let tw = max(sz * 0.13, c.pxu * 1.2);
    match mood {
        "working" => {
            let rr = sz * 0.36;
            let ctr = keep(
                [side * (w * 0.5 + rr * 0.2), by0 + h * 0.2],
                rr * 1.2,
                rr * 2.0,
            );
            felt_shape(
                c,
                &drop_pts(ctr, rr),
                pal.felt.tear,
                pal.felt.tear_edge,
                "sweat-drop",
                true,
            );
            c.out.push(
                ellipse([ctr[0] - rr * 0.35, ctr[1] - rr * 0.1], rr * 0.2, rr * 0.28),
                Style::fill(WHITE).with_opacity(0.8),
                None,
                "shine",
            );
        }
        "happy" => {
            for sx in [-1.0, 1.0] {
                let ctr = keep(
                    [
                        sx * (w * 0.5 + sz * 0.15),
                        by0 + h * 0.05 + if sx == side { 0.0 } else { sz * 0.6 },
                    ],
                    sz * 0.5,
                    sz * 0.5,
                );
                let rr = sz * if sx == side { 0.5 } else { 0.36 };
                let pts: Vec<Pt> = (0..8)
                    .map(|i| {
                        let a = f64::from(i) / 8.0 * TAU - PI / 2.0;
                        let qd = if i % 2 == 1 { rr * 0.34 } else { rr };
                        [ctr[0] + cos(a) * qd, ctr[1] + sin(a) * qd]
                    })
                    .collect();
                felt_shape(
                    c,
                    &pts,
                    pal.felt.sparkle,
                    pal.felt.sparkle_edge,
                    "sparkle",
                    false,
                );
            }
        }
        "waiting" => {
            for i in 0..3 {
                let fi = f64::from(i);
                let ctr = keep(
                    [
                        side * (w * 0.32 + fi * sz * 0.55),
                        by0 - sz * 0.15 - fi * sz * 0.22,
                    ],
                    sz * 0.3,
                    sz * 0.3,
                );
                c.out.push(
                    circle(ctr, sz * (0.13 + fi * 0.03)),
                    Style::fill(ink),
                    None,
                    "french-knot",
                );
            }
        }
        "blocked" => {
            let ctr = keep([side * w * 0.36, by0 + h * 0.16], sz * 0.7, sz * 0.7);
            let rr = sz * 0.5;
            for k in 0..4 {
                let u = dir_of(PI / 4.0 + f64::from(k) * PI / 2.0);
                let a = add(ctr, scl(rot(u, 0.62, None), rr));
                let b = add(ctr, scl(rot(u, -0.62, None), rr));
                let mid = add(ctr, scl(u, rr * 0.42));
                thread(
                    c,
                    &quad(a, mid, b, 8),
                    max(sz * 0.16, c.pxu * 1.3),
                    pal.felt.anger,
                    "anger-mark",
                );
            }
        }
        "sad" => {
            let e = if f.eyes.len() == 1 {
                f.eyes[0]
            } else {
                *f.eyes
                    .iter()
                    .find(|e| e.side == side)
                    .expect("a pair of eyes has one on each side")
            };
            let rr = max(e.r * 0.42, sz * 0.3);
            let ctr = [e.x + side * e.r * 0.25, e.y + e.r * 1.1 + rr * 1.6];
            felt_shape(
                c,
                &drop_pts(ctr, rr),
                pal.felt.tear,
                pal.felt.tear_edge,
                "tear",
                true,
            );
            c.out.push(
                ellipse([ctr[0] - rr * 0.35, ctr[1] - rr * 0.1], rr * 0.2, rr * 0.28),
                Style::fill(WHITE).with_opacity(0.8),
                None,
                "shine",
            );
        }
        _ => {
            let z = |ctr: Pt, hh: f64| {
                [
                    [ctr[0] - hh, ctr[1] - hh],
                    [ctr[0] + hh, ctr[1] - hh],
                    [ctr[0] - hh, ctr[1] + hh],
                    [ctr[0] + hh, ctr[1] + hh],
                ]
            };
            let c1 = keep([side * (w * 0.42), by0 - sz * 0.1], sz * 0.5, sz * 0.5);
            let c2 = keep(
                [c1[0] + side * sz * 0.75, c1[1] - sz * 0.7],
                sz * 0.35,
                sz * 0.35,
            );
            thread(c, &z(c1, sz * 0.36), tw, ink, "zz");
            thread(c, &z(c2, sz * 0.25), tw * 0.8, ink, "zz");
        }
    }
}
