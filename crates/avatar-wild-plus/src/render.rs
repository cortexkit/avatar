//! Assembling the render tree for one mood: parts behind and in front of
//! the body, the body, its surface, the face, the ground shadow and what
//! sits beside the creature.

use avatar_svg::{
    Effect, GradientStop, Mood, Paint, Primitive, RenderTree, Role, Shape, StrokeCap, StrokeJoin,
    Transform, TransformOp,
};
use std::f64::consts::PI;

use crate::coat::Seg;
use crate::creature::Built;
use crate::draw::{
    circle, ellipse, filled, lined, lit, path_of, plain_stroke, prim, q1, q2, q32, rotate,
    round_stroke, scale, solid, stroke, translate, PathB,
};
use crate::face_draw::face_prims;
use crate::geom::{bbox_of, clamp, max, P};
use crate::mood::{mood_turn, spec, way_of};
use crate::palette::{hex, Palette};
use crate::parts::{Layer, Shape as LShape};

/// One part layer as primitives. `ow` is the outline width.
fn layer_prims(l: &Layer, kind: &str, pal: &Palette, ow: f64, out: &mut Vec<Primitive>) {
    let label = &format!("wp-{} wp-{}", kind.replace(' ', "-"), l.cls);
    let op = l.op.filter(|&o| o < 1.0).map(crate::draw::n2);
    let line = l.line.map(|role| pal.ink(role));
    match &l.shape {
        LShape::Poly(pts) => out.push(prim(
            path_of(pts, true),
            solid(pal.ink(l.fill)),
            line.map(|c| stroke(c, q2(ow), StrokeCap::Butt, StrokeJoin::Round)),
            op,
            label,
        )),
        LShape::Set(polys) => {
            // Outline every polygon first, then fill them all: the seams
            // between overlapping pieces disappear.
            let fill = pal.ink(l.fill);
            for p in polys {
                out.push(prim(
                    path_of(p, true),
                    solid(fill),
                    line.map(|c| stroke(c, q2(ow * 2.0), StrokeCap::Butt, StrokeJoin::Round)),
                    None,
                    label,
                ));
            }
            for p in polys {
                out.push(filled(path_of(p, true), fill, None, label));
            }
        }
        LShape::Tube { pts, mw } => {
            let w = max(*mw, 1.6);
            let d = path_of(pts, false);
            if let Some(c) = line {
                out.push(lined(
                    d.clone(),
                    round_stroke(c, q2(w + 2.0 * ow)),
                    None,
                    label,
                ));
            }
            out.push(lined(d, round_stroke(pal.ink(l.fill), q2(w)), None, label));
        }
        LShape::Curve { pts, lw } => {
            if let Some(c) = line {
                out.push(lined(
                    path_of(pts, false),
                    round_stroke(c, q2(*lw)),
                    op,
                    label,
                ));
            }
        }
        LShape::Disc { x, y, r } => out.push(prim(
            circle(q1(*x), q1(*y), q2(*r)),
            solid(pal.ink(l.fill)),
            line.map(|c| plain_stroke(c, q2(ow))),
            op,
            label,
        )),
        LShape::Glow { x, y, r } => {
            let color = pal.ink(l.fill);
            let stops = vec![
                GradientStop::with_opacity(q32(0.2), color, q32(0.6)),
                GradientStop::with_opacity(q32(1.0), color, q32(0.0)),
            ];
            out.push(prim(
                circle(q1(*x), q1(*y), q2(*r)),
                Paint::RadialGradient { stops },
                None,
                None,
                label,
            ));
        }
    }
}

/// The closed body path, every coordinate written with one decimal.
fn body_shape(start: P, segs: &[Seg]) -> Shape {
    let mut d = PathB::new();
    d.m1(start[0], start[1]);
    for s in segs {
        match *s {
            Seg::L(p) => d.l1(p[0], p[1]),
            Seg::C(c1, c2, p) => d.cq([
                q1(c1[0]),
                q1(c1[1]),
                q1(c2[0]),
                q1(c2[1]),
                q1(p[0]),
                q1(p[1]),
            ]),
        }
    }
    d.z();
    d.shape()
}

/// What sits beside the creature for a mood, off the head's upper corners.
fn adjunct(c: &Built, kind: &str) -> Vec<Primitive> {
    let hb = bbox_of(&c.head);
    let pal = &c.p;
    let dx = c.dx;
    let ax = clamp(hb[2] + 2.0, 150.0 - dx, 196.0 - dx);
    let ay = clamp(hb[1] + 18.0, 40.0, 150.0);
    let mut s = Vec::new();
    match kind {
        "zz" => {
            for (x, base, size, op) in [(ax, ay, 14.0, 0.8), (ax + 13.0, ay - 17.0, 19.0, 0.6)] {
                let top = base - size;
                let w = size * 0.6;
                s.push(lined(
                    path_of(&[[x, top], [x + w, top], [x, base], [x + w, base]], false),
                    round_stroke(pal.outline, q2(max(size * 0.14, 1.4))),
                    Some(op),
                    "wp-zz",
                ));
            }
        }
        "sweat" => {
            let x = hb[2] - (hb[2] - 120.0) * 0.2;
            let y = hb[1] + (hb[3] - hb[1]) * 0.1;
            let k = clamp((hb[3] - hb[1]) * 0.12, 9.0, 18.0);
            let mut d = PathB::new();
            d.m1(x, y);
            d.quad_to(x, y, x + k * 0.45, y + k * 0.75, x, y + k);
            d.quad_to(x, y + k, x - k * 0.45, y + k * 0.75, x, y);
            d.z();
            s.push(prim(
                d.shape(),
                solid(hex(0x7fb8e8)),
                Some(plain_stroke(hex(0x4f8fc4), 100)),
                Some(0.9),
                "wp-sweat",
            ));
        }
        "dots" => {
            for (ddx, ddy, rad, op) in [
                (0.0, 0.0, 2.5, 0.5),
                (9.0, -7.0, 3.2, 0.68),
                (20.0, -16.0, 4.0, 0.86),
            ] {
                s.push(filled(
                    circle(q1(ax + ddx), q1(ay + ddy), lit(rad)),
                    pal.outline,
                    Some(op),
                    "wp-dots",
                ));
            }
        }
        "fume" => {
            let (cx, cy) = (ax + 8.0, ay - 10.0);
            for (ddx, ddy) in [(0.94, -0.34), (0.26, -0.97), (-0.57, -0.82), (0.8, 0.6)] {
                let mut d = PathB::new();
                d.m1(cx + ddx * 4.5, cy + ddy * 4.5);
                d.l1(cx + ddx * 12.0, cy + ddy * 12.0);
                s.push(lined(
                    d.shape(),
                    stroke(hex(0xd4513f), 280, StrokeCap::Round, StrokeJoin::Miter),
                    None,
                    "wp-fume",
                ));
            }
        }
        "sparkle" => {
            for (x, y, k) in [
                (
                    clamp(hb[2] + 8.0, 160.0 - dx, 214.0 - dx),
                    clamp(hb[1] + 14.0, 24.0, 120.0),
                    7.5,
                ),
                (
                    clamp(hb[0] - 7.0, 26.0 - dx, 80.0 - dx),
                    clamp(hb[1] + 30.0, 30.0, 140.0),
                    5.5,
                ),
            ] {
                s.push(prim(
                    path_of(
                        &[
                            [x, y - k],
                            [x + k * 0.24, y - k * 0.24],
                            [x + k, y],
                            [x + k * 0.24, y + k * 0.24],
                            [x, y + k],
                            [x - k * 0.24, y + k * 0.24],
                            [x - k, y],
                            [x - k * 0.24, y - k * 0.24],
                        ],
                        true,
                    ),
                    solid(hex(0xf6c443)),
                    Some(stroke(
                        hex(0xc98f1d),
                        80,
                        StrokeCap::Butt,
                        StrokeJoin::Round,
                    )),
                    None,
                    "wp-sparkle",
                ));
            }
        }
        _ => {}
    }
    s
}

fn with_ops(mut prims: Vec<Primitive>, ops: &[TransformOp]) -> Vec<Primitive> {
    if !ops.is_empty() {
        for p in prims.iter_mut() {
            p.transform = Some(Transform { ops: ops.to_vec() });
        }
    }
    prims
}

/// The whole drawing of a built creature in one mood.
pub fn render(c: &Built, mood: Mood) -> RenderTree {
    let pal = &c.p;
    let ow = c.ow;
    let sp = spec(mood);
    let face = face_prims(c, mood);
    let gy = c.ground_y;
    let tm = &c.temper;

    // The outer group's shift for a lopsided creature, then its pose.
    let mut outer_ops = Vec::new();
    if c.dx.abs() > 0.05 {
        outer_ops.push(translate(q1(c.dx), 0));
    }
    let mut pose = outer_ops.clone();
    if sp.lift != 0.0 {
        pose.push(translate(0, q2(-sp.lift * tm.squash)));
    }
    if sp.tilt != 0.0 {
        pose.push(rotate(sp.tilt * tm.tilt * tm.side, lit(120.0), q1(gy)));
    }
    if sp.squash[0] != 1.0 || sp.squash[1] != 1.0 {
        pose.push(translate(lit(120.0), q1(gy)));
        pose.push(scale(
            1.0 + (sp.squash[0] - 1.0) * tm.squash,
            1.0 + (sp.squash[1] - 1.0) * tm.squash,
        ));
        pose.push(translate(lit(-120.0), q1(-gy)));
    }

    let (mut back, mut behind, mut over, mut front) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for part in &c.parts {
        let mut g = Vec::new();
        for l in &part.layers {
            layer_prims(l, part.kind, pal, ow, &mut g);
        }
        if let Some((_, stalk)) = face.stalk.iter().find(|(n, _)| *n == part.name) {
            g.extend(stalk.iter().cloned());
        }
        // A mood turns a limb about its root: down and outward when it
        // droops.
        let deg = mood_turn(part, sp.droop, tm, way_of(part.root[0])) * 180.0 / PI;
        let mut ops = pose.clone();
        if deg.abs() > 0.05 {
            ops.push(rotate(deg, q1(part.root[0]), q1(part.root[1])));
        }
        let g = with_ops(g, &ops);
        if part.back {
            back.extend(g);
        } else if part.front {
            front.extend(g);
        } else if part.over {
            over.extend(g);
        } else {
            behind.extend(g);
        }
    }

    let body = body_shape(c.body_start, &c.body_segs);
    let s = &c.surface;
    // Everything on the surface is clipped to the body.
    let clip = |prims: Vec<Primitive>| -> Vec<Primitive> {
        with_ops(prims, &pose)
            .into_iter()
            .map(|mut p| {
                p.clip = Some(body.clone());
                p
            })
            .collect()
    };
    let mut surface = clip(s.flat.clone());
    if !s.pattern.is_empty() {
        // A mask that hides patterns over the face and lets them fade in
        // round it: white everywhere, and an ellipse whose gradient runs from
        // black at 0.72 of its radius to white at its edge.
        let black = hex(0x000000);
        let white = hex(0xffffff);
        let fade = s.fade;
        let mask = with_ops(
            vec![
                filled(
                    Shape::Rect {
                        x: 0,
                        y: 0,
                        w: 24_000,
                        h: 24_000,
                    },
                    white,
                    None,
                    "wp-fade",
                ),
                prim(
                    ellipse(fade[0], fade[1], fade[2], fade[3]),
                    Paint::RadialGradient {
                        stops: vec![
                            GradientStop::with_opacity(q32(0.72), black, q32(1.0)),
                            GradientStop::with_opacity(q32(1.0), white, q32(1.0)),
                        ],
                    },
                    None,
                    None,
                    "wp-fade",
                ),
            ],
            &pose,
        );
        let masked = Effect::Masked { mask };
        for mut p in clip(s.pattern.clone()) {
            p.effects.push(masked.clone());
            surface.push(p);
        }
    }
    surface.extend(clip(s.belly.clone()));
    surface.extend(clip(s.creases.clone()));
    surface.extend(clip(face.skin));

    let body_prim = prim(
        body.clone(),
        solid(pal.body),
        Some(stroke(
            pal.outline,
            q2(ow),
            StrokeCap::Butt,
            StrokeJoin::Round,
        )),
        None,
        "wp-body",
    );
    // The nose and mouth lie under the eyes, so a beak that reaches up to an
    // eye tucks under it instead of covering it.
    let mut face_group = Vec::new();
    face_group.extend(face.nose);
    face_group.extend(face.mouth);
    face_group.extend(face.eyes);
    face_group.extend(face.lids);
    face_group.extend(face.brows);
    face_group.extend(face.extra);

    // The shadow lies under the body, as wide as a share of the silhouette.
    let shadow_r =
        max(16.0, (c.bbox[2] - c.bbox[0]) * 0.42) * (if sp.lift != 0.0 { 0.93 } else { 1.0 });
    let mut shadow = filled(
        ellipse(lit(120.0), q1(gy + 3.0), q1(shadow_r), lit(6.0)),
        pal.shadow,
        Some(0.14),
        "wp-shadow",
    );
    shadow.role = Some(Role::GroundShadow);

    let mut primitives = with_ops(vec![shadow], &outer_ops);
    primitives.extend(back);
    primitives.extend(behind);
    primitives.extend(with_ops(vec![body_prim], &pose));
    primitives.extend(surface);
    primitives.extend(over);
    primitives.extend(front);
    primitives.extend(with_ops(face_group, &pose));
    if !sp.adj.is_empty() && sp.adj != "tear" {
        primitives.extend(with_ops(adjunct(c, sp.adj), &outer_ops));
    }
    RenderTree {
        canvas: 240,
        background: None,
        pixel_grid: None,
        primitives,
    }
}
