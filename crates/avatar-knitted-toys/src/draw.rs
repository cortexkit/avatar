//! Drawing a built toy in one mood.
//!
//! The toy is drawn in toy units inside one group that scales it into the
//! 200-unit frame. Line widths are set in toy units, with a floor in screen
//! pixels for icons: `pxu` is one screen pixel in toy units. Icons leave out
//! the stitches and thicken the embroidery so the face still reads.

use avatar_svg::{Primitive, Role, Shape, StrokeCap, StrokeJoin, TransformOp, Q32};

use crate::build::{Part, Seam, Toy, MOODS};
use crate::emit::{
    circle, compound, ellipse, poly, q, seg, shade, soft_shadow, stitches, Emitter, Style,
};
use crate::geom::{
    add, clamp, closed_normals, cos, dir_of, dist, js_round, lerp, max, min, mixp, perp, resample,
    rot, scl, sdist, signed_area, sin, sub, unit, Pt, PI, TAU,
};
use crate::knit::band_poly;
use crate::palette::{hsl, rgb, Yarn};
use crate::parts::HAND;
use crate::piece::piece_r;
use crate::pose::FRAME;

/// What one drawing needs besides the toy: the mood, the size it is shown
/// at and what that size keeps.
pub struct Ctx<'a> {
    pub g: &'a Toy,
    pub mood: usize,
    pub px: f64,
    /// 2 at 100 px and up (stitches), 1 from 56 px (rows), 0 below.
    pub detail: u8,
    /// One screen pixel in toy units.
    pub pxu: f64,
    pub out: Emitter,
}

impl Ctx<'_> {
    pub fn mood_name(&self) -> &'static str {
        MOODS[self.mood]
    }

    /// Draws what `f` draws inside a group with these transform operations.
    pub fn group(&mut self, ops: Vec<TransformOp>, f: impl FnOnce(&mut Self)) {
        let depth = self.out.enter(ops);
        f(self);
        self.out.leave(depth);
    }
}

pub fn make_ctx(g: &Toy, mood: usize, px: f64) -> Ctx<'_> {
    let detail = if px >= 100.0 {
        2
    } else if px >= 56.0 {
        1
    } else {
        0
    };
    Ctx {
        g,
        mood,
        px,
        detail,
        pxu: FRAME / px / g.fit.s,
        out: Emitter::default(),
    }
}

/// A run of rows knitted in one yarn other than the part's own.
struct YarnRun {
    yarn: Yarn,
    r0: usize,
    r1: usize,
}

fn runs_of(part: &Part) -> Vec<YarnRun> {
    let mut out: Vec<YarnRun> = Vec::new();
    for (r, y) in part.yarns.iter().enumerate() {
        if y == part.yarn() {
            continue;
        }
        match out.last_mut() {
            Some(last) if &last.yarn == y && last.r1 + 1 == r => last.r1 = r,
            _ => out.push(YarnRun {
                yarn: y.clone(),
                r0: r,
                r1: r,
            }),
        }
    }
    out
}

pub fn rim_width(c: &Ctx) -> f64 {
    max(
        0.7 / c.g.fit.s,
        c.pxu * if c.detail == 2 { 0.65 } else { 0.85 },
    )
}

/// The clip a knitted part's colour changes, stitches and rows are drawn
/// in: its fabric edge at full size, its smooth outline smaller.
fn knit_clip(c: &Ctx, part: &Part) -> Shape {
    poly(
        if c.detail == 2 {
            &part.edge
        } else {
            &part.outline
        },
        true,
    )
}

fn knit_svg(c: &mut Ctx, part: &Part) {
    let g = c.g;
    let det = c.detail;
    let clip = knit_clip(c, part);
    let y = part.yarn();
    let tone = |yy: &Yarn| match det {
        2 => yy.gap,
        1 => yy.mid,
        _ => yy.hex,
    };
    let label = part.cls;
    c.out.push(clip.clone(), Style::fill(tone(y)), None, "yarn");
    // Colour changes: one path per yarn, in the order the yarns first come.
    let mut by_y: Vec<(Yarn, Vec<Vec<Pt>>)> = Vec::new();
    for run in runs_of(part) {
        let band = band_poly(part, run.r0, run.r1);
        match by_y.iter_mut().find(|(yy, _)| *yy == run.yarn) {
            Some((_, list)) => list.push(band),
            None => by_y.push((run.yarn, vec![band])),
        }
    }
    for (yy, bands) in by_y {
        if let Some(shape) = compound(bands.into_iter().map(|b| (b, true)).collect()) {
            c.out.push(
                shape,
                Style::fill(tone(&yy)),
                Some(&clip),
                "yarn colour-change",
            );
        }
    }
    if det == 2 {
        // Each stitch is a V: its shaded leg, then its lit leg.
        let mut groups: Vec<(Yarn, Vec<_>, Vec<_>)> = Vec::new();
        for st in &part.stitches {
            let yy = part.yarns.get(st.row).unwrap_or(y);
            let at = match groups.iter().position(|(k, _, _)| k == yy) {
                Some(i) => i,
                None => {
                    groups.push((yy.clone(), Vec::new(), Vec::new()));
                    groups.len() - 1
                }
            };
            groups[at].1.push(seg(st.l, st.b));
            groups[at].2.push(seg(st.r, st.b));
        }
        // All of a part's stitches sit in one group clipped to its edge.
        let width = g.rh * 0.5;
        let effects = c.out.group_clip(&clip);
        for (yy, lit, mid) in groups {
            if let Some(shape) = stitches(mid) {
                c.out.push_in(
                    shape,
                    Style::line(yy.mid, width, StrokeCap::Round, StrokeJoin::Miter),
                    &effects,
                    "stitches",
                );
            }
            if let Some(shape) = stitches(lit) {
                c.out.push_in(
                    shape,
                    Style::line(yy.lit, width, StrokeCap::Round, StrokeJoin::Miter),
                    &effects,
                    "stitches",
                );
            }
        }
    } else if det == 1 {
        // Mid-size icons show the rows as grooves instead of stitches.
        let runs = part.rows[1..]
            .iter()
            .map(|row| {
                let pts = (0..=14)
                    .map(|i| {
                        row.at(if row.closed {
                            i as f64 / 14.0 * TAU
                        } else {
                            -PI / 2.0 + i as f64 / 14.0 * PI
                        })
                    })
                    .collect();
                (pts, false)
            })
            .collect();
        if let Some(shape) = compound(runs) {
            c.out.push(
                shape,
                Style::line(
                    y.gap,
                    max(g.rh * 0.32, c.pxu * 0.55),
                    StrokeCap::Butt,
                    StrokeJoin::Miter,
                )
                .with_opacity(0.55),
                Some(&clip),
                "stitches rows",
            );
        }
    }
    let shade_style = Style::paint(shade())
        .with_stroke(y.deep, rim_width(c))
        .with_join(if det == 2 {
            StrokeJoin::Round
        } else {
            StrokeJoin::Miter
        });
    c.out.push(clip, shade_style, None, label);
    seam_svg(c, part);
}

/// Where a part is sewn on: the edge of its open end, oversewn with short
/// stitches across it.
fn seam_svg(c: &mut Ctx, part: &Part) {
    if part.seam == Seam::None || c.detail == 0 {
        return;
    }
    let g = c.g;
    let step = max(2.6 / g.fit.s, g.rh * 0.62);
    let half = g.rh * 0.38;
    let mut ds = Vec::new();
    match (part.seam, &part.tube) {
        (Seam::Tube, Some(tb)) => {
            let a = add(tb.spine[0], scl(tb.nrm[0], tb.ws[0]));
            let b = sub(tb.spine[0], scl(tb.nrm[0], tb.ws[0]));
            let n = max(2.0, js_round(dist(a, b) / step)) as usize;
            for i in 0..=n {
                let qp = mixp(a, b, i as f64 / n as f64);
                let t = tb.tan[0];
                ds.push(seg(
                    sub(qp, scl(t, half * 0.4)),
                    add(qp, scl(t, half * 1.3)),
                ));
            }
        }
        (Seam::Ring, _) => {
            let rr = ((part.rx * part.rx + part.ry * part.ry) / 2.0).sqrt();
            let n = max(10.0, js_round(TAU * rr / step)) as usize;
            for i in 0..n {
                let a = i as f64 / n as f64 * TAU;
                let (co, si) = (cos(a), sin(a));
                let p = [part.c[0] + part.rx * co, part.c[1] + part.ry * si];
                let nn = unit([co / part.rx, si / part.ry]);
                ds.push(seg(
                    sub(p, scl(nn, half * 1.1)),
                    add(p, scl(nn, half * 0.7)),
                ));
            }
        }
        _ => {}
    }
    if let Some(shape) = stitches(ds) {
        c.out.push(
            shape,
            Style::line(
                part.yarn().deep,
                max(g.rh * 0.2, c.pxu * 0.5),
                StrokeCap::Round,
                StrokeJoin::Miter,
            ),
            None,
            "seam",
        );
    }
}

/// Felt: flat, cut clean, with a running stitch round its edge at full
/// size.
fn felt_svg(c: &mut Ctx, part: &Part) {
    let g = c.g;
    let fill = part.felt.expect("a felt part has its felt");
    let edge = part.felt_edge.expect("a felt part has its edge");
    let width = max(0.5 / g.fit.s, c.pxu * 0.5);
    c.out.push(
        poly(&part.outline, true),
        Style::fill(fill).with_stroke(edge, width),
        None,
        part.cls,
    );
    if c.detail == 2 && (part.stitched || part.cross_stitch) {
        let colour = if g.pal.dark {
            g.pal.thread
        } else {
            part.thread_on.unwrap_or(edge)
        };
        running_stitch(c, &part.outline, colour, 1.5 / g.fit.s);
    }
    if c.detail == 2 && part.cross_stitch {
        let p = &part.outline;
        let sum = p.iter().fold([0.0, 0.0], |a, &q| add(a, q));
        let centre = scl(sum, 1.0 / p.len() as f64);
        let mut ds = Vec::new();
        for &pt in p {
            let qp = mixp(pt, centre, 0.16);
            let u = scl(unit(sub(centre, pt)), 2.2 / g.fit.s);
            let v = perp(u);
            ds.push(seg(sub(qp, add(u, v)), add(qp, add(u, v))));
            ds.push(seg(sub(qp, sub(u, v)), add(qp, sub(u, v))));
        }
        if let Some(shape) = stitches(ds) {
            c.out.push(
                shape,
                Style::line(
                    g.pal.thread,
                    0.55 / g.fit.s,
                    StrokeCap::Round,
                    StrokeJoin::Miter,
                ),
                None,
                "cross-stitch",
            );
        }
    }
    if c.detail >= 1 && part.vein {
        if let Some(tb) = &part.tube {
            let n = tb.spine.len();
            if n >= 3 {
                let vein = &tb.spine[1..n - 2];
                if vein.len() >= 2 {
                    c.out.push(
                        poly(vein, false),
                        Style::line(edge, width, StrokeCap::Round, StrokeJoin::Miter),
                        None,
                        "vein",
                    );
                }
            }
        }
    }
}

/// A running stitch just inside a felt shape's edge.
pub fn running_stitch(c: &mut Ctx, p: &[Pt], colour: avatar_svg::Rgb, inset: f64) {
    let g = c.g;
    let n0 = closed_normals(p);
    let sgn = if signed_area(p) < 0.0 { -1.0 } else { 1.0 };
    let inner: Vec<Pt> = p
        .iter()
        .zip(&n0)
        .map(|(&q, &n)| sub(q, scl(n, inset * sgn)))
        .collect();
    let st = 2.4 / g.fit.s;
    let gap = 1.5 / g.fit.s;
    let r = resample(&inner, st + gap, true);
    let ds = (0..r.len())
        .map(|i| {
            let a = r[i];
            let b = r[(i + 1) % r.len()];
            seg(a, mixp(a, b, st / (st + gap)))
        })
        .collect();
    if let Some(shape) = stitches(ds) {
        c.out.push(
            shape,
            Style::line(colour, 0.55 / g.fit.s, StrokeCap::Round, StrokeJoin::Miter),
            None,
            "running-stitch",
        );
    }
}

/// A pompom: a ball of cut yarn, a soft round with short tufts round and
/// over it.
fn pompom_svg(c: &mut Ctx, part: &Part) {
    let g = c.g;
    let y = part.yarn();
    let (centre, r) = (part.c, part.r);
    let mut rnd = crate::dna::Stream::new(crate::dna::hash_str(&format!("{}fluff", part.id), HAND));
    if c.detail == 0 {
        c.out
            .push(circle(centre, r), Style::fill(y.hex), None, part.cls);
        let rim = rim_width(c);
        c.out.push(
            circle(centre, r),
            Style::paint(shade()).with_stroke(y.deep, rim),
            None,
            part.cls,
        );
        return;
    }
    // A fringe round the edge, so the silhouette is fuzzy, then tufts over
    // the ball.
    let n = js_round(clamp(r * 1.6, 16.0, 60.0)) as usize;
    let mut edge = Vec::with_capacity(n);
    for i in 0..n {
        let a = i as f64 / n as f64 * TAU + rnd.draw() * 0.2;
        let rr = r * (0.98 + rnd.draw() * 0.14);
        edge.push([centre[0] + cos(a) * rr, centre[1] + sin(a) * rr]);
    }
    c.out
        .push(poly(&edge, true), Style::fill(y.gap), None, part.cls);
    let mut mid = Vec::new();
    let mut lit = Vec::new();
    let count = js_round(clamp(r * r * 0.22, 20.0, 160.0)) as usize;
    for _ in 0..count {
        let a = rnd.draw() * TAU;
        let rr = rnd.draw().sqrt() * r * 0.92;
        let p = [centre[0] + cos(a) * rr, centre[1] + sin(a) * rr];
        let da = a + (rnd.draw() - 0.5) * 1.6;
        let l = r * (0.12 + rnd.draw() * 0.14);
        let s = seg(p, add(p, scl(dir_of(da), l)));
        if rnd.draw() < 0.5 {
            mid.push(s);
        } else {
            lit.push(s);
        }
    }
    let w = max(r * 0.09, g.rh * 0.35);
    for (list, colour) in [(mid, y.mid), (lit, y.lit)] {
        if let Some(shape) = stitches(list) {
            c.out.push(
                shape,
                Style::line(colour, w, StrokeCap::Round, StrokeJoin::Miter),
                None,
                "tufts",
            );
        }
    }
    c.out
        .push(circle(centre, r), Style::paint(shade()), None, "shade");
}

/// Yarn drawn as itself: a tuft's loops, an antenna's or a sprout's stalk.
fn yarn_line_svg(c: &mut Ctx, part: &Part, lines: &[Vec<Pt>], width: f64) {
    let y = part.yarn();
    let runs = || lines.iter().map(|l| (l.clone(), false)).collect();
    let w = max(width, c.pxu * 1.1);
    let round = |colour, width| Style::line(colour, width, StrokeCap::Round, StrokeJoin::Round);
    if let Some(shape) = compound(runs()) {
        c.out
            .push(shape.clone(), round(y.deep, w * 1.25), None, part.cls);
        c.out.push(shape.clone(), round(y.mid, w), None, part.cls);
        if c.detail == 2 {
            c.out.push(
                shape,
                round(y.lit, w * 0.35).with_opacity(0.8),
                None,
                part.cls,
            );
        }
    }
}

pub fn part_svg(c: &mut Ctx, part: &Part) {
    match part.kind {
        "piece" | "tube" | "ring" => knit_svg(c, part),
        "felt" => felt_svg(c, part),
        "pompom" => pompom_svg(c, part),
        "loops" => yarn_line_svg(c, part, &part.loops, part.width),
        "stalk" => {
            let tb = part.tube.as_ref().expect("a stalk has its tube");
            yarn_line_svg(c, part, std::slice::from_ref(&tb.spine), tb.ws[0] * 2.0);
        }
        _ => {}
    }
}

/// The shadow a sewn-on round casts on what it is sewn to.
pub fn drop_shadow(c: &mut Ctx, part: &Part) {
    if part.kind != "ring" && part.kind != "tube" {
        return;
    }
    let o = max(1.1 / c.g.fit.s, c.pxu * 0.6);
    let pts: Vec<Pt> = part
        .outline
        .iter()
        .map(|p| [p[0] + o * 0.3, p[1] + o])
        .collect();
    c.out.push(
        poly(&pts, true),
        Style::fill(rgb(0x2a1c18)).with_opacity(0.2),
        None,
        "drop-shadow",
    );
}

/// The group transform of a rig in this mood.
pub fn rig_ops(g: &Toy, name: &str, mood: usize) -> Vec<TransformOp> {
    let mut ops = Vec::new();
    let Some(r) = g.rig(name) else {
        return ops;
    };
    let Some(ps) = r.pose[mood] else {
        return ops;
    };
    if ps.dx != 0.0 || ps.dy != 0.0 {
        ops.push(TransformOp::Translate {
            tx: crate::emit::m(ps.dx),
            ty: crate::emit::m(ps.dy),
        });
    }
    if ps.rot != 0.0 {
        ops.push(TransformOp::Rotate {
            angle: crate::emit::bam(ps.rot),
            cx: crate::emit::m(r.pivot[0]),
            cy: crate::emit::m(r.pivot[1]),
        });
    }
    // A squash or a stretch about the pivot: move the pivot to the origin,
    // scale, and move it back.
    if let Some((sx, sy)) = ps.scale {
        if sx != 1.0 || sy != 1.0 {
            ops.push(TransformOp::Translate {
                tx: crate::emit::m(r.pivot[0]),
                ty: crate::emit::m(r.pivot[1]),
            });
            ops.push(TransformOp::Scale {
                sx: q(sx),
                sy: q(sy),
            });
            ops.push(TransformOp::Translate {
                tx: crate::emit::m(-r.pivot[0]),
                ty: crate::emit::m(-r.pivot[1]),
            });
        }
    }
    ops
}

/// Draws parts in order, each sewn-on round over its drop shadow.
pub fn emit_parts(c: &mut Ctx, list: &[&Part]) {
    for p in list {
        if p.seam == Seam::Ring && c.detail > 0 {
            drop_shadow(c, p);
        }
        part_svg(c, p);
    }
}

/// The soft shadow an arm casts on the body it is sewn to: the arm's
/// outline in this mood's pose, moved a little down and to the right, and
/// clipped to the body.
fn arm_shadows(c: &mut Ctx, body_clip: &Shape) {
    let g = c.g;
    let o = max(1.4 / g.fit.s, c.pxu * 0.7);
    for part in &g.parts {
        if part.group != "arms" {
            continue;
        }
        let r = g.rig(part.rig).expect("an arm has its rig");
        let ps = r.pose[c.mood].unwrap_or_default();
        let pts: Vec<Pt> = part
            .outline
            .iter()
            .map(|&p| {
                let qp = if ps.rot != 0.0 {
                    rot(p, ps.rot * PI / 180.0, Some(r.pivot))
                } else {
                    p
                };
                [qp[0] + o * 0.35, qp[1] + o]
            })
            .collect();
        c.out.push(
            poly(&pts, true),
            Style::fill(rgb(0x2a1c18)).with_opacity(0.17),
            Some(body_clip),
            "drop-shadow",
        );
    }
}

/// A joint button sewn over a shoulder.
fn button_svg(c: &mut Ctx, b: (Pt, f64)) {
    let pal = &c.g.pal;
    let (centre, r) = b;
    let col = if pal.accent.y > 0.7 {
        hsl(pal.main.h + 180.0, 40.0, 58.0)
    } else {
        hsl(40.0, 40.0, 88.0)
    };
    let edge = rgb(0x6b5a4c);
    let thread_dark = pal.thread_dark;
    let width = max(r * 0.12, c.pxu * 0.5);
    c.out.push(
        circle(centre, r),
        Style::fill(col).with_stroke(edge, width),
        None,
        "button",
    );
    if c.detail >= 1 {
        let hs = [
            [centre[0] - r * 0.32, centre[1]],
            [centre[0] + r * 0.32, centre[1]],
        ];
        for h in hs {
            c.out
                .push(circle(h, r * 0.14), Style::fill(edge), None, "button");
        }
        // The drawing gives this stitch no fill of its own, so it keeps
        // SVG's default black fill, which a single line has no area for.
        if let Some(shape) = stitches(vec![seg(hs[0], hs[1])]) {
            let mut style = Style::line(thread_dark, r * 0.14, StrokeCap::Round, StrokeJoin::Miter);
            style.paint = crate::emit::solid(rgb(0x000000));
            c.out.push(shape, style, None, "button");
        }
    }
}

/// The soft shadow a head casts where it sits on the body.
fn neck_shadow(c: &mut Ctx, body_clip: &Shape) {
    let g = c.g;
    let h = g.head.as_ref().expect("a two-piece toy has a head");
    let b = &g.body;
    let hw = h.half_w(h.y_bot - h.h * 0.12);
    c.out.push(
        ellipse([0.0, h.y_bot - h.h * 0.02], min(hw * 1.05, b.w), h.h * 0.09),
        Style::paint(soft_shadow(rgb(0x1a1010), 0.32)),
        Some(body_clip),
        "contact-shadow",
    );
}

/// The seam where the head is sewn to the body.
fn neck_seam(c: &mut Ctx) {
    let g = c.g;
    if c.detail == 0 {
        return;
    }
    let h = g.head.as_ref().expect("a two-piece toy has a head");
    let b = &g.body;
    let mut pts = Vec::new();
    for i in 0..=60 {
        let t = lerp(0.62, 1.38, i as f64 / 60.0) * PI;
        let r = piece_r(&h.p, t);
        let p = [h.c[0] + r * sin(t), h.c[1] - r * cos(t)];
        if sdist(p, &b.outline) < -1.5 {
            pts.push(p);
        }
    }
    if pts.len() < 3 {
        return;
    }
    let step = max(2.6 / g.fit.s, g.rh * 0.62);
    let half = g.rh * 0.38;
    let r = resample(&pts, step, false);
    let mut ds = Vec::new();
    for i in 1..r.len().saturating_sub(1) {
        let t = unit(sub(r[i + 1], r[i - 1]));
        let n = [t[1], -t[0]];
        ds.push(seg(
            sub(r[i], scl(n, half * 1.2)),
            add(r[i], scl(n, half * 0.6)),
        ));
    }
    if let Some(shape) = stitches(ds) {
        c.out.push(
            shape,
            Style::line(
                g.pal.main.deep,
                max(g.rh * 0.2, c.pxu * 0.5),
                StrokeCap::Round,
                StrokeJoin::Miter,
            ),
            None,
            "seam neck-seam",
        );
    }
}

/// Draws the parts on one rig in that rig's pose, then what `extra` adds
/// in the same group.
fn rig_group(c: &mut Ctx, parts: &[&Part], name: &str, extra: &dyn Fn(&mut Ctx)) {
    let ops = rig_ops(c.g, name, c.mood);
    c.group(ops, |c| {
        let own: Vec<&Part> = parts
            .iter()
            .copied()
            .filter(|p| p.rig == name && p.layer >= 30)
            .collect();
        emit_parts(c, &own);
        extra(c);
    });
}

/// Draws one mood of a built toy at a size of `px` screen pixels.
pub fn draw(g: &Toy, mood: usize, px: f64) -> Vec<Primitive> {
    let mut c = make_ctx(g, mood, px);
    let fit = g.fit;
    let mut parts: Vec<&Part> = g.parts.iter().collect();
    parts.sort_by_key(|p| p.layer);
    let parts = parts.as_slice();
    let select = |pred: &dyn Fn(&Part) -> bool| -> Vec<&Part> {
        parts.iter().copied().filter(|p| pred(p)).collect()
    };
    let body = g
        .parts
        .iter()
        .find(|p| p.id == "body")
        .expect("every toy has a body");
    let body_clip = knit_clip(&c, body);
    // Felt plates and eye bumps stand behind the piece that carries the
    // face.
    let behind = |c: &mut Ctx| {
        emit_parts(c, &select(&|p| p.group == "spines" && p.kind == "felt"));
        for name in ["earR", "earL", "stalkR", "stalkL"] {
            let back = select(&|p| p.rig == name && p.layer < 30);
            if !back.is_empty() {
                c.group(rig_ops(g, name, mood), |c| emit_parts(c, &back));
            }
        }
    };
    let toy_ops = vec![
        TransformOp::Translate {
            tx: crate::emit::m(fit.ox),
            ty: crate::emit::m(fit.oy),
        },
        TransformOp::Scale {
            sx: thousandths(fit.s),
            sy: thousandths(fit.s),
        },
    ];
    let nothing = |_: &mut Ctx| {};
    c.group(toy_ops, |c| {
        let bb = fit.bbox;
        let gw = (bb[2] - bb[0]) * 0.36;
        c.out.push_role(
            ellipse(
                [(bb[0] + bb[2]) / 2.0, g.ground_y + 1.0],
                gw,
                max(gw * 0.12, 2.5),
            ),
            Style::paint(soft_shadow(rgb(0x3a2a20), 0.26)),
            None,
            "ground-shadow",
            Some(Role::GroundShadow),
        );
        c.group(rig_ops(g, "root", mood), |c| {
            if g.rig("tail").is_some() {
                let tail = select(&|p| p.rig == "tail");
                c.group(rig_ops(g, "tail", mood), |c| emit_parts(c, &tail));
            }
            emit_parts(c, &select(&|p| p.group == "legs" && p.layer < 30));
            if g.head.is_none() {
                behind(c);
            }
            emit_parts(
                c,
                &select(&|p| p.rig == "root" && p.layer >= 30 && p.layer < 40),
            );
            if g.head.is_some() {
                neck_shadow(c, &body_clip);
            }
            emit_parts(c, &select(&|p| p.group == "legs" && p.layer >= 40));
            if g.rig("armR").is_some() {
                if c.detail > 0 {
                    arm_shadows(c, &body_clip);
                }
                if g.rig("arm2R").is_some() {
                    rig_group(c, parts, "arm2R", &nothing);
                    rig_group(c, parts, "arm2L", &nothing);
                }
                let buttons = &g.joint_buttons;
                rig_group(c, parts, "armR", &|c: &mut Ctx| {
                    if let Some(&b) = buttons.first() {
                        button_svg(c, b);
                    }
                });
                rig_group(c, parts, "armL", &|c: &mut Ctx| {
                    if let Some(&b) = buttons.get(1) {
                        button_svg(c, b);
                    }
                });
            }
            let head_ops = if g.head.is_some() {
                rig_ops(g, "head", mood)
            } else {
                Vec::new()
            };
            c.group(head_ops, |c| {
                if g.head.is_some() {
                    behind(c);
                    emit_parts(c, &select(&|p| p.id == "head-piece"));
                    neck_seam(c);
                }
                if g.rig("earR").is_some() {
                    rig_group(c, parts, "earR", &nothing);
                    rig_group(c, parts, "earL", &nothing);
                }
                // Eyes on stalks or bumps are drawn with their mounts, so
                // that eye and mount swing together.
                if g.rig("stalkR").is_some() {
                    for name in ["stalkR", "stalkL"] {
                        rig_group(c, parts, name, &|c: &mut Ctx| {
                            let eye =
                                c.g.face()
                                    .eyes
                                    .iter()
                                    .find(|e| e.rig == Some(name))
                                    .copied()
                                    .expect("a mounted eye has its mount");
                            crate::face::draw_eye(c, &eye);
                        });
                    }
                }
                let top = select(&|p| {
                    matches!(p.group, "horns" | "antennae" | "tuft" | "sprout")
                        || (p.group == "spines" && p.kind != "felt")
                });
                emit_parts(c, &top);
                crate::face::draw_face(c, parts);
                crate::face::draw_marks(c);
            });
        });
    });
    c.out.primitives
}

/// The toy's scale as the drawing writes it, rounded to thousandths, as
/// Q32.32.
fn thousandths(s: f64) -> Q32 {
    let k = i128::from(js_round(s * 1000.0) as i64);
    // k / 1000 to the nearest Q32 step.
    Q32((((k << 32) * 2 + 1000) / 2000) as i64)
}
