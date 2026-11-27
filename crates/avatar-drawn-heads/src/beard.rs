//! Facial hair: a mass like the hair, in a darker shade of the hair's
//! colour. Its upper edge climbs from the jaw to the corner of the mouth, the
//! mouth always shows, and it never joins the hair.

use crate::face::{Mouth, MouthLine, MouthPose};
use crate::genes::{dna, Genes};
use crate::geom::{
    clamp, hump, hyp, inside_poly, lerp, max, min, mix, pt, sample, sharp, step, unit_of, Profile,
    Pt,
};
use crate::hair::{lock_pts, Edge, Fill, HairStyle, Mass, PenStroke, Rim, Tone};
use crate::head::{Ears, FaceLayout, Head};
use crate::palette::Palette;

/// A kind of facial hair, in gene order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeardKind {
    None,
    Stubble,
    Moustache,
    Goatee,
    Patch,
    Full,
    Long,
    Wizard,
}

const BEARD_KINDS: [(BeardKind, &str, u32); 8] = [
    (BeardKind::None, "none", 18),
    (BeardKind::Stubble, "stubble", 2),
    (BeardKind::Moustache, "moustache", 5),
    (BeardKind::Goatee, "goatee", 2),
    (BeardKind::Patch, "patch", 1),
    (BeardKind::Full, "full", 2),
    (BeardKind::Long, "long", 1),
    (BeardKind::Wizard, "wizard", 1),
];

/// The kinds of moustache, by weight; the empty name is no moustache.
pub const TASH_KINDS: [(&str, u32); 4] = [
    ("chevron", 3),
    ("handlebar", 2),
    ("walrus", 1),
    ("pencil", 2),
];

impl BeardKind {
    pub fn name(self) -> &'static str {
        BEARD_KINDS[self as usize].1
    }

    /// Every kind's name, in gene order.
    pub fn names() -> Vec<&'static str> {
        BEARD_KINDS.iter().map(|k| k.1).collect()
    }

    /// The beards that grow along the jaw.
    pub fn on_jaw(self) -> bool {
        matches!(self, BeardKind::Full | BeardKind::Long | BeardKind::Wizard)
    }
}

/// Which facial hair a head has. Where the hair falls past the ears, a
/// beard along the jaw keeps to the chin.
pub fn beard_kind_of(g: &Genes, style: HairStyle) -> BeardKind {
    let kind = BEARD_KINDS[g.pick(dna::BEARD, 0, &BEARD_KINDS.map(|k| k.2))].0;
    if style.past_ears() && kind.on_jaw() {
        BeardKind::Goatee
    } else {
        kind
    }
}

#[derive(Clone, Debug)]
pub struct Beard {
    pub kind: BeardKind,
    pub mass: Mass,
    /// The moustache, or the empty string for none.
    pub tash: &'static str,
    /// Whether it covers the chin, so that the mouth gets a rim of skin.
    pub covers: bool,
    /// The angle at which a beard along the jaw begins.
    pub top: f64,
}

pub struct BeardInputs<'a> {
    pub head: &'a Head,
    pub face: &'a FaceLayout,
    pub mouth: &'a Mouth,
    pub ears: &'a Ears,
    pub palette: &'a Palette,
}

const O: usize = dna::BEARD;

struct Builder<'a> {
    g: &'a Genes<'a>,
    hd: &'a Head,
    f: &'a FaceLayout,
    m: &'a Mouth,
    sz: f64,
    yn: f64,
    bd: Beard,
}

fn mirror(pts: &[Pt]) -> Vec<Pt> {
    pts.iter().map(|p| Pt { x: -p.x, ..*p }).collect()
}

fn reversed(mut pts: Vec<Pt>) -> Vec<Pt> {
    pts.reverse();
    pts
}

impl Builder<'_> {
    fn long(&self) -> f64 {
        self.g.unit(O, 5, 3)
    }
    fn tuft_len(&self, i: usize) -> f64 {
        self.g.unit(O + 3 + (i % 8), 0, 3)
    }
    fn tuft_lean(&self, i: usize) -> f64 {
        self.g.signed(O + 3 + (i % 8), 3, 3)
    }

    fn pen(&mut self, ctrl: Vec<Pt>, k: f64, tone: Tone) {
        self.bd.mass.pens.push(PenStroke {
            ctrl,
            k,
            tone,
            press: if tone == Tone::Hair {
                Profile::Out
            } else {
                Profile::Leaf
            },
        });
    }

    fn fill(&mut self, pts: Vec<Pt>, edge: Edge) {
        self.bd.mass.fills.push(Fill::Shape {
            pts,
            edge,
            skin: false,
        });
    }

    /// A moustache: asked for, or present when its gene says so.
    fn tash(&mut self, need: bool) {
        let (hd, m, f, sz, yn) = (self.hd, self.m, self.f, self.sz, self.yn);
        if !need && !self.g.flag(O + 1, 5) {
            return;
        }
        let mut tk = TASH_KINDS[self.g.pick(O + 1, 0, &TASH_KINDS.map(|t| t.1))].0;
        let room = m.y - f.nose_y - 6.5;
        let big = self.g.unit(O + 1, 3, 2);
        // A lip too short for a moustache with body takes a pencilled one.
        if room < 3.2 && tk != "pencil" {
            tk = "pencil";
        }
        let mw = min(
            min(m.w * lerp(0.9, 1.16, big) + 2.0, hd.hw(m.y - 4.0) - 8.0),
            hd.hw(m.y - 4.0) * 0.62,
        );
        let th = clamp(lerp(4.6, 8.6, big) * sz, 3.0, max(3.0, room));
        let yb = yn + th;
        self.bd.tash = tk;
        if tk == "pencil" {
            for s in [-1.0, 1.0] {
                self.pen(
                    vec![
                        pt(s * 1.6, yn + 1.6),
                        pt(s * mw * 0.5, yn + 0.9),
                        pt(s * mw, yn + 3.2),
                    ],
                    1.25 + big * 0.5,
                    Tone::Hair,
                );
            }
            return;
        }
        // One shape: up to a dip under the nose, out to a tip at each side, and back along the lip.
        let tip = match tk {
            "handlebar" => pt(mw + 6.5 * sz, yn - 2.5 * sz),
            "walrus" => pt(mw + 3.0, m.y + 4.5),
            _ => pt(mw + 1.5, min(yb + 2.5, m.y - 0.5)),
        };
        let half = match tk {
            "handlebar" => vec![
                pt(mw * 0.5, yn - 0.6),
                pt(mw + 1.5, yn + th * 0.5),
                sharp(tip.x, tip.y),
                pt(mw + 2.5, yb + 1.2),
                pt(mw * 0.5, yb + 0.4),
            ],
            "walrus" => vec![
                pt(mw * 0.5, yn - 1.0),
                pt(mw + 2.0, yn + th * 0.6),
                sharp(tip.x, tip.y),
                pt(mw * 0.72, m.y - 1.6),
                pt(mw * 0.4, yb + 0.6),
            ],
            _ => vec![
                pt(mw * 0.5, yn - 0.4),
                sharp(tip.x, tip.y),
                pt(mw * 0.5, yb + 0.6),
            ],
        };
        let mut pts = vec![pt(0.0, yn + 1.5)];
        pts.extend_from_slice(&half);
        pts.push(pt(0.0, yb + if tk == "walrus" { -0.8 } else { 0.9 }));
        pts.extend(reversed(mirror(&half)));
        self.fill(pts, Edge::Thin);
        for s in [-1.0, 1.0] {
            self.pen(
                vec![
                    pt(s * mw * 0.2, yn + th * 0.45),
                    pt(s * mw * 0.6, yn + th * 0.5),
                    pt(s * mw * 0.92, lerp(yn + th * 0.5, tip.y, 0.7)),
                ],
                0.85,
                Tone::Line,
            );
        }
    }
}

pub fn make_beard(g: &Genes, c: &BeardInputs, kind: BeardKind) -> Beard {
    let hd = c.head;
    let m = c.mouth;
    let sz = hd.size;
    let yn = c.face.nose_y + 3.4;
    let mut b = Builder {
        g,
        hd,
        f: c.face,
        m,
        sz,
        yn,
        bd: Beard {
            kind,
            mass: Mass::default(),
            tash: "",
            covers: false,
            top: 0.0,
        },
    };
    match kind {
        BeardKind::None => return b.bd,
        BeardKind::Stubble => {
            // Flecks along the jaw and over the lip.
            let mut deg = 118.0;
            let mut i = 0u32;
            while deg <= 242.0 {
                for d in [
                    0.07 + f64::from(i % 2) * 0.05,
                    0.2 + f64::from(i % 3) * 0.04,
                ] {
                    let p = hd.at_depth(deg, 0.0, d);
                    if p.x.abs() < m.w + 4.0 && (p.y - m.y).abs() < 8.0 {
                        continue;
                    }
                    b.bd.mass
                        .ticks
                        .push([p.x, p.y, if i % 2 == 1 { 0.5 } else { -0.4 }, 1.5]);
                }
                deg += 7.0;
                i += 1;
            }
            for u in [-0.85, -0.5, -0.18, 0.18, 0.5, 0.85] {
                b.bd.mass
                    .ticks
                    .push([u * m.w, lerp(yn, m.y, 0.45) + f64::abs(u) * 1.5, 0.3, 1.4]);
            }
            return b.bd;
        }
        BeardKind::Moustache => {
            b.tash(true);
            return b.bd;
        }
        BeardKind::Patch => {
            let len = min(lerp(6.0, 11.0, b.long()) * sz, -m.y - 7.0);
            if len > 3.0 {
                let pts = lock_pts(
                    pt(-3.6, m.y + 5.0),
                    pt(3.6, m.y + 5.0),
                    pt(b.tuft_lean(0) * 1.2, m.y + 5.0 + len),
                    0.12,
                );
                b.fill(pts, Edge::Thin);
            }
            b.tash(false);
            return b.bd;
        }
        BeardKind::Goatee => {
            // A beard on the chin alone, narrowing all the way down to a point a little below it.
            let gw = min(min(m.w * 0.55 + 2.0, hd.hw(-6.0 * sz) * 0.78), 15.0 * sz);
            let tip = lerp(1.5, 4.5, b.long()) * sz;
            let yt = m.y + 6.5;
            let side = [pt(gw * 0.72, yt + 2.6), pt(gw, lerp(yt, 0.0, 0.36))];
            let low = [
                pt(gw * 0.66, lerp(yt, 0.0, 0.78)),
                pt(gw * 0.26, tip * 0.45 - 1.0 + b.tuft_len(0) * 0.8),
            ];
            let mut under = low.to_vec();
            under.push(sharp(b.tuft_lean(1) * 1.2, tip));
            under.extend(reversed(mirror(&low)));
            let mut pts = vec![sharp(0.0, yt)];
            pts.extend_from_slice(&side);
            pts.extend_from_slice(&under);
            pts.extend(reversed(mirror(&side)));
            b.fill(pts, Edge::Thin);
            b.bd.mass.rims.push(Rim {
                pts: under,
                thin: false,
            });
            for u in [-0.4, 0.0, 0.4] {
                b.pen(
                    vec![
                        pt(u * gw, lerp(yt, 0.0, 0.4)),
                        pt(u * gw * 0.5, lerp(yt, tip, 0.86)),
                    ],
                    0.85,
                    Tone::Line,
                );
            }
            b.bd.covers = true;
            b.tash(false);
            return b.bd;
        }
        BeardKind::Full | BeardKind::Long | BeardKind::Wizard => {}
    }
    jaw_beard(&mut b, c);
    b.bd.covers = true;
    b.tash(true);
    b.bd
}

/// The point `t` of the way down one side of a beard that hangs below the
/// chin, from the jaw to its end, bowed outwards.
#[derive(Clone, Copy)]
struct Side {
    s: f64,
    from: Pt,
    to: Pt,
    len: f64,
    u: (f64, f64),
    bow: f64,
}

impl Side {
    fn at(&self, t: f64) -> Pt {
        let p = mix(self.from, self.to, t);
        let b = self.bow * self.len * 4.0 * t * (1.0 - t);
        pt(p.x + self.s * self.u.1 * b, p.y - self.s * self.u.0 * b)
    }
}

/// A beard along the jaw. It begins below the level of the mouth, so that the
/// whole cheek lies bare between it and the hair, and its upper edge climbs
/// from there to the corner of the mouth.
fn jaw_beard(b: &mut Builder, c: &BeardInputs) {
    let (g, hd, m, sz) = (b.g, b.hd, b.m, b.sz);
    let kind = b.bd.kind;
    let full = kind == BeardKind::Full;
    let ear_low = hd.angle_at(c.ears.yc + c.ears.eh * 0.42);
    let v = lerp(2.4, 6.4, g.unit(O + 2, 0, 2)) * sz;
    let shape = g.raw(O + 2, 2, 2);
    let cheek = g.unit(O + 2, 4, 2);
    let mut y_top = lerp(m.y * 0.5, m.y + 4.0, cheek);
    if full {
        while y_top < m.y * 0.3 && y_top - m.y < 0.5 * (hd.hw(y_top) - m.w - 3.0) {
            y_top += 1.5;
        }
    }
    let a_top = max(ear_low + 12.0, hd.angle_at(y_top));
    let long = b.long();
    let hang = match kind {
        BeardKind::Long => lerp(14.0, 30.0, long) * sz,
        BeardKind::Wizard => lerp(32.0, 52.0, long) * sz,
        _ => lerp(0.0, 6.0, long) * sz,
    };
    // How far the beard stands out past the jaw at each angle.
    let below = |deg: f64| {
        let a = (deg - 180.0).abs();
        if hang == 0.0 || !full {
            return 0.0;
        }
        hang * match shape {
            0 => hump(a, 0.0, 36.0),
            1 => hump(a, 0.0, 52.0),
            2 => max(hump(a, 12.0, 22.0), 0.55 * hump(a, 0.0, 28.0)),
            _ => 1.0 - step(20.0, 38.0, a),
        }
    };
    let out = |deg: f64| 1.4 + v * step(a_top, a_top + 26.0, min(deg, 360.0 - deg)) + below(deg);
    let mut outer = Vec::new();
    // Where a beard that hangs below the chin ends, for the strands drawn in it.
    let mut ends: Option<(Side, Side, f64, [Pt; 3])> = None;
    if full {
        for i in 0..=40 {
            let deg = lerp(a_top, 360.0 - a_top, f64::from(i) / 40.0);
            outer.push(hd.at(deg, out(deg)));
        }
    } else {
        // It narrows all the way down, nowhere wider than the jaw it grows from,
        // and ends in a point, a rounded tip, a fork or a long thin point.
        let a_c = [32.0, 40.0, 34.0, 25.0][shape as usize];
        let bow = [0.05, 0.14, 0.07, 0.02][shape as usize];
        let tx = -b.tuft_lean(2) * 2.5 * sz;
        let ty = out(180.0) + hang + lerp(5.0, 11.0, b.tuft_len(2)) * sz;
        let fork = if shape == 2 {
            4.5 * sz + hang * 0.07
        } else {
            0.0
        };
        for i in 0..=12 {
            let deg = lerp(a_top, 180.0 - a_c, f64::from(i) / 12.0);
            outer.push(hd.at(deg, out(deg)));
        }
        let side = |s: f64| {
            let from = hd.at(180.0 - s * a_c, out(180.0 - a_c));
            let to = pt(tx + s * fork, ty);
            Side {
                s,
                from,
                to,
                len: hyp(to.x - from.x, to.y - from.y),
                u: unit_of(from, to),
                bow,
            }
        };
        let right = side(1.0);
        let left = side(-1.0);
        let notch = if shape == 2 {
            min(hang * 0.42, 11.0 * sz)
        } else {
            0.0
        };
        outer.extend([right.at(0.3), right.at(0.62), right.at(0.86)]);
        match shape {
            1 => outer.push(pt(tx, ty)),
            2 => outer.extend([
                sharp(tx + fork, ty),
                pt(tx, ty - notch),
                sharp(tx - fork, ty),
            ]),
            _ => outer.push(sharp(tx, ty)),
        }
        outer.extend([left.at(0.86), left.at(0.62), left.at(0.3)]);
        for i in 0..=12 {
            let deg = lerp(180.0 + a_c, 360.0 - a_top, f64::from(i) / 12.0);
            outer.push(hd.at(deg, out(deg)));
        }
        ends = Some((
            right,
            left,
            a_c,
            [pt(tx + fork, ty), pt(tx, ty - notch), pt(tx - fork, ty)],
        ));
    }
    // The inner edge: from where the beard begins on the jaw, in a line that
    // sags a little to the corner of the mouth, and close under the lower lip.
    let e0 = hd.at_depth(a_top, 0.0, 0.012);
    let inside = |x: f64, y: f64| pt(min(x, hd.hw(y) - 2.0), y);
    let lip = MouthLine::new(
        m,
        &MouthPose {
            c: m.curve,
            ..MouthPose::default()
        },
    );
    // A pout has a short line under its lower lip, which the bare skin there has to hold.
    let gap = if m.kind == "pout" { 9.0 } else { 6.2 };
    // Under a wide mouth a short beard leaves more skin bare.
    let bare = if full {
        max(gap, min(min(m.w * 0.4, gap + 7.0 * sz), -m.y * 0.45))
    } else {
        gap
    };
    let inner_of = |s: f64| {
        let corner = inside(m.w + 3.0, lip.y(s) - 0.5);
        [
            inside(lerp(e0.x, corner.x, 0.5), lerp(e0.y, corner.y, 0.5) + 3.0),
            corner,
            pt(m.w * 0.62, lip.y(s * 0.62) + bare * 0.78),
        ]
        .map(|p| pt(s * p.x, p.y))
    };
    let mut rim = vec![sharp(e0.x, e0.y)];
    rim.extend_from_slice(&outer[1..outer.len() - 1]);
    rim.push(sharp(-e0.x, e0.y));
    let mut shape_pts = rim.clone();
    shape_pts.extend(inner_of(-1.0));
    shape_pts.push(pt(0.0, lip.y(0.0) + bare));
    shape_pts.extend(inner_of(1.0).iter().rev());
    b.bd.top = a_top;
    b.fill(shape_pts.clone(), Edge::Thin);
    b.bd.mass.rims.push(Rim {
        pts: rim.clone(),
        thin: false,
    });
    if let Some((right, left, a_c, tips)) = ends {
        // Two short locks on each side lie along the edge and point down it,
        // and the strands run from the jaw to where the beard ends.
        for i in 0..4 {
            let gi = if i < 2 { i } else { i + 1 };
            let s = if i < 2 { 1.0 } else { -1.0 };
            let side = if s > 0.0 { right } else { left };
            let t = (if i % 2 == 1 { 0.58 } else { 0.26 }) + b.tuft_lean(gi) * 0.05;
            let a = side.at(t - 0.1);
            let bb = side.at(t + 0.1);
            let tip = side.at(t + 0.3);
            let n = unit_of(a, bb);
            let reach = (2.2 + 3.4 * b.tuft_len(gi)) * sz;
            let pts = lock_pts(
                a,
                bb,
                pt(tip.x + s * n.1 * reach, tip.y - s * n.0 * reach),
                0.12,
            );
            b.fill(pts, Edge::Thick);
        }
        for u in [-0.7, -0.36, 0.0, 0.36, 0.7] {
            let from = hd.at(180.0 - u * a_c, -4.5);
            let to = tips[if u > 0.0 {
                0
            } else if u < 0.0 {
                2
            } else {
                1
            }];
            b.pen(
                vec![
                    from,
                    mix(from, pt(to.x + u * 6.0 * sz, to.y), 0.45),
                    mix(from, to, 0.8),
                ],
                0.9,
                Tone::Line,
            );
        }
    } else {
        // Tufts along the lower edge.
        for i in 0..4 {
            let deg = 180.0 + (f64::from(i) - 1.5) * 24.0;
            let len = lerp(2.5, 6.0, b.tuft_len(i as usize)) * sz;
            let pts = lock_pts(
                hd.at(deg - 7.0, out(deg - 7.0) - 2.0),
                hd.at(deg + 7.0, out(deg + 7.0) - 2.0),
                hd.at(deg + b.tuft_lean(i as usize) * 5.0, out(deg) + len),
                0.14,
            );
            b.fill(pts, Edge::Thick);
        }
        // The chin shows through the beard: its outline is drawn again on the hair.
        let chin_to = min(34.0, 180.0 - a_top - 8.0);
        let span = 180.0 - a_top - 5.0;
        let floor = lip.y(0.0) + bare + 2.5;
        // The beard's own shape as a closed polyline, for telling what lies in it.
        let body = sample(&shape_pts, true, 4);
        b.pen(
            [-1.0, -0.5, 0.0, 0.5, 1.0]
                .iter()
                .map(|u| hd.at(180.0 + u * chin_to, -1.2))
                .collect(),
            1.1,
            Tone::Line,
        );
        // Strokes the way the hair grows, by turns high in the beard and at its lower edge.
        for (i, f) in [-0.92, -0.66, -0.4, -0.13, 0.16, 0.43, 0.7, 0.93]
            .into_iter()
            .enumerate()
        {
            let deg = 180.0 + f * span + b.tuft_lean(i % 5) * 3.0;
            let high = (i % 2) as u32 == (shape & 1);
            let len = lerp(0.55, 1.0, b.tuft_len((i + 2) % 5));
            let a = hd.at(deg, if high { -10.0 * sz } else { -0.5 });
            let bb = hd.at(
                lerp(deg, 180.0, 0.16),
                if high {
                    (-10.0 + 7.0 * len) * sz
                } else {
                    -0.5 + (out(deg) - 1.0) * len
                },
            );
            // A stroke high in the beard is drawn only where the beard is.
            if high
                && !(inside_poly(&body, a)
                    && inside_poly(&body, mix(a, bb, 0.5))
                    && (a.x.abs() > m.w + 6.0 || a.y > floor))
            {
                continue;
            }
            b.pen(vec![a, mix(a, bb, 0.5), bb], 0.85, Tone::Line);
        }
    }
    // A very dark beard on pale skin, or a near-black one on strong skin, gets
    // a softer edge: short bristles that cross it.
    let p = c.palette;
    let darker = p.skin_l - p.beard_hsl[2];
    if darker >= 50.0 || (p.beard_hsl[2] < 30.0 && darker >= 35.0) {
        let ticks = &mut b.bd.mass.ticks;
        // Round the outside they point away from the beard and a little down.
        along(&rim, 4.4, |q, u| {
            ticks.push([
                q.x - u.1 * 0.6,
                q.y + u.0 * 0.6,
                u.1 * 3.0,
                -u.0 * 3.0 + 1.4,
            ]);
        });
        // On the cheek they stand above the beard's upper edge and run down into it.
        for s in [-1.0, 1.0] {
            let inner = inner_of(s);
            along(&[pt(s * e0.x, e0.y), inner[0], inner[1]], 4.4, |q, _| {
                ticks.push([q.x, q.y - 3.0, s * 0.7, 3.6]);
            });
        }
    }
}

/// Calls `f` with a point and the direction of the line there, every `gap`
/// units along a line through `pts`.
fn along(pts: &[Pt], gap: f64, mut f: impl FnMut(Pt, (f64, f64))) {
    let mut need = gap / 2.0;
    for i in 1..pts.len() {
        let a = pts[i - 1];
        let b = pts[i];
        let l = hyp(b.x - a.x, b.y - a.y);
        if l < 1e-6 {
            continue;
        }
        let u = ((b.x - a.x) / l, (b.y - a.y) / l);
        let mut d = need;
        while d <= l {
            f(pt(a.x + u.0 * d, a.y + u.1 * d), u);
            d += gap;
        }
        need = d - l;
    }
}
