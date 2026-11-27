//! Parts: limbs, crests, tails and feet, each a named piece of anatomy drawn
//! as its own group of layers.
//!
//! A layer's colour is a palette role name ("body", "part", "bone", ...) or
//! a literal hex colour, resolved when the tree is built.

use crate::genes::{Ext, Reader};
use crate::geom::{clamp, ellipse_poly, fluff_poly, lerp, max, min, rot, unit, P};

/// What a layer draws.
#[derive(Clone, Debug)]
pub enum Shape {
    /// A closed polygon.
    Poly(Vec<P>),
    /// Polygons drawn as one outlined shape.
    Set(Vec<Vec<P>>),
    /// An outlined stroke `mw` wide in model units.
    Tube {
        pts: Vec<P>,
        mw: f64,
    },
    /// A plain stroke `lw` wide in frame units.
    Curve {
        pts: Vec<P>,
        lw: f64,
    },
    Disc {
        x: f64,
        y: f64,
        r: f64,
    },
    /// A radial fade to nothing.
    Glow {
        x: f64,
        y: f64,
        r: f64,
    },
}

/// One drawn layer of a part.
#[derive(Clone, Debug)]
pub struct Layer {
    pub shape: Shape,
    /// Palette role or literal colour; empty for a plain curve.
    pub fill: &'static str,
    /// Palette role of the outline, if outlined.
    pub line: Option<&'static str>,
    pub op: Option<f64>,
    pub cls: &'static str,
    /// The layer the weld replaces when the limb joins the silhouette.
    pub base: bool,
}

impl Layer {
    pub fn poly(pts: Vec<P>, fill: &'static str, line: bool, cls: &'static str) -> Self {
        Layer {
            shape: Shape::Poly(pts),
            fill,
            line: line.then_some("outline"),
            op: None,
            cls,
            base: false,
        }
    }
    pub fn set(polys: Vec<Vec<P>>, fill: &'static str, cls: &'static str) -> Self {
        Layer {
            shape: Shape::Set(polys),
            fill,
            line: Some("outline"),
            op: None,
            cls,
            base: false,
        }
    }
    pub fn tube(pts: Vec<P>, mw: f64, fill: &'static str, cls: &'static str) -> Self {
        Layer {
            shape: Shape::Tube { pts, mw },
            fill,
            line: Some("outline"),
            op: None,
            cls,
            base: false,
        }
    }
    pub fn curve(pts: Vec<P>, lw: f64, op: Option<f64>, cls: &'static str) -> Self {
        Layer {
            shape: Shape::Curve { pts, lw },
            fill: "",
            line: Some("outline"),
            op,
            cls,
            base: false,
        }
    }
    pub fn disc(x: f64, y: f64, r: f64, fill: &'static str, line: bool, cls: &'static str) -> Self {
        Layer {
            shape: Shape::Disc { x, y, r },
            fill,
            line: line.then_some("outline"),
            op: None,
            cls,
            base: false,
        }
    }
    pub fn glow(x: f64, y: f64, r: f64, fill: &'static str, cls: &'static str) -> Self {
        Layer {
            shape: Shape::Glow { x, y, r },
            fill,
            line: None,
            op: None,
            cls,
            base: false,
        }
    }
    pub fn base(mut self) -> Self {
        self.base = true;
        self
    }
    pub fn op(mut self, op: f64) -> Self {
        self.op = Some(op);
        self
    }
}

/// The shape a limb welds into the silhouette instead of being drawn.
#[derive(Clone, Debug)]
pub enum Weld {
    Poly(Vec<P>),
    Circle([f64; 3]),
}

/// An eyeball an eye stalk carries.
#[derive(Clone, Copy, Debug)]
pub struct Eye {
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

/// A named piece of anatomy drawn as its own group.
#[derive(Clone, Debug)]
pub struct Part {
    pub name: String,
    pub kind: &'static str,
    pub side: f64,
    /// The point it grows from: the pivot a mood turns it about.
    pub root: P,
    /// How strongly a mood's droop turns it.
    pub sway: f64,
    pub layers: Vec<Layer>,
    pub weld: Option<Weld>,
    pub eye: Option<Eye>,
    pub front: bool,
    pub back: bool,
    pub over: bool,
    pub odd: bool,
    pub cock: f64,
    pub perk_max: Option<f64>,
}

impl Part {
    pub fn new(
        name: &str,
        kind: &'static str,
        side: f64,
        root: P,
        sway: f64,
        layers: Vec<Layer>,
    ) -> Self {
        Part {
            name: name.to_owned(),
            kind,
            side,
            root,
            sway,
            layers,
            weld: None,
            eye: None,
            front: false,
            back: false,
            over: false,
            odd: false,
            cock: 0.0,
            perk_max: None,
        }
    }
}

fn map_layer(layer: &Layer, f: &dyn Fn(P) -> P, scale: f64) -> Layer {
    let shape = match &layer.shape {
        Shape::Poly(pts) => Shape::Poly(pts.iter().map(|&p| f(p)).collect()),
        Shape::Set(polys) => Shape::Set(
            polys
                .iter()
                .map(|poly| poly.iter().map(|&p| f(p)).collect())
                .collect(),
        ),
        Shape::Tube { pts, mw } => Shape::Tube {
            pts: pts.iter().map(|&p| f(p)).collect(),
            mw: mw * scale,
        },
        Shape::Curve { pts, lw } => Shape::Curve {
            pts: pts.iter().map(|&p| f(p)).collect(),
            lw: *lw,
        },
        Shape::Disc { x, y, r } => {
            let c = f([*x, *y]);
            Shape::Disc {
                x: c[0],
                y: c[1],
                r: r * scale,
            }
        }
        Shape::Glow { x, y, r } => {
            let c = f([*x, *y]);
            Shape::Glow {
                x: c[0],
                y: c[1],
                r: r * scale,
            }
        }
    };
    Layer {
        shape,
        ..layer.clone()
    }
}

pub fn map_part(part: &Part, f: &dyn Fn(P) -> P, scale: f64) -> Part {
    let mut out = part.clone();
    out.root = f(part.root);
    out.layers = part.layers.iter().map(|l| map_layer(l, f, scale)).collect();
    if let Some(eye) = part.eye {
        let c = f([eye.x, eye.y]);
        out.eye = Some(Eye {
            x: c[0],
            y: c[1],
            r: eye.r * scale,
        });
    }
    out
}

pub fn mirror_part(part: &Part) -> Part {
    let mut m = map_part(part, &|p| [-p[0], p[1]], 1.0);
    m.side = -part.side;
    m.weld = None;
    m
}

pub fn part_points(part: &Part) -> Vec<P> {
    let mut pts = Vec::new();
    for l in &part.layers {
        match &l.shape {
            Shape::Poly(p) | Shape::Tube { pts: p, .. } | Shape::Curve { pts: p, .. } => {
                pts.extend_from_slice(p)
            }
            Shape::Set(polys) => {
                for poly in polys {
                    pts.extend_from_slice(poly);
                }
            }
            Shape::Disc { x, y, r } => {
                pts.extend_from_slice(&[[x - r, *y], [x + r, *y], [*x, y - r], [*x, y + r]])
            }
            Shape::Glow { .. } => {}
        }
    }
    pts
}

/// A part scaled about its root, with whatever it welds or carries.
pub fn scale_part(part: &Part, f: f64) -> Part {
    let o = part.root;
    let g = move |q: P| [o[0] + (q[0] - o[0]) * f, o[1] + (q[1] - o[1]) * f];
    let mut out = map_part(part, &g, f);
    out.weld = part.weld.as_ref().map(|w| match w {
        Weld::Circle(c) => {
            let p = g([c[0], c[1]]);
            Weld::Circle([p[0], p[1], c[2] * f])
        }
        Weld::Poly(pts) => Weld::Poly(pts.iter().map(|&q| g(q)).collect()),
    });
    out
}

/// How far a part may reach round the body: `x` to either side of the axis,
/// `top` and `bottom`.
#[derive(Clone, Copy)]
pub struct Reach {
    pub x: f64,
    pub top: f64,
    pub bottom: f64,
}

/// How much a part must shrink about its root to stay inside the reach.
pub fn reach_scale(part: &Part, reach: Reach) -> f64 {
    let o = part.root;
    let mut pts = part_points(part);
    if let Some(eye) = part.eye {
        pts.push([eye.x - eye.r * 1.2, eye.y - eye.r * 1.2]);
        pts.push([eye.x + eye.r * 1.2, eye.y + eye.r * 1.2]);
    }
    if let Some(Weld::Poly(w)) = &part.weld {
        pts.extend_from_slice(w);
    }
    let mut f = 1.0;
    for q in pts {
        if q[0] > reach.x && q[0] > o[0] {
            f = min(f, (reach.x - o[0]) / (q[0] - o[0]));
        }
        if q[0] < -reach.x && q[0] < o[0] {
            f = min(f, (-reach.x - o[0]) / (q[0] - o[0]));
        }
        if q[1] < reach.top && q[1] < o[1] {
            f = min(f, (reach.top - o[1]) / (q[1] - o[1]));
        }
        if q[1] > reach.bottom && q[1] > o[1] {
            f = min(f, (reach.bottom - o[1]) / (q[1] - o[1]));
        }
    }
    clamp(f, 0.42, 1.0)
}

/// The options of one limb outline.
pub struct LimbOpts<'a> {
    pub root: P,
    pub dir: P,
    pub len: f64,
    pub curl: f64,
    pub curl_at: Option<&'a dyn Fn(f64) -> f64>,
    pub base_w: f64,
    pub profile: &'a dyn Fn(f64) -> f64,
    pub cap: Option<f64>,
    pub round: bool,
    pub steps: usize,
}

impl<'a> LimbOpts<'a> {
    pub fn new(
        root: P,
        dir: P,
        len: f64,
        curl: f64,
        base_w: f64,
        profile: &'a dyn Fn(f64) -> f64,
    ) -> Self {
        LimbOpts {
            root,
            dir,
            len,
            curl,
            curl_at: None,
            base_w,
            profile,
            cap: None,
            round: false,
            steps: 14,
        }
    }
    pub fn cap(mut self, cap: f64) -> Self {
        self.cap = Some(cap);
        self
    }
    pub fn round(mut self) -> Self {
        self.round = true;
        self
    }
    pub fn steps(mut self, steps: usize) -> Self {
        self.steps = steps;
        self
    }
    pub fn curl_at(mut self, f: &'a dyn Fn(f64) -> f64) -> Self {
        self.curl_at = Some(f);
        self
    }
}

/// A limb outline and the spine it was built on.
pub struct Limb {
    pub outline: Vec<P>,
    pub spine: Vec<P>,
    pub tang: Vec<P>,
    pub widths: Vec<f64>,
    pub left: Vec<P>,
    pub right: Vec<P>,
    pub tip: P,
    pub tip_dir: P,
    pub end: P,
}

/// A limb outline: a spine from `root` along the unit `dir`, bending by
/// `curl` radians over its length, with half-width `base_w * profile(t)`.
/// `cap` is how far the end point sits beyond the last spine point.
pub fn limb_shape(o: &LimbOpts) -> Limb {
    let steps = o.steps;
    let sf = steps as f64;
    let mut spine = Vec::with_capacity(steps + 1);
    let mut d = o.dir;
    let mut p = o.root;
    for i in 0..=steps {
        spine.push(p);
        if i < steps {
            let k = match o.curl_at {
                Some(f) => f((i as f64 + 0.5) / sf),
                None => 1.0,
            };
            d = rot(d, o.curl / sf * k);
            p = [p[0] + d[0] * o.len / sf, p[1] + d[1] * o.len / sf];
        }
    }
    let mut left = Vec::with_capacity(steps + 1);
    let mut right = Vec::with_capacity(steps + 1);
    let mut widths = Vec::with_capacity(steps + 1);
    let mut tang = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let w = o.base_w * (o.profile)(i as f64 / sf);
        widths.push(w);
        let a = spine[i.saturating_sub(1)];
        let b = spine[if i == 0 { 1 } else { i }];
        let t = unit(b[0] - a[0], b[1] - a[1]);
        tang.push(t);
        left.push([spine[i][0] - t[1] * w, spine[i][1] + t[0] * w]);
        right.push([spine[i][0] + t[1] * w, spine[i][1] - t[0] * w]);
    }
    let tip = spine[steps];
    let tip_dir = tang[steps];
    let w_end = widths[steps];
    let cap = o.cap.unwrap_or(w_end);
    let end = [tip[0] + tip_dir[0] * cap, tip[1] + tip_dir[1] * cap];
    let mut outline = left.clone();
    if o.round && w_end > 1e-6 {
        // A three-point cap reads as round; one point reads as a blunt wedge.
        let k = 0.7;
        outline.push([
            tip[0] + tip_dir[0] * cap * k - tip_dir[1] * w_end * k,
            tip[1] + tip_dir[1] * cap * k + tip_dir[0] * w_end * k,
        ]);
        outline.push(end);
        outline.push([
            tip[0] + tip_dir[0] * cap * k + tip_dir[1] * w_end * k,
            tip[1] + tip_dir[1] * cap * k - tip_dir[0] * w_end * k,
        ]);
    } else {
        outline.push(end);
    }
    outline.extend(right.iter().rev());
    Limb {
        outline,
        spine,
        tang,
        widths,
        left,
        right,
        tip,
        tip_dir,
        end,
    }
}

/// A limb's half-width may not exceed the radius its bend turns on, or the
/// inner edge folds over itself.
pub fn curl_cap(len: f64, curl: f64) -> f64 {
    if curl.abs() > 1e-6 {
        0.8 * len / curl.abs()
    } else {
        f64::INFINITY
    }
}

pub const LIMB_KINDS: [&str; 15] = [
    "nub",
    "horn",
    "antenna",
    "pointed ear",
    "round ear",
    "long ear",
    "fin",
    "feather wing",
    "tentacle",
    "leaf",
    "antler",
    "spikes",
    "pom",
    "claw",
    "flipper",
];
pub const LIMB_WEIGHTS: [f64; 15] = [
    3.0, 3.0, 3.0, 3.0, 2.0, 2.0, 3.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0,
];

fn skin_of(fill: u32) -> &'static str {
    match fill {
        0 => "toneUp",
        1 => "toneDown",
        _ => "body",
    }
}

/// One wild limb slot drawn as `kind`. `at` is the slot's block of new
/// genes. Built for the right side; `odd` asks for the slot's one-sided
/// quirk.
#[allow(clippy::too_many_arguments)]
pub fn build_limb(
    r: &Reader,
    at: usize,
    kind: usize,
    e: &Ext,
    root: P,
    dir: P,
    u: f64,
    limb_fill: &'static str,
    odd: bool,
) -> Part {
    let len = e.len * u;
    let mut layers = Vec::new();
    let mut weld = None;
    let mut sway = 1.0;
    let len_gain = r.lin(at + 1, 0.0, 1.0);
    let wid_gain = r.lin(at + 2, 0.0, 1.0);
    let count = u32::from(r.u8(at + 3));
    let pa = r.unit(at + 4);
    let pb = r.unit(at + 5);
    let tint_pick = r.u8(at + 6);
    let quirk = if odd { r.unit(at + 8) } else { -1.0 };
    let hard = ["bone", "dark", "accent", "part"][usize::from(tint_pick % 4)];
    let soft = ["part", "accent", "light", "part2"][usize::from(tint_pick % 4)];
    // Fleshy limbs are a little knobbly: five genes swell or pinch the limb
    // along its length.
    let lumpy = kind == 0 || kind == 8 || kind == 14;
    let knots: Option<[f64; 5]> = lumpy.then(|| std::array::from_fn(|i| r.sgn(at + 17 + i)));
    let lump = move |t: f64| -> f64 {
        match knots {
            None => 1.0,
            Some(k) => {
                let uu = clamp(t * 5.0 - 0.5, 0.0, 4.0);
                let i = min(3.0, uu.floor()) as usize;
                1.0 + 0.15 * lerp(k[i], k[i + 1], uu - i as f64) * min(1.0, t * 6.0)
            }
        }
    };
    let skin_fill = skin_of(e.fill);
    let inner = ["blush", "toneDown", "dark", "light"][usize::from((tint_pick >> 2) % 4)];
    let ear_inner = if inner == skin_fill { "dark" } else { inner };
    match kind {
        0 => {
            // nub: the wild limb
            let base_w = min(e.width * u, curl_cap(len, e.curl));
            let prof = |t: f64| (1.0 - (1.0 - e.taper) * t) * lump(t);
            let s = limb_shape(
                &LimbOpts::new(root, dir, len, e.curl, base_w, &prof).cap(base_w * e.taper),
            );
            layers.push(Layer::poly(s.outline.clone(), limb_fill, true, "skin").base());
            // Rings across the limb, where it is wide enough to carry them.
            let rings = count % 4;
            if rings != 0 && base_w > 0.07 * u {
                for j in 0..rings {
                    let i = 3 + ((f64::from(j) + 0.5) * 9.0 / f64::from(rings)).floor() as usize;
                    layers.push(Layer::poly(
                        vec![s.left[i], s.left[i + 1], s.right[i + 1], s.right[i]],
                        "limbMark",
                        false,
                        "band",
                    ));
                }
            }
            weld = Some(Weld::Poly(s.outline));
        }
        1 => {
            // horn: a pointed cone with ridges
            let l = len * lerp(0.8, 1.35, len_gain);
            let curl = e.curl * 0.7;
            let base_w = min(e.width * u * lerp(0.9, 1.5, wid_gain), curl_cap(l, curl));
            // A chipped horn stops short.
            let cut = if quirk >= 0.0 {
                lerp(0.5, 0.8, quirk)
            } else {
                1.0
            };
            let prof = |t: f64| (1.0 - t * cut) * (1.0 + 0.5 * t * cut);
            let s =
                limb_shape(
                    &LimbOpts::new(root, dir, l * cut, curl * cut, base_w, &prof)
                        .cap(if cut < 1.0 { 0.0 } else { base_w * 0.1 }),
                );
            let mut pts = s.outline.clone();
            if cut < 1.0 {
                // The break: a ragged end instead of a point.
                let i = s.left.len() - 1;
                let w = s.widths[i];
                let (tip, td) = (s.tip, s.tip_dir);
                pts.splice(
                    i + 1..i + 2,
                    [
                        [
                            tip[0] + td[0] * w * 0.9 - td[1] * w * 0.35,
                            tip[1] + td[1] * w * 0.9 + td[0] * w * 0.35,
                        ],
                        [tip[0] + td[0] * w * 0.15, tip[1] + td[1] * w * 0.15],
                    ],
                );
            }
            layers.push(Layer::poly(pts.clone(), hard, true, "horn").base());
            let ridges = count % 4;
            for j in 0..ridges {
                let i =
                    2 + ((f64::from(j) + 1.0) * 9.0 / (f64::from(ridges) + 1.0)).floor() as usize;
                if i < s.left.len() - 1 {
                    layers.push(Layer::curve(
                        vec![s.left[i], s.right[i]],
                        1.1,
                        Some(0.55),
                        "ridge",
                    ));
                }
            }
            weld = Some(Weld::Poly(pts));
            sway = 0.25;
        }
        2 => {
            // antenna: a thin stalk with a bulb
            let l = len * lerp(0.9, 1.7, len_gain);
            // The odd one bends its own way.
            let curl = if quirk >= 0.0 {
                e.curl * lerp(-1.2, 0.2, quirk)
            } else {
                e.curl
            };
            let one = |_: f64| 1.0;
            let s = limb_shape(&LimbOpts::new(root, dir, l, curl, 0.0, &one));
            let mw = u * lerp(0.035, 0.065, wid_gain);
            let rr = u * lerp(0.075, 0.17, pa);
            layers.push(Layer::tube(s.spine.clone(), mw, "part", "stalk"));
            let bulb = count % 5;
            let (tip, td) = (s.tip, s.tip_dir);
            if bulb == 1 {
                layers.push(Layer::poly(
                    ellipse_poly(
                        tip[0] + td[0] * rr * 0.6,
                        tip[1] + td[1] * rr * 0.6,
                        rr * 1.5,
                        rr * 0.75,
                        td,
                        18,
                    ),
                    "accent",
                    true,
                    "bulb",
                ));
            } else if bulb == 2 {
                let mid = s.spine[9];
                layers.push(Layer::disc(
                    mid[0],
                    mid[1],
                    rr * 0.6,
                    "accent",
                    true,
                    "bulb",
                ));
                layers.push(Layer::disc(tip[0], tip[1], rr, "accent", true, "bulb"));
            } else if bulb == 3 {
                layers.push(Layer::glow(tip[0], tip[1], rr * 2.4, "accent", "glow"));
                layers.push(Layer::disc(
                    tip[0],
                    tip[1],
                    rr * 0.85,
                    "accent",
                    true,
                    "bulb",
                ));
            } else if bulb == 4 {
                // Feathered, like a moth's: barbs both sides of the stalk.
                let n = s.spine.len();
                let mut i = 3;
                while i < n {
                    let b = u * 0.16 * (1.0 - 0.5 * i as f64 / n as f64);
                    let (p, t) = (s.spine[i], s.tang[i]);
                    layers.push(Layer::curve(
                        vec![
                            [
                                p[0] - t[1] * b + t[0] * b * 0.5,
                                p[1] + t[0] * b + t[1] * b * 0.5,
                            ],
                            p,
                            [
                                p[0] + t[1] * b + t[0] * b * 0.5,
                                p[1] - t[0] * b + t[1] * b * 0.5,
                            ],
                        ],
                        1.4,
                        None,
                        "barb",
                    ));
                    i += 2;
                }
            } else {
                layers.push(Layer::disc(tip[0], tip[1], rr, "accent", true, "bulb"));
                layers.push(
                    Layer::disc(
                        tip[0] - rr * 0.3,
                        tip[1] - rr * 0.3,
                        rr * 0.28,
                        "#ffffff",
                        false,
                        "shine",
                    )
                    .op(0.7),
                );
            }
            sway = 1.3;
        }
        3 => {
            // pointed ear
            let l = len * lerp(0.6, 1.05, len_gain);
            // The odd one flops over.
            let curl = if quirk >= 0.0 {
                e.curl * 0.4 + lerp(1.1, 1.9, quirk)
            } else {
                e.curl * 0.4
            };
            let base_w = min(
                min(e.width * u * lerp(1.3, 2.0, wid_gain), curl_cap(l, curl)),
                l * 0.7,
            );
            let prof = |t: f64| (1.0 - t) * (1.0 + 0.8 * t);
            let s =
                limb_shape(&LimbOpts::new(root, dir, l, curl, base_w, &prof).cap(base_w * 0.08));
            layers.push(Layer::poly(s.outline.clone(), skin_fill, true, "skin").base());
            let si = limb_shape(
                &LimbOpts::new(
                    s.spine[2],
                    s.tang[2],
                    l * 0.62,
                    curl * 0.62,
                    base_w * 0.5,
                    &prof,
                )
                .cap(0.0)
                .steps(8),
            );
            layers.push(Layer::poly(si.outline, ear_inner, false, "inner"));
            if count % 3 == 0 {
                // Lynx tufts off the tip.
                for a in [-0.35, 0.3] {
                    let d2 = rot(s.tip_dir, a);
                    layers.push(Layer::curve(
                        vec![
                            s.end,
                            [s.end[0] + d2[0] * l * 0.22, s.end[1] + d2[1] * l * 0.22],
                        ],
                        1.5,
                        None,
                        "tuft",
                    ));
                }
            }
            weld = Some(Weld::Poly(s.outline));
            sway = 0.9;
        }
        4 => {
            // round ear: a disc with an inner disc
            let rr = u * lerp(0.2, 0.4, wid_gain) * (0.75 + e.width);
            let c = [root[0] + dir[0] * rr * 0.95, root[1] + dir[1] * rr * 0.95];
            layers.push(Layer::disc(c[0], c[1], rr, skin_fill, true, "skin").base());
            layers.push(Layer::disc(
                c[0] - dir[0] * rr * 0.12,
                c[1] - dir[1] * rr * 0.12,
                rr * lerp(0.45, 0.68, pa),
                ear_inner,
                false,
                "inner",
            ));
            weld = Some(Weld::Circle([c[0], c[1], rr]));
            sway = 0.3;
        }
        5 => {
            // long ear: a leaf-shaped lop
            let l = len * lerp(1.15, 1.9, len_gain);
            let curl = if quirk >= 0.0 {
                e.curl + (if e.curl < 0.0 { -1.0 } else { 1.0 }) * lerp(1.0, 1.7, quirk)
            } else {
                e.curl
            };
            let base_w = min(
                e.width * u * lerp(0.9, 1.4, wid_gain),
                curl_cap(l, curl) * 0.8,
            );
            let prof = |t: f64| (0.5 + 2.2 * t * (1.0 - t)) * (1.0 - 0.6 * t * t * t * t);
            let s = limb_shape(
                &LimbOpts::new(root, dir, l, curl, base_w, &prof)
                    .cap(base_w * 0.2)
                    .round(),
            );
            layers.push(Layer::poly(s.outline.clone(), skin_fill, true, "skin").base());
            let prof_in = |t: f64| (0.4 + 2.2 * t * (1.0 - t)) * (1.0 - t * t);
            let si = limb_shape(
                &LimbOpts::new(
                    s.spine[2],
                    s.tang[2],
                    l * 0.7,
                    curl * 0.7,
                    base_w * 0.48,
                    &prof_in,
                )
                .cap(0.0)
                .steps(10),
            );
            layers.push(Layer::poly(si.outline, ear_inner, false, "inner"));
            weld = Some(Weld::Poly(s.outline));
            sway = 1.4;
        }
        6 => {
            // fin: a membrane fanned over rays
            let rays = 3 + (count % 3) as usize;
            let spread = lerp(0.3, 0.8, pa);
            let dip = lerp(0.06, 0.4, pb);
            let l = len * lerp(0.85, 1.35, len_gain);
            let b = max(e.width * u * 0.8, 0.06 * u);
            let mut tips = Vec::with_capacity(rays);
            for j in 0..rays {
                let uu = if rays == 1 {
                    0.0
                } else {
                    j as f64 / (rays as f64 - 1.0) * 2.0 - 1.0
                };
                let d2 = rot(dir, uu * spread + e.curl * 0.25);
                let ll = l * (1.0 - 0.3 * uu * uu) * lerp(0.75, 1.1, r.unit(at + 9 + j));
                tips.push([root[0] + d2[0] * ll, root[1] + d2[1] * ll]);
            }
            // The base is a short edge across the root; the first ray leans
            // to the same side as the base point it follows.
            let mut pts = vec![[root[0] + dir[1] * b, root[1] - dir[0] * b]];
            for j in 0..rays {
                pts.push(tips[j]);
                if j + 1 < rays {
                    let mx = (tips[j][0] + tips[j + 1][0]) / 2.0;
                    let my = (tips[j][1] + tips[j + 1][1]) / 2.0;
                    pts.push([lerp(mx, root[0], dip), lerp(my, root[1], dip)]);
                }
            }
            pts.push([root[0] - dir[1] * b, root[1] + dir[0] * b]);
            layers.push(Layer::poly(pts, soft, true, "membrane"));
            for tp in tips {
                layers.push(Layer::curve(vec![root, tp], 1.2, Some(0.6), "ray"));
            }
            sway = 0.8;
        }
        7 => {
            // feather wing: overlapping feathers, longest first
            let feathers = 3 + (count % 3) as usize;
            let spread = lerp(0.5, 1.1, pa);
            let l = len * lerp(1.0, 1.5, len_gain);
            // Each feather keeps its own outline, so they read as shingles.
            // They fan downward whichever way the limb points.
            for j in 0..feathers {
                let uu = j as f64 / (feathers as f64 - 1.0);
                let d2 = rot(
                    dir,
                    (uu - 0.25) * spread * (if dir[0] >= 0.0 { 1.0 } else { -1.0 }),
                );
                let ll = l * (1.0 - 0.45 * uu) * lerp(0.85, 1.1, r.unit(at + 9 + j));
                let prof = |t: f64| 0.45 + 2.2 * t * (1.0 - t);
                let s = limb_shape(
                    &LimbOpts::new(
                        root,
                        d2,
                        ll,
                        e.curl * 0.3,
                        max(e.width * u, 0.1 * u) * 0.9,
                        &prof,
                    )
                    .cap(0.02 * u)
                    .steps(8)
                    .round(),
                );
                layers.push(Layer::poly(
                    s.outline,
                    if j % 2 == 1 { soft } else { limb_fill },
                    true,
                    "feather",
                ));
            }
            sway = 0.9;
        }
        8 => {
            // tentacle: long, S-bent, with suckers
            let l = len * lerp(1.2, 1.85, len_gain);
            let base_w = min(
                e.width * u * lerp(0.8, 1.2, wid_gain),
                curl_cap(l * 0.5, e.curl * 1.6),
            );
            let curl_at = |t: f64| if t < 0.5 { 1.0 } else { -1.6 };
            let prof = |t: f64| (1.0 - 0.8 * t) * lump(t);
            let s = limb_shape(
                &LimbOpts::new(root, dir, l, e.curl, base_w, &prof)
                    .curl_at(&curl_at)
                    .round()
                    .steps(18),
            );
            layers.push(Layer::poly(s.outline.clone(), limb_fill, true, "skin").base());
            if base_w > 0.09 * u {
                let mut i = 4;
                while i < 16 {
                    let (w, p, t) = (s.widths[i], s.spine[i], s.tang[i]);
                    layers.push(Layer::disc(
                        p[0] + t[1] * w * 0.3,
                        p[1] - t[0] * w * 0.3,
                        w * 0.36,
                        "light",
                        false,
                        "sucker",
                    ));
                    i += 3;
                }
            }
            weld = Some(Weld::Poly(s.outline));
            sway = 1.2;
        }
        9 => {
            // leaf: a stem, a pointed blade and a midrib
            let l = len * lerp(0.9, 1.5, len_gain);
            let base_w = min(
                max(e.width * u, 0.14 * u) * lerp(1.0, 1.7, wid_gain),
                l * 0.4,
            );
            let prof = |t: f64| {
                if t < 0.2 {
                    return 0.18;
                }
                let uu = (t - 0.2) / 0.8;
                0.18 * (1.0 - uu) + 3.8 * uu * (1.0 - uu)
            };
            // prof peaks at 1.04 halfway along the blade.
            let scaled = |t: f64| prof(t) / 1.04;
            let s = limb_shape(
                &LimbOpts::new(root, dir, l, e.curl * 0.6, base_w, &scaled).cap(base_w * 0.25),
            );
            layers.push(Layer::poly(s.outline.clone(), soft, true, "blade"));
            layers.push(Layer::curve(s.spine[2..].to_vec(), 1.2, Some(0.55), "rib"));
            let mut i = 5;
            while i < 13 {
                let (p, q, w, t) = (
                    s.spine[i],
                    s.spine[i + 1],
                    s.widths[i + 1] * 0.75,
                    s.tang[i + 1],
                );
                layers.push(Layer::curve(
                    vec![
                        [q[0] - t[1] * w, q[1] + t[0] * w],
                        p,
                        [q[0] + t[1] * w, q[1] - t[0] * w],
                    ],
                    0.9,
                    Some(0.4),
                    "vein",
                ));
                i += 3;
            }
            sway = 1.1;
        }
        10 => {
            // antler: a beam with tines
            let l = len * lerp(1.0, 1.55, len_gain);
            let curl = e.curl * 0.6;
            let base_w = min(max(e.width * u * 0.55, 0.055 * u), curl_cap(l, curl));
            // A broken antler.
            let snap = if quirk >= 0.0 {
                lerp(0.45, 0.75, quirk)
            } else {
                1.0
            };
            let prof = |t: f64| 1.0 - 0.7 * t * snap;
            let s = limb_shape(
                &LimbOpts::new(root, dir, l * snap, curl * snap, base_w, &prof)
                    .cap(base_w * 0.3)
                    .round(),
            );
            let mut polys = vec![s.outline.clone()];
            let tines = 2 + (count % 3) as usize;
            let outer = if pa < 0.5 { 1.0 } else { -1.0 };
            for j in 0..tines {
                let i = 3 + (j as f64 * 9.0 / tines as f64).floor() as usize;
                if i >= s.spine.len() - 1 {
                    break;
                }
                let flip = (if j % 2 == 0 || pb < 0.5 { 1.0 } else { -1.0 }) * outer;
                let d2 = rot(s.tang[i], flip * lerp(0.6, 1.1, r.unit(at + 9 + j)));
                let tl = l
                    * lerp(0.22, 0.48, r.unit(at + 13 + j))
                    * (1.0 - 0.3 * j as f64 / tines as f64);
                let tprof = |t: f64| 1.0 - 0.75 * t;
                polys.push(
                    limb_shape(
                        &LimbOpts::new(s.spine[i], d2, tl, -flip * 0.5, s.widths[i] * 0.85, &tprof)
                            .cap(base_w * 0.2)
                            .steps(6)
                            .round(),
                    )
                    .outline,
                );
            }
            layers.push(Layer::set(polys, hard, "antler"));
            sway = 0.3;
        }
        11 => {
            // spikes: a fan of quills
            let quills = 3 + (count % 3) as usize;
            let spread = lerp(0.35, 0.9, pa);
            let mut polys = Vec::with_capacity(quills);
            let l = len * lerp(0.7, 1.2, len_gain);
            let b = max(e.width * u * 0.42, 0.045 * u);
            for j in 0..quills {
                let uu = j as f64 / (quills as f64 - 1.0) * 2.0 - 1.0;
                let d2 = rot(dir, uu * spread);
                let ll = l * lerp(0.6, 1.15, r.unit(at + 9 + j));
                let o = [root[0] + d2[0] * 0.02 * u, root[1] + d2[1] * 0.02 * u];
                polys.push(vec![
                    [o[0] - d2[1] * b, o[1] + d2[0] * b],
                    [o[0] + d2[0] * ll, o[1] + d2[1] * ll],
                    [o[0] + d2[1] * b, o[1] - d2[0] * b],
                ]);
            }
            layers.push(Layer::set(polys, hard, "quills"));
            sway = 0.4;
        }
        12 => {
            // pom: a fluffy ball on a stalk
            let l = len * lerp(0.6, 1.2, len_gain);
            let one = |_: f64| 1.0;
            let s = limb_shape(&LimbOpts::new(root, dir, l, e.curl * 0.8, 0.0, &one));
            let rr = u * lerp(0.13, 0.26, pa);
            layers.push(Layer::tube(
                s.spine.clone(),
                u * lerp(0.035, 0.06, wid_gain),
                "part",
                "stalk",
            ));
            layers.push(Layer::poly(
                fluff_poly(
                    s.tip[0] + s.tip_dir[0] * rr * 0.5,
                    s.tip[1] + s.tip_dir[1] * rr * 0.5,
                    rr,
                    6 + (count % 4) as usize,
                    0.2,
                ),
                soft,
                true,
                "pom",
            ));
            sway = 1.3;
        }
        13 => {
            // claw: an arm that ends in a pincer
            let base_w = min(max(e.width * u, 0.13 * u), curl_cap(len, e.curl));
            let taper = max(e.taper, 0.55);
            let prof = |t: f64| 1.0 - (1.0 - taper) * t;
            let s = limb_shape(&LimbOpts::new(root, dir, len, e.curl, base_w, &prof).cap(0.0));
            let mut polys = vec![s.outline.clone()];
            let tw = base_w * taper;
            let open = lerp(0.35, 0.8, pa);
            polys.push(ellipse_poly(
                s.tip[0],
                s.tip[1],
                tw * 1.25,
                tw * 1.25,
                [1.0, 0.0],
                14,
            ));
            for sgn in [-1.0, 1.0] {
                let d2 = rot(s.tip_dir, sgn * open);
                let o = [s.tip[0] + d2[0] * tw * 0.4, s.tip[1] + d2[1] * tw * 0.4];
                let jprof = |t: f64| (1.0 - t) * (1.0 + 0.6 * t);
                let jaw_len = len
                    * lerp(0.4, 0.65, len_gain)
                    * (if sgn < 0.0 { 1.0 } else { lerp(0.6, 1.0, pb) });
                polys.push(
                    limb_shape(
                        &LimbOpts::new(o, d2, jaw_len, -sgn * open * 1.5, tw * 0.85, &jprof)
                            .cap(0.0)
                            .steps(7),
                    )
                    .outline,
                );
            }
            layers.push(Layer::set(polys, skin_fill, "skin"));
            sway = 0.7;
        }
        _ => {
            // flipper: a paddle, wide in the middle
            let l = len * lerp(0.75, 1.2, len_gain);
            let base_w = min(
                min(
                    max(e.width * u, 0.12 * u) * lerp(1.0, 1.5, wid_gain),
                    curl_cap(l, e.curl),
                ),
                l * 0.45,
            );
            let prof = |t: f64| (0.55 + 1.8 * t * (1.0 - t) - 0.2 * t) * lump(t);
            let s = limb_shape(&LimbOpts::new(root, dir, l, e.curl, base_w, &prof).round());
            layers.push(Layer::poly(s.outline.clone(), limb_fill, true, "skin").base());
            let marks = (count % 3) as usize;
            for j in 0..marks {
                let i = 9 + j * 2;
                let (w, p, t) = (s.widths[i] * 0.5, s.spine[i], s.tang[i]);
                layers.push(Layer::curve(
                    vec![
                        [p[0] - t[1] * w, p[1] + t[0] * w],
                        [p[0] + t[1] * w, p[1] - t[0] * w],
                    ],
                    1.1,
                    Some(0.5),
                    "crease",
                ));
            }
            weld = Some(Weld::Poly(s.outline));
            sway = 1.0;
        }
    }
    let mut part = Part::new("", LIMB_KINDS[kind], 1.0, root, sway, layers);
    part.weld = weld;
    part
}
