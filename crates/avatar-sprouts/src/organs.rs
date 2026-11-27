//! Organs: leaves, flowers, fruit, thorns and tendrils, and the lobes of a
//! rosette.

use crate::creature::Creature;
use crate::geometry::{
    box_of, curve, curve_v, disc, group, join_box, path_item, polygon, pt, ptt, shape, Bx, Node,
    PathD, Pt, Style,
};
use crate::growth::Organ;
use crate::maths::{
    clamp, cos, grounded, js_round, lerp, max, min, rot, sign, toward, unit, DOWN, TAU, UP, V,
};
use crate::mood::Mood;
use crate::palette::WHITE;
use crate::reader::{dna, Reader};

/// The outline of a blade: how many points a side has, where it is widest,
/// how round, how blunt its ends are, and its lobes, teeth or heart shape.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BladeForm {
    pub(crate) n: usize,
    pub(crate) peak: f64,
    pub(crate) round: f64,
    pub(crate) tip: f64,
    pub(crate) base: f64,
    pub(crate) heart: f64,
    pub(crate) lobed: bool,
    pub(crate) toothed: bool,
}

/// A leaf form. A compound form is several blades on one stalk; `side` is
/// how far it reaches to either side of its midline, as a share of its
/// length. A spray form stays a spray when several leaves overlap.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LeafForm {
    pub(crate) name: &'static str,
    pub(crate) blade: BladeForm,
    pub(crate) compound: bool,
    pub(crate) side: f64,
    pub(crate) wide: f64,
    pub(crate) long: f64,
    pub(crate) spray: bool,
}

const fn simple(
    name: &'static str,
    blade: BladeForm,
    wide: f64,
    long: f64,
    spray: bool,
) -> LeafForm {
    LeafForm {
        name,
        blade,
        compound: false,
        side: 0.0,
        wide,
        long,
        spray,
    }
}

const fn compound(name: &'static str, side: f64, wide: f64, long: f64, spray: bool) -> LeafForm {
    LeafForm {
        name,
        blade: BladeForm {
            n: 0,
            peak: 0.0,
            round: 0.0,
            tip: 0.0,
            base: 0.0,
            heart: 0.0,
            lobed: false,
            toothed: false,
        },
        compound: true,
        side,
        wide,
        long,
        spray,
    }
}

const fn bf(n: usize, peak: f64, round: f64, tip: f64, base: f64) -> BladeForm {
    BladeForm {
        n,
        peak,
        round,
        tip,
        base,
        heart: 0.0,
        lobed: false,
        toothed: false,
    }
}

pub(crate) const LEAF_FORMS: [LeafForm; 14] = [
    simple("ovate", bf(6, 0.6, 0.75, 0.25, 0.5), 0.42, 1.0, false),
    simple("lance", bf(6, 0.8, 0.3, 0.0, 0.2), 0.24, 1.2, true),
    simple("round", bf(6, 1.0, 1.0, 1.0, 0.6), 0.6, 0.8, false),
    simple(
        "heart",
        BladeForm {
            heart: 0.9,
            ..bf(8, 0.55, 0.9, 0.15, 0.0)
        },
        0.56,
        0.9,
        false,
    ),
    simple(
        "lobed",
        BladeForm {
            lobed: true,
            ..bf(12, 1.0, 0.8, 0.8, 0.3)
        },
        0.46,
        1.0,
        false,
    ),
    simple(
        "toothed",
        BladeForm {
            toothed: true,
            ..bf(10, 0.7, 0.6, 0.0, 0.3)
        },
        0.42,
        1.0,
        false,
    ),
    simple("spoon", bf(6, 1.9, 0.9, 1.0, 0.15), 0.38, 0.9, false),
    simple("strap", bf(8, 0.7, 0.35, 0.1, 0.3), 0.15, 1.6, true),
    compound("trefoil", 0.5, 0.5, 1.0, false),
    compound("palmate", 0.45, 0.5, 1.0, false),
    compound("pinnate", 0.3, 0.5, 1.15, true),
    compound("needles", 0.45, 0.4, 0.9, true),
    compound("fan", 0.6, 0.6, 0.85, false),
    compound("crozier", 0.3, 0.3, 1.2, true),
];
const LEAF_WEIGHTS: [u32; 14] = [5, 3, 3, 3, 3, 3, 2, 3, 2, 2, 3, 1, 1, 2];
/// The flower gene picks one of these slots; aster and rose each hold a
/// second, smaller slot.
const FLOWER_SLOTS: [&str; 9] = [
    "daisy", "star", "tulip", "bell", "cluster", "aster", "rose", "aster", "rose",
];
const FLOWER_WEIGHTS: [u32; 9] = [5, 3, 4, 3, 3, 1, 1, 3, 2];
pub const FRUIT_FORMS: [&str; 4] = ["berry", "pair", "drop", "bean"];
const FRUIT_WEIGHTS: [u32; 4] = [4, 2, 2, 2];

#[derive(Clone, Copy, Debug)]
pub(crate) struct Leaf {
    pub(crate) form: &'static LeafForm,
    pub(crate) len: f64,
    pub(crate) wide_k: f64,
    /// A smooth blunt blade about as wide as it is long: it reads as a ball.
    pub(crate) ball: bool,
    pub(crate) peak_k: f64,
    pub(crate) curl: f64,
    pub(crate) parts: u32,
    pub(crate) depth: f64,
    pub(crate) veins: usize,
    pub(crate) pale: usize,
    pub(crate) stalk: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Flower {
    pub(crate) form: &'static str,
    pub(crate) size: f64,
    pub(crate) petals: u32,
    pub(crate) wide: f64,
    pub(crate) centre: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Fruit {
    pub(crate) form: &'static str,
    pub(crate) size: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Thorn {
    pub(crate) len: f64,
    pub(crate) hook: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tendril {
    pub(crate) coil: f64,
    pub(crate) size: f64,
}

/// What the plant's organs look like; each is read only if the plant grew it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Organs {
    pub(crate) leaf: Option<Leaf>,
    pub(crate) flower: Option<Flower>,
    pub(crate) fruit: Option<Fruit>,
    pub(crate) thorn: Option<Thorn>,
    pub(crate) tendril: Option<Tendril>,
}

/// Which organs the plant bears: leaves, flowers, fruit, thorns, tendrils.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Has {
    pub(crate) l: bool,
    pub(crate) u: bool,
    pub(crate) k: bool,
    pub(crate) t: bool,
    pub(crate) c: bool,
}

pub(crate) fn build_organs(r: &Reader<'_>, has: Has) -> Organs {
    let mut out = Organs::default();
    let o = dna::LEAF;
    if has.l {
        let form = &LEAF_FORMS[r.pick(o, &LEAF_WEIGHTS)];
        let simple = !form.compound;
        let name = form.name;
        let wide_k = r.lin(o + 2, 0.8, 1.25);
        let b = form.blade;
        out.leaf = Some(Leaf {
            form,
            len: r.lin(o + 1, 26.0, 46.0),
            wide_k,
            ball: simple
                && !b.lobed
                && !b.toothed
                && b.tip >= 0.9
                && 2.0 * form.wide * wide_k >= 0.9,
            peak_k: if simple { r.lin(o + 3, 0.8, 1.3) } else { 1.0 },
            curl: if simple || name == "pinnate" || name == "needles" {
                r.lin(o + 4, -0.45, 0.75)
            } else {
                0.0
            },
            parts: if b.lobed || name == "pinnate" || name == "needles" {
                r.one(o + 5, 3)
            } else {
                0
            },
            depth: if b.lobed || b.toothed {
                r.lin(o + 6, 0.22, 0.45)
            } else {
                0.0
            },
            veins: if name == "needles" || name == "crozier" {
                0
            } else {
                r.pick(o + 7, &[2, 4, 2])
            },
            pale: if simple { r.pick(o + 8, &[7, 2]) } else { 0 },
            stalk: if simple { r.lin(o + 9, 0.0, 0.26) } else { 0.0 },
        });
    }
    let o = dna::ORGANS;
    if has.u {
        let form = FLOWER_SLOTS[r.pick(o, &FLOWER_WEIGHTS)];
        let rayed = form == "daisy" || form == "aster";
        out.flower = Some(Flower {
            form,
            size: r.lin(o + 2, 8.5, 14.5),
            petals: if rayed { 5 + r.one(o + 1, 8) } else { 5 },
            wide: if rayed { r.lin(o + 3, 0.3, 0.5) } else { 0.4 },
            centre: if rayed || form == "star" || form == "cluster" {
                r.lin(o + 10, 0.32, 0.5)
            } else {
                0.4
            },
        });
    }
    if has.k {
        out.fruit = Some(Fruit {
            form: FRUIT_FORMS[r.pick(o + 4, &FRUIT_WEIGHTS)],
            size: r.lin(o + 5, 4.5, 7.5),
        });
    }
    if has.t {
        out.thorn = Some(Thorn {
            len: r.lin(o + 6, 6.5, 11.0),
            hook: r.lin(o + 7, -0.25, 0.45),
        });
    }
    if has.c {
        out.tendril = Some(Tendril {
            coil: r.lin(o + 8, 0.8, 1.7),
            size: r.lin(o + 9, 9.0, 16.0),
        });
    }
    out
}

/// One blade: its outline, its midrib and the directions along it, and the
/// two edges.
pub(crate) struct Blade {
    pub(crate) out: Vec<Pt>,
    pub(crate) mid: Vec<V>,
    pub(crate) hs: Vec<V>,
    pub(crate) right: Vec<V>,
    pub(crate) left: Vec<V>,
}

/// A blade joined to its stem at `at`, leaving along `dir`; `bend` is how
/// far its midrib turns from base to tip.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blade(
    at: V,
    dir: V,
    len: f64,
    wid: f64,
    form: &BladeForm,
    bend: f64,
    parts: f64,
    depth: f64,
) -> Blade {
    let n = form.n;
    let nf = n as f64;
    let mut mid = vec![at];
    let mut hs = vec![dir];
    let mut h = dir;
    let mut p = at;
    for _ in 1..=n {
        h = rot(h, bend / nf);
        p = [p[0] + h[0] * len / nf, p[1] + h[1] * len / nf];
        mid.push(p);
        hs.push(h);
    }
    let mut right = Vec::new();
    let mut left = Vec::new();
    let mut right_t = Vec::new();
    let mut left_t = Vec::new();
    for i in 1..n {
        let t = i as f64 / nf;
        let s = t / (t + form.peak * (1.0 - t));
        let lens = 4.0 * s * (1.0 - s);
        let mut w = wid * lerp(lens, lens.sqrt(), form.round);
        if form.lobed {
            w *= 1.0 - depth * (0.5 + 0.5 * cos(TAU * parts * t));
        }
        if form.toothed && i % 2 == 0 {
            w *= 1.0 - depth * 0.6;
        }
        // A heart-shaped blade swings its two lobes back past the stem.
        let hh = hs[i];
        let back = if form.heart != 0.0 {
            form.heart * w * max(0.0, 1.0 - t / 0.4)
        } else {
            0.0
        };
        let cx = mid[i][0] - hh[0] * back;
        let cy = mid[i][1] - hh[1] * back;
        let rp = [cx - hh[1] * w, cy + hh[0] * w];
        let lp = [cx + hh[1] * w, cy - hh[0] * w];
        right.push(rp);
        left.push(lp);
        let tension = form.toothed.then_some(0.15);
        right_t.push(Pt {
            x: rp[0],
            y: rp[1],
            t: tension,
        });
        left_t.push(Pt {
            x: lp[0],
            y: lp[1],
            t: tension,
        });
    }
    let mut out = vec![ptt(at[0], at[1], form.base)];
    out.extend(right_t);
    out.push(ptt(mid[n][0], mid[n][1], form.tip));
    out.extend(left_t.into_iter().rev());
    Blade {
        out,
        mid,
        hs,
        right,
        left,
    }
}

/// A petal or a leaflet as two cubics: path data, far tip and box. `round`
/// runs from 0, pointed at the far end, to 1, blunt there.
fn lens_parts(at: V, dir: V, len: f64, wid: f64, round: f64) -> (PathD, V, Bx) {
    let n = [-dir[1], dir[0]];
    let tip = [at[0] + dir[0] * len, at[1] + dir[1] * len];
    let k1 = 1.33 * wid * (1.0 - 0.3 * round);
    let k2 = 1.33 * wid;
    let fwd = len * 0.3;
    let back = len * 0.32 * (1.0 - round);
    let c = |s: f64| {
        (
            [
                at[0] + dir[0] * fwd + n[0] * k1 * s,
                at[1] + dir[1] * fwd + n[1] * k1 * s,
            ],
            [
                tip[0] - dir[0] * back + n[0] * k2 * s,
                tip[1] - dir[1] * back + n[1] * k2 * s,
            ],
        )
    };
    // The outline runs clockwise, so lobes welded to a body fill with it.
    let a = c(-1.0);
    let b = c(1.0);
    let mx = at[0] + dir[0] * len * 0.6;
    let my = at[1] + dir[1] * len * 0.6;
    let mut d = PathD::default();
    d.push_cubics(at, &[(a.0, a.1, tip), (b.1, b.0, at)], true);
    let bx = box_of([
        at,
        tip,
        [mx + n[0] * wid, my + n[1] * wid],
        [mx - n[0] * wid, my - n[1] * wid],
    ]);
    (d, tip, bx)
}

pub(crate) fn lens(at: V, dir: V, len: f64, wid: f64, round: f64) -> Node {
    let (d, _, bx) = lens_parts(at, dir, len, wid, round);
    path_item(d, bx, Style::default())
}

/// How blunt the far end of a petal is, by petal kind.
const PETAL: f64 = 0.8;
const PETAL_SHARP: f64 = 0.0;
const PETAL_CUP: f64 = 0.35;

/// A ring of petals round a point, as one path.
#[allow(clippy::too_many_arguments)]
fn petal_ring(
    c: V,
    dir: V,
    n: u32,
    inner: f64,
    len: f64,
    wid: f64,
    round: f64,
    phase: f64,
    style: Style,
) -> Node {
    let mut d = PathD::default();
    let mut bx = None;
    for j in 0..n {
        let v = rot(dir, TAU * (f64::from(j) + phase) / f64::from(n));
        let (zd, _, zb) = lens_parts(
            [c[0] + v[0] * inner, c[1] + v[1] * inner],
            v,
            len,
            wid,
            round,
        );
        d.append(zd);
        bx = join_box(bx, Some(zb));
    }
    path_item(d, bx.expect("a ring has petals"), style)
}

fn with_style(mut node: Node, style: Style) -> Node {
    *node.style_mut() = style;
    node
}

fn leaf_items(c: &Creature, o: &Organ, m: &Mood) -> Node {
    let l = c.o.leaf.expect("a leaf organ has a leaf form");
    let p = &c.p;
    let form = l.form;
    let mut items = Vec::new();
    // The leaf wilts towards the ground or lifts to the light, then folds
    // along its stem for the night.
    let mut dir = o.h;
    if m.droop > 0.0 {
        dir = toward(dir, DOWN, m.droop * 0.85, o.sign);
    } else {
        dir = toward(dir, UP, c.hb.light - m.droop * 0.35, o.sign);
    }
    if m.fold > 0.0 {
        dir = toward(
            dir,
            if c.t.fold_up {
                o.stem
            } else {
                [-o.stem[0], -o.stem[1]]
            },
            m.fold * 0.8,
            o.sign,
        );
    }
    let len = l.len
        * form.long
        * o.size
        * if m.droop < 0.0 {
            1.0 - m.droop * 0.08
        } else {
            1.0
        };
    // A folded or wilted blade shows less of its face.
    let shut = 1.0 - 0.55 * m.fold - if m.droop > 0.0 { 0.18 * m.droop } else { 0.0 };
    let wid = len * form.wide * l.wide_k * shut;
    // A leaf that would hang into the ground lies along it instead.
    let floor = m.floor - 3.0 - (if form.compound { form.side * len } else { wid }) * 0.8;
    let far = len * (1.15 + l.stalk);
    let low = dir[1]
        > (floor
            - (if m.droop > 0.0 {
                0.3 * m.droop * len
            } else {
                0.0
            })
            - o.p[1])
            / far;
    if low {
        dir = grounded(o.p, dir, far, floor);
    }
    let mut bend = l.curl
        * o.sign
        * (if m.droop < 0.0 { 0.6 } else { 1.0 })
        * (if m.bristle > 1.5 { 0.3 } else { 1.0 });
    if m.droop > 0.0 {
        bend += (if dir[0].abs() > 0.05 {
            sign(dir[0])
        } else {
            o.sign
        }) * 0.75
            * m.droop;
    }
    if low {
        bend = 0.0;
    }
    let green = || Style::filled(p.leaf, p.leaf_dark, 1.3, "sp-blade").round_join();
    let rib = |pts: &[V]| curve_v(pts, Style::line(p.leaf_dark, 1.1, "sp-vein"));
    let stalk = |pts: &[V]| curve_v(pts, Style::line(p.leaf_dark, 2.2, "sp-leaf-stalk"));
    let along = |k: f64| [o.p[0] + dir[0] * len * k, o.p[1] + dir[1] * len * k];
    // The leaflets of a compound leaf are drawn as one path, their ribs as
    // another.
    let mut lobes = PathD::default();
    let mut ribs = PathD::default();
    let mut lobes_box: Option<Bx> = None;
    let mut leaflet = |at: V, d: V, ll: f64, w: f64, round: f64| {
        let (zd, tip, zb) = lens_parts(at, d, ll, w * l.wide_k * shut, round);
        lobes.append(zd);
        lobes_box = join_box(lobes_box, Some(zb));
        if l.veins != 0 {
            ribs.push_line(at, [lerp(at[0], tip[0], 0.78), lerp(at[1], tip[1], 0.78)]);
        }
    };
    if !form.compound {
        let mut f = form.blade;
        f.peak = form.blade.peak * l.peak_k;
        // Two points to a lobe, one at its crest and one in the notch.
        if f.lobed {
            f.n = 2 * (2 + l.parts as usize);
        }
        // The blade may stand off from the stem on a short stalk.
        let at = if l.stalk > 0.04 { along(l.stalk) } else { o.p };
        if l.stalk > 0.04 {
            items.push(stalk(&[o.p, at]));
        }
        let b = blade(
            at,
            dir,
            len,
            wid,
            &f,
            bend,
            2.0 + f64::from(l.parts),
            l.depth,
        );
        items.push(shape(&b.out, green()));
        if l.pale != 0 {
            items.push(with_style(
                lens(at, dir, len * 0.8, wid * 0.36, 0.2),
                Style::fill(p.leaf_light, "sp-blade-pale"),
            ));
        }
        let nf = f.n as f64;
        let idx = |k: f64| js_round(nf * k) as usize;
        if l.veins != 0 {
            items.push(rib(&[
                b.mid[0],
                b.mid[idx(0.3)],
                b.mid[idx(0.6)],
                b.mid[idx(0.86)],
            ]));
        }
        if l.veins == 2 && l.pale == 0 {
            let mut d = PathD::default();
            let mut ends = Vec::new();
            for k in [0.3, 0.5, 0.7] {
                let i = idx(k);
                let a = b.mid[i - 1];
                for side in [&b.right, &b.left] {
                    let e = [
                        lerp(a[0], side[i - 1][0], 0.72),
                        lerp(a[1], side[i - 1][1], 0.72),
                    ];
                    d.push_line(a, e);
                    ends.push(e);
                }
            }
            items.push(path_item(
                d,
                box_of(ends),
                Style::line(p.leaf_dark, 0.8, "sp-vein").round_cap(),
            ));
        }
    } else if form.name == "trefoil" {
        let cc = along(0.42);
        let open = 1.15 * (1.0 - 0.8 * m.fold);
        let ll = len * 0.56;
        items.push(stalk(&[o.p, cc]));
        for a in [-open, open, 0.0] {
            leaflet(cc, rot(dir, a), ll, ll * 0.5, 1.0);
        }
    } else if form.name == "palmate" {
        let cc = along(0.22);
        let open = 1.0 - 0.6 * m.fold;
        items.push(stalk(&[o.p, cc]));
        for [a, k] in [
            [-1.2, 0.5],
            [1.2, 0.5],
            [-0.62, 0.7],
            [0.62, 0.7],
            [0.0, 0.82],
        ] {
            leaflet(cc, rot(dir, a * open), len * k, len * k * 0.2, 0.15);
        }
    } else if form.name == "pinnate" {
        let pairs = 2 + l.parts;
        let n = 6;
        let spine = blade(
            o.p,
            dir,
            len,
            1.0,
            &bf(n, 1.0, 0.0, 0.0, 0.0),
            bend,
            0.0,
            0.0,
        );
        let open = 1.0 - 0.75 * m.fold;
        let ll = len * 0.36;
        items.push(stalk(&[
            spine.mid[0],
            spine.mid[2],
            spine.mid[4],
            spine.mid[6],
        ]));
        for j in 1..=pairs {
            let i = js_round(n as f64 * f64::from(j) / (f64::from(pairs) + 1.3)) as usize;
            for s in [-1.0, 1.0] {
                leaflet(
                    spine.mid[i],
                    rot(spine.hs[i], s * open),
                    ll * (1.0 - 0.1 * f64::from(j)),
                    ll * 0.36,
                    0.15,
                );
            }
        }
        leaflet(spine.mid[n], spine.hs[n], ll, ll * 0.4, 0.15);
    } else if form.name == "needles" {
        let cnt = 4 + l.parts;
        let spread = 0.55 * (1.0 - 0.7 * m.fold) * min(1.3, m.spread);
        let mut ends = vec![o.p];
        let mut d = PathD::default();
        for j in 0..cnt {
            let v = rot(
                dir,
                lerp(-spread, spread, f64::from(j) / f64::from(cnt - 1)) + bend * 0.3,
            );
            let ll = len * (0.72 + 0.14 * f64::from((j * 7) % 3));
            let e = [o.p[0] + v[0] * ll, o.p[1] + v[1] * ll];
            d.push_line(o.p, e);
            ends.push(e);
        }
        let bx = box_of(ends);
        items.push(path_item(
            d.clone(),
            bx,
            Style::line(p.leaf_dark, 3.6, "sp-needle-edge").round_cap(),
        ));
        items.push(path_item(
            d,
            bx,
            Style::line(p.leaf, 2.0, "sp-needle").round_cap(),
        ));
    } else if form.name == "fan" {
        let open = 0.78 * (1.0 - 0.6 * m.fold);
        let rr0 = len * 0.9;
        let mut pts = vec![ptt(o.p[0], o.p[1], 0.0)];
        let mut ribs2 = Vec::new();
        for j in 0..=8 {
            let v = rot(dir, lerp(-open, open, f64::from(j) / 8.0));
            let rr = rr0
                * if j == 4 {
                    0.74
                } else if j % 2 == 1 {
                    1.0
                } else {
                    0.94
                };
            pts.push(ptt(
                o.p[0] + v[0] * rr,
                o.p[1] + v[1] * rr,
                if j == 0 || j == 8 {
                    0.3
                } else if j == 4 {
                    0.2
                } else {
                    1.0
                },
            ));
            if j == 2 || j == 6 {
                ribs2.push([o.p[0] + v[0] * rr * 0.8, o.p[1] + v[1] * rr * 0.8]);
            }
        }
        items.push(shape(&pts, green()));
        if l.veins != 0 {
            for e in ribs2 {
                items.push(rib(&[o.p, e]));
            }
        }
    } else {
        // A crozier, the coiled young frond of a fern: it unrolls when the
        // creature perks up and coils tight for the night.
        let n = 16;
        let coil = clamp(
            1.0 + 0.5 * m.fold
                + (if m.droop < 0.0 {
                    m.droop * 0.9
                } else {
                    -0.25 * m.droop
                })
                + (if m.bristle > 1.5 { 0.25 } else { 0.0 }),
            0.4,
            1.6,
        );
        let mut pts = vec![o.p];
        // Close to the ground it coils upwards.
        let turn = if o.p[1] > m.floor - len {
            if dir[0] > 0.0 {
                -1.0
            } else {
                1.0
            }
        } else {
            o.sign
        };
        let mut h = dir;
        let mut q = o.p;
        for i in 1..=n {
            let t = f64::from(i) / f64::from(n);
            h = rot(h, turn * coil * 0.94 * t * t);
            if m.droop > 0.0 && turn == o.sign {
                h = toward(h, DOWN, 0.1 * m.droop, o.sign);
            }
            q = [
                q[0] + h[0] * len * 1.25 / f64::from(n) * (1.0 - 0.6 * t),
                q[1] + h[1] * len * 1.25 / f64::from(n) * (1.0 - 0.6 * t),
            ];
            pts.push(q);
        }
        let parts = [(&pts[0..10], 6.0 * o.size), (&pts[8..], 3.4 * o.size)];
        for (run, w) in parts {
            let mut style = Style::line(p.leaf_dark, 2.6, "sp-blade-edge");
            style.wm = w;
            items.push(curve_v(run, style));
        }
        for (run, w) in parts {
            let mut style = Style::line(p.leaf, 0.0, "sp-blade");
            style.wm = w;
            items.push(curve_v(run, style));
        }
    }
    if !lobes.is_empty() {
        let bx = lobes_box.expect("leaflets have a box");
        items.push(path_item(lobes, bx, green()));
        if !ribs.is_empty() {
            items.push(path_item(
                ribs,
                bx,
                Style::line(p.leaf_dark, 1.1, "sp-vein").round_cap(),
            ));
        }
    }
    group(items)
}

fn flower_items(c: &Creature, o: &Organ, m: &Mood) -> Node {
    let f = c.o.flower.expect("a flower organ has a flower form");
    let p = &c.p;
    let s = f.size * o.size;
    let mut items = Vec::new();
    // A flower close to the ground is lifted clear of it.
    let op = if o.p[1] > m.floor - s * 1.2 {
        [o.p[0], m.floor - s * 1.2]
    } else {
        o.p
    };
    // How far open: a late flower is still a bud when its twin blooms.
    let b = clamp(m.bloom * if o.late { 0.3 } else { 1.0 }, 0.0, 1.1);
    let mut dir = o.h;
    if m.droop > 0.0 {
        dir = toward(dir, DOWN, m.droop * 0.8, o.sign);
    }
    dir = grounded(op, dir, s * 2.6, m.floor - 2.0);
    let petal = || Style::filled(p.petal, p.petal_dark, 1.1, "sp-petal").round_join();
    let along = |k: f64| [op[0] + dir[0] * s * k, op[1] + dir[1] * s * k];
    if b < 0.2 {
        let l = s * 1.25;
        items.push(with_style(lens(op, dir, l, l * 0.3, PETAL_CUP), petal()));
        for a in [-0.5, 0.5] {
            items.push(with_style(
                lens(op, rot(dir, a * 0.7), l * 0.62, l * 0.2, 0.15),
                Style::filled(p.leaf, p.leaf_dark, 1.0, "sp-sepal").round_join(),
            ));
        }
        return group(items);
    }
    if f.form == "daisy" || f.form == "aster" || f.form == "star" {
        let n = if f.form == "star" {
            5
        } else if f.form == "aster" {
            f.petals + 7
        } else {
            f.petals
        };
        let cc = along(0.35);
        let len = s * (0.5 + 0.62 * b);
        let wid = if f.form == "star" {
            len * 0.42
        } else {
            min(len * f.wide, (s * 0.3 + len * 0.5) * 3.1 / f64::from(n))
        };
        items.push(petal_ring(
            cc,
            dir,
            n,
            s * 0.22,
            len,
            wid,
            if f.form == "star" { PETAL_SHARP } else { PETAL },
            0.5,
            petal(),
        ));
        items.push(disc(
            cc[0],
            cc[1],
            s * f.centre,
            Style::filled(p.centre, p.petal_dark, 1.0, "sp-flower-centre"),
        ));
    } else if f.form == "tulip" {
        let open = 0.1 + 0.5 * b;
        let l = s * 1.7;
        for a in [-open, open] {
            items.push(with_style(
                lens(op, rot(dir, a), l, l * 0.3, PETAL_CUP),
                petal(),
            ));
        }
        let mut light = petal();
        light.fill = Some(p.petal_light);
        items.push(with_style(
            lens(op, dir, l * 1.04, l * 0.3, PETAL_CUP),
            light,
        ));
    } else if f.form == "bell" {
        // A bell hangs whichever way its stalk points, and flares as it opens.
        let g = grounded(
            op,
            unit(dir[0] * 0.3, dir[1] * 0.3 + 0.9),
            s * 1.7,
            m.floor - 2.0,
        );
        let n = [-g[1], g[0]];
        let l = s * 1.5;
        let w = s * 0.62;
        let wide = w + s * 0.42 * b;
        let at = |u: f64, v: f64| pt(op[0] + g[0] * u + n[0] * v, op[1] + g[1] * u + n[1] * v);
        let att = |u: f64, v: f64, t: f64| Pt {
            t: Some(t),
            ..at(u, v)
        };
        items.push(shape(
            &[
                att(0.0, 0.0, 0.6),
                at(l * 0.3, w * 0.8),
                at(l * 0.65, w * 0.95),
                att(l * 1.02, wide, 0.1),
                at(l * 0.9, wide * 0.45),
                att(l * 1.08, 0.0, 0.2),
                at(l * 0.9, -wide * 0.45),
                att(l * 1.02, -wide, 0.1),
                at(l * 0.65, -w * 0.95),
                at(l * 0.3, -w * 0.8),
            ],
            petal(),
        ));
    } else if f.form == "cluster" {
        for [a, k, far] in [[-2.0, 0.55, 0.62], [2.0, 0.55, 0.62], [0.0, 0.62, 0.8]] {
            let v = rot(dir, a);
            let cc = [op[0] + v[0] * s * far, op[1] + v[1] * s * far];
            let rr = s * k;
            let len = rr * (0.45 + 0.5 * b);
            items.push(petal_ring(
                cc,
                dir,
                5,
                rr * 0.2,
                len,
                len * 0.46,
                PETAL,
                0.5,
                petal(),
            ));
            items.push(disc(
                cc[0],
                cc[1],
                rr * f.centre * 0.85,
                Style::filled(p.centre, p.petal_dark, 0.9, "sp-flower-centre"),
            ));
        }
    } else {
        let cc = along(0.4);
        let rr = s * (0.5 + 0.35 * b);
        items.push(disc(cc[0], cc[1], rr, petal()));
        items.push(curve(
            &[
                pt(cc[0] - rr * 0.55, cc[1] + rr * 0.1),
                pt(cc[0] - rr * 0.1, cc[1] - rr * 0.5),
                pt(cc[0] + rr * 0.5, cc[1] - rr * 0.15),
                pt(cc[0] + rr * 0.2, cc[1] + rr * 0.4),
                pt(cc[0] - rr * 0.15, cc[1] + rr * 0.15),
            ],
            Style::line(p.petal_dark, 1.1, "sp-petal-line"),
        ));
    }
    group(items)
}

fn fruit_items(c: &Creature, o: &Organ, m: &Mood) -> Node {
    let f = c.o.fruit.expect("a fruit organ has a fruit form");
    let p = &c.p;
    let r = f.size * o.size;
    let mut items = Vec::new();
    let op = if o.p[1] > m.floor - r * 2.4 {
        [o.p[0], m.floor - r * 2.4]
    } else {
        o.p
    };
    let g = grounded(
        op,
        unit(o.h[0] * 0.45, o.h[1] * 0.45 + 0.8),
        r * 3.4,
        m.floor - 2.0,
    );
    let skin = || Style::filled(p.fruit, p.fruit_dark, 1.1, "sp-fruit-skin").round_join();
    let mut berry = |dir: V, rr: f64| {
        let cc = [op[0] + dir[0] * (rr + 3.0), op[1] + dir[1] * (rr + 3.0)];
        items.push(curve_v(
            &[op, [cc[0] - dir[0] * rr, cc[1] - dir[1] * rr]],
            Style::line(p.outline, 1.4, "sp-fruit-stalk"),
        ));
        items.push(disc(cc[0], cc[1], rr, skin()));
        items.push(disc(
            cc[0] - rr * 0.32,
            cc[1] - rr * 0.34,
            rr * 0.24,
            Style::fill(WHITE, "sp-shine").op(0.55),
        ));
    };
    match f.form {
        "berry" => berry(g, r),
        "pair" => {
            berry(rot(g, -0.5), r * 0.9);
            berry(rot(g, 0.5), r * 0.9);
        }
        "drop" => {
            let b = blade(op, g, r * 2.5, r, &bf(6, 1.7, 1.0, 1.0, 0.4), 0.0, 0.0, 0.0);
            items.push(shape(&b.out, skin()));
        }
        _ => {
            let b = blade(
                op,
                g,
                r * 3.2,
                r * 0.62,
                &bf(6, 1.0, 0.9, 0.5, 0.4),
                0.5 * o.sign,
                0.0,
                0.0,
            );
            items.push(shape(&b.out, skin()));
        }
    }
    group(items)
}

fn thorn_item(c: &Creature, o: &Organ, m: &Mood) -> Node {
    let t = c.o.thorn.expect("a thorn organ has a thorn form");
    let l = t.len * o.size * m.bristle;
    let n = [-o.h[1], o.h[0]];
    let w = t.len * o.size * 0.3;
    let tip = [
        o.p[0] + o.h[0] * l + n[0] * t.hook * l * o.sign,
        o.p[1] + o.h[1] * l + n[1] * t.hook * l * o.sign,
    ];
    polygon(
        &[
            [
                o.p[0] - n[0] * w - o.h[0] * 2.0,
                o.p[1] - n[1] * w - o.h[1] * 2.0,
            ],
            tip,
            [
                o.p[0] + n[0] * w - o.h[0] * 2.0,
                o.p[1] + n[1] * w - o.h[1] * 2.0,
            ],
        ],
        Style::filled(c.p.thorn, c.p.outline, 1.1, "sp-thorn").round_join(),
    )
}

fn tendril_item(c: &Creature, o: &Organ, m: &Mood) -> Node {
    let t = c.o.tendril.expect("a tendril organ has a tendril form");
    let n = 14;
    let mut pts = vec![o.p];
    // A tendril coils tighter for the night and hangs slack when the
    // creature wilts. Close to the ground it coils upwards.
    let coil = t.coil
        * clamp(
            1.0 + 0.5 * m.fold - 0.45 * max(0.0, m.droop) + if m.droop < 0.0 { 0.15 } else { 0.0 },
            0.35,
            1.7,
        );
    let low = o.p[1] > m.floor - t.size * o.size * 1.2;
    let turn = if low {
        if o.h[0] > 0.0 {
            -1.0
        } else {
            1.0
        }
    } else {
        o.sign
    };
    let mut h = if low {
        grounded(o.p, o.h, t.size * o.size, m.floor - 3.0)
    } else {
        o.h
    };
    let mut q = o.p;
    for i in 1..=n {
        let tt = f64::from(i) / f64::from(n);
        let step = t.size * o.size * 0.22 * (1.0 - 0.55 * tt);
        h = rot(h, turn * coil * (0.1 + 0.62 * tt));
        if m.droop > 0.0 && !low {
            h = toward(h, DOWN, 0.12 * m.droop, o.sign);
        }
        q = [q[0] + h[0] * step, q[1] + h[1] * step];
        pts.push(q);
    }
    curve_v(&pts, Style::line(c.p.leaf_dark, 2.4, "sp-tendril"))
}

/// The items one organ draws, as a group or a single item.
pub(crate) fn organ_item(c: &Creature, o: &Organ, m: &Mood) -> Node {
    match o.kind {
        'L' => leaf_items(c, o, m),
        'U' => flower_items(c, o, m),
        'K' => fruit_items(c, o, m),
        'T' => thorn_item(c, o, m),
        _ => tendril_item(c, o, m),
    }
}

/// The lobes of a rosette: how many, how long against the body's
/// half-width, how wide, and how blunt.
#[derive(Clone, Debug)]
pub(crate) struct Lobes {
    pub(crate) n: u32,
    pub(crate) len: f64,
    pub(crate) wide: f64,
    pub(crate) round: f64,
    /// Each lobe at rest as four points, for telling whether a point is in
    /// the head.
    pub(crate) quads: Vec<[V; 4]>,
}

pub(crate) fn build_lobes(r: &Reader<'_>) -> Lobes {
    let o = dna::EXTRA;
    Lobes {
        n: 7 + r.one(o, 6),
        len: r.lin(o + 1, 0.42, 0.7),
        wide: r.lin(o + 2, 0.34, 0.5),
        round: [0.85, 0.2, 0.5][r.pick(o + 4, &[3, 2, 2])],
        quads: Vec::new(),
    }
}

/// The lobes at rest as four points each: where a lobe joins the body, its
/// tip, and its two sides where it is widest.
pub(crate) fn lobe_quads(x: &Lobes, body_w: f64, body_top: f64, body_h: f64) -> Vec<[V; 4]> {
    let len = body_w * x.len;
    let wid = min(len * x.wide, 3.4 * (body_w + len * 0.4) / f64::from(x.n));
    (0..x.n)
        .map(|j| {
            let v = rot(UP, TAU * (f64::from(j) + 0.5) / f64::from(x.n));
            let at = [
                v[0] * body_w * 0.82,
                body_top + body_h / 2.0 + v[1] * body_h * 0.41,
            ];
            let mid = [at[0] + v[0] * len * 0.6, at[1] + v[1] * len * 0.6];
            [
                at,
                [mid[0] - v[1] * wid, mid[1] + v[0] * wid],
                [at[0] + v[0] * len, at[1] + v[1] * len],
                [mid[0] + v[1] * wid, mid[1] - v[0] * wid],
            ]
        })
        .collect()
}

/// The lobes in one mood: shapes welded to the body, and the crease down the
/// middle of each.
pub(crate) fn lobe_shapes(c: &Creature, m: &Mood) -> (Vec<(PathD, Bx)>, Node) {
    let b = &c.b;
    let x = c.lobes.as_ref().expect("only a rosette has lobes");
    let cc = [0.0, b.top + b.h / 2.0];
    let spiky = m.bristle > 1.5;
    let open = if m.fold > 0.5 {
        0.6
    } else if m.droop > 0.5 {
        0.88
    } else if m.droop < -0.3 {
        1.12
    } else {
        1.0
    };
    let len = b.w * x.len * open * if spiky { 1.08 } else { 1.0 };
    let wid = min(len * x.wide, 3.4 * (b.w + len * 0.4) / f64::from(x.n))
        * if spiky {
            0.6
        } else if m.fold > 0.5 {
            0.85
        } else {
            1.0
        };
    let mut shapes = Vec::new();
    let mut creases = PathD::default();
    let mut bx = None;
    for j in 0..x.n {
        // Half a step round from the top, so no lobe stands on the crown.
        let v0 = rot(UP, TAU * (f64::from(j) + 0.5) / f64::from(x.n));
        let mut v = v0;
        if m.droop > 0.0 {
            v = toward(
                v,
                DOWN,
                m.droop * 0.45,
                if v0[0] >= 0.0 { 1.0 } else { -1.0 },
            );
        }
        let at = [cc[0] + v0[0] * b.w * 0.82, cc[1] + v0[1] * b.h * 0.41];
        let (zd, _, zb) = lens_parts(at, v, len, wid, if spiky { 0.0 } else { x.round });
        shapes.push((zd, zb));
        creases.push_line(
            [at[0] + v[0] * len * 0.3, at[1] + v[1] * len * 0.3],
            [at[0] + v[0] * len * 0.74, at[1] + v[1] * len * 0.74],
        );
        bx = join_box(bx, Some(zb));
    }
    let crease = path_item(
        creases,
        bx.expect("a rosette has lobes"),
        Style::line(c.p.tone_down, 1.4, "sp-lobe-crease").round_cap(),
    );
    (shapes, crease)
}
