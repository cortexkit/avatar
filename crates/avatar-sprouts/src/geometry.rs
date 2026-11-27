//! Splines, outlines and the drawing items a pose is made of.
//!
//! Shapes are lists of points with an optional tension: how round the path
//! is at that point, from 1 (smooth) to 0 (a sharp corner). A path through
//! them is a cardinal spline written as cubic Béziers. Path coordinates are
//! rounded to one decimal when an item is made, as the reference writes them
//! into its SVG; boxes and every measurement use the exact values.

use avatar_svg::{Rgb, Segment, StrokeCap, StrokeJoin};

use crate::maths::{cos, hyp, hyp_or, lerp, max, min, sin, tenths, unit, TAU, V};

/// A spline control point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pt {
    pub(crate) x: f64,
    pub(crate) y: f64,
    /// The tension at this point; `None` is the reference's missing third
    /// coordinate, which counts as 1.
    pub(crate) t: Option<f64>,
}

pub(crate) fn pt(x: f64, y: f64) -> Pt {
    Pt { x, y, t: None }
}

pub(crate) fn ptt(x: f64, y: f64, t: f64) -> Pt {
    Pt { x, y, t: Some(t) }
}

impl Pt {
    pub(crate) fn v(self) -> V {
        [self.x, self.y]
    }
}

impl From<V> for Pt {
    fn from(v: V) -> Self {
        pt(v[0], v[1])
    }
}

/// A point of a limb's axis and the limb's width there.
pub(crate) type W = [f64; 3];

/// A box: left, top, right, bottom.
pub(crate) type Bx = [f64; 4];

pub(crate) fn box_of<I: IntoIterator<Item = V>>(points: I) -> Bx {
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in points {
        if p[0] < x0 {
            x0 = p[0];
        }
        if p[0] > x1 {
            x1 = p[0];
        }
        if p[1] < y0 {
            y0 = p[1];
        }
        if p[1] > y1 {
            y1 = p[1];
        }
    }
    [x0, y0, x1, y1]
}

pub(crate) fn join_box(a: Option<Bx>, b: Option<Bx>) -> Option<Bx> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some([
            min(a[0], b[0]),
            min(a[1], b[1]),
            max(a[2], b[2]),
            max(a[3], b[3]),
        ]),
    }
}

pub(crate) fn pad_box(b: Bx, m: f64) -> Bx {
    [b[0] - m, b[1] - m, b[2] + m, b[3] + m]
}

/// One Bézier piece of a spline.
pub(crate) struct Piece {
    pub(crate) p0: V,
    pub(crate) c1: V,
    pub(crate) c2: V,
    pub(crate) p1: V,
    /// Both handles are zero, so the piece is written as a line.
    pub(crate) line: bool,
}

pub(crate) fn segs_of(pts: &[Pt], closed: bool) -> Vec<Piece> {
    let n = pts.len();
    let mut tangents = Vec::with_capacity(n);
    for i in 0..n {
        let a = if closed {
            pts[(i + n - 1) % n]
        } else {
            pts[i.saturating_sub(1)]
        };
        let b = if closed {
            pts[(i + 1) % n]
        } else {
            pts[(i + 1).min(n - 1)]
        };
        let p = pts[i];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = hyp_or(dx, dy, 1e-9);
        let t = p.t.unwrap_or(1.0);
        // A handle is never longer than 0.4 of the piece it bends, so
        // unevenly spaced points cannot make the curve loop.
        let len_in = hyp(p.x - a.x, p.y - a.y);
        let len_out = hyp(b.x - p.x, b.y - p.y);
        tangents.push([
            dx / len,
            dy / len,
            min(len / 6.0, 0.4 * len_in) * t,
            min(len / 6.0, 0.4 * len_out) * t,
        ]);
    }
    let count = if closed { n } else { n.saturating_sub(1) };
    (0..count)
        .map(|i| {
            let j = (i + 1) % n;
            let (p, q, tp, tq) = (pts[i], pts[j], tangents[i], tangents[j]);
            Piece {
                p0: p.v(),
                c1: [p.x + tp[0] * tp[3], p.y + tp[1] * tp[3]],
                c2: [q.x - tq[0] * tq[2], q.y - tq[1] * tq[2]],
                p1: q.v(),
                line: tp[3] == 0.0 && tq[2] == 0.0,
            }
        })
        .collect()
}

/// One contour of path data, rounded as the reference writes it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sub {
    pub(crate) segments: Vec<Segment>,
    pub(crate) closed: bool,
}

/// Path data: the reference's `d` string as contours.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PathD {
    pub(crate) subs: Vec<Sub>,
}

fn move_seg(p: V) -> Segment {
    Segment::MoveTo {
        x: tenths(p[0]),
        y: tenths(p[1]),
    }
}

fn line_seg(p: V) -> Segment {
    Segment::LineTo {
        x: tenths(p[0]),
        y: tenths(p[1]),
    }
}

fn cubic_seg(c1: V, c2: V, p: V) -> Segment {
    Segment::CubicTo {
        cp1x: tenths(c1[0]),
        cp1y: tenths(c1[1]),
        cp2x: tenths(c2[0]),
        cp2y: tenths(c2[1]),
        x: tenths(p[0]),
        y: tenths(p[1]),
    }
}

impl PathD {
    pub(crate) fn append(&mut self, other: PathD) {
        self.subs.extend(other.subs);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }

    /// `M a L b`: one open stroke.
    pub(crate) fn push_line(&mut self, a: V, b: V) {
        self.subs.push(Sub {
            segments: vec![move_seg(a), line_seg(b)],
            closed: false,
        });
    }

    /// `M at C .. C .. Z`: a closed contour of cubics.
    pub(crate) fn push_cubics(&mut self, start: V, pieces: &[(V, V, V)], closed: bool) {
        let mut segments = vec![move_seg(start)];
        for &(c1, c2, p) in pieces {
            segments.push(cubic_seg(c1, c2, p));
        }
        self.subs.push(Sub { segments, closed });
    }
}

/// The spline through `pts`, closed or open.
pub(crate) fn path_of(pts: &[Pt], closed: bool) -> PathD {
    if pts.len() < 2 {
        return PathD::default();
    }
    let mut segments = vec![move_seg(pts[0].v())];
    for s in segs_of(pts, closed) {
        segments.push(if s.line {
            line_seg(s.p1)
        } else {
            cubic_seg(s.c1, s.c2, s.p1)
        });
    }
    PathD {
        subs: vec![Sub { segments, closed }],
    }
}

/// Straight lines through `pts`.
pub(crate) fn line_path(pts: &[V], closed: bool) -> PathD {
    let mut segments = vec![move_seg(pts[0])];
    for &p in &pts[1..] {
        segments.push(line_seg(p));
    }
    PathD {
        subs: vec![Sub { segments, closed }],
    }
}

/// The spline as a dense polyline, `per` points to a piece.
pub(crate) fn sample_of(pts: &[Pt], closed: bool, per: usize) -> Vec<V> {
    let mut out = Vec::new();
    for s in segs_of(pts, closed) {
        for k in 0..per {
            let t = k as f64 / per as f64;
            let u = 1.0 - t;
            let a = u * u * u;
            let b = 3.0 * u * u * t;
            let c = 3.0 * u * t * t;
            let e = t * t * t;
            out.push([
                a * s.p0[0] + b * s.c1[0] + c * s.c2[0] + e * s.p1[0],
                a * s.p0[1] + b * s.c1[1] + c * s.c2[1] + e * s.p1[1],
            ]);
        }
    }
    if !closed {
        out.push(pts[pts.len() - 1].v());
    }
    out
}

/// Twice the signed area: positive when the points run clockwise on screen.
pub(crate) fn area2(pts: &[V]) -> f64 {
    let mut s = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        s += a[0] * b[1] - b[0] * a[1];
    }
    s
}

/// The same outline, running clockwise. Shapes filled together as one path
/// must all turn the same way, or their overlaps come out as holes.
pub(crate) fn clockwise(mut pts: Vec<Pt>) -> Vec<Pt> {
    let flat: Vec<V> = pts.iter().map(|p| p.v()).collect();
    if area2(&flat) < 0.0 {
        pts.reverse();
    }
    pts
}

fn clockwise_v(mut pts: Vec<V>) -> Vec<V> {
    if area2(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

/// Even-odd point-in-polygon test.
pub(crate) fn inside(poly: &[V], x: f64, y: f64) -> bool {
    let mut hit = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        if (a[1] > y) != (b[1] > y) && x < a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) {
            hit = !hit;
        }
        j = i;
    }
    hit
}

/// A circle as four cubics, clockwise, for use as one piece of a larger path.
pub(crate) fn circle_d(cx: f64, cy: f64, r: f64) -> PathD {
    let k = 0.5523 * r;
    let mut d = PathD::default();
    d.push_cubics(
        [cx + r, cy],
        &[
            ([cx + r, cy + k], [cx + k, cy + r], [cx, cy + r]),
            ([cx - k, cy + r], [cx - r, cy + k], [cx - r, cy]),
            ([cx - r, cy - k], [cx - k, cy - r], [cx, cy - r]),
            ([cx + k, cy - r], [cx + r, cy - k], [cx + r, cy]),
        ],
        true,
    );
    d
}

/// A smooth run of points through control points `[x, y, w]`, carrying the
/// widths along with the positions.
pub(crate) fn smooth_axis(ctrl: &[W], per: usize) -> Vec<W> {
    let flat: Vec<Pt> = ctrl.iter().map(|p| pt(p[0], p[1])).collect();
    let pos = sample_of(&flat, false, per);
    let mut out = Vec::with_capacity(pos.len());
    for (i, p) in pos.iter().enumerate() {
        let f = i as f64 / per as f64;
        let k = min((ctrl.len() - 2) as f64, f.floor());
        let t = f - k;
        let k = k as usize;
        out.push([p[0], p[1], lerp(ctrl[k][2], ctrl[k + 1][2], t)]);
    }
    out
}

/// The outline of a limb: the axis thickened to its widths, cut square
/// where it starts and rounded at the tip.
fn ribbon(axis: &[W]) -> Vec<Pt> {
    let n = axis.len();
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        let a = axis[i.saturating_sub(1)];
        let b = axis[(i + 1).min(n - 1)];
        let t = unit(b[0] - a[0], b[1] - a[1]);
        let h = axis[i][2] / 2.0;
        left.push([axis[i][0] + t[1] * h, axis[i][1] - t[0] * h]);
        right.push([axis[i][0] - t[1] * h, axis[i][1] + t[0] * h]);
    }
    let a = axis[n - 2];
    let b = axis[n - 1];
    let t = unit(b[0] - a[0], b[1] - a[1]);
    let h = b[2] / 2.0;
    let mut out = vec![ptt(left[0][0], left[0][1], 0.0)];
    for p in &left[1..] {
        out.push(pt(p[0], p[1]));
    }
    out.push(pt(b[0] + t[0] * h, b[1] + t[1] * h));
    for i in (1..n).rev() {
        out.push(pt(right[i][0], right[i][1]));
    }
    out.push(ptt(right[0][0], right[0][1], 0.0));
    out
}

/// Whether an axis bends gently enough for its two edges not to cross.
fn gentle(axis: &[W]) -> bool {
    for i in 1..axis.len().saturating_sub(1) {
        let (a, p, b) = (axis[i - 1], axis[i], axis[i + 1]);
        let u = unit(p[0] - a[0], p[1] - a[1]);
        let v = unit(b[0] - p[0], b[1] - p[1]);
        let turn = (u[0] * v[1] - u[1] * v[0]).abs();
        let step = min(hyp(p[0] - a[0], p[1] - a[1]), hyp(b[0] - p[0], b[1] - p[1]));
        if u[0] * v[0] + u[1] * v[1] < 0.2 || turn * p[2] * 0.6 > step {
            return false;
        }
    }
    true
}

/// The path of a limb and the box it covers. A gently bent limb is one
/// smooth outline; a sharply bent one is a quadrilateral for each stretch
/// and a disc at each joint, which fill to the same shape without loops.
pub(crate) fn limb_path(axis: &[W]) -> (PathD, Bx) {
    if axis.len() >= 2 && gentle(axis) {
        return (path_of(&clockwise(ribbon(axis)), true), limb_box(axis));
    }
    let mut d = PathD::default();
    for i in 0..axis.len() {
        let p = axis[i];
        if i > 0 {
            d.append(circle_d(p[0], p[1], p[2] / 2.0));
        }
        if i == axis.len() - 1 {
            break;
        }
        let q = axis[i + 1];
        let t = unit(q[0] - p[0], q[1] - p[1]);
        let hp = p[2] / 2.0;
        let hq = q[2] / 2.0;
        d.append(line_path(
            &clockwise_v(vec![
                [p[0] + t[1] * hp, p[1] - t[0] * hp],
                [q[0] + t[1] * hq, q[1] - t[0] * hq],
                [q[0] - t[1] * hq, q[1] + t[0] * hq],
                [p[0] - t[1] * hp, p[1] + t[0] * hp],
            ]),
            true,
        ));
    }
    (d, limb_box(axis))
}

pub(crate) fn limb_box(axis: &[W]) -> Bx {
    let mut bx: Option<Bx> = None;
    for p in axis {
        bx = join_box(
            bx,
            Some([
                p[0] - p[2] / 2.0,
                p[1] - p[2] / 2.0,
                p[0] + p[2] / 2.0,
                p[1] + p[2] / 2.0,
            ]),
        );
    }
    bx.expect("a limb axis has points")
}

/// Keeps the part of a polygon where `(p - o) . n <= 0`, and returns the
/// points where the line cut it.
pub(crate) fn clip_half(poly: &[V], o: V, n: V) -> (Vec<V>, Vec<V>) {
    let mut out = Vec::new();
    let mut cuts = Vec::new();
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let da = (a[0] - o[0]) * n[0] + (a[1] - o[1]) * n[1];
        let db = (b[0] - o[0]) * n[0] + (b[1] - o[1]) * n[1];
        if da <= 0.0 {
            out.push(a);
        }
        if (da <= 0.0) != (db <= 0.0) {
            let t = da / (da - db);
            let c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            out.push(c);
            cuts.push(c);
        }
    }
    (out, cuts)
}

pub(crate) fn ellipse_poly(cx: f64, cy: f64, rx: f64, ry: f64, count: usize) -> Vec<V> {
    (0..count)
        .map(|i| {
            let a = TAU * i as f64 / count as f64;
            [cx + rx * cos(a), cy + ry * sin(a)]
        })
        .collect()
}

/// How an item is painted. `sw` is a stroke width in units of the
/// 240-unit frame; `wm` is added to it in the creature's own units.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Style {
    pub(crate) fill: Option<Rgb>,
    pub(crate) stroke: Option<Rgb>,
    pub(crate) sw: f64,
    pub(crate) wm: f64,
    pub(crate) cap: Option<StrokeCap>,
    pub(crate) join: Option<StrokeJoin>,
    pub(crate) op: Option<f64>,
    pub(crate) cls: String,
}

impl Style {
    pub(crate) fn fill(colour: Rgb, cls: &str) -> Self {
        Self {
            fill: Some(colour),
            cls: cls.to_owned(),
            ..Self::default()
        }
    }

    /// A fill with an outline drawn in `stroke`.
    pub(crate) fn filled(colour: Rgb, stroke: Rgb, sw: f64, cls: &str) -> Self {
        Self {
            fill: Some(colour),
            stroke: Some(stroke),
            sw,
            cls: cls.to_owned(),
            ..Self::default()
        }
    }

    /// A line with no fill.
    pub(crate) fn line(stroke: Rgb, sw: f64, cls: &str) -> Self {
        Self {
            stroke: Some(stroke),
            sw,
            cls: cls.to_owned(),
            ..Self::default()
        }
    }

    pub(crate) fn round_join(mut self) -> Self {
        self.join = Some(StrokeJoin::Round);
        self
    }

    pub(crate) fn round_cap(mut self) -> Self {
        self.cap = Some(StrokeCap::Round);
        self
    }

    pub(crate) fn op(mut self, op: f64) -> Self {
        self.op = Some(op);
        self
    }
}

/// What an item draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Geo {
    Path(PathD),
    Circle { cx: f64, cy: f64, r: f64 },
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64 },
}

/// One drawing item, or a group of them, with the box it covers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Node {
    Item { geo: Geo, style: Style, bx: Bx },
    Group { items: Vec<Node>, bx: Option<Bx> },
}

impl Node {
    pub(crate) fn bx(&self) -> Option<Bx> {
        match self {
            Node::Item { bx, .. } => Some(*bx),
            Node::Group { bx, .. } => *bx,
        }
    }

    pub(crate) fn style_mut(&mut self) -> &mut Style {
        match self {
            Node::Item { style, .. } => style,
            Node::Group { .. } => unreachable!("only items carry a style"),
        }
    }
}

pub(crate) fn group(items: Vec<Node>) -> Node {
    let bx = items.iter().fold(None, |b, it| join_box(b, it.bx()));
    Node::Group { items, bx }
}

pub(crate) fn path_item(d: PathD, bx: Bx, style: Style) -> Node {
    Node::Item {
        geo: Geo::Path(d),
        style,
        bx,
    }
}

/// A closed smooth shape through points.
pub(crate) fn shape(pts: &[Pt], style: Style) -> Node {
    path_item(path_of(pts, true), box_of(pts.iter().map(|p| p.v())), style)
}

/// A closed shape with straight edges.
pub(crate) fn polygon(pts: &[V], style: Style) -> Node {
    path_item(line_path(pts, true), box_of(pts.iter().copied()), style)
}

/// An open smooth line through points, with round caps and joins unless the
/// style says otherwise.
pub(crate) fn curve(pts: &[Pt], mut style: Style) -> Node {
    style.fill = None;
    style.cap = Some(style.cap.unwrap_or(StrokeCap::Round));
    style.join = Some(style.join.unwrap_or(StrokeJoin::Round));
    path_item(
        path_of(pts, false),
        box_of(pts.iter().map(|p| p.v())),
        style,
    )
}

pub(crate) fn curve_v(pts: &[V], style: Style) -> Node {
    let pts: Vec<Pt> = pts.iter().map(|&p| p.into()).collect();
    curve(&pts, style)
}

pub(crate) fn disc(cx: f64, cy: f64, r: f64, style: Style) -> Node {
    Node::Item {
        geo: Geo::Circle { cx, cy, r },
        style,
        bx: [cx - r, cy - r, cx + r, cy + r],
    }
}

pub(crate) fn oval(cx: f64, cy: f64, rx: f64, ry: f64, style: Style) -> Node {
    let m = max(rx, ry);
    Node::Item {
        geo: Geo::Ellipse { cx, cy, rx, ry },
        style,
        bx: [cx - m, cy - m, cx + m, cy + m],
    }
}
