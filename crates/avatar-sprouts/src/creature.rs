//! A whole creature: built from its genome, posed in each mood and fitted
//! into the 240-unit frame.

use avatar_svg::Rgb;

use crate::body::{build_body, Body, Weld};
use crate::face::{build_face, face_items, Face};
use crate::geometry::{
    circle_d, inside, join_box, limb_path, pad_box, smooth_axis, Bx, Node, PathD,
};
use crate::growth::{build_habit, build_rules, build_sites, interpret, Habit, Site};
use crate::limits::{
    blooms_too_close, bunches_too_close, hold_out, limit_bunches, limit_ears, limit_held,
    limit_reach,
};
use crate::maths::{clamp, max, min, r3, rot, DEG, V};
use crate::mood::{build_temper, mood_of, Mood, Temper, MOODS, NEUTRAL};
use crate::organs::{
    build_lobes, build_organs, lobe_quads, lobe_shapes, organ_item, Has, Lobes, Organs,
};
use crate::palette::{build_accents, build_palette, Palette};
use crate::reader::{dna, Reader, GENOME_LEN};
use crate::roots::{build_roots, leg_shapes, Roots};
use crate::skin::{build_coat, build_skin, coat_items, Coat, Skin};

/// A shape drawn as part of a mass that gets one outline: the body, the
/// stems, the legs.
#[derive(Clone, Debug)]
pub(crate) struct Mass {
    pub(crate) d: PathD,
    pub(crate) bx: Bx,
    pub(crate) cls: String,
    pub(crate) fill: Option<Rgb>,
}

/// Everything a creature is drawn from in one mood, in layers from back to
/// front. Items in every layer but the legs are in the body's own
/// coordinates and are moved by the mood's transform when drawn.
pub(crate) struct Pose {
    pub(crate) mood: Mood,
    pub(crate) leaves: Vec<Node>,
    pub(crate) stems: Vec<Mass>,
    pub(crate) blooms: Vec<Node>,
    pub(crate) body: Vec<Mass>,
    pub(crate) front: Vec<Node>,
    pub(crate) legs: Vec<Mass>,
    pub(crate) back_legs: Vec<Mass>,
    pub(crate) face: Vec<Node>,
    /// How far the body moves sideways and up, in the body's units.
    pub(crate) shift: f64,
    pub(crate) lift: f64,
    pub(crate) feet: Bx,
    pub(crate) bx: Bx,
}

/// One scale and one position for all seven moods.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fit {
    pub(crate) s: f64,
    pub(crate) x: f64,
    pub(crate) y: f64,
}

/// A built creature.
pub(crate) struct Creature {
    pub(crate) lone: bool,
    pub(crate) p: Palette,
    pub(crate) b: Body,
    pub(crate) hb: Habit,
    pub(crate) t: Temper,
    pub(crate) f: Face,
    pub(crate) lobes: Option<Lobes>,
    pub(crate) s: Vec<Site>,
    pub(crate) has: Has,
    pub(crate) o: Organs,
    pub(crate) rt: Roots,
    pub(crate) k: Coat,
    pub(crate) skin: Skin,
    pub(crate) poses: Vec<Pose>,
    pub(crate) fit: Fit,
}

impl Creature {
    pub(crate) fn mood(&self, index: usize) -> Mood {
        mood_of(
            index,
            &self.t,
            self.b.bottom,
            self.b.w,
            self.b.h,
            self.rt.lift,
        )
    }
}

/// Why a build stopped: the creature is built again with plainer pairs.
enum Again {
    /// With a pair that bears no flowers.
    Shy,
    /// With arms that bear single leaves.
    Lone,
}

fn build(bytes: &[u8; GENOME_LEN], shy: bool, lone: bool) -> Result<Creature, Again> {
    let r = Reader::new(bytes);
    let mut p = build_palette(&r);
    let b = build_body(&r);
    let g = build_rules(&r);
    let hb = build_habit(&r);
    let t = build_temper(&r);
    let f = build_face(&r, &b);
    let lobes = (b.kind == crate::body::Kind::Rosette).then(|| {
        let mut lobes = build_lobes(&r);
        lobes.quads = lobe_quads(&lobes, b.w, b.top, b.h);
        lobes
    });
    let s = build_sites(&r, &b, &f, &g, shy, lone);
    // Which organs the plant bears decides which genes for their forms and
    // colours are read at all.
    let grown: String = s.iter().map(|site| site.string.as_str()).collect();
    let has = Has {
        l: grown.contains('L'),
        u: grown.contains('U'),
        k: grown.contains('K'),
        t: grown.contains('T'),
        c: grown.contains('C'),
    };
    let o = build_organs(&r, has);
    // A rosette stands high enough for the lobes under it to clear the
    // ground.
    let least = lobes.as_ref().map_or(0.0, |x| b.w * x.len * 1.15 + 5.0);
    let rt = build_roots(&r, &b, least);
    let k = build_coat(&r, &b);
    build_accents(&r, &mut p, has.u, has.k, has.t || k.kind == "thorns");
    let skin = build_skin(&r, &b, &p);
    let mut c = Creature {
        lone,
        p,
        b,
        hb,
        t,
        f,
        lobes,
        s,
        has,
        o,
        rt,
        k,
        skin,
        poses: Vec::new(),
        fit: Fit {
            s: 1.0,
            x: 120.0,
            y: 222.0,
        },
    };
    limit_ears(&mut c);
    limit_reach(&mut c, None);
    // Single leaves are held out before their flower is judged.
    if lone {
        hold_out(&mut c);
    }
    limit_bunches(&mut c);
    limit_held(&mut c);
    if !shy && blooms_too_close(&c) {
        return Err(Again::Shy);
    }
    // A bunch is held out only after the pair's flowers are judged as they
    // grew. An arm made longer has what it holds measured again.
    if !lone && hold_out(&mut c) {
        limit_bunches(&mut c);
        limit_held(&mut c);
        if !shy && blooms_too_close(&c) {
            return Err(Again::Shy);
        }
        if bunches_too_close(&c) {
            return Err(Again::Lone);
        }
    }
    let mut all: Option<Bx> = None;
    for index in 0..MOODS.len() {
        let pose = pose_of(&c, index);
        all = join_box(all, Some(pose.bx));
        c.poses.push(pose);
    }
    // The fit: one scale and one position for all seven moods, standing on
    // a ground line near the bottom of the frame. The resting pose keeps a
    // margin of 9 units; a mood may come to within 4 of the edge.
    let b = all.expect("a creature has poses");
    let nb = c.poses[NEUTRAL].bx;
    let mut s = min(
        min(
            min(222.0 / (nb[2] - nb[0]), 212.0 / -nb[1]),
            min(232.0 / (b[2] - b[0]), 216.0 / -b[1]),
        ),
        1.5,
    );
    let dx = clamp(-(b[0] + b[2]) / 2.0 * s, -14.0, 14.0);
    s = min(
        min(
            min(s, (116.0 - dx) / max(1.0, b[2])),
            min(
                (116.0 + dx) / max(1.0, -b[0]),
                (111.0 - dx) / max(1.0, nb[2]),
            ),
        ),
        (111.0 + dx) / max(1.0, -nb[0]),
    );
    // Some creatures are smaller than others, but none so small that its
    // face stops reading on an icon.
    s = max(
        s * (0.84 + 0.16 * r.unit(dna::BODY + 1).sqrt()),
        min(s, 9.5 / c.f.r),
    );
    c.fit = Fit {
        s: r3(s),
        x: r3(120.0 + dx),
        y: 222.0,
    };
    Ok(c)
}

/// Builds a creature, building it again with plainer pairs when a flower or
/// a bunch sat too close beside the head.
pub(crate) fn creature(bytes: &[u8; GENOME_LEN]) -> Creature {
    let mut shy = false;
    let mut lone = false;
    loop {
        match build(bytes, shy, lone) {
            Ok(c) => return c,
            Err(Again::Shy) => shy = true,
            // The single leaves are a new pair: the flower at its tip is
            // judged afresh.
            Err(Again::Lone) => {
                lone = true;
                shy = false;
            }
        }
    }
}

fn pose_of(c: &Creature, index: usize) -> Pose {
    let m = c.mood(index);
    let b = &c.b;
    let p = &c.p;
    let lift = r3(c.rt.lift * m.lift);
    let shift = r3(m.shift);
    let tilt = m.tilt * DEG;
    // Where a point of the body is in this mood: squashed about its base,
    // tipped about it, carried on its legs.
    let tf = |q: V| {
        let q = rot([q[0] * m.sx, (q[1] - b.bottom) * m.sy], tilt);
        [q[0] + shift, q[1] - lift]
    };
    let mut leaves = Vec::new();
    let mut stems = Vec::new();
    let mut blooms = Vec::new();
    let mut body = Vec::new();
    let mut front = Vec::new();
    let mut legs = Vec::new();
    let mut back_legs = Vec::new();
    for site in &c.s {
        let sides: &[f64] = if site.pair { &[1.0, -1.0] } else { &[1.0] };
        for &side in sides {
            let mut g = interpret(&c.hb, site, side, &m);
            let cls = format!(
                "sp-stem sp-shoot-{}{}",
                site.name.name(),
                if site.pair {
                    if side > 0.0 {
                        "-r"
                    } else {
                        "-l"
                    }
                } else {
                    ""
                }
            );
            let late = site.pair && site.late_on(side);
            // Leaves, thorns and tendrils go behind the stems. Flowers and
            // fruit go in front of them, and in front of the body too when
            // they grow on something welded to it, unless that would put
            // them over the body.
            let mut behind = Vec::new();
            let mut after = Vec::new();
            let mut ahead = Vec::new();
            for o in g.organs.iter_mut() {
                o.late = late;
                let it = organ_item(c, o, &m);
                let ib = it.bx().expect("an organ has a box");
                if o.kind != 'U' && o.kind != 'K' {
                    behind.push(it);
                } else if site.weld
                    && !inside(&b.poly, (ib[0] + ib[2]) / 2.0, (ib[1] + ib[3]) / 2.0)
                {
                    ahead.push(it);
                } else {
                    after.push(it);
                }
            }
            if !behind.is_empty() {
                leaves.push(crate::geometry::group(behind));
            }
            if !after.is_empty() {
                blooms.push(crate::geometry::group(after));
            }
            if !ahead.is_empty() {
                front.push(crate::geometry::group(ahead));
            }
            for ax in &g.axes {
                let (d, bx) = limb_path(&ax.pts);
                let mass = Mass {
                    d,
                    bx,
                    cls: cls.clone(),
                    fill: site.stalk.then_some(p.leaf_dark),
                };
                if site.weld && ax.from.is_none() {
                    body.push(mass);
                } else {
                    stems.push(mass);
                }
            }
        }
    }
    body.push(Mass {
        d: b.d.clone(),
        bx: b.bx,
        cls: "sp-body".to_owned(),
        fill: None,
    });
    for w in &b.welds {
        match w {
            Weld::Disc(d) => body.push(Mass {
                d: circle_d(d[0], d[1], d[2]),
                bx: [d[0] - d[2], d[1] - d[2], d[0] + d[2], d[1] + d[2]],
                cls: "sp-body-lump".to_owned(),
                fill: None,
            }),
            Weld::Axis(axis) => {
                let (d, bx) = limb_path(&smooth_axis(axis, 4));
                body.push(Mass {
                    d,
                    bx,
                    cls: "sp-body-curl".to_owned(),
                    fill: None,
                });
            }
        }
    }
    if c.lobes.is_some() {
        let (shapes, crease) = lobe_shapes(c, &m);
        for (d, bx) in shapes {
            body.push(Mass {
                d,
                bx,
                cls: "sp-lobe".to_owned(),
                fill: None,
            });
        }
        front.push(crease);
    }
    leaves.extend(coat_items(&c.k, p, &m));
    for (i, leg) in c.rt.legs.iter().enumerate() {
        let sides: &[f64] = if leg.pair { &[1.0, -1.0] } else { &[1.0] };
        for &side in sides {
            let cls = format!(
                "sp-root sp-root-{}{}",
                i + 1,
                if leg.pair {
                    if side > 0.0 {
                        "-r"
                    } else {
                        "-l"
                    }
                } else {
                    ""
                }
            );
            for (d, bx) in leg_shapes(leg, side, &tf) {
                let mass = Mass {
                    d,
                    bx,
                    cls: cls.clone(),
                    fill: None,
                };
                if leg.back {
                    back_legs.push(mass);
                } else {
                    legs.push(mass);
                }
            }
        }
    }
    let face = face_items(c, &m);

    let mut bx: Option<Bx> = None;
    let mut grow = |b: Option<Bx>| {
        let Some(b) = b else { return };
        for corner in [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]] {
            let q = tf(corner);
            bx = join_box(bx, Some([q[0], q[1], q[0], q[1]]));
        }
    };
    for it in &leaves {
        grow(it.bx());
    }
    for it in &stems {
        grow(Some(it.bx));
    }
    for it in &blooms {
        grow(it.bx());
    }
    for it in &body {
        grow(Some(it.bx));
    }
    for it in &front {
        grow(it.bx());
    }
    for it in &face {
        grow(it.bx());
    }
    let mut feet: Option<Bx> = None;
    for it in legs.iter().chain(back_legs.iter()) {
        feet = join_box(feet, Some(it.bx));
    }
    let all = pad_box(join_box(bx, feet).expect("a pose covers a box"), 2.0);
    Pose {
        mood: m,
        leaves,
        stems,
        blooms,
        body,
        front,
        legs,
        back_legs,
        face,
        shift,
        lift,
        feet: feet.unwrap_or([-20.0, 0.0, 20.0, 0.0]),
        bx: all,
    }
}
