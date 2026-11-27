//! Hair: a mass of shapes laid out round the head by angle (0 is straight
//! up, 90 to the right). A mass is drawn outlines first and fills second, so
//! its shapes join into one silhouette with one line round it.

use crate::face::{Brows, Eyes};
use crate::genes::{dna, Genes};
use crate::geom::{
    box_of, clamp, hump, hyp, lerp, max, min, mix, pt, sample, sharp, sin, step, unit_of, up_dir,
    Profile, Pt, DEG,
};
use crate::head::{Ears, FaceLayout, Head, Neck};

/// A hair style, in gene order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairStyle {
    Bald,
    Wisps,
    Horseshoe,
    Combover,
    Buzz,
    Crop,
    Quiff,
    Spikes,
    Tousle,
    Curls,
    Afro,
    Slick,
    Curtains,
    Bob,
    Long,
    Wavy,
    Mullet,
    Mohawk,
    Dreads,
}

const HAIR_STYLES: [(HairStyle, &str, u32); 19] = [
    (HairStyle::Bald, "bald", 3),
    (HairStyle::Wisps, "wisps", 4),
    (HairStyle::Horseshoe, "horseshoe", 3),
    (HairStyle::Combover, "combover", 2),
    (HairStyle::Buzz, "buzz", 3),
    (HairStyle::Crop, "crop", 6),
    (HairStyle::Quiff, "quiff", 4),
    (HairStyle::Spikes, "spikes", 4),
    (HairStyle::Tousle, "tousle", 5),
    (HairStyle::Curls, "curls", 4),
    (HairStyle::Afro, "afro", 3),
    (HairStyle::Slick, "slick", 3),
    (HairStyle::Curtains, "curtains", 4),
    (HairStyle::Bob, "bob", 4),
    (HairStyle::Long, "long", 4),
    (HairStyle::Wavy, "wavy", 3),
    (HairStyle::Mullet, "mullet", 2),
    (HairStyle::Mohawk, "mohawk", 2),
    (HairStyle::Dreads, "dreads", 1),
];

impl HairStyle {
    pub fn pick(g: &Genes) -> HairStyle {
        HAIR_STYLES[g.pick(dna::HAIR, 0, &HAIR_STYLES.map(|s| s.2))].0
    }

    pub fn name(self) -> &'static str {
        HAIR_STYLES[self as usize].1
    }

    /// Every style's name, in gene order.
    pub fn names() -> Vec<&'static str> {
        HAIR_STYLES.iter().map(|s| s.1).collect()
    }

    /// The styles that fall past the ears.
    pub fn past_ears(self) -> bool {
        matches!(
            self,
            HairStyle::Curtains
                | HairStyle::Bob
                | HairStyle::Long
                | HairStyle::Wavy
                | HairStyle::Dreads
                | HairStyle::Afro
                | HairStyle::Mullet
        )
    }
}

/// How heavy the line round a shape of a mass is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    None,
    Thin,
    Thick,
}

/// One filled shape of a mass.
#[derive(Clone, Debug)]
pub enum Fill {
    /// A closed outline; a `skin` shape is a parting, filled with the skin's colour.
    Shape {
        pts: Vec<Pt>,
        edge: Edge,
        skin: bool,
    },
    Circle {
        x: f64,
        y: f64,
        r: f64,
        edge: Edge,
    },
    /// A rope of hair: a thick line through points.
    Tube {
        pts: Vec<Pt>,
        w: f64,
        edge: Edge,
    },
}

/// An open line drawn round part of a mass.
#[derive(Clone, Debug)]
pub struct Rim {
    pub pts: Vec<Pt>,
    pub thin: bool,
}

/// Which colour a stroke of the pen in a mass takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// A single hair on the skin: the hair's own colour.
    Hair,
    /// A strand drawn in the hair: its darker line.
    Line,
}

#[derive(Clone, Debug)]
pub struct PenStroke {
    pub ctrl: Vec<Pt>,
    pub k: f64,
    pub tone: Tone,
    pub press: Profile,
}

/// A mass of hair or facial hair.
#[derive(Clone, Debug, Default)]
pub struct Mass {
    pub fills: Vec<Fill>,
    pub rims: Vec<Rim>,
    pub pens: Vec<PenStroke>,
    /// Stubble dots as (x, y, r).
    pub dots: Vec<(f64, f64, f64)>,
    /// Short strokes as (x, y, dx, dy).
    pub ticks: Vec<[f64; 4]>,
    /// Whether the short strokes take the hair's shade (a clipped head) or
    /// its pen colour, faint (facial hair).
    pub shade_ticks: bool,
}

impl Mass {
    /// The points the mass reaches to, for measuring.
    pub fn reach_pts(&self, out: &mut Vec<Pt>) {
        for s in &self.fills {
            match s {
                Fill::Circle { x, y, r, .. } => out.extend(crate::geom::disc_pts(*x, *y, *r)),
                Fill::Tube { pts, w, .. } => {
                    for p in pts {
                        out.extend(crate::geom::disc_pts(p.x, p.y, w / 2.0));
                    }
                }
                Fill::Shape { pts, .. } => out.extend_from_slice(pts),
            }
        }
        for p in &self.pens {
            if p.tone == Tone::Hair {
                out.extend_from_slice(&p.ctrl);
            }
        }
    }
}

/// A lock of hair: a tongue that tapers from a root (two points) to a tip.
/// `belly` swells its sides, as a share of the root's width.
pub fn lock_pts(ra: Pt, rb: Pt, tip: Pt, belly: f64) -> Vec<Pt> {
    let n = unit_of(ra, rb);
    let w = hyp(rb.x - ra.x, rb.y - ra.y) * belly;
    let ma = mix(ra, tip, 0.5);
    let mb = mix(rb, tip, 0.5);
    vec![
        sharp(ra.x, ra.y),
        pt(ma.x - n.0 * w, ma.y - n.1 * w),
        sharp(tip.x, tip.y),
        pt(mb.x + n.0 * w, mb.y + n.1 * w),
        sharp(rb.x, rb.y),
    ]
}

/// A head's hair.
#[derive(Clone, Debug)]
pub struct Hair {
    pub style: HairStyle,
    /// Drawn behind the head.
    pub back: Mass,
    /// Drawn over the head.
    pub front: Mass,
    /// How low the hairline comes in the middle of the forehead.
    pub fore: f64,
    /// The angle down to which hair lies on the side of the head.
    pub low: f64,
}

/// What hair is built round.
pub struct HairInputs<'a> {
    pub head: &'a Head,
    pub face: &'a FaceLayout,
    pub eyes: &'a Eyes,
    pub brows: &'a Brows,
    pub ears: &'a Ears,
    pub neck: Option<&'a Neck>,
    pub beard_on_jaw: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Front,
    Back,
}

/// Where hair may not lie: over the brows and the eyes.
#[derive(Clone, Copy)]
struct Clear {
    limit_y: f64,
    limit_x: f64,
    eye_out: f64,
    eye_y: f64,
    eye_bu: f64,
    eye_bl: f64,
}

impl Clear {
    /// Over the brows hair comes no lower than a little above them, and
    /// beside the eyes it stays outside them.
    fn at(&self, p: Pt) -> Pt {
        let mut x = p.x;
        let mut y = p.y;
        let lim = self.limit_y + 400.0 * step(self.limit_x - 5.0, self.limit_x + 7.0, x.abs());
        if y > lim {
            y = lim;
        }
        if y > self.eye_y - self.eye_bu - 5.0
            && y < self.eye_y + self.eye_bl + 5.0
            && x.abs() < self.eye_out
        {
            x = (if x < 0.0 { -1.0 } else { 1.0 }) * self.eye_out;
        }
        pt(x, y)
    }
}

/// The gene readers and builders every style shares.
struct Kit<'a> {
    g: &'a Genes<'a>,
    hd: &'a Head,
    e: &'a Eyes,
    sz: f64,
    style: HairStyle,
    clear: Clear,
    /// The forehead: from the top of the skull down to the limit over the brows.
    fore_h: f64,
    /// The angle at which the top of the ear meets the head.
    ear_top: f64,
    /// How far from the middle hair may lie at any height without `clear` moving it.
    brow_out: f64,
    beard_on_jaw: bool,
    hr: Hair,
}

const O: usize = dna::HAIR;

impl Kit<'_> {
    fn vol(&self) -> f64 {
        self.g.unit(O + 1, 0, 3)
    }
    fn fore(&self) -> f64 {
        self.g.unit(O + 1, 3, 3)
    }
    fn line_kind(&self) -> usize {
        self.g.raw(O + 1, 6, 2) as usize
    }
    fn part_side(&self) -> f64 {
        self.g.side(O + 2, 0)
    }
    fn part_pos(&self) -> f64 {
        self.g.unit(O + 2, 1, 2)
    }
    fn burns(&self) -> f64 {
        self.g.unit(O + 2, 3, 2)
    }
    fn cowlick(&self) -> bool {
        self.g.flag(O + 2, 5)
    }
    fn count(&self) -> u32 {
        self.g.raw(O + 2, 6, 2)
    }
    fn length(&self) -> f64 {
        self.g.unit(O + 3, 0, 3)
    }
    fn ends(&self) -> u32 {
        self.g.raw(O + 3, 3, 2)
    }
    fn shaved(&self) -> bool {
        self.g.flag(O + 3, 5)
    }
    /// Each lock has a byte of its own: how long it is, which way it leans, how wide it is.
    fn lock_len(&self, i: usize) -> f64 {
        self.g.unit(O + 4 + (i % 16), 0, 3)
    }
    fn lock_lean(&self, i: usize) -> f64 {
        self.g.signed(O + 4 + (i % 16), 3, 3)
    }
    fn lock_wide(&self, i: usize) -> f64 {
        self.g.unit(O + 4 + (i % 16), 6, 2)
    }

    fn mass(&mut self, which: Which) -> &mut Mass {
        match which {
            Which::Front => &mut self.hr.front,
            Which::Back => &mut self.hr.back,
        }
    }

    /// A hairline is placed by a share of the forehead.
    fn mid(&self, low: f64, high: f64) -> f64 {
        self.fore_h * lerp(low, high, self.fore())
    }

    /// A cap: hair between an outer edge, `out` units outside the outline at
    /// each angle, and a hairline `depth` units inside it.
    #[allow(clippy::too_many_arguments)]
    fn cap(
        &mut self,
        a0: f64,
        a1: f64,
        n: usize,
        out: &dyn Fn(f64) -> f64,
        depth: &dyn Fn(f64) -> f64,
        rim: CapRim,
    ) {
        let hd = self.hd;
        let mut outer = Vec::with_capacity(n + 1);
        let mut inner = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let deg = lerp(a0, a1, i as f64 / n as f64);
            outer.push(hd.at(deg, out(deg)));
            inner.push(self.clear.at(hd.at(deg, -depth(deg))));
        }
        let e0 = mix(outer[0], inner[0], 0.5);
        let e1 = mix(outer[n], inner[n], 0.5);
        let c0 = sharp(e0.x, e0.y);
        let c1 = sharp(e1.x, e1.y);
        let mut outside = vec![c0];
        outside.extend_from_slice(&outer[1..n]);
        outside.push(c1);
        let mut pts = outside.clone();
        pts.extend(inner[1..n].iter().rev());
        let front = &mut self.hr.front;
        front.fills.push(Fill::Shape {
            pts,
            edge: if rim == CapRim::Outer {
                Edge::Thin
            } else {
                Edge::None
            },
            skin: false,
        });
        match rim {
            CapRim::Outer => front.rims.push(Rim {
                pts: outside,
                thin: false,
            }),
            CapRim::Hairline => {
                let mut line = vec![c0];
                line.extend_from_slice(&inner[1..n]);
                line.push(c1);
                front.rims.push(Rim {
                    pts: line,
                    thin: true,
                });
            }
            CapRim::None => {}
        }
        for p in &inner {
            if p.x.abs() < self.e.x {
                self.hr.fore = max(self.hr.fore, p.y);
            }
        }
        self.hr.low = max(max(self.hr.low, a0.abs()), a1.abs());
    }

    fn strand_in(&mut self, pts: Vec<Pt>, k: f64, which: Which) {
        self.mass(which).pens.push(PenStroke {
            ctrl: pts,
            k,
            tone: Tone::Line,
            press: Profile::Leaf,
        });
    }

    fn strand(&mut self, pts: Vec<Pt>, k: f64) {
        self.strand_in(pts, k, Which::Front);
    }

    /// A single hair on the skin.
    fn hair_pen(&mut self, ctrl: Vec<Pt>, k: f64) {
        self.hr.front.pens.push(PenStroke {
            ctrl,
            k,
            tone: Tone::Hair,
            press: Profile::Out,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn lock_in(&mut self, ra: Pt, rb: Pt, tip: Pt, edge: Edge, belly: f64, which: Which) {
        self.mass(which).fills.push(Fill::Shape {
            pts: lock_pts(ra, rb, tip, belly),
            edge,
            skin: false,
        });
        if tip.x.abs() < self.e.x && edge == Edge::Thin {
            self.hr.fore = max(self.hr.fore, tip.y);
        }
    }

    fn lock(&mut self, ra: Pt, rb: Pt, tip: Pt, edge: Edge, belly: f64) {
        self.lock_in(ra, rb, tip, edge, belly, Which::Front);
    }

    /// Where a sideburn ends. Under a beard along the jaw it stops at the ear.
    fn burn_end(&self) -> f64 {
        self.ear_top
            + if self.beard_on_jaw {
                4.0
            } else {
                lerp(4.0, 24.0, self.burns())
            }
    }

    /// The hairline of short hair; see [`short_line`].
    fn short_line(&self, d0: f64, recede: f64, peak: f64, end: f64) -> impl Fn(f64) -> f64 {
        short_line(self.sz, self.ear_top, d0, recede, peak, end)
    }

    /// The outer edge of short hair; see [`short_out`].
    fn short_out(&self, v0: f64, v1: f64, end: f64) -> impl Fn(f64) -> f64 {
        short_out(self.ear_top, v0, v1, end)
    }

    /// A lock that stands up from the crown, off to one side.
    fn add_cowlick(&mut self, side: f64, out: &dyn Fn(f64) -> f64) {
        let hd = self.hd;
        let deg = side * 16.0;
        self.lock(
            hd.at(deg - 5.0, out(deg - 5.0) - 1.5),
            hd.at(deg + 5.0, out(deg + 5.0) - 1.5),
            hd.at(deg + side * 15.0, out(deg) + 12.0 * self.sz),
            Edge::Thick,
            0.1,
        );
    }

    /// Short locks that break the outline of hair that lies flat.
    fn tufts(&mut self, out: &dyn Fn(f64) -> f64, list: &[[f64; 4]]) {
        let hd = self.hd;
        for t in list {
            let tip = hd.at(
                t[0] + t[1],
                out(t[0]) + t[2] * self.sz * lerp(0.8, 1.3, self.lock_len(t[3] as usize)),
            );
            self.lock(
                hd.at(t[0] - 5.5, out(t[0] - 5.5) - 1.6),
                hd.at(t[0] + 5.5, out(t[0] + 5.5) - 1.6),
                tip,
                Edge::Thick,
                0.12,
            );
        }
    }

    /// Loose locks that hang from a hairline onto the forehead, with gaps between them.
    fn fringe(
        &mut self,
        m: u32,
        depth: &dyn Fn(f64) -> f64,
        width: f64,
        reach: [f64; 2],
        first: usize,
        sweep: f64,
    ) {
        let hd = self.hd;
        for j in 0..m as usize {
            let deg = (j as f64 - (f64::from(m) - 1.0) / 2.0) * width
                + self.lock_lean(j + first) * width * 0.24;
            let half = width * 0.46;
            let tip = self.clear.at(hd.at(
                deg + self.lock_lean(j + first + 2) * 7.0 + sweep,
                -(depth(deg) + (reach[0] + reach[1] * self.lock_len(j + first + 2)) * self.sz),
            ));
            self.lock(
                hd.at(deg - half, -(depth(deg - half) - 2.5)),
                hd.at(deg + half, -(depth(deg + half) - 2.5)),
                tip,
                Edge::Thin,
                0.12,
            );
        }
    }

    /// A parting: a sliver of scalp that runs from the hairline back into the hair.
    fn parting(&mut self, at: &dyn Fn(f64, f64) -> Pt, part: f64, lean: f64) {
        let pts = [
            at(part - 1.8, -0.04),
            at(part + 1.8, -0.04),
            at(part + lean, 0.72),
        ]
        .iter()
        .map(|p| sharp(p.x, p.y))
        .collect();
        self.hr.front.fills.push(Fill::Shape {
            pts,
            edge: Edge::None,
            skin: true,
        });
        let strand = vec![
            at(part + if lean < 0.0 { 1.8 } else { -1.8 }, 0.0),
            at(part + lean * 0.5, 0.4),
            at(part + lean, 0.76),
        ];
        self.strand(strand, 0.8);
    }
}

/// Which edges of a cap are drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CapRim {
    /// A heavy line round the outside and a light one along the hairline.
    Outer,
    /// The light line alone, for a cap whose outside lies on hair behind the head.
    Hairline,
    None,
}

/// The hairline of short hair, in units inside the outline at each angle:
/// `d0` over the forehead, a strip over the ears, thinning to nothing down
/// the sideburn. `recede` cuts the corners back, `peak` adds a point.
fn short_line(
    sz: f64,
    ear_top: f64,
    d0: f64,
    recede: f64,
    peak: f64,
    end: f64,
) -> impl Fn(f64) -> f64 {
    move |deg: f64| {
        let a = deg.abs();
        let d = lerp(d0, 10.5 * sz, step(ear_top * 0.4, ear_top * 0.86, a))
            - recede * sz * hump(a, ear_top * 0.5, ear_top * 0.27)
            + peak * sz * hump(deg, 0.0, 10.0);
        max(1.0, d * (1.0 - 0.92 * step(ear_top - 3.0, end, a)))
    }
}

/// The outer edge of short hair: `v0` over the top, `v1` at the sides, and
/// close to the skull down the sideburn.
fn short_out(ear_top: f64, v0: f64, v1: f64, end: f64) -> impl Fn(f64) -> f64 {
    move |deg: f64| {
        let a = deg.abs();
        lerp(
            lerp(v0, v1, step(8.0, 62.0, a)),
            1.3,
            step(ear_top - 16.0, end, a),
        )
    }
}

/// A point of a cap: `f` of the way from its hairline (0) to its outer edge (1).
fn within<'b>(
    hd: &'b Head,
    clear: Clear,
    out: &'b dyn Fn(f64) -> f64,
    depth: &'b dyn Fn(f64) -> f64,
) -> impl Fn(f64, f64) -> Pt + 'b {
    move |deg: f64, f: f64| mix(clear.at(hd.at(deg, -depth(deg))), hd.at(deg, out(deg)), f)
}

pub fn make_hair(g: &Genes, c: &HairInputs, style: HairStyle) -> Hair {
    let hd = c.head;
    let e = c.eyes;
    let b = c.brows;
    let limit_y = b.y - b.w / 2.0 - 7.0;
    let limit_x = e.x + max(e.a, b.half) + 3.0;
    let eye_out = e.x + e.a * 1.14 + 5.0;
    let mut k = Kit {
        g,
        hd,
        e,
        sz: hd.size,
        style,
        clear: Clear {
            limit_y,
            limit_x,
            eye_out,
            eye_y: e.y,
            eye_bu: e.bu,
            eye_bl: e.bl,
        },
        fore_h: limit_y + hd.h,
        ear_top: hd.angle_at(c.ears.yc - c.ears.eh * 0.5),
        brow_out: limit_x + 8.0,
        beard_on_jaw: c.beard_on_jaw,
        hr: Hair {
            style,
            back: Mass::default(),
            front: Mass::default(),
            fore: -hd.h * 0.86,
            low: 0.0,
        },
    };
    match style {
        HairStyle::Bald => {}
        HairStyle::Wisps => wisps(&mut k),
        HairStyle::Horseshoe | HairStyle::Combover => side_hair(&mut k),
        HairStyle::Buzz => buzz(&mut k, c),
        HairStyle::Crop => crop(&mut k),
        HairStyle::Quiff => quiff(&mut k),
        HairStyle::Slick => slick(&mut k),
        HairStyle::Spikes => spikes(&mut k),
        HairStyle::Tousle => tousle(&mut k),
        HairStyle::Curls | HairStyle::Afro => curls(&mut k),
        HairStyle::Mohawk => mohawk(&mut k),
        HairStyle::Mullet => mullet(&mut k, c),
        _ => long_hair(&mut k, c),
    }
    k.hr
}

/// A bare head with one to four single hairs on top.
fn wisps(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let n = 1 + k.count();
    for i in 0..n as usize {
        let deg = (i as f64 - (f64::from(n) - 1.0) / 2.0) * 9.0 + k.lock_lean(i) * 3.0;
        let len = lerp(10.0, 22.0, k.lock_len(i)) * sz;
        let lean = k.lock_lean(i + 4) * 24.0;
        let ctrl = if n == 1 && k.lock_wide(0) > 0.5 {
            vec![
                hd.at(deg, -3.0),
                hd.at(deg + 9.0, 9.0 * sz),
                hd.at(deg + 3.0, 16.0 * sz),
                hd.at(deg - 8.0, 10.0 * sz),
                hd.at(deg - 3.0, 4.5 * sz),
            ]
        } else {
            vec![
                hd.at(deg, -3.0),
                hd.at(deg + lean * 0.3, len * 0.6),
                hd.at(deg + lean, len),
            ]
        };
        k.hair_pen(ctrl, 1.05);
    }
}

/// Horseshoe hair and a combover: hair at the sides of a bare crown. Each
/// side is a ragged wedge behind the head that stands out past the skull
/// above and behind the ear.
fn side_hair(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let ear_top = k.ear_top;
    let a1 = max(22.0, ear_top + 3.0 - lerp(34.0, 48.0, k.fore()));
    let widest = ear_top - 6.0;
    let a2 = ear_top + 15.0;
    let wide = lerp(9.0, 14.0, k.vol()) * sz;
    let hang = lerp(8.0, 14.0, k.line_kind() as f64 / 3.0);
    // How far the wedge stands out past the skull at an angle.
    let reach = |a: f64| {
        wide * if a < widest {
            clamp((a - a1) / (widest - a1), 0.0, 1.0)
        } else {
            lerp(1.0, 0.4, clamp((a - widest) / (a2 - widest), 0.0, 1.0))
        }
    };
    for sd in [-1.0, 1.0] {
        // What the locks grow from. It closes inside the head, which hides that part of it.
        let start = hd.at(sd * a1, -1.0);
        let mut body = vec![sharp(start.x, start.y)];
        for i in 1..=10 {
            let a = lerp(a1, a2, f64::from(i) / 10.0);
            body.push(hd.at(sd * a, 0.55 * reach(a)));
        }
        body.push(hd.at(sd * a2, -6.0));
        body.push(hd.at(sd * lerp(a1, a2, 0.5), -7.0));
        k.hr.back.fills.push(Fill::Shape {
            pts: body,
            edge: Edge::Thick,
            skin: false,
        });
        let n = 5;
        let first = a1 + (widest - a1) * 0.34;
        let gap = (a2 - 7.0 - first) / f64::from(n - 1);
        let half = gap * 0.72;
        for i in 0..n {
            let gi = i as usize + if sd > 0.0 { 5 } else { 0 };
            let a = first + f64::from(i) * gap;
            let ra = hd.at(sd * (a - half), 0.55 * reach(a - half) - 1.5);
            let rb = hd.at(sd * (a + half), 0.55 * reach(a + half) - 1.5);
            let tip = hd.at(
                sd * (a + hang + k.lock_lean(gi) * 3.0),
                reach(a) * lerp(0.95, 1.5, k.lock_len(gi)) + 2.0 * sz,
            );
            let m = mix(ra, rb, 0.5);
            k.lock_in(ra, rb, tip, Edge::Thick, 0.14, Which::Back);
            // A stroke along every other lock, the way the hair grows.
            if i % 2 == 1 {
                k.strand_in(vec![mix(m, tip, 0.25), mix(m, tip, 0.74)], 0.8, Which::Back);
            }
        }
        // Where the wedge begins it frays into two single hairs on the bare scalp.
        if k.style == HairStyle::Horseshoe {
            for j in 0..2 {
                let j = f64::from(j);
                let gi = if sd > 0.0 { 15 } else { 14 };
                let from = a1 + 6.0 - 3.0 * j;
                let far = lerp(9.0, 17.0, k.lock_len(gi)) * (1.0 - 0.25 * j);
                let ctrl = vec![
                    hd.at(sd * from, -(1.4 + 2.4 * j) * sz),
                    hd.at(
                        sd * (from - far * 0.5),
                        -(2.4 + 2.8 * j + k.lock_lean(gi)) * sz,
                    ),
                    hd.at(sd * (from - far), -(1.8 + 3.2 * j) * sz),
                ];
                k.hair_pen(ctrl, 1.0);
            }
        }
    }
    // The tips of the wedge's lowest locks reach this far down, behind the ear.
    k.hr.low = ear_top + 25.0;
    if k.style == HairStyle::Combover {
        // A few long strands laid over the crown from one side to the other.
        let sd = k.part_side();
        let n = 3 + k.count();
        for i in 0..n as usize {
            let kf = i as f64;
            let far = -sd * (16.0 + 9.0 * kf + k.lock_lean(i) * 5.0);
            let ctrl = vec![
                hd.at(sd * (a1 + 5.0 + 2.5 * kf), 0.0),
                hd.at(sd * (a1 - 16.0), -(2.4 + 2.2 * kf) * sz),
                hd.at(sd * 4.0 + far * 0.3, -(3.4 + 2.7 * kf) * sz),
                k.clear
                    .at(hd.at(far, -(4.0 + 2.6 * kf + k.lock_len(i) * 2.5) * sz)),
            ];
            k.hair_pen(ctrl, 1.0);
        }
    }
}

/// A buzz cut, drawn by its hairline: a wide M, with stubble strokes set on
/// an uneven grid inside it.
fn buzz(k: &mut Kit, c: &HairInputs) {
    let hd = k.hd;
    let sz = k.sz;
    let ear_top = k.ear_top;
    let end = ear_top
        + if k.beard_on_jaw {
            4.0
        } else {
            lerp(4.0, 14.0, k.burns())
        };
    let lk = k.line_kind();
    let d0 = k.mid(0.55, 0.3);
    let y_f = -hd.h + d0;
    let peak = [0.0, 0.0, 5.0, 9.0][lk] * sz;
    let y_e = c.ears.yc - c.ears.eh * 0.5;
    // The round corner: this far above the middle of the hairline.
    let rise = min(
        min(d0 * [0.3, 0.42, 0.28, 0.4][lk], 11.0 * sz),
        max(0.0, d0 - 5.0 * sz),
    );
    let y_r = y_f - rise;
    let w_r = hd.hw(y_r);
    let strip = clamp(w_r * 0.25, 6.5 * sz, 12.0 * sz);
    let x_r = max(5.0, w_r - strip);
    // Where a high hairline leaves the corners little room to rise, its middle comes down instead.
    let dip = if lk > 1 {
        peak
    } else {
        max(0.0, 6.0 * sz - rise)
    };
    let brow_out = k.brow_out;
    // A point of the strip's inner edge: `w` units in from the outline, and never over the brows.
    let in_from = |p: Pt, w: f64| {
        let half = hd.hw(p.y);
        pt(max(half - w, min(half - 1.5, brow_out)), p.y)
    };
    let down = |t: f64, w: f64| in_from(pt(0.0, lerp(y_r, y_e, t)), w);
    // One side of the hairline, from the middle of the forehead to the tip of the sideburn.
    let mut side = vec![pt(0.0, y_f + dip).with_sharp(lk > 1)];
    if lk > 1 {
        side.push(pt(min(7.0 * sz, x_r * 0.3), y_f + 0.8 * sz));
    }
    side.extend([
        pt(x_r * 0.46, y_f - rise * 0.06),
        pt(x_r * 0.82, y_f - rise * 0.6),
        pt(x_r, y_r),
        down(0.3, strip * 0.74),
        down(0.64, strip * 0.6),
        down(0.95, strip * 0.48),
        in_from(
            hd.at(lerp(ear_top, end, 0.55), 0.0),
            min(strip * 0.34, 4.0 * sz),
        ),
        hd.at(end, -1.0),
    ]);
    // The outside: `top` units off the skull over the top, a step at each temple, close to the skull below.
    let top = max(5.0, lerp(5.5, 8.0, k.vol()) * sz);
    let a_s = clamp(hd.angle_at(lerp(y_r, y_e, 0.3)), 26.0, ear_top - 10.0);
    let out = |deg: f64| {
        let a = deg.abs();
        if a < a_s {
            top * (1.0 - 0.15 * a / a_s)
        } else {
            lerp(0.9, 0.8, (a - a_s) / (end - a_s))
        }
    };
    let mut outer = Vec::new();
    for i in 0..=6 {
        let deg = -lerp(end, a_s + 1.2, f64::from(i) / 6.0);
        outer.push(hd.at(deg, out(deg)).with_sharp(i == 6));
    }
    for i in 0..=20 {
        let deg = lerp(-(a_s - 1.2), a_s - 1.2, f64::from(i) / 20.0);
        outer.push(hd.at(deg, out(deg)).with_sharp(i % 20 == 0));
    }
    for i in 0..=6 {
        let deg = lerp(a_s + 1.2, end, f64::from(i) / 6.0);
        outer.push(hd.at(deg, out(deg)).with_sharp(i == 0));
    }
    let mut inner = Vec::new();
    for s in [-1.0, 1.0] {
        let from = if s < 0.0 { 0 } else { 1 };
        let mut half: Vec<Pt> = side[from..]
            .iter()
            .map(|q| k.clear.at(pt(s * q.x, q.y)).with_sharp(q.sharp))
            .collect();
        if s < 0.0 {
            half.reverse();
        }
        inner.extend(half);
    }
    let e0 = mix(outer[0], inner[0], 0.5);
    let e1 = mix(outer[outer.len() - 1], inner[inner.len() - 1], 0.5);
    let mut outside = vec![sharp(e0.x, e0.y)];
    outside.extend_from_slice(&outer[1..outer.len() - 1]);
    outside.push(sharp(e1.x, e1.y));
    let mut shape = outside.clone();
    shape.extend(inner[1..inner.len() - 1].iter().rev());
    k.hr.front.fills.push(Fill::Shape {
        pts: shape.clone(),
        edge: Edge::Thin,
        skin: false,
    });
    k.hr.front.rims.push(Rim {
        pts: outside,
        thin: false,
    });
    for p in &inner {
        if p.x.abs() < k.e.x {
            k.hr.fore = max(k.hr.fore, p.y);
        }
    }
    k.hr.low = end;
    // The hair itself: short strokes in a darker shade, each along the way
    // the hair grows, set unevenly on a grid of staggered rows. A stroke is
    // drawn only where the hair has a little room for it on every side.
    let poly = sample(&shape, true, 3);
    let bx = box_of(&poly);
    let crown = pt(0.0, -hd.h * 0.9);
    // Where the level line at height `y` crosses the edge of the hair.
    let cuts_at = |y: f64| {
        let mut xs = Vec::new();
        let mut j = poly.len() - 1;
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[j];
            if (a.y > y) != (b.y > y) {
                xs.push(a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y));
            }
            j = i;
        }
        xs
    };
    let in_hair = |xs: &[f64], x: f64| xs.iter().filter(|&&c| x < c).count() % 2 == 1;
    let strokes = |pack: f64| {
        let mut list = Vec::new();
        let mut row = 0u32;
        let mut y = bx[1] + 2.5;
        while y < bx[3] - 2.0 {
            let line = |j: f64| cuts_at(y + 0.7 * j);
            let rowf = f64::from(row);
            let mut col = 0u32;
            let mut x = bx[0] + 2.5 + f64::from(row % 2) * 3.1 * pack;
            while x < bx[2] - 2.0 {
                let colf = f64::from(col);
                let kk = (1.6 * sin(rowf * 1.9 + colf * 4.1)).round();
                let px = x + 1.3 * sin(rowf * 2.4 + colf * 1.7);
                let py = y + 0.7 * kk;
                let here = line(kk);
                if in_hair(&here, px - 1.4)
                    && in_hair(&here, px + 1.4)
                    && in_hair(&line(kk - 2.0), px)
                    && in_hair(&line(kk + 2.0), px)
                {
                    let u = unit_of(crown, pt(px, py));
                    let lean = 0.3 * sin(rowf * 3.1 + colf * 2.3);
                    let len = 3.4 + 0.8 * sin(rowf * 1.3 + colf * 5.2);
                    let dx = (u.0 - u.1 * lean) * len;
                    let dy = (u.1 + u.0 * lean) * len;
                    list.push([px - dx / 2.0, py - dy / 2.0, dx, dy]);
                }
                col += 1;
                x += 6.2 * pack;
            }
            row += 1;
            y += 5.2 * pack;
        }
        list
    };
    // A small patch with under 28 strokes is still a smooth shell, so its strokes are set closer.
    k.hr.front.shade_ticks = true;
    for pack in [1.0, 0.85, 0.72] {
        k.hr.front.ticks = strokes(pack);
        if k.hr.front.ticks.len() >= 28 {
            break;
        }
    }
}

/// Short hair combed over from a parting. On its far side the hair is
/// fuller and the fringe comes lower; on its near side it is cut shorter.
fn crop(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let ear_top = k.ear_top;
    let sd = k.part_side();
    let part = sd * lerp(16.0, 34.0, k.part_pos());
    let end = k.burn_end();
    let base = k.short_line(
        k.mid(0.5, 0.26),
        [2.5, 5.0, 8.0, 4.0][k.line_kind()],
        0.0,
        end,
    );
    let sweep = lerp(4.0, 10.0, k.lock_wide(3)) * sz;
    let top = move |deg: f64| 1.0 - step(ear_top * 0.5, ear_top * 0.82, deg.abs());
    let far = move |deg: f64| clamp(-sd * (deg - part) / (ear_top * 0.62), 0.0, 1.0) * top(deg);
    let depth = |deg: f64| {
        max(
            1.0,
            base(deg) + sweep * far(deg) - 2.5 * sz * step(0.0, 12.0, sd * (deg - part)) * top(deg),
        )
    };
    let v0 = lerp(5.0, 11.0, k.vol()) * sz;
    let lie = k.short_out(v0, 3.4, end);
    let out = |deg: f64| {
        max(
            1.3,
            lie(deg) * (1.0 + 0.28 * far(deg)) - 0.45 * v0 * hump(deg, part, 7.0),
        )
    };
    k.cap(-end, end, 36, &out, &depth, CapRim::Outer);
    let at = within(hd, k.clear, &out, &depth);
    k.parting(&at, part, -sd * 2.0);
    for f in [0.3, 0.55, 0.8] {
        k.strand(
            vec![
                at(part - sd * 7.0, f),
                at(part - sd * 28.0, f + 0.04),
                at(part - sd * 54.0, f - 0.02),
            ],
            1.0,
        );
    }
    for f in [0.4, 0.76] {
        k.strand(
            vec![
                at(part + sd * 7.0, f),
                at(part + sd * 18.0, f + 0.03),
                at(part + sd * 32.0, f),
            ],
            1.0,
        );
    }
    if k.cowlick() {
        k.add_cowlick(-sd, &out);
    }
    // Where the swept hair ends, on the far side, a lock or two stand off the head.
    let n = k.count() as usize;
    let list = [
        [-sd * (ear_top - 24.0), -sd * 15.0, 5.0, 6.0],
        [-sd * (ear_top - 44.0), -sd * 13.0, 4.5, 7.0],
        [-sd * (ear_top - 62.0), -sd * 11.0, 4.0, 8.0],
    ];
    k.tufts(&out, &list[..n.min(3)]);
}

/// Short at the sides; on the far side of a parting the hair stands up tall
/// and falls away towards the ear.
fn quiff(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let sd = k.part_side();
    let part = sd * lerp(24.0, 40.0, k.part_pos());
    let end = k.burn_end();
    let base = k.short_line(
        k.mid(0.42, 0.22),
        [2.5, 5.0, 8.0, 4.0][k.line_kind()],
        0.0,
        end,
    );
    let depth = |deg: f64| max(1.0, base(deg) - 5.5 * sz * hump(deg, part, 7.0));
    let tall = lerp(15.0, 32.0, k.vol()) * sz;
    let lie = k.short_out(4.5 * sz, 2.4, end);
    let out = |deg: f64| {
        max(
            1.3,
            lie(deg)
                + tall * hump(deg, -sd * 12.0, 46.0) * (0.55 + 0.45 * step(-34.0, 14.0, sd * deg))
                - 1.8 * sz * hump(deg, part, 6.0),
        )
    };
    k.cap(-end, end, 36, &out, &depth, CapRim::Outer);
    let at = within(hd, k.clear, &out, &depth);
    k.parting(&at, part, -sd * 3.0);
    for i in 0..4 {
        let a0 = part - sd * (9.0 + 13.0 * f64::from(i));
        k.strand(
            vec![
                at(a0, 0.07),
                at(a0 - sd * 8.0, 0.5),
                at(a0 - sd * 21.0, 0.9),
            ],
            1.0,
        );
    }
}

/// Combed back off the forehead and over to one side, with nothing in it
/// even: deep uneven corners, a wave on one side where the hair lifts off
/// the skull, comb lines each at its own place and length, and ends that
/// stand off the head behind each ear.
fn slick(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let ear_top = k.ear_top;
    let sd = k.part_side();
    let end = k.burn_end();
    let lk = k.line_kind();
    let parted = lk < 2;
    let part = if parted {
        sd * lerp(22.0, 36.0, k.part_pos())
    } else {
        0.0
    };
    let base = k.short_line(
        k.mid(0.52, 0.3),
        [7.0, 10.0, 13.0, 9.0][lk],
        if parted { 0.0 } else { 9.0 },
        end,
    );
    let depth = |deg: f64| {
        max(
            1.0,
            base(deg) - 3.0 * sz * hump(deg, sd * ear_top * 0.5, ear_top * 0.27)
                + (if parted { 4.5 } else { 2.5 }) * sz * hump(deg, -sd * 15.0, 17.0)
                - if parted {
                    5.0 * sz * hump(deg, part, 7.0)
                } else {
                    0.0
                },
        )
    };
    let v0 = lerp(3.6, 7.5, k.vol()) * sz;
    let lie = k.short_out(v0, 3.0, end);
    let lift = lerp(5.5, 10.0, k.lock_wide(5)) * sz;
    let out = |deg: f64| {
        max(
            1.3,
            lie(deg) + lift * hump(deg, -sd * 20.0, 40.0)
                - if parted {
                    0.45 * v0 * hump(deg, part, 7.0)
                } else {
                    0.0
                },
        )
    };
    k.cap(-end, end, 36, &out, &depth, CapRim::Outer);
    let at = within(hd, k.clear, &out, &depth);
    if parted {
        k.parting(&at, part, -sd * 2.5);
    }
    let from = if parted {
        part - sd * 8.0
    } else {
        sd * (ear_top - 30.0)
    };
    let to = -sd * (ear_top - 16.0);
    let n = (if parted { 5 } else { 6 }) + (k.count() >> 1) as usize;
    for i in 0..n {
        let a0 = lerp(from, to, (i as f64 + 0.5 + 0.3 * k.lock_lean(i)) / n as f64);
        let f0 = if k.lock_len(i) > 0.55 { 0.28 } else { 0.06 };
        let f1 = lerp(0.72, 0.96, k.lock_len(i + 5));
        let sweep = -sd * lerp(7.0, 17.0, k.lock_wide(i));
        let weight = lerp(0.85, 1.2, k.lock_len(i + 9));
        k.strand(
            vec![
                at(a0, f0),
                at(a0 + sweep * 0.4, lerp(f0, f1, 0.5)),
                at(a0 + sweep, f1),
            ],
            weight,
        );
    }
    if parted {
        k.strand(
            vec![
                at(part + sd * 7.0, 0.42),
                at(part + sd * 17.0, 0.46),
                at(part + sd * 30.0, 0.42),
            ],
            0.9,
        );
    }
    let mut list = vec![
        [-(ear_top - 22.0), -17.0, 5.5, 2.0],
        [ear_top - 22.0, 17.0, 5.5, 3.0],
    ];
    if k.count() & 1 == 1 {
        list.push([-sd * 42.0, -sd * 14.0, 4.5, 4.0]);
    }
    k.tufts(&out, &list);
}

/// Short at the sides, with a row of spikes standing up from the top that
/// all lean one way, and two or three short points over the forehead.
fn spikes(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let end = k.burn_end();
    let base = k.short_line(k.mid(0.5, 0.26), 4.0, 0.0, end);
    let depth = |deg: f64| {
        max(
            1.0,
            base(deg) + 2.0 * sz * sin(deg * 0.36) * (1.0 - step(40.0, 70.0, deg.abs())),
        )
    };
    let out = k.short_out(4.6 * sz, 3.2, end);
    k.cap(-end, end, 34, &out, &depth, CapRim::Outer);
    let n = if k.count() < 2 { 7 } else { 9 };
    let spread = 66.0;
    let gap = 2.0 * spread / f64::from(n - 1);
    let v = k.vol();
    let sweep = k.part_side() * lerp(9.0, 17.0, k.part_pos());
    for i in 0..n as usize {
        let deg = -spread + i as f64 * gap + k.lock_lean(i) * gap * 0.16;
        let t = deg / spread;
        let len = lerp(13.0, 23.0, k.lock_len(i)) * lerp(0.85, 1.4, v) * sz * (1.0 - 0.34 * t * t);
        let lean = sweep + k.lock_lean(i + 8) * 5.0;
        let ra = hd.at(deg - gap * 0.8, 2.0);
        let rb = hd.at(deg + gap * 0.8, 2.0);
        let tip = hd.at(deg + lean, 2.0 + len);
        let m = mix(ra, rb, 0.5);
        k.lock(ra, rb, tip, Edge::Thick, 0.1);
        k.strand(vec![mix(m, tip, 0.2), mix(m, tip, 0.7)], 0.85);
    }
    k.fringe(2 + (k.count() & 1), &depth, 19.0, [6.5, 5.5], 3, 0.0);
}

/// Unruly hair: tufts that point every way, and a fringe of loose locks.
fn tousle(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let end = k.burn_end();
    let depth = k.short_line(k.mid(0.5, 0.28), 3.0, 0.0, end);
    let out = k.short_out(lerp(6.0, 11.0, k.vol()) * sz, 5.5 * sz, end);
    k.cap(-end, end, 34, &out, &depth, CapRim::Outer);
    let n = 7 + k.count();
    let spread = k.ear_top - 10.0;
    let gap = 2.0 * spread / f64::from(n - 1);
    let sweep = k.part_side() * 9.0;
    for i in 0..n as usize {
        let deg = -spread + i as f64 * gap + k.lock_lean(i + 2) * gap * 0.2;
        let len = lerp(5.0, 16.0, k.lock_len(i)) * sz;
        let lean = k.lock_lean(i) * 26.0 + sweep;
        let half = gap * lerp(0.5, 0.74, k.lock_wide(i));
        k.lock(
            hd.at(deg - half, out(deg - half) - 2.0),
            hd.at(deg + half, out(deg + half) - 2.0),
            hd.at(deg + lean, out(deg) + len),
            Edge::Thick,
            0.14,
        );
    }
    let m = 3 + (k.count() >> 1);
    k.fringe(m, &depth, 56.0 / f64::from(m), [5.0, 10.0], 9, sweep * 0.5);
    let at = within(hd, k.clear, &out, &depth);
    for deg in [-44.0, 8.0, 50.0] {
        k.strand(
            vec![
                at(deg, 0.3),
                at(deg + sweep, 0.62),
                at(deg + sweep * 2.2, 0.9),
            ],
            0.9,
        );
    }
}

/// Curly hair and an afro: a cloud of rounds along the outer edge and along
/// the hairline, where they hang onto the forehead.
fn curls(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let ear_top = k.ear_top;
    let afro = k.style == HairStyle::Afro;
    let v = k.vol();
    let rb = (if afro {
        lerp(10.5, 14.0, k.lock_wide(0))
    } else {
        lerp(7.0, 11.0, v)
    }) * sz;
    let full = if afro {
        lerp(12.0, 26.0, v) * sz
    } else {
        rb * 0.4
    };
    let depth = k.short_line(k.mid(0.5, 0.26), 2.0, 0.0, ear_top + 6.0);
    let edge_to = if afro { 100.0 } else { ear_top - 1.0 };
    // Short curls may be piled higher on top.
    let piled = if afro {
        0.0
    } else {
        lerp(0.0, 13.0, k.length()) * sz
    };
    // An afro is fullest over the top and draws in towards the ears.
    let reach_at = |deg: f64| {
        full * if afro {
            1.0 - 0.6 * step(60.0, 100.0, deg.abs())
        } else {
            1.0
        }
    };
    let body = if afro { Which::Back } else { Which::Front };
    // The rounds of the outer edge, spaced by their own size.
    let mut deg = -edge_to;
    let mut i = 0;
    while deg <= edge_to + 0.01 {
        let reach = reach_at(deg);
        let r = rb * lerp(0.86, 1.16, k.lock_len(i));
        let p = hd.at(
            deg,
            if afro {
                reach - r * 0.55
            } else {
                r * 0.25 + piled * hump(deg, 0.0, 52.0)
            },
        );
        k.mass(body).fills.push(Fill::Circle {
            x: p.x,
            y: p.y,
            r,
            edge: Edge::Thick,
        });
        deg += max(4.0, (r * 1.22 / (hd.rim(deg) + reach)) / DEG);
        i += 1;
    }
    if afro {
        // What fills the cloud between its rounds and the head.
        let mut fill = Vec::new();
        let mut deg = -edge_to;
        while deg <= edge_to {
            fill.push(hd.at(deg, reach_at(deg) - rb * 0.6));
            deg += 12.0;
        }
        k.hr.back.fills.push(Fill::Shape {
            pts: fill,
            edge: Edge::None,
            skin: false,
        });
    }
    k.cap(
        -(ear_top + 6.0),
        ear_top + 6.0,
        28,
        &|_| 1.6,
        &depth,
        CapRim::None,
    );
    let rh = (if afro {
        lerp(5.5, 7.5, k.lock_wide(1))
    } else {
        lerp(4.6, 6.4, k.lock_wide(1))
    }) * sz;
    let line_to = min(64.0, ear_top - 22.0);
    let mut deg = -line_to;
    let mut i = 0;
    while deg <= line_to + 0.01 {
        let r = rh * lerp(0.88, 1.14, k.lock_len(i + 8));
        let p = k.clear.at(hd.at(deg, -depth(deg)));
        // A round of the hairline may hang onto the forehead, but its lowest point keeps clear of the brows.
        let y = k.clear.at(pt(p.x, p.y + r)).y - r;
        k.hr.front.fills.push(Fill::Circle {
            x: p.x,
            y,
            r,
            edge: Edge::Thin,
        });
        if p.x.abs() < k.e.x {
            k.hr.fore = max(k.hr.fore, y + r);
        }
        deg += max(4.0, (r * 1.3 / max(20.0, hd.rim(deg) - depth(deg))) / DEG);
        i += 1;
    }
    // A few curls drawn in.
    let edge = if afro { full * 0.5 } else { rb * 0.5 };
    let edge_fn = move |_: f64| edge;
    let at = within(hd, k.clear, &edge_fn, &depth);
    let degs: &[f64] = if afro {
        &[-70.0, -34.0, 0.0, 34.0, 70.0]
    } else {
        &[-48.0, -16.0, 16.0, 48.0]
    };
    for &deg in degs {
        let c = at(deg + k.lock_lean(5) * 4.0, if afro { 0.9 } else { 0.72 });
        k.strand_in(
            vec![
                pt(c.x - 3.4, c.y + 1.2),
                pt(c.x - 1.0, c.y - 3.0),
                pt(c.x + 3.2, c.y - 0.6),
                pt(c.x + 1.2, c.y + 2.8),
            ],
            0.9,
            body,
        );
    }
}

/// A crest down the middle of the skull: a narrow strip of hair that comes
/// to a point over the forehead, and tall locks so close they are one fin.
fn mohawk(k: &mut Kit) {
    let hd = k.hd;
    let sz = k.sz;
    let fan = lerp(7.0, 12.0, k.part_pos());
    let n = if k.count() < 2 { 5 } else { 7 };
    let v = k.vol();
    let d0 = k.mid(0.4, 0.2);
    let depth = |deg: f64| {
        max(
            1.0,
            lerp(d0, 3.0, step(0.0, fan + 4.0, deg.abs())) + 4.0 * sz * hump(deg, 0.0, 6.0),
        )
    };
    k.cap(-(fan + 4.0), fan + 4.0, 10, &|_| 2.4, &depth, CapRim::Outer);
    let gap = 2.0 * fan / f64::from(n - 1);
    for i in 0..n as usize {
        let deg = -fan + i as f64 * gap;
        let t = deg / fan;
        let len = lerp(30.0, 50.0, v) * sz * (1.0 - 0.45 * t * t) * lerp(0.9, 1.08, k.lock_len(i));
        let ra = hd.at(deg - gap * 0.9, 1.6);
        let rb = hd.at(deg + gap * 0.9, 1.6);
        let tip = hd.at(deg * 1.2 + k.lock_lean(i) * 2.5, 1.6 + len);
        let m = mix(ra, rb, 0.5);
        k.lock(ra, rb, tip, Edge::Thick, 0.08);
        k.strand(vec![mix(m, tip, 0.12), mix(m, tip, 0.74)], 0.85);
    }
    if k.shaved() {
        for sd in [-1.0, 1.0] {
            for i in 0..6 {
                let p = hd.at(
                    sd * lerp(fan + 20.0, k.ear_top - 4.0, f64::from(i) / 5.0),
                    -(if i % 2 == 1 { 6.0 } else { 13.0 }) * sz,
                );
                k.hr.front.dots.push((p.x, p.y, 1.1));
            }
        }
    }
}

/// Short and feathered on top, long at the nape.
fn mullet(k: &mut Kit, c: &HairInputs) {
    let hd = k.hd;
    let sz = k.sz;
    let end = k.burn_end();
    let depth = k.short_line(k.mid(0.5, 0.28), 4.0, 0.0, end);
    let out = k.short_out(lerp(5.0, 9.5, k.vol()) * sz, 4.4 * sz, end);
    k.cap(-end, end, 34, &out, &depth, CapRim::Outer);
    let n = if k.count() < 2 { 7 } else { 9 };
    let spread = k.ear_top - 14.0;
    let gap = 2.0 * spread / f64::from(n - 1);
    let sweep = k.part_side() * 8.0;
    for i in 0..n as usize {
        let deg = -spread + i as f64 * gap + k.lock_lean(i + 2) * gap * 0.18;
        let len = lerp(4.0, 11.0, k.lock_len(i)) * sz;
        let lean = k.lock_lean(i) * 14.0 + sweep;
        let half = gap * 0.62;
        k.lock(
            hd.at(deg - half, out(deg - half) - 2.0),
            hd.at(deg + half, out(deg + half) - 2.0),
            hd.at(deg + lean, out(deg) + len),
            Edge::Thick,
            0.14,
        );
    }
    k.fringe(
        3 + (k.count() & 1),
        &depth,
        17.0,
        [4.5, 8.0],
        9,
        sweep * 0.5,
    );
    let at = within(hd, k.clear, &out, &depth);
    for deg in [-46.0, 6.0, 48.0] {
        k.strand(
            vec![
                at(deg, 0.3),
                at(deg + sweep, 0.62),
                at(deg + sweep * 2.0, 0.9),
            ],
            0.9,
        );
    }
    // The long hair at the nape, seen from the front: behind each ear, locks that flick out and down.
    for s in [-1.0, 1.0] {
        let m = 2 + (k.count() & 1) as usize;
        for j in 0..m {
            let gi = j + if s > 0.0 { 13 } else { 9 };
            let ya = c.ears.yc + c.ears.eh * 0.45 + j as f64 * 7.0 * sz;
            let xa = hd.hw(ya) - 3.0;
            let len = lerp(14.0, 27.0, k.length()) * sz * lerp(0.8, 1.15, k.lock_len(gi));
            let pts = lock_pts(
                pt(s * xa, ya - 5.0 * sz),
                pt(s * xa, ya + 6.0 * sz),
                pt(
                    s * (xa + len * 0.75),
                    ya + len * 0.8 + k.lock_lean(gi) * 3.0,
                ),
                0.16,
            );
            k.hr.back.fills.push(Fill::Shape {
                pts,
                edge: Edge::Thick,
                skin: false,
            });
        }
    }
}

/// The styles that fall past the ears: curtains (to the jaw), a bob (to the
/// chin), long and wavy hair (to the bottom of the picture) and ropes. Long
/// hair hangs behind the head in two falls that never join under the chin;
/// on top, a cap with a parting gives the hairline.
fn long_hair(k: &mut Kit, c: &HairInputs) {
    let hd = k.hd;
    let sz = k.sz;
    let style = k.style;
    // How far down the picture goes: the base of the neck, or a little under the chin.
    let base = c.neck.map_or(12.0 * sz, |n| n.len);
    let sd = k.part_side();
    let part = sd * k.part_pos() * 17.0;
    let v = k.vol();
    let wavy = style == HairStyle::Wavy;
    let ropes = style == HairStyle::Dreads;
    let bob = style == HairStyle::Bob;
    let v0 = lerp(4.5, 12.0, v) * sz;
    let v1 = (if ropes {
        lerp(4.5, 7.0, v)
    } else if bob {
        lerp(10.0, 18.0, v)
    } else {
        lerp(7.0, 14.0, v)
    }) * sz;
    let t_end = k.ear_top - 2.0;
    let side = lerp(10.0, 24.0, k.fore()) * sz;
    // The bare forehead under the parting is never taller than 32% of the head.
    let deepest = max(3.0, k.mid(1.0, 1.0) - 0.32 * hd.h);
    let side_deep = max(side, deepest + lerp(3.0, 8.0, k.fore()) * sz);
    // Over the top the hair is at least a round dome standing on straight sides.
    let dome_r = hd.at(90.0, v1).x;
    let dome_up = hd.rim(0.0) + v0 - dome_r;
    let dome = |deg: f64| {
        let u = up_dir(deg);
        let ux = u.0.abs();
        let uy = -u.1;
        let mut r = if ux * dome_up <= uy * dome_r || dome_up <= 0.0 {
            uy * dome_up
                + max(
                    0.0,
                    uy * uy * dome_up * dome_up - dome_up * dome_up + dome_r * dome_r,
                )
                .sqrt()
        } else {
            dome_r / ux
        };
        if dome_up <= 0.0 {
            r = min(r, hd.rim(0.0) + v0);
        }
        0.97 * r - hd.rim(deg)
    };
    let out = |deg: f64| {
        let a = deg.abs();
        let lie = lerp(v0, v1, step(18.0, 80.0, a))
            + if wavy {
                1.6 * sin(a * 0.22) * step(30.0, 60.0, a)
            } else {
                0.0
            };
        max(
            1.3,
            max(lie, if a < 90.0 { dome(deg) } else { 0.0 }) - 0.4 * v0 * hump(deg, part, 6.0),
        )
    };
    let depth = |deg: f64| {
        max(
            1.0,
            lerp(deepest, side_deep, step(0.0, 40.0, (deg - part).abs()))
                * (1.0 - 0.92 * step(t_end - 26.0, t_end, deg.abs())),
        )
    };
    k.cap(-t_end, t_end, 40, &out, &depth, CapRim::Hairline);
    let at = within(hd, k.clear, &out, &depth);
    k.parting(&at, part, 0.0);
    for s in [-1.0, 1.0] {
        let near = part + s * 9.0;
        let far = s * (t_end - 16.0);
        k.strand(
            vec![at(near, 0.3), at(lerp(near, far, 0.5), 0.38), at(far, 0.5)],
            1.0,
        );
        k.strand(
            vec![
                at(near + s * 8.0, 0.62),
                at(lerp(near, far, 0.45), 0.68),
                at(lerp(near, far, 0.8), 0.72),
            ],
            1.0,
        );
    }
    // Behind the head: round the skull, down the outside of each fall to its
    // end, and back up its inner edge to an arch that the head hides.
    let y_end = if ropes {
        0.0
    } else if style == HairStyle::Curtains {
        lerp(c.face.mouth_y - 6.0, -hd.h * 0.02, k.length())
    } else if bob {
        lerp(c.face.mouth_y + 2.0, 6.0 * sz, k.length())
    } else {
        base + 2.0
    };
    let mut right = Vec::new();
    let mut deg = 0.0;
    while deg <= 96.0 {
        right.push(hd.at(deg, out(deg)));
        deg += 8.0;
    }
    // Hair hangs: below a waist in the skull it comes straight down from the
    // widest point it has reached.
    if !ropes {
        let mut widest: f64 = 0.0;
        for p in &mut right {
            let mut below: f64 = 0.0;
            let mut y = p.y;
            while y <= y_end - 5.0 {
                below = max(below, hd.hw(y) + 5.0 * sz);
                y += 4.0;
            }
            widest = max(widest, p.x);
            p.x = max(p.x, min(widest, below));
        }
    }
    let top = right[right.len() - 1];
    if ropes {
        // Ropes of hair hang behind the head at each side, each to its own length.
        let mut half = right.clone();
        half.extend([
            pt(top.x - 4.0, top.y + 16.0 * sz),
            pt(top.x * 0.6, top.y + 22.0 * sz),
            pt(0.0, top.y + 18.0 * sz),
        ]);
        let mut pts = half.clone();
        pts.extend(half[1..half.len() - 1].iter().rev().map(|p| pt(-p.x, p.y)));
        k.hr.back.fills.push(Fill::Shape {
            pts,
            edge: Edge::Thick,
            skin: false,
        });
        let n = 4 + (k.count() >> 1) as usize;
        for s in [-1.0, 1.0] {
            for i in 0..n {
                let gi = i + if s > 0.0 { 8 } else { 0 };
                let x = top.x - 5.0 * sz + i as f64 * 9.5 * sz;
                let ye = lerp(c.face.mouth_y - 2.0, base, k.lock_len(gi)) - i as f64 * 4.0 * sz;
                let pts = vec![
                    pt(s * min(x, hd.w - 4.0), top.y - 14.0 * sz),
                    pt(s * (x + 1.5 * sz), lerp(top.y, ye, 0.5)),
                    pt(s * (x + k.lock_lean(gi) * 3.0 * sz), ye),
                ];
                let w = lerp(7.5, 10.0, k.lock_wide(gi)) * sz;
                k.hr.back.fills.push(Fill::Tube {
                    pts,
                    w,
                    edge: Edge::Thick,
                });
            }
        }
        // The roots of the ropes, drawn on the cap.
        for deg in [-70.0, -48.0, -26.0, 26.0, 48.0, 70.0] {
            k.strand(vec![at(deg, 0.2), at(deg * 1.04, 0.9)], 1.0);
        }
        return;
    }
    let flare = (if bob {
        lerp(-2.0, 5.0, v)
    } else {
        lerp(0.0, 8.0, v)
    }) * sz;
    let amp = if wavy {
        lerp(3.4, 5.6, k.lock_wide(2)) * sz
    } else {
        0.0
    };
    let e = k.ends();
    let n = if bob || style == HairStyle::Curtains {
        3
    } else {
        6
    };
    // Down the outside of the fall, which never turns back in under a jaw.
    let mut reach = top.x;
    for i in 1..=n {
        let t = f64::from(i) / f64::from(n);
        let y = lerp(top.y, y_end - 5.0, t);
        reach = max(max(reach, top.x + flare * t), hd.hw(y) + 5.0 * sz);
        right.push(pt(
            reach + amp * sin(f64::from(i) * 1.9 + k.lock_lean(3) * 2.0),
            y,
        ));
    }
    let xo = right[right.len() - 1].x;
    // The inner edge of a fall keeps its distance from the neck, and starts behind the head.
    let xi = min(
        min(
            max(
                xo - lerp(16.0, 30.0, v) * sz,
                c.neck.map_or(0.0, |n| n.w) + 13.0 * sz,
            ),
            hd.w - 6.0,
        ),
        xo - 11.0 * sz,
    );
    let w = xo - xi;
    // How the fall ends: cut to two points, rounded off, flicked outwards, or cut in steps.
    let tips = match e {
        1 => vec![
            sharp(xo - w * 0.08, y_end),
            sharp(xi + w * 0.06, y_end - 0.6),
        ],
        2 => vec![
            pt(xo - w * 0.25, y_end + 1.5),
            pt(xi + w * 0.3, y_end + 0.5),
            pt(xi, y_end - 6.0),
        ],
        3 => vec![
            sharp(xo + 5.0 * sz, y_end - 1.0),
            sharp(xo - w * 0.45, y_end - 5.0),
            sharp(xi + w * 0.25, y_end + 1.5),
            pt(xi, y_end - 7.0),
        ],
        _ => vec![
            sharp(xo - w * 0.22, y_end + 2.0 + k.lock_len(6) * 3.0),
            sharp(xo - w * 0.5, y_end - 5.0),
            sharp(xi + w * 0.24, y_end + k.lock_len(7) * 3.0),
            pt(xi, y_end - 7.0),
        ],
    };
    // The arch between the falls sits where the head is still wider than their inner edges.
    let mut y_a = min(y_end - 12.0, 0.0);
    while hd.hw(y_a) < xi + 4.0 && y_a > -hd.h * 0.7 {
        y_a -= 3.0;
    }
    y_a -= 7.0;
    let mut half = right;
    half.extend(tips);
    half.extend([
        pt(xi, lerp(y_end, y_a, 0.5)),
        pt(xi * 0.9, y_a),
        pt(0.0, y_a - 7.0),
    ]);
    let mut pts = half.clone();
    pts.extend(
        half[1..half.len() - 1]
            .iter()
            .rev()
            .map(|p| Pt { x: -p.x, ..*p }),
    );
    k.hr.back.fills.push(Fill::Shape {
        pts,
        edge: Edge::Thick,
        skin: false,
    });
    let wave = |i: f64| {
        if wavy {
            amp * 0.8 * sin(i * 1.9 + 1.0)
        } else {
            0.0
        }
    };
    for s in [-1.0, 1.0] {
        k.strand_in(
            vec![
                pt(s * (xo - w * 0.3 + wave(1.0)), lerp(top.y, y_end, 0.3)),
                pt(s * (xo - w * 0.34 + wave(2.0)), lerp(top.y, y_end, 0.62)),
                pt(s * (xo - w * 0.3 + wave(3.0)), y_end - 8.0),
            ],
            1.0,
            Which::Back,
        );
        if w > 17.0 * sz {
            k.strand_in(
                vec![
                    pt(s * (xi + w * 0.3 + wave(2.0)), lerp(top.y, y_end, 0.5)),
                    pt(s * (xi + w * 0.32 + wave(3.0)), lerp(top.y, y_end, 0.75)),
                    pt(s * (xi + w * 0.3), y_end - 7.0),
                ],
                0.9,
                Which::Back,
            );
        }
    }
}
