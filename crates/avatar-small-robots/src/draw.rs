//! The drawing kit: how fine a drawing is at its size, where primitives are
//! collected, and the pieces every part is drawn from (shells, rods, discs,
//! plates, seams, rivets, lamps and rays).

use avatar_svg::{
    FillRule, Paint, Primitive, Rgb, Role, Shape, Stroke, StrokeCap, StrokeJoin, TransformOp, Q32,
};

use crate::geom::{
    box_of, h1, h2, line_path, place, poly_path, poly_sample, quad_at, shift_pts, tree_ops,
    unit_of, up_dir, Cp, Op, Path,
};
use crate::kit::{blend, clamp, hex, hyp, max, min, Material, P2};

/// How fine one drawing is. Under 96 px the rivets, seams and other small
/// details are left out and lines are kept about a pixel wide, so that the
/// outline and the face still read in an icon.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ctx {
    pub(crate) size: f64,
    pub(crate) px: f64,
    pub(crate) fine: bool,
    pub(crate) shade: bool,
    pub(crate) line_w: f64,
    pub(crate) face_w: f64,
    pub(crate) thin: f64,
    pub(crate) mark_k: f64,
}

impl Ctx {
    pub(crate) fn new(size: f64) -> Self {
        let px = size / 240.0;
        Self {
            size,
            px,
            fine: size >= 96.0,
            shade: size >= 56.0,
            line_w: clamp(1.05 / px, 2.2, 8.5),
            face_w: clamp(1.3 / px, 3.2, 7.8),
            thin: 1.3 / px,
            mark_k: clamp(64.0 / size, 1.0, 1.5),
        }
    }
}

/// One element as the reference writes it: a shape, its fill (none when
/// unset), an optional stroke, opacity and clip, and its class.
pub(crate) struct El {
    shape: Option<Shape>,
    fill: Option<Rgb>,
    stroke: Option<(Rgb, i64)>,
    cap: StrokeCap,
    join: StrokeJoin,
    opacity: Option<f64>,
    clip: Option<Shape>,
    class: Option<&'static str>,
    role: Option<Role>,
}

impl El {
    fn of(shape: Option<Shape>) -> Self {
        Self {
            shape,
            fill: None,
            stroke: None,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
            opacity: None,
            clip: None,
            class: None,
            role: None,
        }
    }

    pub(crate) fn path(d: &Path) -> Self {
        Self::of(d.shape(FillRule::NonZero))
    }

    pub(crate) fn path_rule(d: &Path, rule: FillRule) -> Self {
        Self::of(d.shape(rule))
    }

    pub(crate) fn circle(p: P2, r: f64) -> Self {
        Self::of(Some(Shape::Circle {
            cx: h1(p[0]),
            cy: h1(p[1]),
            r: h1(r),
        }))
    }

    pub(crate) fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Self {
        Self::of(Some(Shape::Ellipse {
            cx: h1(cx),
            cy: h1(cy),
            rx: h1(rx),
            ry: h1(ry),
        }))
    }

    pub(crate) fn rect(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self::of(Some(Shape::Rect {
            x: h1(x),
            y: h1(y),
            w: h1(w),
            h: h1(h),
        }))
    }

    pub(crate) fn fill(mut self, c: Rgb) -> Self {
        self.fill = Some(c);
        self
    }

    /// A stroke `w` units wide, rounded to the hundredth the reference writes.
    pub(crate) fn stroke(mut self, c: Rgb, w: f64) -> Self {
        self.stroke = Some((c, h2(w)));
        self
    }

    pub(crate) fn round_cap(mut self) -> Self {
        self.cap = StrokeCap::Round;
        self
    }

    pub(crate) fn square_cap(mut self) -> Self {
        self.cap = StrokeCap::Square;
        self
    }

    pub(crate) fn round_join(mut self) -> Self {
        self.join = StrokeJoin::Round;
        self
    }

    pub(crate) fn round(self) -> Self {
        self.round_cap().round_join()
    }

    pub(crate) fn opacity(mut self, o: f64) -> Self {
        self.opacity = Some(o);
        self
    }

    pub(crate) fn clip(mut self, clip: Option<Shape>) -> Self {
        self.clip = clip;
        self
    }

    pub(crate) fn class(mut self, class: &'static str) -> Self {
        self.class = Some(class);
        self
    }

    pub(crate) fn ground_shadow(mut self) -> Self {
        self.role = Some(Role::GroundShadow);
        self
    }
}

/// The clip a closed path makes.
pub(crate) fn clip_of(d: &Path) -> Option<Shape> {
    d.shape(FillRule::NonZero)
}

/// An opacity written to a hundredth, as the nearest Q32.32 value; `None`
/// when it is fully opaque.
fn opacity_q32(o: f64) -> Option<Q32> {
    let hundredths = clamp(h2(o) as f64, 0.0, 100.0) as i64;
    (hundredths < 100).then(|| Q32((((i128::from(hundredths) << 32) + 50) / 100) as i64))
}

/// Where a drawing's primitives are collected, in painter's order, with the
/// transform of the groups being drawn into.
#[derive(Clone, Debug, Default)]
pub(crate) struct Out {
    ops: Vec<TransformOp>,
    label: &'static str,
    pub(crate) prims: Vec<Primitive>,
}

impl Out {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// An empty collection in the same group, for something drawn first and
    /// placed later.
    pub(crate) fn sub(&self) -> Self {
        Self {
            ops: self.ops.clone(),
            label: self.label,
            prims: Vec::new(),
        }
    }

    pub(crate) fn extend(&mut self, other: Out) {
        self.prims.extend(other.prims);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.prims.is_empty()
    }

    /// Draws `f` in a group named `class` that applies `ops`.
    pub(crate) fn group(&mut self, class: &'static str, ops: &[Op], f: impl FnOnce(&mut Out)) {
        let depth = self.ops.len();
        let label = std::mem::replace(&mut self.label, class);
        self.ops.extend(tree_ops(ops));
        f(self);
        self.ops.truncate(depth);
        self.label = label;
    }

    pub(crate) fn put(&mut self, e: El) {
        let Some(shape) = e.shape else { return };
        let ops = self.ops.clone();
        self.prims.push(Primitive {
            shape,
            transform: (!ops.is_empty()).then_some(avatar_svg::Transform { ops }),
            paint: e.fill.map_or(Paint::None, |color| Paint::Solid { color }),
            stroke: e.stroke.map(|(color, width)| Stroke {
                color,
                width,
                cap: e.cap,
                join: e.join,
            }),
            opacity: e.opacity.and_then(opacity_q32),
            clip: e.clip,
            effects: Vec::new(),
            label: e.class.unwrap_or(self.label).to_owned(),
            role: e.role,
        });
    }
}

/// Options of a hard-edged piece: how thick its outline is, what is painted
/// on it, whether it is flat and whether it shows a line of light.
pub(crate) struct ShellOpts {
    pub(crate) line_k: f64,
    pub(crate) over: Option<Out>,
    pub(crate) flat: bool,
    pub(crate) light: bool,
}

impl Default for ShellOpts {
    fn default() -> Self {
        Self {
            line_k: 1.0,
            over: None,
            flat: false,
            light: true,
        }
    }
}

/// The line of light along the top of a piece: the part of its outline that
/// faces up, moved a little way inside.
fn light_line(pts: &[Cp], inset: f64) -> Path {
    let poly = poly_sample(pts, 5);
    let n = poly.len();
    let mut area = 0.0;
    for i in 0..n {
        let p = poly[i];
        let q = poly[(i + 1) % n];
        area += p[0] * q[1] - q[0] * p[1];
    }
    let sign = if area > 0.0 { 1.0 } else { -1.0 };
    let mut runs: Vec<Vec<P2>> = Vec::new();
    let mut open = false;
    for i in 0..n {
        let a = poly[(i + n - 1) % n];
        let b = poly[(i + 1) % n];
        let mut l = hyp(b[0] - a[0], b[1] - a[1]);
        if l == 0.0 || l.is_nan() {
            l = 1e-9;
        }
        // The normal that points into the piece.
        let nx = -(b[1] - a[1]) / l * sign;
        let ny = (b[0] - a[0]) / l * sign;
        if ny > 0.62 {
            if !open {
                runs.push(Vec::new());
                open = true;
            }
            let run = runs.last_mut().expect("a run was opened");
            run.push([poly[i][0] + nx * inset, poly[i][1] + ny * inset]);
        } else {
            open = false;
        }
    }
    let mut d = Path::new();
    for r in runs {
        if r.len() > 1 {
            d.append(line_path(&r, false));
        }
    }
    d
}

/// A piece with a hard edge, painted or metal: the fill, a shade along its
/// underside, a line of light along its top, and its outline.
pub(crate) fn shell(out: &mut Out, ctx: &Ctx, pts: &[Cp], mat: &Material, o: ShellOpts) {
    let d = poly_path(pts);
    let lw = ctx.line_w * o.line_k;
    if !ctx.shade || o.flat {
        if let Some(over) = o.over.filter(|over| !over.is_empty()) {
            out.put(El::path(&d).fill(mat.base));
            out.extend(over);
            out.put(
                El::path(&d)
                    .class("rb-line")
                    .stroke(mat.line, lw)
                    .round_join(),
            );
        } else {
            out.put(
                El::path(&d)
                    .fill(mat.base)
                    .stroke(mat.line, lw)
                    .round_join(),
            );
        }
        return;
    }
    let b = box_of(&pts.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>());
    let rise = clamp((b[3] - b[1]) * 0.15, 2.0, 16.0);
    out.put(El::path(&d).class("rb-shade").fill(mat.dark));
    out.put(
        El::path(&poly_path(&shift_pts(pts, 0.0, -rise)))
            .class("rb-fill")
            .fill(mat.base)
            .clip(clip_of(&d)),
    );
    if let Some(over) = o.over {
        out.extend(over);
    }
    if ctx.fine && o.light {
        let light = light_line(pts, 4.4);
        if !light.is_empty() {
            out.put(
                El::path(&light)
                    .class("rb-sheen")
                    .stroke(mat.lite, 2.4)
                    .round()
                    .opacity(0.85),
            );
        }
    }
    out.put(
        El::path(&d)
            .class("rb-line")
            .stroke(mat.line, lw)
            .round_join(),
    );
}

/// A flat piece: a fill and an outline, no shading.
pub(crate) fn plate(out: &mut Out, ctx: &Ctx, pts: &[Cp], mat: &Material) {
    plate_k(out, ctx, pts, mat, 1.0);
}

pub(crate) fn plate_k(out: &mut Out, ctx: &Ctx, pts: &[Cp], mat: &Material, line_k: f64) {
    shell(
        out,
        ctx,
        pts,
        mat,
        ShellOpts {
            line_k,
            flat: true,
            ..ShellOpts::default()
        },
    );
}

/// A rod with round ends along any path. Drawn narrower than about a pixel
/// and a half, it is one dark stroke instead of a fill inside an outline.
pub(crate) fn rod(out: &mut Out, ctx: &Ctx, d: &Path, w: f64, mat: &Material, light: bool) {
    if w * ctx.px < 1.7 {
        out.put(
            El::path(d)
                .stroke(
                    blend(mat.line, mat.base, 0.3),
                    max(w, ctx.thin) + ctx.line_w * 0.5,
                )
                .round(),
        );
        return;
    }
    out.put(El::path(d).stroke(mat.line, w + 1.6 * ctx.line_w).round());
    out.put(El::path(d).stroke(mat.base, w).round());
    if ctx.fine && w >= 6.0 && light {
        out.put(El::path(d).stroke(mat.lite, w * 0.24).round().opacity(0.75));
    }
}

/// A limb segment: a straight rod from `a` to `b`.
pub(crate) fn bone(out: &mut Out, ctx: &Ctx, a: P2, b: P2, w: f64, mat: &Material) {
    rod(out, ctx, &line_path(&[a, b], false), w, mat, true);
}

/// A bent rod or a hose from `a` to `b` that bends towards `c`, with `ribs`
/// across it when that is not 0.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hose(
    out: &mut Out,
    ctx: &Ctx,
    a: P2,
    c: P2,
    b: P2,
    w: f64,
    mat: &Material,
    ribs: u32,
) {
    let mut d = Path::new();
    d.move_to(a);
    d.quad_to(a, c, b);
    rod(out, ctx, &d, w, mat, ribs == 0);
    if ctx.fine && ribs > 0 && w * ctx.px >= 1.7 {
        let mut r = Path::new();
        for i in 1..ribs {
            let t = f64::from(i) / f64::from(ribs);
            let p = quad_at(a, c, b, t);
            let u = unit_of(quad_at(a, c, b, t - 0.02), quad_at(a, c, b, t + 0.02));
            r.move_to(place(p, u, -w / 2.0, 0.0));
            r.line_to(place(p, u, w / 2.0, 0.0));
        }
        out.put(El::path(&r).stroke(mat.line, 1.3).opacity(0.8));
    }
}

/// Options of a disc: keep it a disc however small, its own fill, how thick
/// its outline is, and whether it shows a dark middle.
#[derive(Clone, Copy)]
pub(crate) struct DiscOpts {
    pub(crate) keep: bool,
    pub(crate) fill: Option<Rgb>,
    pub(crate) line_k: f64,
    pub(crate) dot: bool,
}

impl Default for DiscOpts {
    fn default() -> Self {
        Self {
            keep: false,
            fill: None,
            line_k: 0.8,
            dot: true,
        }
    }
}

/// No dark middle.
pub(crate) const NO_DOT: DiscOpts = DiscOpts {
    keep: false,
    fill: None,
    line_k: 0.8,
    dot: false,
};

/// A joint, a hub or a rivet head: a disc with a line round it.
pub(crate) fn disc(out: &mut Out, ctx: &Ctx, p: P2, r: f64, mat: &Material, o: DiscOpts) {
    if r * ctx.px < 0.9 && !o.keep {
        out.put(
            El::circle(p, max(r, ctx.thin * 0.6) + ctx.line_w * 0.25)
                .fill(blend(mat.line, mat.base, 0.3)),
        );
        return;
    }
    out.put(
        El::circle(p, r)
            .fill(o.fill.unwrap_or(mat.base))
            .stroke(mat.line, ctx.line_w * o.line_k),
    );
    if ctx.fine && o.dot && r >= 3.5 {
        out.put(El::circle(p, r * 0.36).fill(mat.dark));
    }
}

/// A thin line drawn on a surface: a seam, a slat, a scratch. Only at sizes
/// that show such detail.
pub(crate) fn seam(out: &mut Out, ctx: &Ctx, d: &Path, colour: Rgb, w: f64, opacity: Option<f64>) {
    if !ctx.fine {
        return;
    }
    let mut e = El::path(d).stroke(colour, w).round();
    if let Some(o) = opacity {
        e = e.opacity(o);
    }
    out.put(e);
}

/// Rivet heads at the given points.
pub(crate) fn rivets(out: &mut Out, ctx: &Ctx, list: &[P2], mat: &Material) {
    if !ctx.fine {
        return;
    }
    for p in list {
        out.put(
            El::circle(*p, 1.7)
                .class("rb-rivet")
                .fill(mat.lite)
                .stroke(mat.line, 0.9),
        );
    }
}

/// A small light, never drawn smaller than about a pixel across.
pub(crate) fn lamp(out: &mut Out, ctx: &Ctx, p: P2, r: f64, colour: Rgb, mat: &Material) {
    let rr = max(r, 1.15 / ctx.px);
    out.put(
        El::circle(p, rr)
            .class("rb-light")
            .fill(colour)
            .stroke(mat.line, min(ctx.line_w * 0.7, rr * 0.5)),
    );
    if ctx.fine && r >= 3.6 {
        out.put(
            El::circle([p[0] - r * 0.3, p[1] - r * 0.32], r * 0.24)
                .fill(hex(0xffffff))
                .opacity(0.7),
        );
    }
}

/// The directions of the rays round a flashing light, in degrees from up.
pub(crate) const RAYS: [f64; 5] = [-64.0, -32.0, 0.0, 32.0, 64.0];

/// Short rays round a light that is flashing.
pub(crate) fn rays(out: &mut Out, ctx: &Ctx, p: P2, r: f64, colour: Rgb, spread: &[f64]) {
    if ctx.size < 48.0 {
        return;
    }
    let mut d = Path::new();
    for a in spread {
        let u = up_dir(*a);
        d.move_to([p[0] + u[0] * (r + 3.5), p[1] + u[1] * (r + 3.5)]);
        d.line_to([p[0] + u[0] * (r + 8.5), p[1] + u[1] * (r + 8.5)]);
    }
    out.put(
        El::path(&d)
            .class("rb-rays")
            .stroke(colour, max(1.8, ctx.face_w * 0.5))
            .round_cap(),
    );
}
