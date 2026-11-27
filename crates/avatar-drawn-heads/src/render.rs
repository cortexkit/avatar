//! Drawing a decoded head as a render tree, in the reference's painter's
//! order. Each element of the reference's SVG becomes one primitive with
//! the same geometry, written to the same precision.

use avatar_svg::wild::render::{opacity, rotate, scale};
use avatar_svg::{
    FillRule, Mood, Paint, Primitive, RenderTree, Rgb, Shape, Stroke, StrokeCap, StrokeJoin,
    Subpath, Transform, TransformOp,
};
use std::f64::consts::{PI, TAU};

use crate::creature::Creature;
use crate::face::{brow_pts, eye_geom, EyePose, MouthForm, MouthLine, MouthPose};
use crate::geom::{
    box_of, clamp, cos, curve, drop_contour, hundredth, lerp, max, min, mix, polyline, pt, round2,
    round3, sample, sharp, stroke_pts, tenth, through, unit_of, up_dir, Contour, Op, Profile, Pt,
};
use crate::hair::{Edge, Fill, Mass, Tone};
use crate::mood::{head_ops, MarkKind, Pose};

/// How fine one drawing is. Lines are kept about a pixel wide, so they grow
/// in frame units as the picture shrinks; under 96 px the lines of age, the
/// freckles and the strands of hair are left out, and under 56 px more.
struct Ctx {
    size: f64,
    px: f64,
    fine: bool,
    tiny: bool,
    /// The outline of the head, the ears and the hair.
    line_w: f64,
    /// The features of the face.
    pen_w: f64,
    mark_k: f64,
}

impl Ctx {
    fn new(size: u32) -> Self {
        let size = f64::from(size);
        let px = size / 240.0;
        Self {
            size,
            px,
            fine: size >= 96.0,
            tiny: size < 56.0,
            line_w: clamp(1.2 / px, 3.0, 8.4),
            pen_w: clamp(1.25 / px, 2.7, 7.6),
            mark_k: clamp(64.0 / size, 1.0, 1.5),
        }
    }
}

/// The primitives drawn so far, and the transform the next one is drawn under.
struct Canvas {
    prims: Vec<Primitive>,
    ops: Vec<TransformOp>,
    depth: Vec<usize>,
}

/// Fill and line of one element.
struct Look {
    fill: Option<Rgb>,
    stroke: Option<Stroke>,
    opacity: Option<f64>,
    clip: Option<Shape>,
}

fn solid(fill: Rgb) -> Look {
    Look {
        fill: Some(fill),
        stroke: None,
        opacity: None,
        clip: None,
    }
}

fn stroke(color: Rgb, width: i64, cap: StrokeCap, join: StrokeJoin) -> Stroke {
    Stroke {
        color,
        width,
        cap,
        join,
    }
}

/// The transform operations of a group, as the reference writes them: an
/// operation that rounds to nothing is left out.
fn tree_ops(ops: &[Op]) -> Vec<TransformOp> {
    ops.iter()
        .filter_map(|o| match *o {
            Op::T(x, y) => (round2(x) != 0.0 || round2(y) != 0.0).then(|| TransformOp::Translate {
                tx: hundredth(x),
                ty: hundredth(y),
            }),
            Op::R(d) => (round2(d) != 0.0).then(|| rotate(round2(d), 0.0, 0.0)),
            Op::S(x, y) => {
                (round3(x) != 1.0 || round3(y) != 1.0).then(|| scale(round3(x), round3(y)))
            }
        })
        .collect()
}

impl Canvas {
    fn push(&mut self, ops: &[Op]) {
        self.depth.push(self.ops.len());
        self.ops.extend(tree_ops(ops));
    }

    fn pop(&mut self) {
        let len = self.depth.pop().expect("a group to close");
        self.ops.truncate(len);
    }

    fn add(&mut self, shape: Shape, look: Look, label: &str) {
        self.prims.push(Primitive {
            shape,
            transform: (!self.ops.is_empty()).then(|| Transform {
                ops: self.ops.clone(),
            }),
            paint: look
                .fill
                .map_or(Paint::None, |color| Paint::Solid { color }),
            stroke: look.stroke,
            opacity: look.opacity.and_then(opacity),
            clip: look.clip,
            effects: Vec::new(),
            label: label.to_owned(),
            role: None,
        });
    }
}

fn path_shape(c: Contour) -> Shape {
    Shape::Path {
        segments: c.segments,
        closed: c.closed,
        fill_rule: FillRule::NonZero,
    }
}

fn compound(contours: Vec<Contour>, fill_rule: FillRule) -> Shape {
    Shape::CompoundPath {
        subpaths: contours
            .into_iter()
            .map(|c| Subpath {
                segments: c.segments,
                closed: c.closed,
            })
            .collect(),
        fill_rule,
    }
}

/// A circle whose centre and radius are written to a tenth.
fn circle(x: f64, y: f64, r: f64) -> Shape {
    Shape::Circle {
        cx: tenth(x),
        cy: tenth(y),
        r: tenth(r),
    }
}

/// A circle with a fixed radius in hundredths.
fn circle_r(x: f64, y: f64, r: i64) -> Shape {
    Shape::Circle {
        cx: tenth(x),
        cy: tenth(y),
        r,
    }
}

/// A stroke of the pen: the filled shape round its centre line, with a thin
/// line of the same colour round it to blunt its points.
#[allow(clippy::too_many_arguments)]
fn pen(
    cv: &mut Canvas,
    ctrl: &[Pt],
    w: f64,
    colour: Rgb,
    profile: Profile,
    n: Option<usize>,
    opacity: Option<f64>,
    label: &str,
) {
    let hw = max(0.5, w / 2.0 - 0.45);
    if let Some(c) = curve(&stroke_pts(ctrl, hw, profile, n), true) {
        cv.add(
            path_shape(c),
            Look {
                fill: Some(colour),
                stroke: Some(stroke(colour, 90, StrokeCap::Butt, StrokeJoin::Round)),
                opacity,
                clip: None,
            },
            label,
        );
    }
}

/// A plain line of one width, for the few places where a stroke must not swell.
fn line(
    cv: &mut Canvas,
    contours: Vec<Contour>,
    w: f64,
    colour: Rgb,
    opacity: Option<f64>,
    clip: Option<Shape>,
    label: &str,
) {
    let shape = if contours.len() == 1 {
        path_shape(contours.into_iter().next().expect("one contour"))
    } else {
        compound(contours, FillRule::NonZero)
    };
    cv.add(
        shape,
        Look {
            fill: None,
            stroke: Some(stroke(
                colour,
                hundredth(w),
                StrokeCap::Round,
                StrokeJoin::Round,
            )),
            opacity,
            clip,
        },
        label,
    );
}

/// A filled shape with an ink line of the given width round it.
fn outlined(fill: Option<Rgb>, ink: Rgb, w: f64) -> Look {
    Look {
        fill,
        stroke: Some(stroke(
            ink,
            hundredth(w),
            StrokeCap::Butt,
            StrokeJoin::Round,
        )),
        opacity: None,
        clip: None,
    }
}

fn smooth(pts: &[Pt]) -> Vec<Pt> {
    pts.iter().map(|p| p.smooth()).collect()
}

/// Draws a head in one mood. `detail_px` is the size the image will be
/// shown at; `None` draws what the reference draws at 256 px.
pub fn render(c: &Creature, mood: Mood, detail_px: Option<u32>) -> RenderTree {
    let ctx = Ctx::new(detail_px.unwrap_or(256));
    let pose = c.pose(mood);
    let p = &c.palette;
    let hops = head_ops(&c.head, pose);
    let nod = [Op::T(pose.nod.0, pose.nod.1)];
    let f = c.fit;
    let mut cv = Canvas {
        prims: Vec::new(),
        ops: Vec::new(),
        depth: Vec::new(),
    };
    let hair_tone = MassTone {
        fill: p.hair,
        line: p.hair_line,
        pen: p.hair_pen,
        shade: p.hair_shade,
    };
    cv.push(&[Op::T(f.x, f.y), Op::S(f.s, f.s)]);
    cv.push(&hops);
    mass(&mut cv, c, &c.hair.back, &ctx, &hair_tone);
    cv.pop();
    neck(&mut cv, c, &ctx);
    cv.push(&hops);
    ears(&mut cv, c, &ctx);
    cv.add(path_shape(c.head.outline.clone()), solid(p.skin), "dh-skin");
    let heavy = c.head.heavy;
    let line_w = ctx.line_w;
    cv.add(
        compound(
            ring_path(&c.head.poly, 3, |t| {
                line_w * (1.0 + 0.34 * cos(TAU * (t - heavy)))
            }),
            FillRule::EvenOdd,
        ),
        solid(p.ink),
        "dh-outline",
    );
    cv.push(&nod);
    cheeks(&mut cv, c, pose, &ctx);
    age(&mut cv, c, &ctx);
    cv.pop();
    if c.beard.kind != crate::beard::BeardKind::None {
        let tone = MassTone {
            fill: p.beard,
            line: p.beard_line,
            pen: p.beard_pen,
            shade: p.beard_pen,
        };
        mass(&mut cv, c, &c.beard.mass, &ctx, &tone);
    }
    cv.push(&nod);
    nose(&mut cv, c, &ctx);
    mouth(&mut cv, c, &pose.mouth, &ctx, c.beard.covers);
    eye(&mut cv, c, -1.0, &pose.eye_l, &ctx, false);
    eye(&mut cv, c, 1.0, &pose.eye_r, &ctx, false);
    eye(&mut cv, c, -1.0, &pose.eye_l, &ctx, true);
    eye(&mut cv, c, 1.0, &pose.eye_r, &ctx, true);
    brows(&mut cv, c, pose, &ctx);
    // A tear under one eye.
    if pose.tear != 0.0 {
        let e = &c.eyes;
        cv.add(
            path_shape(drop_contour(pose.tear * e.x, e.y + e.bl + 9.0, 4.4)),
            Look {
                fill: Some(p.drop),
                stroke: Some(stroke(p.drop_edge, 100, StrokeCap::Butt, StrokeJoin::Miter)),
                opacity: None,
                clip: None,
            },
            "dh-tear",
        );
    }
    cv.pop();
    mass(&mut cv, c, &c.hair.front, &ctx, &hair_tone);
    cv.pop();
    marks(&mut cv, c, pose, &ctx);
    cv.pop();
    RenderTree {
        canvas: 240,
        background: None,
        pixel_grid: None,
        primitives: cv.prims,
    }
}

/// A closed outline drawn as a line whose weight changes round it: the band
/// between a curve a little outside the points and one a little inside.
fn ring_path(poly: &[Pt], every: usize, weight: impl Fn(f64) -> f64) -> Vec<Contour> {
    let n = poly.len();
    let mut cx = 0.0;
    let mut cy = 0.0;
    for p in poly {
        cx += p.x / n as f64;
        cy += p.y / n as f64;
    }
    let mut outer = Vec::new();
    let mut inner = Vec::new();
    let mut i = 0;
    while i < n {
        let p = poly[i];
        let t = unit_of(poly[(i + n - every) % n], poly[(i + every) % n]);
        // The normal that points away from the middle.
        let mut nx = -t.1;
        let mut ny = t.0;
        if nx * (p.x - cx) + ny * (p.y - cy) < 0.0 {
            nx = -nx;
            ny = -ny;
        }
        let h = weight(i as f64 / n as f64) / 2.0;
        outer.push(pt(p.x + nx * h, p.y + ny * h));
        inner.push(pt(p.x - nx * h, p.y - ny * h));
        i += every;
    }
    inner.reverse();
    vec![
        curve(&outer, true).expect("a head outline"),
        curve(&inner, true).expect("a head outline"),
    ]
}

/// The neck's fill ends in an open curve with no line under it.
fn neck(cv: &mut Canvas, c: &Creature, ctx: &Ctx) {
    let Some(n) = &c.neck else {
        return;
    };
    let p = &c.palette;
    if let Some(shape) = curve(&n.pts, true) {
        cv.add(path_shape(shape), solid(p.skin_dark), "dh-neck-skin");
    }
    for sd in [-1.0, 1.0] {
        pen(
            cv,
            &n.side(sd),
            ctx.line_w,
            p.ink,
            Profile::Out,
            Some(3),
            None,
            "dh-neck-line",
        );
    }
}

fn ears(cv: &mut Canvas, c: &Creature, ctx: &Ctx) {
    let p = &c.palette;
    for sd in [-1.0, 1.0] {
        let s = c.ears.shape(&c.head, sd);
        if let Some(shape) = curve(&s.pts, true) {
            cv.add(
                path_shape(shape),
                outlined(Some(p.skin), p.ink, ctx.line_w),
                "dh-ear",
            );
        }
        if let (Some(curl), false) = (&s.curl, ctx.tiny) {
            pen(
                cv,
                curl,
                ctx.pen_w * 0.8,
                p.ink,
                Profile::Leaf,
                Some(3),
                Some(0.8),
                "dh-ear-curl",
            );
        }
    }
}

/// The colours of a mass: its fill, the darker line of a strand in it, the
/// colour of a single hair on the skin, and the shade of stubble.
struct MassTone {
    fill: Rgb,
    line: Rgb,
    pen: Rgb,
    shade: Rgb,
}

/// A mass of hair: every outline first, then every fill, then what is drawn on it.
fn mass(cv: &mut Canvas, c: &Creature, m: &Mass, ctx: &Ctx, tone: &MassTone) {
    let p = &c.palette;
    let thick = ctx.line_w;
    let thin = ctx.line_w * 0.5;
    let width = |e: Edge| match e {
        Edge::Thick => thick,
        Edge::Thin => thin,
        Edge::None => 0.0,
    };
    for s in &m.fills {
        match s {
            Fill::Circle { x, y, r, edge } => {
                let w = width(*edge);
                if w != 0.0 {
                    cv.add(
                        circle(*x, *y, *r),
                        Look {
                            fill: None,
                            stroke: Some(stroke(
                                p.ink,
                                hundredth(2.0 * w),
                                StrokeCap::Butt,
                                StrokeJoin::Miter,
                            )),
                            opacity: None,
                            clip: None,
                        },
                        "dh-hair-edge",
                    );
                }
            }
            Fill::Tube { pts, w: tw, edge } => {
                let w = width(*edge);
                if let Some(d) = curve(pts, false) {
                    line(cv, vec![d], tw + 2.0 * w, p.ink, None, None, "dh-hair-edge");
                }
            }
            Fill::Shape { pts, edge, .. } => {
                let w = width(*edge);
                if w != 0.0 {
                    if let Some(d) = curve(pts, true) {
                        cv.add(
                            path_shape(d),
                            outlined(None, p.ink, 2.0 * w),
                            "dh-hair-edge",
                        );
                    }
                }
            }
        }
    }
    for r in &m.rims {
        if let Some(d) = curve(&r.pts, false) {
            cv.add(
                path_shape(d),
                outlined(None, p.ink, 2.0 * if r.thin { thin } else { thick }),
                "dh-hair-rim",
            );
        }
    }
    for s in &m.fills {
        match s {
            Fill::Circle { x, y, r, .. } => {
                cv.add(circle(*x, *y, *r), solid(tone.fill), "dh-hair");
            }
            Fill::Tube { pts, w, .. } => {
                if let Some(d) = curve(pts, false) {
                    line(cv, vec![d], *w, tone.fill, None, None, "dh-hair");
                }
            }
            Fill::Shape { pts, skin, .. } => {
                if let Some(d) = curve(pts, true) {
                    // A parting is a shape of the skin's colour, laid on the hair.
                    cv.add(
                        path_shape(d),
                        solid(if *skin { p.skin } else { tone.fill }),
                        "dh-hair",
                    );
                }
            }
        }
    }
    for s in &m.pens {
        // Single hairs are the hair itself and are always drawn; strands inside a mass are detail.
        if s.tone == Tone::Hair {
            pen(
                cv,
                &s.ctrl,
                ctx.pen_w * s.k,
                tone.pen,
                s.press,
                Some(3),
                None,
                "dh-hair-pen",
            );
        } else if ctx.fine {
            pen(
                cv,
                &s.ctrl,
                ctx.pen_w * 0.86 * s.k,
                tone.line,
                s.press,
                Some(3),
                None,
                "dh-hair-strand",
            );
        }
    }
    if ctx.fine {
        for d in &m.dots {
            cv.add(circle(d.0, d.1, d.2), solid(tone.line), "dh-hair-dot");
        }
        // Short strokes, all in one path: the stubble of a clipped head in a
        // darker shade of its hair, or the flecks and bristles of facial hair,
        // which lie on the skin and are drawn a little faint.
        if !m.ticks.is_empty() {
            let d = m
                .ticks
                .iter()
                .map(|t| polyline(&[(t[0], t[1]), (t[0] + t[2], t[1] + t[3])]))
                .collect();
            if m.shade_ticks {
                line(cv, d, 1.8, tone.shade, None, None, "dh-ticks");
            } else {
                line(cv, d, 1.5, tone.pen, Some(0.7), None, "dh-ticks");
            }
        }
    }
}

fn eye(cv: &mut Canvas, c: &Creature, sd: f64, st: &EyePose, ctx: &Ctx, lid_part: bool) {
    let e = &c.eyes;
    let k = &e.k;
    let p = &c.palette;
    let g = eye_geom(e, sd, st);
    let upper = if k.upper != 0.0 { k.upper } else { 1.15 };
    let w_u = min(ctx.pen_w * upper, e.r * 0.5 + 1.2);
    let w_l = min(ctx.pen_w * 0.62, e.r * 0.3 + 0.8);
    if let Some(shut) = &g.shut {
        if lid_part {
            pen(
                cv,
                shut,
                max(w_u, ctx.pen_w * 1.1),
                p.ink,
                Profile::Full,
                Some(2),
                None,
                "dh-lid",
            );
        }
        return;
    }
    let outline = curve(&g.poly, true).expect("an open eye has an outline");
    let clip = || Some(path_shape(outline.clone()));
    let look = st.look;
    if k.ink {
        if !lid_part {
            cv.add(
                path_shape(outline.clone()),
                Look {
                    fill: Some(p.ink),
                    stroke: Some(stroke(p.ink, 80, StrokeCap::Butt, StrokeJoin::Round)),
                    opacity: None,
                    clip: None,
                },
                "dh-eye",
            );
            if !ctx.tiny && e.r * g.k >= 9.0 {
                cv.add(
                    circle(
                        g.cx - e.glint_side * g.a * 0.3,
                        g.cy - g.bu * 0.34,
                        max(1.3, g.a * 0.22),
                    ),
                    Look {
                        clip: clip(),
                        ..solid(p.white)
                    },
                    "dh-glint",
                );
            }
        } else {
            // A lid that cuts an ink eye shows as a line a little longer than the eye.
            if g.lidded {
                let t = &g.top;
                let n = t.len();
                let mut over = vec![mix(t[1], t[0], 1.5)];
                over.extend(smooth(&t[1..n - 1]));
                over.push(mix(t[n - 2], t[n - 1], 1.5));
                pen(
                    cv,
                    &over,
                    w_u,
                    p.ink,
                    Profile::Full,
                    Some(1),
                    None,
                    "dh-lid",
                );
            }
            if g.raised {
                pen(
                    cv,
                    &smooth(&g.bot),
                    w_l * 1.2,
                    p.ink,
                    Profile::Leaf,
                    Some(1),
                    None,
                    "dh-lid",
                );
            }
        }
    } else {
        let rp = k.pupil * e.r * g.k * e.pupil_k * st.pupil;
        let px = g.cx + look.0 * max(0.0, g.a - rp) * 0.62;
        let py = g.cy
            + look.1 * max(1.5, g.bl - rp * 0.4) * 0.6
            + if k.open { g.bl * 0.12 } else { 0.0 };
        if !lid_part {
            if !k.open {
                cv.add(path_shape(outline.clone()), solid(p.white), "dh-eye-white");
            }
            if e.iris != 0.0 {
                cv.add(
                    circle(px, py, e.iris * e.r * g.k),
                    Look {
                        clip: clip(),
                        ..solid(p.iris)
                    },
                    "dh-iris",
                );
            }
            cv.add(
                circle(px, py, rp),
                Look {
                    clip: clip(),
                    ..solid(p.ink)
                },
                "dh-pupil",
            );
            if !ctx.tiny {
                cv.add(
                    circle(
                        px - e.glint_side * rp * 0.36,
                        py - rp * 0.38,
                        max(1.1, rp * 0.3),
                    ),
                    Look {
                        clip: clip(),
                        ..solid(p.white)
                    },
                    "dh-glint",
                );
            }
        } else {
            // The upper lid is the heavier line. An open eye's lid line runs on past the pupil to both corners.
            let upper = smooth(if k.open { &g.lid } else { &g.top });
            pen(
                cv,
                &upper,
                w_u,
                p.ink,
                Profile::Full,
                Some(1),
                None,
                "dh-lid",
            );
            if e.lower != 0.0 {
                pen(
                    cv,
                    &smooth(&g.bot),
                    max(1.4, w_l * if g.raised { 1.5 } else { 1.0 }),
                    p.ink,
                    Profile::Leaf,
                    Some(1),
                    None,
                    "dh-lid",
                );
            }
        }
    }
    if lid_part && !ctx.tiny {
        // A crease follows the upper lid from a little above it.
        if e.crease && ctx.fine {
            let lift = g.bu * (if k.ink { 0.75 } else { 0.4 }) + 2.4;
            let src = if k.ink {
                [
                    pt(g.cx - sd * g.a * 1.25, g.cy - g.bu * 0.5),
                    pt(g.cx, g.cy - g.bu),
                    pt(g.cx + sd * g.a * 1.25, g.cy - g.bu * 0.5),
                ]
            } else {
                [g.lid[2], g.lid[6], g.lid[10]]
            };
            let pts: Vec<Pt> = src.iter().map(|q| pt(q.x, q.y - lift)).collect();
            pen(
                cv,
                &pts,
                ctx.pen_w * 0.6,
                p.ink,
                Profile::Leaf,
                Some(3),
                Some(0.75),
                "dh-crease",
            );
        }
        // Lashes: short strokes up and out from the outer corner.
        let line = if k.open { &g.lid } else { &g.top };
        let end = line[line.len() - 1];
        for i in 0..e.lash {
            let i = f64::from(i);
            pen(
                cv,
                &[
                    pt(end.x - sd * i * 2.6, end.y - i * 1.1),
                    pt(end.x + sd * (4.6 - i * 1.4), end.y - 3.2 - i * 2.2),
                ],
                ctx.pen_w * 0.72,
                p.ink,
                Profile::Out,
                None,
                None,
                "dh-lash",
            );
        }
    }
}

fn brows(cv: &mut Canvas, c: &Creature, pose: &Pose, ctx: &Ctx) {
    let b = &c.brows;
    let p = &c.palette;
    for sd in [-1.0, 1.0] {
        let st = if sd < 0.0 { &pose.brow_l } else { &pose.brow_r };
        let pts = brow_pts(b, &c.eyes, sd, st);
        let w = max(b.w, ctx.pen_w * 0.95);
        pen(cv, &pts, w, p.brow, b.k.press, Some(4), None, "dh-brow");
        if b.kind == "bushy" && ctx.fine {
            let mid = sample(&pts, false, 4);
            for i in [2, 4, 6] {
                let m = mid[i];
                pen(
                    cv,
                    &[
                        pt(m.x, m.y - w * 0.2),
                        pt(m.x + sd * 2.2, m.y - w * 0.5 - 3.4),
                    ],
                    ctx.pen_w * 0.8,
                    p.brow,
                    Profile::Out,
                    None,
                    None,
                    "dh-brow",
                );
            }
        }
    }
}

fn nose(cv: &mut Canvas, c: &Creature, ctx: &Ctx) {
    let n = &c.nose;
    let p = &c.palette;
    let e = &c.eyes;
    let w = n.w;
    let sd = n.side;
    let yb = c.face.nose_y;
    let ye = e.y;
    let span = yb - ye;
    let pw = ctx.pen_w;
    let stroke_of = |cv: &mut Canvas, pts: &[Pt], k: f64, press: Profile| {
        pen(cv, pts, pw * k, p.ink, press, Some(3), None, "dh-nose");
    };
    let dot = |cv: &mut Canvas, x: f64, y: f64, r: f64| {
        cv.add(circle(x, y, max(r, pw * 0.5)), solid(p.ink), "dh-nose");
    };
    let blob = |cv: &mut Canvas, pts: &[Pt]| {
        if let Some(d) = curve(pts, true) {
            cv.add(
                path_shape(d),
                outlined(Some(p.skin_dark), p.ink, pw * 0.9),
                "dh-nose",
            );
        }
    };
    match n.kind {
        "hook" => {
            let yt = ye + span * lerp(0.1, 0.42, n.high);
            stroke_of(
                cv,
                &[
                    pt(-sd * w * 0.15, yt),
                    pt(sd * w * 0.25, lerp(yt, yb, 0.55)),
                    pt(sd * w * 0.95, yb - 2.5),
                    pt(sd * w * 0.55, yb + 1.8),
                    pt(-sd * w * 0.12, yb + 0.6),
                ],
                1.0,
                Profile::Full,
            );
        }
        "long" => {
            let yt = ye - e.bu * lerp(0.1, 0.5, n.high);
            stroke_of(
                cv,
                &[
                    pt(-sd * w * 0.1, yt),
                    pt(sd * w * 0.08, lerp(yt, yb, 0.6)),
                    pt(sd * w * 0.8, yb - 2.0),
                    pt(sd * w * 0.42, yb + 2.0),
                    pt(-sd * w * 0.28, yb + 0.5),
                ],
                1.0,
                Profile::Full,
            );
        }
        "u" => stroke_of(
            cv,
            &[
                pt(-w * 0.75, yb - 3.5),
                pt(-w * 0.45, yb + 1.2),
                pt(0.0, yb + 2.4),
                pt(w * 0.45, yb + 1.2),
                pt(w * 0.75, yb - 3.5),
            ],
            1.0,
            Profile::Full,
        ),
        "dots" => {
            dot(cv, -w * 0.5, yb, 1.5 + n.tip * 0.9);
            dot(cv, w * 0.5, yb, 1.5 + n.tip * 0.9);
        }
        "c" => stroke_of(
            cv,
            &[
                pt(sd * w * 0.35, yb - 6.0 - 5.0 * n.high),
                pt(sd * w * 0.98, yb - 3.0),
                pt(sd * w * 0.3, yb + 1.6),
            ],
            1.0,
            Profile::Leaf,
        ),
        "angle" => stroke_of(
            cv,
            &[
                pt(-sd * w * 0.2, yb - span * lerp(0.42, 0.74, n.high)),
                sharp(sd * w * 0.92, yb - 0.6),
                pt(-sd * w * 0.5, yb + 0.6),
            ],
            1.0,
            Profile::Leaf,
        ),
        "bulb" => {
            let r = w * lerp(0.72, 0.98, n.tip);
            let cy = yb - r * 0.5;
            blob(
                cv,
                &[
                    pt(0.0, cy - r * 0.84),
                    pt(r * 0.72, cy - r * 0.56),
                    pt(r, cy),
                    pt(r * 0.7, cy + r * 0.62),
                    pt(0.0, cy + r * 0.84),
                    pt(-r * 0.7, cy + r * 0.62),
                    pt(-r, cy),
                    pt(-r * 0.72, cy - r * 0.56),
                ],
            );
        }
        "wide" => {
            stroke_of(
                cv,
                &[
                    pt(-w * 0.5, yb - 0.5),
                    pt(0.0, yb + 2.6),
                    pt(w * 0.5, yb - 0.5),
                ],
                1.0,
                Profile::Full,
            );
            stroke_of(
                cv,
                &[
                    pt(-w * 0.62, yb - 5.5),
                    pt(-w * 1.05, yb - 1.5),
                    pt(-w * 0.6, yb + 1.2),
                ],
                0.9,
                Profile::Leaf,
            );
            stroke_of(
                cv,
                &[
                    pt(w * 0.62, yb - 5.5),
                    pt(w * 1.05, yb - 1.5),
                    pt(w * 0.6, yb + 1.2),
                ],
                0.9,
                Profile::Leaf,
            );
        }
        "dash" => stroke_of(
            cv,
            &[
                pt(sd * 0.6, yb - 5.5 - 3.0 * n.high),
                pt(sd * 1.5, yb - 0.5),
            ],
            1.0,
            Profile::Leaf,
        ),
        "potato" => {
            let top = yb - span * lerp(0.5, 0.8, n.high);
            blob(
                cv,
                &[
                    pt(0.0, top),
                    pt(w * 0.62, lerp(top, yb, 0.5)),
                    pt(w, yb - 2.5),
                    pt(w * 0.45, yb + 2.6),
                    pt(-w * 0.45, yb + 2.6),
                    pt(-w, yb - 2.5),
                    pt(-w * 0.62, lerp(top, yb, 0.5)),
                ],
            );
        }
        "v" => stroke_of(
            cv,
            &[
                pt(-w * 0.56, yb - 4.2),
                sharp(0.0, yb + 1.2),
                pt(w * 0.56, yb - 4.2),
            ],
            1.0,
            Profile::Leaf,
        ),
        "snub" => {
            stroke_of(
                cv,
                &[
                    pt(-w * 0.62, yb + 0.6),
                    pt(0.0, yb - 3.6),
                    pt(w * 0.62, yb + 0.6),
                ],
                1.0,
                Profile::Leaf,
            );
            if !ctx.tiny {
                dot(cv, -w * 0.3, yb + 2.6, 1.2);
                dot(cv, w * 0.3, yb + 2.6, 1.2);
            }
        }
        "button" => {
            let r = w * lerp(0.36, 0.5, n.tip);
            cv.add(
                circle(0.0, yb - r * 0.4, r),
                Look {
                    fill: Some(p.skin_dark),
                    stroke: Some(stroke(
                        p.ink,
                        hundredth(pw * 0.9),
                        StrokeCap::Butt,
                        StrokeJoin::Miter,
                    )),
                    opacity: None,
                    clip: None,
                },
                "dh-nose",
            );
        }
        _ => {
            stroke_of(
                cv,
                &[
                    pt(-w * 0.55, yb - 8.5),
                    pt(-w * 0.96, yb - 4.0),
                    pt(-w * 0.55, yb + 0.8),
                ],
                1.0,
                Profile::Leaf,
            );
            stroke_of(
                cv,
                &[
                    pt(w * 0.55, yb - 8.5),
                    pt(w * 0.96, yb - 4.0),
                    pt(w * 0.55, yb + 0.8),
                ],
                1.0,
                Profile::Leaf,
            );
        }
    }
}

/// A mouth in a pose. `halo` gives it a rim of skin, as inside a beard.
fn mouth(cv: &mut Canvas, c: &Creature, m: &MouthPose, ctx: &Ctx, halo: bool) {
    let mm = &c.mouth;
    let p = &c.palette;
    let l = MouthLine::new(mm, m);
    let w = l.w;
    let pw = ctx.pen_w * 1.08;
    let at = |u: f64| pt(l.mx + u * w, l.y(u));
    let lipped = mm.kind == "lips";
    // The skin round a closed mouth inside a beard has the shape of lips:
    // `hu` above the line in the middle and `hl` below, narrowing to a point
    // just past each corner.
    let lips = |cv: &mut Canvas, hu: f64, hl: f64| {
        let full = |u: f64| {
            let q = max(0.0, 1.0 - u * u);
            lerp(q, q.sqrt(), 0.5)
        };
        let mut top = Vec::new();
        let mut bot = Vec::new();
        for u in [-0.72, -0.36, 0.0, 0.36, 0.72] {
            let q = at(u);
            top.push(pt(q.x, q.y - hu * full(u)));
            bot.push(pt(q.x, q.y + hl * full(u)));
        }
        let a = at(-1.0);
        let b = at(1.0);
        let mut pts = vec![sharp(a.x - 2.4, a.y)];
        pts.extend(top);
        pts.push(sharp(b.x + 2.4, b.y));
        pts.extend(bot.into_iter().rev());
        if let Some(d) = curve(&pts, true) {
            cv.add(path_shape(d), solid(p.skin), "dh-mouth-skin");
        }
    };
    match m.form {
        MouthForm::O => {
            // A small round mouth.
            let rx = w * 0.42;
            let ry = w * lerp(0.3, 0.62, m.open);
            let o = pt(l.mx + m.off * w, l.my);
            let ring = [
                pt(o.x, o.y - ry),
                pt(o.x + rx, o.y),
                pt(o.x, o.y + ry),
                pt(o.x - rx, o.y),
            ];
            let d = curve(&ring, true).expect("a ring");
            if halo {
                line(
                    cv,
                    vec![d.clone()],
                    pw + 7.0,
                    p.skin,
                    None,
                    None,
                    "dh-mouth-skin",
                );
            }
            cv.add(
                path_shape(d),
                outlined(
                    Some(p.mouth_in),
                    if lipped { p.lip } else { p.ink },
                    if lipped { pw * 1.5 } else { pw },
                ),
                "dh-mouth",
            );
            return;
        }
        MouthForm::Zig => {
            // A line pressed into a zigzag.
            let pts: Vec<Pt> = (0..=6)
                .map(|i| {
                    let u = -1.0 + f64::from(i) / 3.0;
                    sharp(
                        l.mx + u * w,
                        l.y(u) + (if i % 2 == 1 { -1.0 } else { 1.0 }) * w * 0.13,
                    )
                })
                .collect();
            let d = curve(&pts, false).expect("a zigzag");
            if halo {
                lips(cv, pw / 2.0 + 2.8 + w * 0.13, pw / 2.0 + 3.4 + w * 0.13);
            }
            line(cv, vec![d], pw, p.ink, None, None, "dh-mouth");
            return;
        }
        _ => {}
    }
    let grit = m.form == MouthForm::Grit;
    if m.open > 0.04 || grit {
        // An open mouth: the upper lip keeps the line's curve, the lower one drops away from it.
        let deep = if grit { w * 0.36 } else { w * m.open * 0.95 };
        let flat = if grit { 0.6 } else { 0.5 };
        let up = |u: f64| {
            l.my + (l.y(u) - l.my) * flat
                + if grit {
                    -deep * 0.5 * (1.0 - u * u * 0.4)
                } else {
                    0.0
                }
        };
        let dn = |u: f64| {
            up(u)
                + deep
                    * if grit {
                        1.0 - u * u * 0.4
                    } else {
                        max(0.0, 1.0 - u * u).sqrt()
                    }
        };
        const N: usize = 8;
        let mut top = Vec::new();
        let mut bot = Vec::new();
        for i in 0..=N {
            let u = -cos(PI * i as f64 / N as f64);
            top.push(pt(l.mx + u * w, up(u)));
            bot.push(pt(l.mx + u * w, dn(u)));
        }
        let poly = if grit {
            let mut v = top.clone();
            v.extend(bot.iter().rev());
            v
        } else {
            let mut v = vec![top[0].with_sharp(true)];
            v.extend_from_slice(&top[1..N]);
            v.push(top[N].with_sharp(true));
            v.extend(bot[1..N].iter().rev());
            v
        };
        let d = curve(&poly, true).expect("an open mouth");
        let clip = || Some(path_shape(d.clone()));
        if halo {
            line(
                cv,
                vec![d.clone()],
                pw + 7.0,
                p.skin,
                None,
                None,
                "dh-mouth-skin",
            );
        }
        cv.add(
            path_shape(d.clone()),
            solid(if grit { p.white } else { p.mouth_in }),
            "dh-mouth",
        );
        if grit {
            // Clenched teeth: a row of them, with the line where they meet.
            let mid = (up(0.0) + dn(0.0)) / 2.0;
            let mut teeth = vec![polyline(&[(l.mx - w, mid), (l.mx + w, mid)])];
            for u in [-0.6, -0.2, 0.2, 0.6] {
                teeth.push(polyline(&[
                    (l.mx + u * w, up(u) - 1.0),
                    (l.mx + u * w, dn(u) + 1.0),
                ]));
            }
            line(
                cv,
                teeth,
                max(1.2, pw * 0.5),
                p.ink,
                None,
                clip(),
                "dh-teeth",
            );
        } else {
            if m.teeth {
                let mut pts: Vec<Pt> = top.iter().map(|q| pt(q.x, q.y - 1.0)).collect();
                let drop = min(deep * 0.34, 6.5);
                pts.extend(top.iter().rev().map(|q| pt(q.x, q.y + drop)));
                if let Some(t) = curve(&pts, true) {
                    cv.add(
                        path_shape(t),
                        Look {
                            clip: clip(),
                            ..solid(p.white)
                        },
                        "dh-teeth",
                    );
                }
            }
            if m.tongue && deep > 7.0 {
                cv.add(
                    Shape::Ellipse {
                        cx: tenth(l.mx),
                        cy: tenth(dn(0.0) - deep * 0.06),
                        rx: tenth(w * 0.52),
                        ry: tenth(deep * 0.44),
                    },
                    Look {
                        clip: clip(),
                        ..solid(p.tongue)
                    },
                    "dh-tongue",
                );
            }
        }
        cv.add(path_shape(d), outlined(None, p.ink, pw), "dh-mouth");
        if lipped && !grit {
            pen(
                cv,
                &[
                    pt(l.mx - w * 0.6, dn(-0.6) + 3.2),
                    pt(l.mx, dn(0.0) + 3.6 + mm.full * 1.5),
                    pt(l.mx + w * 0.6, dn(0.6) + 3.2),
                ],
                3.2 + mm.full * 2.2,
                p.lip,
                Profile::Leaf,
                Some(3),
                None,
                "dh-lip",
            );
        }
        return;
    }
    // A closed mouth.
    let pts: Vec<Pt> = [-1.0, -0.6, -0.2, 0.2, 0.6, 1.0].map(at).to_vec();
    if halo {
        lips(cv, pw / 2.0 + 3.6, pw / 2.0 + 4.6);
    }
    if lipped {
        // The upper lip rises to two peaks with a dip between them; the lower one is a fuller arc.
        let hu = 3.0 + mm.full * 2.6;
        let hl = 4.0 + mm.full * 3.4;
        let mut upper = vec![
            pts[0].with_sharp(true),
            pt(l.mx - w * 0.36, l.y(-0.36) - hu),
            pt(l.mx, l.y(0.0) - hu * 0.55),
            pt(l.mx + w * 0.36, l.y(0.36) - hu),
            pts[5].with_sharp(true),
        ];
        upper.extend(pts[1..5].iter().rev());
        let mut lower = vec![pts[0].with_sharp(true)];
        lower.extend_from_slice(&pts[1..5]);
        lower.extend([
            pts[5].with_sharp(true),
            pt(l.mx + w * 0.5, l.y(0.5) + hl * 0.8),
            pt(l.mx, l.y(0.0) + hl),
            pt(l.mx - w * 0.5, l.y(-0.5) + hl * 0.8),
        ]);
        for lip in [upper, lower] {
            if let Some(d) = curve(&lip, true) {
                cv.add(path_shape(d), solid(p.lip), "dh-lip");
            }
        }
    }
    pen(
        cv,
        &pts,
        pw,
        p.ink,
        Profile::Full,
        Some(2),
        None,
        "dh-mouth",
    );
    if !ctx.tiny {
        if mm.kind == "pout" && !m.plain {
            pen(
                cv,
                &[
                    pt(l.mx - w * 0.36, l.y(-0.36) + 5.5),
                    pt(l.mx, l.y(0.0) + 7.4),
                    pt(l.mx + w * 0.36, l.y(0.36) + 5.5),
                ],
                pw * 0.62,
                p.ink,
                Profile::Leaf,
                Some(3),
                Some(0.8),
                "dh-pout",
            );
        }
        // Two front teeth under the lip, each a small shape with a rounded lower edge.
        if mm.kind == "teeth" && !m.plain {
            for s in [-1.0, 1.0] {
                let x = l.mx + if s < 0.0 { -4.6 } else { 0.3 };
                let y = l.y(0.0) + 0.4;
                let tooth = [
                    sharp(x, y),
                    sharp(x + 4.3, y),
                    pt(x + 4.3, y + 4.2),
                    pt(x + 2.15, y + 5.5),
                    pt(x, y + 4.2),
                ];
                if let Some(d) = curve(&tooth, true) {
                    cv.add(
                        path_shape(d),
                        outlined(Some(p.white), p.ink, min(pw * 0.55, 1.6)),
                        "dh-tooth",
                    );
                }
            }
        }
        // Marks at the corners: a dimple at each, or one tick at the higher corner of a crooked mouth.
        let smirk = mm.kind == "smirk";
        if mm.dimples || smirk || m.ticks {
            for s in [-1.0, 1.0] {
                if smirk && !m.ticks && s != mm.side {
                    continue;
                }
                let q = at(s);
                pen(
                    cv,
                    &[
                        pt(q.x + s * 1.2, q.y - 3.6),
                        pt(q.x + s * 2.8, q.y),
                        pt(q.x + s * 1.2, q.y + 3.4),
                    ],
                    pw * 0.62,
                    p.ink,
                    Profile::Leaf,
                    Some(3),
                    None,
                    "dh-dimple",
                );
            }
        }
    }
    // The tip of the tongue at one corner, as when concentrating.
    if m.tongue_out != 0.0 {
        let s = m.tongue_out;
        let o = at(s * 0.5);
        let r = max(5.6, w * 0.34);
        let tip = [
            sharp(o.x - r, o.y),
            pt(o.x - r * 0.86, o.y + r * 0.8),
            pt(o.x, o.y + r * 1.25),
            pt(o.x + r * 0.86, o.y + r * 0.8),
            sharp(o.x + r, o.y),
        ];
        if let Some(d) = curve(&tip, true) {
            cv.add(
                path_shape(d),
                outlined(Some(p.tongue), p.ink, pw * 0.7),
                "dh-tongue",
            );
        }
    }
    // A drop at the corner of a mouth that has fallen asleep.
    if m.drool != 0.0 && !ctx.tiny {
        let q = at(m.drool);
        cv.add(
            path_shape(drop_contour(q.x + m.drool * 1.5, q.y + 6.5, 3.4)),
            Look {
                fill: Some(p.drop),
                stroke: Some(stroke(p.drop_edge, 90, StrokeCap::Butt, StrokeJoin::Miter)),
                opacity: None,
                clip: None,
            },
            "dh-drool",
        );
    }
}

fn cheeks(cv: &mut Canvas, c: &Creature, pose: &Pose, ctx: &Ctx) {
    let k = &c.marks;
    let p = &c.palette;
    let e = &c.eyes;
    let sz = c.head.size;
    let cy = lerp(e.y + e.bl, c.mouth.y, 0.42);
    let rx = 10.5 * sz;
    let ry = 6.8 * sz;
    let cx = min(e.x + e.a * 0.35, c.head.hw(cy) - rx - 3.0);
    let own = k.cheek & 1 == 1;
    let blush = own || pose.blush;
    let flush = pose.flush;
    if blush || flush {
        for s in [-1.0, 1.0] {
            cv.add(
                Shape::Ellipse {
                    cx: tenth(s * cx),
                    cy: tenth(cy),
                    rx: tenth(rx * if flush { 1.15 } else { 1.0 }),
                    ry: tenth(ry * if flush { 1.2 } else { 1.0 }),
                },
                Look {
                    opacity: Some(if flush {
                        0.36
                    } else if pose.blush && own {
                        0.6
                    } else {
                        0.44
                    }),
                    ..solid(if flush { p.flush } else { p.blush })
                },
                "dh-blush",
            );
        }
    }
    if k.cheek & 2 == 2 && ctx.fine {
        let ink = crate::palette::blend(p.skin_dark, p.ink, 0.3);
        for s in [-1.0, 1.0] {
            for f in [
                [-9.0, 0.0],
                [-4.5, -3.0],
                [0.0, 1.2],
                [4.5, -2.4],
                [9.0, 0.6],
                [-2.4, 4.2],
                [3.4, 3.6],
            ] {
                cv.add(
                    circle_r(s * (cx + f[0] * sz * 0.9), cy - 2.0 + f[1] * sz * 0.9, 115),
                    solid(ink),
                    "dh-freckle",
                );
            }
        }
    }
}

/// Lines of age, drawn only from 96 px up, and a mole.
fn age(cv: &mut Canvas, c: &Creature, ctx: &Ctx) {
    let k = &c.marks;
    let p = &c.palette;
    let e = &c.eyes;
    let b = &c.brows;
    let m = &c.mouth;
    let f = &c.face;
    let hd = &c.head;
    let sz = hd.size;
    if ctx.fine {
        let thin = |cv: &mut Canvas, pts: &[Pt], w: f64| {
            pen(
                cv,
                pts,
                ctx.pen_w * w,
                p.ink,
                Profile::Leaf,
                Some(3),
                Some(0.62),
                "dh-age",
            );
        };
        for i in 0..k.brow {
            let i = f64::from(i);
            let y = b.y - b.w / 2.0 - 6.0 - 5.6 * i;
            let l = e.x + e.a * 0.4 - i * 3.5;
            thin(
                cv,
                &[pt(-l, y + 1.2), pt(0.0, y - 0.7), pt(l, y + 1.2)],
                0.62,
            );
        }
        for s in [-1.0, 1.0] {
            let inside = |x: f64, y: f64| pt(s * min(x, hd.hw(y) - 3.5), y);
            if k.fold != 0 {
                let mut pts = vec![
                    inside(c.nose.w * 0.9 + 3.6, f.nose_y - 1.5),
                    inside((m.w + 5.5) * 0.86, lerp(f.nose_y, m.y, 0.55)),
                    inside(m.w + 6.0, m.y - 2.5),
                ];
                if k.fold > 1 {
                    pts.push(inside(m.w + 5.0, m.y + 5.0));
                }
                thin(cv, &pts, 0.62);
            }
            if k.bag != 0 {
                let y = e.y + e.bl + 3.4;
                thin(
                    cv,
                    &[
                        pt(s * (e.x - e.a * 0.72), y - 0.6),
                        pt(s * e.x, y + 1.7),
                        pt(s * (e.x + e.a * 0.78), y - 0.8),
                    ],
                    0.62,
                );
                if k.bag > 1 {
                    thin(
                        cv,
                        &[
                            pt(s * (e.x - e.a * 0.4), y + 3.0),
                            pt(s * (e.x + e.a * 0.08), y + 4.4),
                            pt(s * (e.x + e.a * 0.56), y + 2.8),
                        ],
                        0.62,
                    );
                }
            }
            let x0 = e.x + e.a + 2.8;
            if k.crow && x0 + 6.6 <= hd.hw(e.y) - 2.0 {
                thin(
                    cv,
                    &[pt(s * x0, e.y - 0.9), pt(s * (x0 + 6.0), e.y - 3.7)],
                    0.62,
                );
                thin(
                    cv,
                    &[pt(s * (x0 + 0.4), e.y + 1.6), pt(s * (x0 + 6.2), e.y + 2.7)],
                    0.62,
                );
            }
            if k.frown {
                thin(cv, &[pt(s * 2.7, b.y - 1.5), pt(s * 3.4, b.y + 5.5)], 0.62);
            }
            if k.gaunt {
                let y = lerp(e.y + e.bl, m.y, 0.42);
                let x = hd.hw(y) - 5.5;
                thin(cv, &[pt(s * x, y - 3.0), pt(s * (x - 8.5), y + 6.5)], 0.62);
            }
        }
        if k.chin == 1 {
            thin(cv, &[pt(0.0, -10.5 * sz), pt(0.5, -4.5 * sz)], 0.7);
        }
        if k.chin == 2 {
            thin(
                cv,
                &[
                    pt(-8.5 * sz, -9.5 * sz),
                    pt(0.0, -7.0 * sz),
                    pt(8.5 * sz, -9.5 * sz),
                ],
                0.62,
            );
        }
    }
    if k.mole >= 0 && !ctx.tiny {
        let q = [
            pt(m.w + 5.5, m.y - 7.0),
            pt(-(m.w + 5.5), m.y - 7.0),
            pt(e.x + 3.0, lerp(e.y + e.bl, m.y, 0.45)),
        ][k.mole as usize];
        cv.add(circle_r(q.x, q.y, 180), solid(p.ink), "dh-mole");
    }
}

/// A cog wheel, as an outline with a hole in it.
fn cog(cx: f64, cy: f64, r: f64) -> [Contour; 2] {
    let mut pts = Vec::new();
    let mut hole = Vec::new();
    for k in 0..8 {
        let k = f64::from(k);
        for t in [[-13.0, 0.72], [-8.0, 1.0], [8.0, 1.0], [13.0, 0.72]] {
            let u = up_dir(k * 45.0 + t[0]);
            pts.push(sharp(cx + u.0 * r * t[1], cy + u.1 * r * t[1]));
        }
        let v = up_dir(k * 45.0 + 22.5);
        hole.push(sharp(cx + v.0 * r * 0.33, cy + v.1 * r * 0.33));
    }
    [
        curve(&pts, true).expect("a cog"),
        curve(&hole, true).expect("a cog"),
    ]
}

/// The marks that float beside the head: cogs and sweat (working), dots
/// (waiting), an anger mark and steam (blocked), Zz (sleepy). They hang just
/// outside the head and its hair and do not tip with it.
fn marks(cv: &mut Canvas, c: &Creature, pose: &Pose, ctx: &Ctx) {
    let p = &c.palette;
    let f = c.fit;
    let hm = crate::geom::matrix_of(&head_ops(&c.head, pose));
    let pts = through(&hm, &c.reach);
    let hb = box_of(&pts);
    let edge_at = |sd: f64| {
        if sd > 0.0 {
            (232.0 - f.x) / f.s
        } else {
            (f.x - 8.0) / f.s
        }
    };
    // How far the head reaches on one side, between two heights.
    let reach_of = |sd: f64, y0: f64, y1: f64| {
        let mut r: f64 = 0.0;
        for q in &pts {
            if q.y > y0 && q.y < y1 {
                r = max(r, sd * q.x);
            }
        }
        r
    };
    // Where the mark beside the head goes. It is placed first, so that the steam can keep clear of it.
    let sd = if pose.mark_side != 0.0 {
        pose.mark_side
    } else {
        1.0
    };
    let k = ctx.mark_k;
    let mark = pose.mark;
    let big = match mark {
        Some(MarkKind::Anger) => 15.0,
        Some(MarkKind::Busy) => 27.0,
        _ => 29.0,
    };
    let up = match mark {
        Some(MarkKind::Anger) => 13.0,
        Some(MarkKind::Dots) => 21.0,
        Some(MarkKind::Busy) => 24.0,
        _ => 28.0,
    };
    let y0 = max(hb[1] + (hb[3] - hb[1]) * 0.14, (8.0 - f.y) / f.s + up);
    let x0 = sd
        * min(
            reach_of(sd, y0 - up - 6.0, y0 + 10.0) + 8.0,
            edge_at(sd) - big - 5.0,
        );
    if pose.steam && ctx.size >= 48.0 {
        // Over each side of the crown a cloud of three rounds, with a small
        // puff drifting up and away from it, never lower than the top tenth of the head.
        let roof = (8.0 - f.y) / f.s;
        let half = (hb[2] - hb[0]) / 2.0;
        let mid = (hb[0] + hb[2]) / 2.0;
        let edge_w = clamp(0.85 / ctx.px, 1.3, 2.6);
        // The highest point of the head between two x.
        let top_of = |xa: f64, xb: f64| {
            let mut y = hb[3];
            for q in &pts {
                if q.x >= xa && q.x <= xb && q.y < y {
                    y = q.y;
                }
            }
            y
        };
        let mut clouds = Vec::new();
        for s in [-1.0, 1.0] {
            // The middle of the cloud: 13 units above the head there, where the frame allows it.
            let place = |share: f64| {
                let x = mid + s * share * half;
                let y = min(
                    top_of(x - 11.0, x + 11.0) - 13.0,
                    hb[1] + (hb[3] - hb[1]) * 0.1,
                );
                (x, max(y, roof + 12.5), y - 12.5 - roof)
            };
            let mut best: Option<(f64, f64, f64)> = None;
            for share in [0.42, 0.58, 0.74, 0.9, 1.06] {
                let cl = place(share);
                // Inside the frame, and clear of the anger mark.
                if s * cl.0 + 13.5 > edge_at(s) {
                    continue;
                }
                if mark == Some(MarkKind::Anger)
                    && (cl.0 + s * 1.5 - (x0 + sd * 6.0)).abs() < 23.0
                    && (cl.1 - 3.0 - (y0 - 3.0)).abs() < 21.0
                {
                    continue;
                }
                if best.is_none_or(|b| cl.2 > b.2) {
                    best = Some(cl);
                }
                if cl.2 >= 0.0 {
                    break;
                }
            }
            clouds.push((s, best.unwrap_or_else(|| place(0.42))));
        }
        let rounds = |s: f64| [[0.0, 0.0, 6.6], [s * 6.2, 1.6, 5.0], [-s * 5.6, 2.0, 4.6]];
        for &(s, cl) in &clouds {
            for r in rounds(s) {
                cv.add(
                    circle(cl.0 + r[0], cl.1 + r[1], r[2]),
                    Look {
                        fill: None,
                        stroke: Some(stroke(
                            p.steam_edge,
                            hundredth(2.0 * edge_w),
                            StrokeCap::Butt,
                            StrokeJoin::Miter,
                        )),
                        opacity: None,
                        clip: None,
                    },
                    "dh-steam-edge",
                );
            }
        }
        for &(s, cl) in &clouds {
            for r in rounds(s) {
                cv.add(
                    circle(cl.0 + r[0], cl.1 + r[1], r[2]),
                    solid(p.steam),
                    "dh-steam",
                );
            }
            cv.add(
                circle_r(cl.0 + s * 10.0, cl.1 - 9.2, 310),
                Look {
                    stroke: Some(stroke(
                        p.steam_edge,
                        hundredth(edge_w),
                        StrokeCap::Butt,
                        StrokeJoin::Miter,
                    )),
                    ..solid(p.steam)
                },
                "dh-steam",
            );
        }
    }
    // Two beads of sweat fly off a head that is hard at work, on the side away from its mark.
    if pose.sweat != 0.0 {
        let s = pose.sweat;
        let y = hb[1] + (hb[3] - hb[1]) * 0.26;
        let x = s * min(reach_of(s, y - 16.0, y + 12.0) + 8.0, edge_at(s) - 12.0);
        cv.add(
            compound(
                vec![
                    drop_contour(x, y, 4.8 * ctx.mark_k),
                    drop_contour(x + s * 6.5, y - 12.0, 3.4 * ctx.mark_k),
                ],
                FillRule::NonZero,
            ),
            Look {
                fill: Some(p.drop),
                stroke: Some(stroke(p.drop_edge, 100, StrokeCap::Butt, StrokeJoin::Miter)),
                opacity: None,
                clip: None,
            },
            "dh-sweat",
        );
    }
    match mark {
        None => {}
        Some(MarkKind::Dots) => {
            for i in 0..3 {
                let i = f64::from(i);
                cv.add(
                    circle(x0 + sd * i * 9.5, y0 - i * 5.5, 3.3 * k),
                    solid(p.mark_ink),
                    "dh-mark",
                );
            }
        }
        Some(MarkKind::Anger) => {
            // Four corners round a gap: the mark comics draw on a cross forehead.
            let cx = x0 + sd * 6.0;
            let cy = y0 - 3.0;
            let mut d = Vec::new();
            for u in [1.0, -1.0] {
                for v in [1.0, -1.0] {
                    d.push(polyline(&[
                        (cx + u * 7.0, cy + v * 2.0),
                        (cx + u * 2.0, cy + v * 2.0),
                        (cx + u * 2.0, cy + v * 7.0),
                    ]));
                }
            }
            line(cv, d, 2.7 * k, p.anger, None, None, "dh-mark");
        }
        Some(MarkKind::Busy) => {
            let mut d = cog(x0 + sd * 9.0, y0 - 4.0, 8.6 * min(k, 1.25)).to_vec();
            d.extend(cog(x0 + sd * 21.5, y0 - 16.5, 5.6 * min(k, 1.25)));
            cv.add(compound(d, FillRule::EvenOdd), solid(p.mark_ink), "dh-mark");
        }
        Some(MarkKind::Zz) => {
            let zed = |x: f64, y: f64, s: f64| {
                polyline(&[(x, y), (x + s, y), (x, y + s), (x + s, y + s)])
            };
            let a = (if sd > 0.0 { x0 } else { x0 - 8.0 }, y0 - 6.0);
            let b = (if sd > 0.0 { x0 + 10.0 } else { x0 - 22.0 }, y0 - 24.0);
            line(
                cv,
                vec![zed(a.0, a.1, 8.0), zed(b.0, b.1, 12.0)],
                2.5 * k,
                p.mark_ink,
                None,
                None,
                "dh-mark",
            );
        }
    }
}
