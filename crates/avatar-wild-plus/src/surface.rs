//! The surface: hide patterns, creases and the belly, all drawn inside the
//! body's outline.

use avatar_svg::wild::math::{cos, sin};
use avatar_svg::{GradientStop, Paint, Primitive, Rgb, StrokeCap, StrokeJoin};
use std::f64::consts::{PI, TAU};

use crate::creature::SurfaceInput;
use crate::draw::{
    circle, ellipse, filled, lined, n2, path_of, plain_stroke, prim, q1, q2, q32, round_stroke,
    stroke, PathB,
};
use crate::face::Face;
use crate::genes::{Reader, Wild};
use crate::geom::{
    bbox_of, centroid, dist, js_round, lerp, max, min, smooth01, span_through, spline_closed_pts, P,
};
use crate::palette::Palette;

pub const SURFACES: [&str; 12] = [
    "plain",
    "pebbles",
    "bands",
    "tiger stripes",
    "spots",
    "patches",
    "scales",
    "dipped",
    "blaze",
    "speckles",
    "shaded",
    "chevrons",
];

/// What the surface draws, in the order the body's clip holds it.
pub struct Surface {
    /// Drawn first, unmasked.
    pub flat: Vec<Primitive>,
    /// Faded out over the face by the mask in `fade`.
    pub pattern: Vec<Primitive>,
    pub belly: Vec<Primitive>,
    pub creases: Vec<Primitive>,
    /// The fade ellipse round the face: centre and radii, in tree units.
    pub fade: [i64; 4],
    pub notes: Vec<String>,
    pub kind: usize,
    pub second: usize,
}

/// Clips a polygon to the half plane nearer `a` than `b`.
fn clip_halfplane(poly: &[P], a: P, b: P) -> Vec<P> {
    let mx = (a[0] + b[0]) / 2.0;
    let my = (a[1] + b[1]) / 2.0;
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let side = |q: P| (q[0] - mx) * dx + (q[1] - my) * dy;
    let n = poly.len();
    let mut out = Vec::new();
    for i in 0..n {
        let cur = poly[i];
        let nxt = poly[(i + 1) % n];
        let sc = side(cur);
        let sn = side(nxt);
        if sc <= 0.0 {
            out.push(cur);
        }
        if (sc < 0.0 && sn > 0.0) || (sc > 0.0 && sn < 0.0) {
            let t = sc / (sc - sn);
            out.push([
                cur[0] + (nxt[0] - cur[0]) * t,
                cur[1] + (nxt[1] - cur[1]) * t,
            ]);
        }
    }
    out
}

fn voronoi_cells(sites: &[P], bx: [f64; 4]) -> Vec<Vec<P>> {
    let base = vec![
        [bx[0], bx[1]],
        [bx[2], bx[1]],
        [bx[2], bx[3]],
        [bx[0], bx[3]],
    ];
    sites
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let mut poly = base.clone();
            for (j, &o) in sites.iter().enumerate() {
                if i != j && !poly.is_empty() {
                    poly = clip_halfplane(&poly, s, o);
                }
            }
            poly
        })
        .collect()
}

/// A Voronoi cell drawn as a rounded pebble, shrunk about its centre.
fn pebble_path(poly: &[P], inset: f64) -> PathB {
    let n = poly.len();
    let c = centroid(poly);
    let pts: Vec<P> = poly
        .iter()
        .map(|q| [c[0] + (q[0] - c[0]) * inset, c[1] + (q[1] - c[1]) * inset])
        .collect();
    let mut d = PathB::new();
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let cur = pts[i];
        let next = pts[(i + 1) % n];
        let ax = (cur[0] + prev[0]) / 2.0;
        let ay = (cur[1] + prev[1]) / 2.0;
        let bx = (cur[0] + next[0]) / 2.0;
        let by = (cur[1] + next[1]) / 2.0;
        if i == 0 {
            d.m1(ax, ay);
        } else {
            d.l1(ax, ay);
        }
        d.quad_to(ax, ay, cur[0], cur[1], bx, by);
    }
    d.z();
    d
}

/// The genes a pattern reads one after another.
struct Stream<'a> {
    r: &'a Reader<'a>,
    cur: usize,
}

impl Stream<'_> {
    fn next(&mut self) -> f64 {
        let at = if self.cur < 2400 {
            self.cur += 1;
            self.cur - 1
        } else {
            2399
        };
        self.r.unit(at)
    }
}

pub fn build_surface(
    r: &Reader,
    w: &Wild,
    pal: &mut Palette,
    f: &Face,
    c: &SurfaceInput,
) -> Surface {
    let p = &f.p;
    let rr = p.r;
    let outline = c.outline;
    let plan = c.plan;
    let [bx0, by0, bx1, by1] = c.bbox;
    let hh = by1 - by0;
    let half_w = max(120.0 - bx0, bx1 - 120.0);
    let mut notes = Vec::new();
    let mut flat: Vec<Primitive> = Vec::new();
    let mut pattern: Vec<Primitive> = Vec::new();
    let mut belly: Vec<Primitive> = Vec::new();
    let mut creases: Vec<Primitive> = Vec::new();

    // The face's box, which patterns keep clear of.
    let on_head: Vec<_> = f.eyes.iter().filter(|e| e.part.is_none()).collect();
    let mut face_top = p.mouth_y - rr * 2.4;
    for e in &on_head {
        face_top = min(face_top, e.y - e.r * 1.25);
    }
    face_top -= rr * 1.2;
    let face_bot = p.mouth_y + rr * 1.5;
    let mut face_half = rr * 1.7;
    for e in &on_head {
        face_half = max(face_half, (e.x - 120.0).abs() + e.r * 1.25);
    }
    face_half += rr * 0.7;
    let fade = [
        q1(120.0),
        q1((face_top + face_bot) / 2.0),
        q1(face_half * 1.35),
        q1((face_bot - face_top) / 2.0 * 1.35),
    ];

    // Arms on a torso take the torso's colour, which is the body's.
    pal.torso = pal.body;
    let pal = &*pal;
    // ---- creases: where one mass overlaps the next ----
    for z in c.zones {
        if !(z.role == "lobe"
            || z.role == "segment"
            || (z.role == "head" && (plan == 1 || plan == 2 || plan == 4)))
        {
            continue;
        }
        let n = z.poly.len();
        let keep: Vec<bool> = z
            .poly
            .iter()
            .map(|q| {
                q[1] > z.cy
                    && (c.depth)(q[0], q[1]) > 3.5
                    && !((q[0] - 120.0).abs() < face_half && q[1] > face_top && q[1] < face_bot)
            })
            .collect();
        let Some(start) = keep.iter().position(|k| !k) else {
            continue;
        };
        let mut run: Vec<P> = Vec::new();
        for k in 1..=n {
            let i = (start + k) % n;
            if keep[i] {
                run.push(z.poly[i]);
                continue;
            }
            if run.len() >= 4 {
                creases.push(lined(
                    path_of(&run, false),
                    round_stroke(pal.outline, 130),
                    Some(0.5),
                    "wp-crease",
                ));
            }
            run.clear();
        }
    }

    let (mut kind, mut second);
    if w.voronoi {
        kind = 1;
        second = [0, 7, 8, 10, 4, 2, 3][r.pick(1856, &[4.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0])];
    } else {
        kind = [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][r.pick(
            1856,
            &[4.0, 3.0, 3.0, 4.0, 3.0, 2.0, 3.0, 2.0, 2.0, 3.0, 2.0],
        )];
        second = 0;
    }
    // A lower body dipped in a second colour reads as trousers on a torso,
    // and bands across it as a striped jumper, so those bodies get flank
    // stripes instead.
    let under = plan == 1 || plan == 4;
    let stood_in = under && (kind == 7 || kind == 2 || second == 7 || second == 2);
    if under && (kind == 7 || kind == 2) {
        kind = 3;
    }
    if under && (second == 7 || second == 2) {
        second = 3;
    }
    let tone = [pal.mark, pal.mark_deep, pal.mark_pale][r.mod_(1857, 3) as usize];
    // The same choice kept within the body's own hue: darker or paler.
    let shade = if tone == pal.mark_pale {
        pal.mark_pale
    } else {
        pal.mark_deep
    };
    let alpha = n2(r.lin(1858, 0.6, 0.95));
    let g1 = r.unit(1859);
    let g2 = r.unit(1860);
    let g3 = u32::from(r.u8(1861));
    let mut stream = Stream { r, cur: 1888 };
    let half_at = |y: f64| match span_through(outline, y, 120.0) {
        Some(s) => min(120.0 - s[0], s[1] - 120.0),
        None => 0.0,
    };
    let fill = |shape: avatar_svg::Shape, color: Rgb, op: f64, label: &str| {
        filled(shape, color, Some(op), label)
    };

    let draw = |k: usize,
                stream: &mut Stream,
                pattern: &mut Vec<Primitive>,
                flat: &mut Vec<Primitive>| {
        if k == 1 {
            // The wild pebble hide: relaxed Voronoi cells, two shades.
            let bx = [bx0, by0, bx1, by1];
            let mut sites: Vec<P> = w
                .sites
                .iter()
                .map(|s| {
                    [
                        120.0 + s.x * (bx1 - bx0) / 2.0,
                        (by0 + by1) / 2.0 + s.y * (by1 - by0) / 2.0,
                    ]
                })
                .collect();
            for _ in 0..2 {
                let cells = voronoi_cells(&sites, bx);
                for (i, s) in sites.iter_mut().enumerate() {
                    if cells[i].len() >= 3 {
                        *s = centroid(&cells[i]);
                    }
                }
            }
            for (i, poly) in voronoi_cells(&sites, bx).iter().enumerate() {
                let shade_k = w.sites[i].shade;
                if poly.len() < 3 || shade_k == 2 {
                    continue;
                }
                let m = pal.mottle[if shade_k == 0 { 1 } else { 0 }];
                pattern.push(fill(pebble_path(poly, 0.8).shape(), m.0, m.1, "wp-pebble"));
            }
        } else if k == 2 {
            // Bands round the body: a bar on either flank that bows with the
            // body and ends rounded before it reaches the belly.
            let sag = hh * lerp(0.03, 0.085, g1);
            let n = 3 + (g2 * 4.99).floor() as usize;
            let nf = n as f64;
            let mut ys = Vec::with_capacity(n);
            for i in 0..n {
                let yc = lerp(by0 + 0.1 * hh, by1 - 0.04 * hh, (i as f64 + 0.5) / nf)
                    + (stream.next() - 0.5) * hh / nf * 0.5;
                let t = hh / nf * lerp(0.22, 0.6, stream.next());
                ys.push((yc, t));
            }
            for (i, (yc, t)) in ys.into_iter().enumerate() {
                let half = half_at(yc);
                let far = lerp(0.42, 0.62, stream.next())
                    * (if (i as u32 + g3) % 2 == 1 { 0.6 } else { 1.0 });
                if half < 6.0 {
                    continue;
                }
                let on = |u: f64| -> P {
                    let dx = half * (1.0 - far * u);
                    [dx, yc - sag * (dx / half) * (dx / half)]
                };
                let mid = on(0.5);
                let end = on(1.0);
                for m in [-1.0, 1.0] {
                    let x = |v: f64| 120.0 + m * v;
                    let mut d = PathB::new();
                    d.m1(x(half + 6.0), yc - sag - 0.5 * t);
                    d.cubic_to(
                        [x(mid[0]), mid[1] - 0.46 * t],
                        [x(end[0]), end[1] - 0.4 * t],
                        [x(end[0]), end[1]],
                    );
                    d.cubic_to(
                        [x(end[0]), end[1] + 0.4 * t],
                        [x(mid[0]), mid[1] + 0.46 * t],
                        [x(half + 6.0), yc - sag + 0.5 * t],
                    );
                    d.z();
                    pattern.push(fill(d.shape(), shade, alpha, "wp-band"));
                }
            }
        } else if k == 3 {
            // Stripes that run in from the flanks and down from the crown.
            let stripe_tone = if stood_in || c.limbs_in_skin {
                shade
            } else {
                tone
            };
            let n = 3 + (g2 * 3.99).floor() as usize;
            let nf = n as f64;
            let mut above = 0.0;
            for i in 0..n {
                let y = by0
                    + hh * (0.14 + 0.78 * (i as f64 + 0.5) / nf)
                    + (stream.next() - 0.5) * hh * 0.05;
                let mut share = min(lerp(0.3, 0.8, stream.next()), 0.66);
                if share > 0.5 && above > 0.5 {
                    share *= 0.6;
                }
                let half = half_at(y);
                let len = half * share;
                let wd = hh * lerp(0.022, 0.055, stream.next());
                let bend = hh * lerp(-0.05, 0.06, g1);
                if half < 6.0 {
                    continue;
                }
                above = share;
                for s in [-1.0, 1.0] {
                    let xe = 120.0 + s * (half + 6.0);
                    let xt = 120.0 + s * (half - len);
                    let xm = 120.0 + s * (half - len * 0.45);
                    let mut d = PathB::new();
                    d.m1(xe, y - wd);
                    d.quad_to(xe, y - wd, xm, y - wd * 0.5 + bend * 0.4, xt, y + bend);
                    d.quad_to(xt, y + bend, xm, y + wd * 0.9 + bend * 0.5, xe, y + wd);
                    d.z();
                    pattern.push(fill(d.shape(), stripe_tone, alpha, "wp-stripe"));
                }
            }
            let crown = g3 % 4;
            let top = f.hb[1];
            for j in 0..crown {
                let off = f64::from(j) - (f64::from(crown) - 1.0) / 2.0;
                let x = 120.0 + off * half_w * 0.22;
                let len = hh * lerp(0.1, 0.2, stream.next()) * (1.0 - 0.25 * off.abs());
                let wd = hh * 0.022;
                let mut d = PathB::new();
                d.m1(x - wd, top - 8.0);
                d.quad_to(
                    x - wd,
                    top - 8.0,
                    x - wd * 0.7,
                    top + len * 0.55,
                    x,
                    top + len,
                );
                d.quad_to(
                    x,
                    top + len,
                    x + wd * 0.7,
                    top + len * 0.55,
                    x + wd,
                    top - 8.0,
                );
                d.z();
                flat.push(fill(d.shape(), stripe_tone, alpha, "wp-stripe"));
            }
        } else if k == 4 {
            let n = 5 + (g2 * 8.99).floor() as usize;
            let ringed = g3 & 1 == 1;
            for _ in 0..n {
                let u = lerp(0.04, 0.98, stream.next());
                let v = lerp(0.03, 0.97, stream.next());
                let size = stream.next();
                let rad = hh * lerp(0.016, 0.066, size * size);
                let asp = lerp(0.75, 1.3, g1);
                for s in [-1.0, 1.0] {
                    let (cx, cy) = (q1(120.0 + s * u * half_w), q1(by0 + v * hh));
                    pattern.push(fill(
                        ellipse(cx, cy, q2(rad * asp), q2(rad)),
                        tone,
                        alpha,
                        "wp-spot",
                    ));
                    if ringed && rad > hh * 0.03 {
                        pattern.push(fill(
                            ellipse(cx, cy, q2(rad * asp * 0.5), q2(rad * 0.5)),
                            pal.body,
                            alpha,
                            "wp-spot",
                        ));
                    }
                }
            }
        } else if k == 5 {
            // A few large blotches; none covers an eye.
            let n = 2 + (g2 * 2.99).floor() as usize;
            let lopsided = g3 & 1 == 1;
            let sided = if !lopsided && tone == pal.mark {
                shade
            } else {
                tone
            };
            let blot_tone = if pal.recipe == 3 && sided == pal.mark_deep {
                pal.mark_pale
            } else {
                sided
            };
            for _ in 0..n {
                let u = lerp(0.1, 0.95, stream.next());
                let v = lerp(0.08, 0.95, stream.next());
                let s0 = if stream.next() < 0.5 { -1.0 } else { 1.0 };
                let cx = 120.0 + s0 * u * half_w;
                let cy = by0 + v * hh;
                let mut rad = hh * lerp(0.1, 0.24, stream.next());
                for e in &on_head {
                    for m in [1.0, -1.0] {
                        rad = min(
                            rad,
                            dist(if m > 0.0 { cx } else { 240.0 - cx }, cy, e.x, e.y)
                                - e.r * 1.5
                                - 2.0,
                        );
                    }
                }
                rad = min(
                    rad,
                    min(
                        dist(cx, cy, 120.0, p.mouth_y) - rr * 1.2,
                        dist(240.0 - cx, cy, 120.0, p.mouth_y) - rr * 1.2,
                    ),
                );
                let lumps: Vec<f64> = (0..7).map(|_| lerp(0.7, 1.25, stream.next())).collect();
                if rad < hh * 0.05 {
                    continue;
                }
                let ctrl: Vec<P> = lumps
                    .iter()
                    .enumerate()
                    .map(|(j, f)| {
                        let a = TAU * j as f64 / 7.0;
                        [cx + cos(a) * rad * f, cy + sin(a) * rad * f]
                    })
                    .collect();
                let blot = spline_closed_pts(&ctrl, 5);
                flat.push(fill(path_of(&blot, true), blot_tone, alpha, "wp-patch"));
                if !lopsided {
                    let mirrored: Vec<P> = blot.iter().map(|q| [240.0 - q[0], q[1]]).collect();
                    flat.push(fill(path_of(&mirrored, true), blot_tone, alpha, "wp-patch"));
                }
            }
        } else if k == 6 {
            // Rows of scales; the genome also picks which are filled in.
            let dy = hh * lerp(0.065, 0.1, g1);
            let wd = dy * lerp(1.5, 2.2, g2);
            let start = if g3 & 1 == 1 {
                face_bot - rr
            } else {
                by0 + dy * 0.4
            };
            let mut rim = PathB::new();
            let mut solid = PathB::new();
            let (mut bit, mut word) = (0u32, 0u32);
            let mut row = 0u32;
            let mut y = start;
            while y < by1 {
                let half = half_at(y + dy * 0.5);
                if half >= 2.0 {
                    let mut x = if row % 2 == 1 { wd / 2.0 } else { 0.0 };
                    while x - wd / 2.0 < half {
                        if bit % 8 == 0 {
                            word = js_round(stream.next() * 255.0) as u32;
                        }
                        let on = (word >> (bit % 8)) & 1;
                        bit += 1;
                        let sides: &[f64] = if x == 0.0 { &[1.0] } else { &[1.0, -1.0] };
                        for &s in sides {
                            let xc = 120.0 + s * x;
                            rim.m1(xc - wd / 2.0, y);
                            rim.quad_to(xc - wd / 2.0, y, xc, y + dy * 1.5, xc + wd / 2.0, y);
                            if on == 1 && bit % 3 == 0 {
                                solid.m1(xc - wd / 2.0, y);
                                solid.quad_to(xc - wd / 2.0, y, xc, y + dy * 1.5, xc + wd / 2.0, y);
                                solid.z();
                            }
                        }
                        x += wd;
                    }
                }
                row += 1;
                y += dy;
            }
            if !solid.is_empty() {
                pattern.push(fill(solid.shape(), tone, n2(alpha * 0.7), "wp-scale-fill"));
            }
            if !rim.is_empty() {
                pattern.push(lined(
                    rim.shape(),
                    stroke(
                        if pal.dark { pal.light } else { pal.outline },
                        120,
                        StrokeCap::Round,
                        StrokeJoin::Miter,
                    ),
                    Some(0.45),
                    "wp-scales",
                ));
            }
        } else if k == 7 {
            // The lower body dipped in a second colour.
            let mut yb = lerp(max(face_bot + 2.0, by0 + 0.45 * hh), by1 - 0.12 * hh, g1);
            if yb < face_bot + 2.0 {
                yb = max(by1 - 0.2 * hh, face_bot + 1.0);
            }
            let edge = g3 % 4;
            let teeth = 2 * (2 + (g2 * 3.99).floor() as usize) + 1;
            let wd = (2.0 * half_w + 12.0) / teeth as f64;
            let a = hh * 0.035;
            let x0 = 120.0 - half_w - 6.0;
            let mut d = PathB::new();
            d.m1(x0, yb);
            for i in 0..teeth {
                let xa = x0 + i as f64 * wd;
                let xb = xa + wd;
                let up = if i % 2 == 0 { -1.0 } else { 1.0 };
                match edge {
                    0 => d.l1(xb, yb),
                    1 => d.quad_to(xa, yb, (xa + xb) / 2.0, yb + up * a * 2.0, xb, yb),
                    2 => {
                        d.l1((xa + xb) / 2.0, yb + up * a);
                        d.l1(xb, yb);
                    }
                    _ => d.quad_to(xa, yb, (xa + xb) / 2.0, yb - a * 2.4, xb, yb),
                }
            }
            d.l1(x0 + teeth as f64 * wd, by1 + 8.0);
            d.l1(x0, by1 + 8.0);
            d.z();
            let color =
                [pal.part, pal.mark_deep, pal.mark_pale, pal.part2][((g3 >> 2) % 4) as usize];
            flat.push(filled(d.shape(), color, None, "wp-dip"));
        } else if k == 8 {
            // A blaze: a stripe from the crown down between the eyes.
            let top = f.hb[1];
            let mid: Vec<_> = on_head
                .iter()
                .filter(|e| (e.x - 120.0).abs() < 1.0)
                .collect();
            let end = if !mid.is_empty() {
                let mut m = f64::INFINITY;
                for e in &mid {
                    m = min(m, e.y - e.r * 1.4);
                }
                m - rr * 0.3
            } else {
                p.mouth_y - rr * (if f.nose != 0 { 1.25 } else { 0.75 })
            };
            let gap = if on_head.len() > mid.len() {
                let mut m = f64::INFINITY;
                for e in on_head.iter().filter(|e| (e.x - 120.0).abs() >= 1.0) {
                    m = min(m, (e.x - 120.0).abs() - e.r * 1.3);
                }
                m
            } else {
                rr * 2.0
            };
            let w_top = min(
                half_at(top + (end - top) * 0.25) * 0.6,
                rr * lerp(1.0, 2.4, g1),
            );
            let w_mid = max(rr * 0.22, min(gap * 0.72, w_top));
            if end - top > rr * 1.5 {
                let y_mid = lerp(top, end, 0.6);
                let mut d = PathB::new();
                d.m1(120.0 - w_top, top - 8.0);
                d.quad_to(
                    120.0 - w_top,
                    top - 8.0,
                    120.0 - w_top,
                    lerp(top, y_mid, 0.6),
                    120.0 - w_mid,
                    y_mid,
                );
                d.quad_to(120.0 - w_mid, y_mid, 120.0 - w_mid, end, 120.0, end);
                d.quad_to(120.0, end, 120.0 + w_mid, end, 120.0 + w_mid, y_mid);
                d.quad_to(
                    120.0 + w_mid,
                    y_mid,
                    120.0 + w_top,
                    lerp(top, y_mid, 0.6),
                    120.0 + w_top,
                    top - 8.0,
                );
                d.z();
                let color = [pal.mark_pale, pal.mark_pale, pal.mark, pal.mark_deep]
                    [((g3 >> 2) % 4) as usize];
                flat.push(filled(d.shape(), color, None, "wp-blaze"));
            }
        } else if k == 9 {
            let n = 20 + (g2 * 27.99).floor() as usize;
            let top_heavy = g3 & 1 == 1;
            for _ in 0..n {
                let u = lerp(0.03, 0.98, stream.next());
                let t = stream.next();
                let v = if top_heavy { t * t } else { 1.0 - t * t };
                let rad = lerp(0.9, 2.3, stream.next());
                for s in [-1.0, 1.0] {
                    pattern.push(fill(
                        circle(
                            q1(120.0 + s * u * half_w),
                            q1(by0 + lerp(0.03, 0.97, v) * hh),
                            q2(rad),
                        ),
                        tone,
                        alpha,
                        "wp-speckle",
                    ));
                }
            }
        } else if k == 10 {
            // One radial shade over the body.
            let mode = g3 % 4;
            let size = max(hh, 2.0 * half_w);
            let cy = match mode {
                1 => by1,
                2 => by0,
                _ => by0 + hh * lerp(0.35, 0.6, g1),
            };
            let rad = match mode {
                0 => size * 0.62,
                3 => size * lerp(0.3, 0.45, g2),
                _ => hh * lerp(0.6, 0.95, g2),
            };
            let col = match mode {
                0 => pal.mark_deep,
                3 => pal.mark_pale,
                _ => tone,
            };
            let stops: &[(f64, f64)] = if mode == 0 {
                &[(0.45, 0.0), (1.0, 0.75)]
            } else {
                &[(0.0, 0.85), (0.35, 0.7), (1.0, 0.0)]
            };
            let stops = stops
                .iter()
                .map(|&(o, a)| GradientStop::with_opacity(q32(o), col, q32(a)))
                .collect();
            let paint = Paint::radial_gradient_at(q1(120.0), q1(cy), q1(rad), None, stops);
            flat.push(prim(
                avatar_svg::Shape::Rect {
                    x: q1(bx0 - 8.0),
                    y: q1(by0 - 8.0),
                    w: q1(bx1 - bx0 + 16.0),
                    h: q1(hh + 16.0),
                },
                paint,
                None,
                None,
                "wp-shade",
            ));
        } else if k == 11 {
            // Chevrons down the front, below the face if there is room.
            let below = by1 - 0.06 * hh - (face_bot + 4.0) > hh * 0.14;
            let ya = if below {
                face_bot + 4.0 + hh * 0.05
            } else {
                f.hb[1] + hh * 0.07
            };
            let yz = if below { by1 - 0.05 * hh } else { face_top };
            let n = max(
                1.0,
                min(2.0 + (g2 * 3.99).floor(), ((yz - ya) / (hh * 0.07)).floor()),
            ) as usize;
            let t = hh * lerp(0.02, 0.042, g1);
            let depth = hh * lerp(0.035, 0.08, stream.next());
            let chevron_tone = if under || plan == 2 { shade } else { tone };
            for i in 0..n {
                let y = lerp(ya, yz, (i as f64 + 0.6) / n as f64);
                let wd = half_at(y - depth * 0.5) * lerp(0.28, 0.5, stream.next());
                if wd < 4.0 || yz - ya < hh * 0.05 {
                    continue;
                }
                let mut d = PathB::new();
                d.m1(120.0 - wd, y - depth);
                d.lq(12_000, q1(y));
                d.l1(120.0 + wd, y - depth);
                flat.push(lined(
                    d.shape(),
                    round_stroke(chevron_tone, q2(t)),
                    Some(alpha),
                    "wp-chevron",
                ));
            }
        }
    };
    for k in [kind, second] {
        if k == 0 {
            continue;
        }
        draw(k, &mut stream, &mut pattern, &mut flat);
        notes.push(SURFACES[k].to_owned());
    }

    // ---- skin marks: small flecks scattered over the hide, each placed by
    // its own three genes and not mirrored, as birthmarks are not ----
    let fleck = r.pick(2636, &[2.0, 3.0, 2.0, 2.0, 2.0]);
    if fleck != 0 {
        let count = 14 + r.mod_(2637, 23) as usize;
        let pen = if pal.dark { pal.light } else { pal.outline };
        let mut d = PathB::new();
        let mut dots = Vec::new();
        for i in 0..count {
            let x = lerp(bx0, bx1, r.unit(2640 + 3 * i));
            let y = lerp(by0, by1, r.unit(2641 + 3 * i));
            let g = u32::from(r.u8(2642 + 3 * i));
            let size = 1.6 + f64::from(g & 15) / 15.0 * 2.6;
            let a = f64::from(g >> 4) / 16.0 * PI;
            let ux = cos(a) * size;
            let uy = sin(a) * size;
            match fleck {
                1 => dots.push(filled(
                    circle(q1(x), q1(y), q2(0.7 + f64::from(g & 15) / 15.0 * 1.1)),
                    pen,
                    Some(0.42),
                    "wp-fleck",
                )),
                2 => {
                    d.m1(x - ux, y - uy);
                    d.l1(x + ux, y + uy);
                }
                3 => {
                    d.m1(x - ux, y - uy);
                    d.l1(x + uy * 0.6, y - ux * 0.6);
                    d.l1(x + ux, y + uy);
                }
                _ => dots.push(lined(
                    circle(q1(x), q1(y), q2(size * 0.7)),
                    plain_stroke(pen, 90),
                    Some(0.42),
                    "wp-fleck",
                )),
            }
        }
        pattern.extend(dots);
        if !d.is_empty() {
            pattern.push(lined(
                d.shape(),
                round_stroke(pen, 100),
                Some(0.42),
                "wp-fleck",
            ));
        }
    }

    // ---- belly ----
    let belly_kind = if plan == 5 {
        1
    } else {
        r.pick(2400, &[5.0, 2.0, 2.0, 2.0, 2.0])
    };
    if belly_kind != 1 {
        // The wild belly patch: below the face, as wide as a share of the
        // body at each height, tapering to a point at the top.
        let region: &[P] = if plan == 4 || plan == 6 {
            c.head
        } else {
            outline
        };
        let rb = bbox_of(region);
        let top_y = max(p.eye_y + rr * 3.4, p.mouth_y + rr * 1.5);
        let bottom_full = rb[3] - (rb[3] - top_y) * 0.06;
        // Under a head, a torso or a segment covered edge to edge by the
        // belly colour reads as a dress, so there the patch stays narrow.
        let stacked = plan == 1 || plan == 2;
        let scale = if stacked {
            0.6
        } else if belly_kind == 3 {
            0.95
        } else {
            0.72
        };
        let mut bottom = top_y;
        if bottom_full > top_y {
            for i in 0..=80 {
                let y = lerp(top_y, bottom_full, f64::from(i) / 80.0);
                if span_through(region, y, 120.0).is_none() {
                    break;
                }
                bottom = y;
            }
        }
        let mut left = Vec::new();
        let mut right = Vec::new();
        // Six genes swell or pinch the patch down its length.
        let swell: Vec<f64> = (0..6).map(|i| r.sgn(2402 + i)).collect();
        if bottom > top_y {
            for i in 0..=40 {
                let t = f64::from(i) / 40.0;
                let y = lerp(top_y, bottom, t);
                let Some(span) = span_through(region, y, 120.0) else {
                    break;
                };
                let u = t * 5.0;
                let k = min(4.0, u.floor()) as usize;
                let hw = min(120.0 - span[0], span[1] - 120.0)
                    * min(
                        0.97,
                        scale * (1.0 + 0.13 * lerp(swell[k], swell[k + 1], smooth01(u - k as f64))),
                    )
                    * t.sqrt()
                    * (1.0 - 0.3 * t * t * t);
                left.push([120.0 - hw, y]);
                right.push([120.0 + hw, y]);
            }
        }
        let patch: Vec<P> = if left.len() >= 3 {
            let mut v = left.clone();
            v.extend(right.iter().rev());
            v
        } else {
            Vec::new()
        };
        let pb = if patch.is_empty() {
            [0.0; 4]
        } else {
            bbox_of(&patch)
        };
        if !patch.is_empty()
            && pb[2] - pb[0] >= (rb[2] - rb[0]) * 0.1
            && pb[3] - pb[1] >= (rb[3] - rb[1]) * 0.08
        {
            if belly_kind == 4 {
                // A round spot on the belly in place of the patch.
                belly.push(filled(
                    ellipse(
                        q1(120.0),
                        q1(lerp(pb[1], pb[3], 0.55)),
                        q1((pb[2] - pb[0]) * 0.36),
                        q1((pb[3] - pb[1]) * 0.36),
                    ),
                    pal.belly,
                    None,
                    "wp-belly",
                ));
            } else {
                belly.push(filled(path_of(&patch, true), pal.belly, None, "wp-belly"));
                if belly_kind == 2 {
                    // Plates: creases across the patch.
                    let gap = max(5.0, (pb[3] - pb[1]) / (3.0 + f64::from(r.one(2408, 4))));
                    let mut d = PathB::new();
                    let mut y = pb[1] + gap;
                    while y < pb[3] - gap * 0.3 {
                        if let Some(span) = span_through(&patch, y, 120.0) {
                            d.m1(span[0], y);
                            d.quad_to(span[0], y, 120.0, y + gap * 0.3, span[1], y);
                        }
                        y += gap;
                    }
                    if !d.is_empty() {
                        belly.push(lined(
                            d.shape(),
                            plain_stroke(pal.outline, 120),
                            Some(0.4),
                            "wp-belly-plates",
                        ));
                    }
                }
            }
            if belly_kind != 0 && !(belly_kind == 3 && stacked) {
                notes.push(
                    ["", "", "plated belly", "wide belly", "belly spot"][belly_kind].to_owned(),
                );
            }
        }
    }
    Surface {
        flat,
        pattern,
        belly,
        creases,
        fade,
        notes,
        kind,
        second,
    }
}
