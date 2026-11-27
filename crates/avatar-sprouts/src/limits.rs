//! Holding each shoot to what its place allows, so the body and its face
//! keep their share of the picture, and the checks that build a creature
//! again with plainer pairs when a flower or a bunch sits beside the eyes.

use std::collections::HashMap;

use crate::creature::Creature;
use crate::geometry::{inside, join_box, limb_box, Bx};
use crate::growth::{count, interpret, Grown, Organ, SiteName};
use crate::maths::{clamp, hyp, lerp, max, max3, min, rot};
use crate::mood::{Mood, BLOCKED, HAPPY, NEUTRAL};
use crate::organs::organ_item;

fn sides(pair: bool) -> &'static [f64] {
    if pair {
        &[1.0, -1.0]
    } else {
        &[1.0]
    }
}

/// How far one shoot goes at rest against what its place allows (1 is the
/// limit): past the body's side, its growth above its foot, its stems above
/// it. Nothing for a shoot that draws nothing.
pub(crate) struct Over {
    pub(crate) wide: f64,
    high: f64,
    stem_high: f64,
}

pub(crate) fn over_reach(c: &Creature, index: usize, m: &Mood) -> Option<Over> {
    let b = &c.b;
    let site = &c.s[index];
    let mut bx: Option<Bx> = None;
    let mut stem_top = site.root[1];
    for &side in sides(site.pair) {
        let g = interpret(&c.hb, site, side, m);
        for ax in &g.axes {
            bx = join_box(bx, Some(limb_box(&ax.pts)));
            for p in &ax.pts {
                stem_top = min(stem_top, p[1]);
            }
        }
        for o in &g.organs {
            bx = join_box(bx, organ_item(c, o, m).bx());
        }
    }
    let bx = bx?;
    let room = |y: f64| max(8.0, site.root[1] - y);
    Some(Over {
        // An arm made longer to hold a bunch out may go further out by a
        // quarter of the added length.
        wide: (max(bx[2], -bx[0]) - b.w) / (site.reach + 0.25 * site.lead),
        high: (site.root[1] - bx[1]) / room(site.ceil),
        stem_high: site
            .stem_ceil
            .map_or(0.0, |ceil| (site.root[1] - stem_top) / room(ceil)),
    })
}

/// Holds each shoot inside its reach. An arm that rises too high is first
/// carried lower; a shoot that still grows too far is drawn smaller, and
/// when its stems are as short as they may get, its organs are drawn
/// smaller too. Given `only`, it holds that one shoot.
pub(crate) fn limit_reach(c: &mut Creature, only: Option<usize>) {
    let m = c.mood(NEUTRAL);
    for index in 0..c.s.len() {
        if only.is_some_and(|o| o != index) {
            continue;
        }
        for pass in 0..8 {
            let Some(over) = over_reach(c, index, &m) else {
                break;
            };
            let (wide, high, stem_high) = (over.wide, over.high, over.stem_high);
            if max3(wide, high, stem_high) <= 1.03 {
                break;
            }
            let site = &mut c.s[index];
            if max(high, stem_high) > 1.03
                && max(high, stem_high) >= wide
                && site.name == SiteName::Arm
                && !site.cactus
                && site.dir[1] < 0.3
                && pass < 4
            {
                site.dir = rot(site.dir, 0.22);
                continue;
            }
            let most = max3(wide, high, stem_high);
            let organ_k = site.organ_k;
            if site.scale > 0.45 {
                site.scale = max(0.45, site.scale * max(0.62, 1.0 / most));
            } else if organ_k > 0.62 && max(wide, high) > 1.03 {
                site.organ_k = max(0.62, organ_k * max(0.8, 1.0 / max(wide, high)));
            } else {
                break;
            }
        }
    }
}

/// Holds the leaves of an ear to a size that reads as foliage on a stem.
pub(crate) fn limit_ears(c: &mut Creature) {
    let Some(l) = c.o.leaf else { return };
    let m = c.mood(NEUTRAL);
    let head = 2.0 * c.b.w;
    // A leaf of size 1: how long it is with its own stalk, and how wide.
    let along = l.len * l.form.long * (1.0 + l.stalk);
    let across = 2.0
        * l.len
        * l.form.long
        * if l.form.compound {
            l.form.side
        } else {
            l.form.wide * l.wide_k
        };
    for index in 0..c.s.len() {
        if c.s[index].name != SiteName::Ear {
            continue;
        }
        let mut over = 1.0_f64;
        for side in [1.0, -1.0] {
            for o in interpret(&c.hb, &c.s[index], side, &m).organs {
                over = max3(
                    over,
                    across * o.size / (0.3 * head),
                    along * o.size / (0.4 * head),
                );
            }
        }
        if over > 1.0 {
            c.s[index].organ_k /= over;
        }
    }
}

fn same_point(a: [f64; 2], b: [f64; 3]) -> bool {
    a[0] == b[0] && a[1] == b[1]
}

/// Leaves as round as a ball read as a boxing glove when two or more share
/// a node of an arm, and as a ball in the hand at its last node: such a
/// bunch is drawn smaller until it spans no more than three tenths of the
/// body's width.
pub(crate) fn limit_bunches(c: &mut Creature) {
    if !c.o.leaf.is_some_and(|l| l.ball) {
        return;
    }
    let m = c.mood(NEUTRAL);
    let most = 0.3 * 2.0 * c.b.w;
    for index in 0..c.s.len() {
        if c.s[index].name != SiteName::Arm {
            continue;
        }
        for _ in 0..3 {
            let mut held = true;
            for side in [1.0, -1.0] {
                let g = interpret(&c.hb, &c.s[index], side, &m);
                let stem = &g.axes[0].pts;
                let end = stem[stem.len() - 1];
                // The leaves of one node share the point they grow from.
                let mut bunches: HashMap<usize, (Option<Bx>, usize)> = HashMap::new();
                for o in &g.organs {
                    if o.kind != 'L' {
                        continue;
                    }
                    let entry = bunches.entry(o.node).or_insert((None, 0));
                    entry.0 = join_box(entry.0, organ_item(c, o, &m).bx());
                    entry.1 += 1;
                }
                for (i, o) in g.organs.iter().enumerate() {
                    if o.kind != 'L' {
                        continue;
                    }
                    let (bx, n) = bunches[&o.node];
                    let bx = bx.expect("a leaf has a box");
                    let span = max(bx[2] - bx[0], bx[3] - bx[1]);
                    if span <= most * 1.02 || (n < 2 && !same_point(o.p, end)) {
                        continue;
                    }
                    let site = &mut c.s[index];
                    if site.bunch_k.len() <= i {
                        site.bunch_k.resize(i + 1, None);
                    }
                    site.bunch_k[i] = Some(max(0.3, site.bunch_k[i].unwrap_or(1.0) * most / span));
                    held = false;
                }
            }
            if held {
                break;
            }
        }
    }
}

/// A fruit, a flower shut to a bud, or a rose at the end of an arm is a
/// ball in a hand: none is larger than a third of the body's width in any
/// mood, or the arm's flowers and fruit are all drawn smaller.
pub(crate) fn limit_held(c: &mut Creature) {
    let rose = c.o.flower.is_some_and(|f| f.form == "rose");
    let most = 0.32 * 2.0 * c.b.w;
    for index in 0..c.s.len() {
        let site = &c.s[index];
        if site.name != SiteName::Arm || count(&site.string, "UK") == 0 {
            continue;
        }
        let mut span = 0.0_f64;
        for mood in [NEUTRAL, HAPPY, BLOCKED] {
            let m = c.mood(mood);
            for side in [1.0, -1.0] {
                let site = &c.s[index];
                let mut g = interpret(&c.hb, site, side, &m);
                let stem = &g.axes[0].pts;
                let end = stem[stem.len() - 1];
                let late = site.late_on(side);
                for o in g.organs.iter_mut() {
                    let ball = o.kind == 'K'
                        || (o.kind == 'U'
                            && (rose || m.bloom * if late { 0.3 } else { 1.0 } < 0.2));
                    if !ball || !same_point(o.p, end) {
                        continue;
                    }
                    o.late = late;
                    let bx = organ_item(c, o, &m).bx().expect("an organ has a box");
                    span = max3(span, bx[2] - bx[0], bx[3] - bx[1]);
                }
            }
        }
        if span > most {
            c.s[index].held_k = Some(most / span);
        }
    }
}

/// How much stem shows outside the head (or, with `body_only`, outside the
/// body's outline) between a shoot's foot and the node an organ grows from.
fn stem_outside(c: &Creature, g: &Grown, o: &Organ, body_only: bool) -> f64 {
    let mut len = 0.0;
    let mut ax = o.on;
    let mut at = o.at;
    while let Some(index) = ax {
        let axis = &g.axes[index];
        for i in 1..=at {
            let a = axis.pts[i - 1];
            let b = axis.pts[i];
            let d = hyp(b[0] - a[0], b[1] - a[1]);
            for k in 0..4 {
                let t = (f64::from(k) + 0.5) / 4.0;
                let (x, y) = (lerp(a[0], b[0], t), lerp(a[1], b[1], t));
                let covered = if body_only {
                    inside(&c.b.poly, x, y)
                } else {
                    head_has(c, x, y)
                };
                if !covered {
                    len += d / 4.0;
                }
            }
        }
        at = axis.from_at;
        ax = axis.from;
    }
    len
}

/// A small flower close beside the head at the height of the eyes reads as
/// an earring. True when a flower of the pair falls short of the stem it
/// must be held out on: the creature is then built again with a pair that
/// bears no flowers.
pub(crate) fn blooms_too_close(c: &Creature) -> bool {
    let b = &c.b;
    let f = &c.f;
    let m = c.mood(NEUTRAL);
    for site in &c.s {
        if !site.pair || count(&site.string, "U") == 0 {
            continue;
        }
        for side in [1.0, -1.0] {
            let mut g = interpret(&c.hb, site, side, &m);
            let late = site.late_on(side);
            for i in 0..g.organs.len() {
                if g.organs[i].kind != 'U' {
                    continue;
                }
                // As it is drawn at rest: one of a pair may still be in bud.
                g.organs[i].late = late;
                let o = &g.organs[i];
                let bx = organ_item(c, o, &m).bx().expect("an organ has a box");
                let high = 1.0 + (f.ey - (bx[1] + bx[3]) / 2.0) / f.ry;
                if high > 0.0 && stem_outside(c, &g, o, true) < 0.5 * b.w * min(1.0, high) {
                    return true;
                }
            }
        }
    }
    false
}

/// Whether a point is inside the head: the body's outline, whatever is
/// welded to it, and the lobes of a rosette as they lie at rest.
fn head_has(c: &Creature, x: f64, y: f64) -> bool {
    let b = &c.b;
    if inside(&b.poly, x, y) {
        return true;
    }
    for w in &b.welds {
        if let crate::body::Weld::Disc(d) = w {
            if hyp(x - d[0], y - d[1]) < d[2] {
                return true;
            }
        }
    }
    if let Some(lobes) = &c.lobes {
        for q in &lobes.quads {
            if inside(q, x, y) {
                return true;
            }
        }
    }
    false
}

/// How far a point stands from the head; 0 inside it.
fn head_gap(c: &Creature, x: f64, y: f64) -> f64 {
    if head_has(c, x, y) {
        return 0.0;
    }
    let b = &c.b;
    let mut gap = f64::INFINITY;
    let mut edge = |a: [f64; 2], e: [f64; 2]| {
        let vx = e[0] - a[0];
        let vy = e[1] - a[1];
        let denominator = vx * vx + vy * vy;
        let t = clamp(
            ((x - a[0]) * vx + (y - a[1]) * vy)
                / if denominator == 0.0 { 1.0 } else { denominator },
            0.0,
            1.0,
        );
        gap = min(gap, hyp(x - a[0] - vx * t, y - a[1] - vy * t));
    };
    for i in 0..b.poly.len() {
        edge(b.poly[i], b.poly[(i + 1) % b.poly.len()]);
    }
    for w in &b.welds {
        if let crate::body::Weld::Disc(d) = w {
            let g = hyp(x - d[0], y - d[1]) - d[2];
            gap = min(gap, g);
        }
    }
    if let Some(lobes) = &c.lobes {
        for q in &lobes.quads {
            for i in 0..4 {
                let (a, e) = (q[i], q[(i + 1) % 4]);
                let vx = e[0] - a[0];
                let vy = e[1] - a[1];
                let denominator = vx * vx + vy * vy;
                let t = clamp(
                    ((x - a[0]) * vx + (y - a[1]) * vy)
                        / if denominator == 0.0 { 1.0 } else { denominator },
                    0.0,
                    1.0,
                );
                gap = min(gap, hyp(x - a[0] - vx * t, y - a[1] - vy * t));
            }
        }
    }
    gap
}

/// A bunch one side of a pair of arms holds at rest.
struct Bunch {
    high: f64,
    out: f64,
}

/// The bunches one side of a pair of arms holds at rest: three or more
/// leaves and flowers whose boxes overlap and that together span more than
/// a third of the head's width. `high` is how high a bunch sits against the
/// eyes (0 at their lower edge, 1 at their middle); `out` how far it is held
/// out from the head.
fn bunches_of(c: &Creature, index: usize, side: f64, m: &Mood) -> Vec<Bunch> {
    let f = &c.f;
    let site = &c.s[index];
    let head = 2.0 * c.b.w;
    let late = site.late_on(side);
    let mut g = interpret(&c.hb, site, side, m);
    struct Member {
        organ: usize,
        bx: Bx,
        weight: u32,
        set: usize,
    }
    let mut mem: Vec<Member> = Vec::new();
    for i in 0..g.organs.len() {
        let kind = g.organs[i].kind;
        let spray = c.o.leaf.is_some_and(|l| l.form.spray);
        if kind != 'U' && (kind != 'L' || spray) {
            continue;
        }
        // As it is drawn at rest: one of a pair may still be in bud.
        g.organs[i].late = late;
        let bx = organ_item(c, &g.organs[i], m)
            .bx()
            .expect("an organ has a box");
        let big = kind == 'U'
            && (c.o.flower.is_some_and(|fl| fl.form == "cluster")
                || max(bx[2] - bx[0], bx[3] - bx[1]) > head / 3.0);
        let weight = if big {
            3
        } else if kind == 'L' && c.o.leaf.is_some_and(|l| l.form.compound) {
            2
        } else {
            1
        };
        let set = if c.lone { 0 } else { mem.len() };
        mem.push(Member {
            organ: i,
            bx,
            weight,
            set,
        });
    }
    // Leaves and flowers whose boxes overlap, directly or by way of others,
    // end up in one set.
    for _ in 0..mem.len() {
        for ai in 0..mem.len() {
            for bi in 0..mem.len() {
                let (a, b) = (&mem[ai], &mem[bi]);
                if a.set != b.set
                    && a.bx[0] < b.bx[2]
                    && b.bx[0] < a.bx[2]
                    && a.bx[1] < b.bx[3]
                    && b.bx[1] < a.bx[3]
                {
                    let set = a.set.min(b.set);
                    mem[ai].set = set;
                    mem[bi].set = set;
                }
            }
        }
    }
    let mut bunches = Vec::new();
    for s in 0..mem.len() {
        let mut weight = 0;
        let mut bx: Option<Bx> = None;
        let mut out = f64::INFINITY;
        for x in mem.iter().filter(|x| x.set == s) {
            weight += x.weight;
            bx = join_box(bx, Some(x.bx));
            let o = &g.organs[x.organ];
            out = min(
                out,
                if site.cactus {
                    head_gap(c, o.p[0], o.p[1])
                } else {
                    stem_outside(c, &g, o, false)
                },
            );
        }
        let Some(bx) = bx else { continue };
        if !c.lone && (weight < 3 || max(bx[2] - bx[0], bx[3] - bx[1]) <= head / 3.0) {
            continue;
        }
        bunches.push(Bunch {
            high: 1.0 + (f.ey - lerp(bx[1], bx[3], 0.25)) / f.ry,
            out,
        });
    }
    bunches
}

/// The most any bunch of one pair of arms falls short of being held out,
/// in units: nothing above 0 means every bunch is held out far enough.
fn bunch_short(c: &Creature, index: usize) -> f64 {
    let m = c.mood(NEUTRAL);
    let mut short = f64::NEG_INFINITY;
    for side in [1.0, -1.0] {
        for b in bunches_of(c, index, side, &m) {
            if b.high > 0.0 {
                short = max(short, 0.5 * c.b.w * min(1.0, b.high) - b.out);
            }
        }
    }
    short
}

/// Holds the bunches of the arms out with a longer first stretch of stem,
/// then holds the arm to its reach again. True when an arm was made longer.
pub(crate) fn hold_out(c: &mut Creature) -> bool {
    let m = c.mood(NEUTRAL);
    let most = 0.8 * c.b.w;
    let mut longer = false;
    for index in 0..c.s.len() {
        if c.s[index].name != SiteName::Arm {
            continue;
        }
        for _ in 0..6 {
            let short = bunch_short(c, index);
            if short <= 0.0 || c.s[index].lead >= most {
                break;
            }
            c.s[index].lead = min(most, c.s[index].lead + short + 1.0);
            limit_reach(c, Some(index));
            longer = true;
        }
        // A pair of single leaves has no plainer form to fall back on, so it
        // is held out as far as its reach allows.
        for _ in 0..3 {
            if !(c.lone && c.s[index].lead != 0.0) {
                break;
            }
            let wide = over_reach(c, index, &m).expect("an arm draws").wide;
            if wide <= 1.03 {
                break;
            }
            let site = &mut c.s[index];
            site.lead = max(0.0, site.lead - (wide - 1.0) * site.reach);
        }
    }
    longer
}

/// True when an arm cannot hold its bunch out: the creature is then built
/// again with a pair that bears single leaves.
pub(crate) fn bunches_too_close(c: &Creature) -> bool {
    let m = c.mood(NEUTRAL);
    for index in 0..c.s.len() {
        if c.s[index].name != SiteName::Arm {
            continue;
        }
        if bunch_short(c, index) > 0.5
            || (c.s[index].lead != 0.0
                && over_reach(c, index, &m).expect("an arm draws").wide > 1.1)
        {
            return true;
        }
    }
    false
}
