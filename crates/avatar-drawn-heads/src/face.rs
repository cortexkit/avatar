//! Eyes, lids, brows, nose and mouth: what each one is, and its pose.

use crate::genes::{dna, Genes};
use crate::geom::{clamp, cos, lerp, max, min, pt, sin, Profile, Pt};
use crate::head::{FaceLayout, Head};
use std::f64::consts::PI;

/// One kind of eye. An eye is a lens: `a` is its half-width, `bu` how far it
/// rises above its corners and `bl` how far it falls below them, as shares
/// of the eye's radius; `round` runs from 0 (two arcs meeting in points) to 1
/// (an ellipse). An `ink` eye is the lens filled with ink; the others are
/// white with a pupil. An `open` eye has no white and no lower lid.
#[derive(Clone, Copy, Debug)]
pub struct EyeKind {
    pub name: &'static str,
    pub w: u32,
    pub a: f64,
    pub bu: f64,
    pub bl: f64,
    pub round: f64,
    pub ink: bool,
    pub crease: bool,
    pub pupil: f64,
    /// The size of a coloured iris this kind always has, or 0.
    pub iris: f64,
    /// How heavy the upper lid's line is, or 0 for the default.
    pub upper: f64,
    pub lower: f64,
    /// How far the upper lid stays open at rest, or 0 to read it from a gene.
    pub rest: f64,
    pub open: bool,
}

const fn eye_kind(name: &'static str, w: u32, a: f64, bu: f64, bl: f64, round: f64) -> EyeKind {
    EyeKind {
        name,
        w,
        a,
        bu,
        bl,
        round,
        ink: false,
        crease: false,
        pupil: 0.0,
        iris: 0.0,
        upper: 0.0,
        lower: 0.0,
        rest: 0.0,
        open: false,
    }
}

pub const EYE_KINDS: [EyeKind; 9] = [
    EyeKind {
        ink: true,
        ..eye_kind("dot", 6, 0.66, 0.66, 0.66, 1.0)
    },
    EyeKind {
        ink: true,
        crease: true,
        ..eye_kind("bead", 2, 0.56, 0.56, 0.56, 1.0)
    },
    EyeKind {
        ink: true,
        ..eye_kind("oval", 4, 0.54, 0.94, 0.94, 1.0)
    },
    EyeKind {
        pupil: 0.44,
        upper: 1.2,
        lower: 0.55,
        ..eye_kind("round", 5, 1.0, 1.0, 1.0, 1.0)
    },
    EyeKind {
        pupil: 0.3,
        iris: 0.6,
        upper: 1.35,
        lower: 0.5,
        ..eye_kind("almond", 5, 1.24, 0.68, 0.5, 0.3)
    },
    EyeKind {
        pupil: 0.42,
        upper: 1.4,
        lower: 0.5,
        rest: 0.6,
        crease: true,
        ..eye_kind("lidded", 3, 1.08, 0.92, 0.8, 0.9)
    },
    EyeKind {
        pupil: 0.42,
        upper: 1.2,
        open: true,
        ..eye_kind("arc", 3, 0.95, 0.62, 0.9, 0.6)
    },
    EyeKind {
        pupil: 0.37,
        upper: 1.1,
        lower: 0.7,
        ..eye_kind("wide", 2, 1.12, 1.12, 1.12, 1.0)
    },
    EyeKind {
        pupil: 0.36,
        upper: 1.3,
        lower: 0.55,
        ..eye_kind("slit", 2, 1.26, 0.38, 0.32, 0.35)
    },
];

/// Both eyes of a head.
#[derive(Clone, Debug)]
pub struct Eyes {
    pub kind: &'static str,
    pub k: EyeKind,
    pub r: f64,
    pub a: f64,
    pub bu: f64,
    pub bl: f64,
    pub y: f64,
    /// How far each eye's centre is from the axis.
    pub x: f64,
    pub rest: f64,
    pub slant: f64,
    pub tilt: f64,
    pub glint_side: f64,
    pub pupil_k: f64,
    pub iris: f64,
    pub lash: u32,
    pub lower: f64,
    pub crease: bool,
    /// Which eye is larger: -1, 1, or 0 for neither.
    pub odd: f64,
    pub odd_k: f64,
}

pub fn make_eyes(g: &Genes, hd: &Head, f: &FaceLayout) -> Eyes {
    let o = dna::EYES;
    let k = EYE_KINDS[g.pick(o, 0, &EYE_KINDS.map(|k| k.w))];
    let mut r = clamp(hd.hw(f.eye_y) * g.lin(o, 5, 3, 0.15, 0.25), 9.5, 17.0);
    let tall = if k.ink {
        1.0
    } else {
        g.lin(o + 3, 5, 3, 0.86, 1.16)
    };
    let mut a = k.a * r;
    let mut bu = k.bu * r * tall;
    let mut bl = k.bl * r * tall;
    // Two eyes and the gap between them fit inside the outline at their height.
    let room = min(
        min(hd.hw(f.eye_y - bu), hd.hw(f.eye_y)),
        hd.hw(f.eye_y + bl),
    ) - 6.0;
    let most = (room - 4.0) / 2.0;
    if a > most {
        let s = most / a;
        a *= s;
        bu *= s;
        bl *= s;
        r *= s;
    }
    let x = clamp(
        room * g.lin(o + 1, 0, 3, 0.4, 0.62),
        a + max(4.0, a * 0.34),
        room - a,
    );
    let rest = if k.rest != 0.0 {
        k.rest
    } else {
        [1.0, 0.95, 0.86, 0.75][g.raw(o + 1, 3, 2) as usize]
    };
    let slant = g.signed(o + 1, 5, 2) * 0.16;
    let tilt = if k.ink {
        0.0
    } else {
        g.signed(o + 2, 2, 2) * 0.1
    };
    let glint_side = g.side(o + 1, 7);
    let pupil_k = if k.ink {
        1.0
    } else {
        g.lin(o + 2, 0, 2, 0.86, 1.16)
    };
    let coloured = k.iris != 0.0 || (k.name != "slit" && g.flag(o + 2, 4));
    let iris = if !k.ink && !k.open && coloured {
        if k.iris != 0.0 {
            k.iris
        } else {
            0.68
        }
    } else {
        0.0
    };
    let lash = [0, 0, 1, 2][g.raw(o + 2, 5, 2) as usize];
    let lower = if k.ink || k.open { 0.0 } else { k.lower };
    let crease = k.crease || (!k.ink && g.flag(o + 2, 7));
    let odd_raw = g.raw(o + 3, 0, 2);
    let odd = if odd_raw == 0 { 0.0 } else { g.side(o + 3, 2) };
    let mut odd_k = [1.0, 1.07, 1.12, 1.18][odd_raw as usize];
    // Keep the larger eye inside the outline too.
    if odd != 0.0 {
        odd_k = min(min(odd_k, (room - x) / a), (x - 3.0) / a);
    }
    Eyes {
        kind: k.name,
        k,
        r,
        a,
        bu,
        bl,
        y: f.eye_y,
        x,
        rest,
        slant,
        tilt,
        glint_side,
        pupil_k,
        iris,
        lash,
        lower,
        crease,
        odd,
        odd_k,
    }
}

/// How an eye is shut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shut {
    /// An arc that bows up.
    Happy,
    /// An arc that bows down.
    Sleep,
}

/// An eye in a mood: how far the upper lid is open (1 fully), how far the
/// lower lid is raised, how the lids slant, where the eye looks.
#[derive(Clone, Copy, Debug)]
pub struct EyePose {
    pub open: f64,
    pub low: f64,
    pub slant: f64,
    pub look: (f64, f64),
    pub closed: Option<Shut>,
    /// The pupil's size, as a share of its size at rest.
    pub pupil: f64,
}

/// Where an eye's outline and lids are in one pose.
pub struct EyeGeom {
    pub cx: f64,
    pub cy: f64,
    pub a: f64,
    pub bu: f64,
    pub bl: f64,
    pub k: f64,
    /// The arc of a shut eye.
    pub shut: Option<Vec<Pt>>,
    pub lid: Vec<Pt>,
    pub top: Vec<Pt>,
    pub bot: Vec<Pt>,
    pub poly: Vec<Pt>,
    pub lidded: bool,
    pub raised: bool,
}

pub fn eye_geom(e: &Eyes, sd: f64, st: &EyePose) -> EyeGeom {
    let kk = &e.k;
    let k = if e.odd == sd { e.odd_k } else { 1.0 };
    let a = e.a * k;
    let bu = e.bu * k;
    let bl = e.bl * k;
    let look = st.look;
    let cx = sd * e.x + if kk.ink { look.0 * a * 0.24 } else { 0.0 };
    let cy = e.y + if kk.ink { look.1 * bu * 0.22 } else { 0.0 };
    let at = |u: f64, y: f64| pt(cx + sd * a * u, cy + y);
    let mut g = EyeGeom {
        cx,
        cy,
        a,
        bu,
        bl,
        k,
        shut: None,
        lid: Vec::new(),
        top: Vec::new(),
        bot: Vec::new(),
        poly: Vec::new(),
        lidded: false,
        raised: false,
    };
    if let Some(closed) = st.closed {
        let w = max(a, 0.78 * e.r * k);
        let h = max(bu, 0.6 * e.r * k);
        let tip = st.slant * 0.3 * w;
        let wat = |u: f64, y: f64| pt(cx + sd * w * u, cy + y - tip * u);
        g.shut = Some(match closed {
            Shut::Happy => vec![
                wat(-1.0, h * 0.24),
                wat(-0.5, -h * 0.26),
                wat(0.0, -h * 0.42),
                wat(0.5, -h * 0.26),
                wat(1.0, h * 0.24),
            ],
            Shut::Sleep => vec![
                wat(-1.0, -h * 0.1),
                wat(-0.5, h * 0.26),
                wat(0.0, h * 0.38),
                wat(0.5, h * 0.26),
                wat(1.0, -h * 0.1),
            ],
        });
        return g;
    }
    let shape = |u: f64| {
        let q = max(0.0, 1.0 - u * u);
        lerp(q, q.sqrt(), kk.round)
    };
    let top0 = |u: f64| -bu * shape(u) - e.tilt * a * u;
    let bot0 = |u: f64| bl * shape(u) - e.tilt * a * u;
    let open = clamp(st.open, 0.0, 1.0);
    let low = st.low;
    let slant = st.slant;
    // Both lids hinge at the corners. Fully open, the upper one lies just
    // outside the eye's edge; as it closes it comes down to a line that
    // bows a little below the corners. The lower lid rises into an arch.
    let lid =
        |u: f64| lerp(bl * 0.55, -bu * 1.03, open) * shape(u) - (slant * 0.42 + e.tilt) * a * u;
    let low_lid = |u: f64| {
        lerp(
            bl * 1.03 * shape(u),
            bl * 0.12 - bl * 0.62 * (1.0 - u * u),
            low,
        ) - e.tilt * a * u
    };
    const N: usize = 12;
    let mut us = Vec::with_capacity(N + 1);
    let mut ts = Vec::with_capacity(N + 1);
    let mut bs = Vec::with_capacity(N + 1);
    for i in 0..=N {
        let u = -cos(PI * i as f64 / N as f64);
        us.push(u);
        ts.push(max(top0(u), lid(u)));
        bs.push(min(bot0(u), low_lid(u)));
    }
    // The longest stretch along which the eye is open.
    let (mut i0, mut i1): (i64, i64) = (-1, -1);
    let mut start: i64 = -1;
    for i in 0..=(N as i64 + 1) {
        let is_open = i <= N as i64 && bs[i as usize] - ts[i as usize] > 0.0;
        if is_open && start < 0 {
            start = i;
        }
        if !is_open && start >= 0 {
            if i - 1 - start > i1 - i0 || i0 < 0 {
                i0 = start;
                i1 = i - 1;
            }
            start = -1;
        }
    }
    let mut tallest: f64 = 0.0;
    for i in i0.max(0)..=i1 {
        let i = i as usize;
        tallest = max(tallest, bs[i] - ts[i]);
    }
    for &u in &us {
        g.lid.push(at(u, clamp(lid(u), top0(u), bot0(u))));
    }
    if i0 < 0 || tallest < 1.1 {
        g.shut = Some(vec![g.lid[0], g.lid[3], g.lid[6], g.lid[9], g.lid[12]]);
        return g;
    }
    let (i0, i1) = (i0 as usize, i1 as usize);
    // Where the two lids meet at each end of that stretch.
    let corner = |inside: usize, outside: i64| {
        if outside < 0 || outside > N as i64 {
            return at(us[inside], (ts[inside] + bs[inside]) / 2.0);
        }
        let outside = outside as usize;
        let hi = bs[inside] - ts[inside];
        let ho = bs[outside] - ts[outside];
        let t = hi / (hi - ho);
        at(
            lerp(us[inside], us[outside], t),
            lerp(
                (ts[inside] + bs[inside]) / 2.0,
                (ts[outside] + bs[outside]) / 2.0,
                t,
            ),
        )
    };
    let cl = corner(i0, i0 as i64 - 1);
    let cr = corner(i1, i1 as i64 + 1);
    let sharp = kk.round < 0.8 || low > 0.25 || slant.abs() > 0.3 || open < 0.8;
    g.top.push(cl.with_sharp(sharp));
    g.bot.push(cr.with_sharp(sharp));
    for i in i0..=i1 {
        g.top.push(at(us[i], ts[i]));
    }
    for i in (i0..=i1).rev() {
        g.bot.push(at(us[i], bs[i]));
    }
    g.top.push(cr.with_sharp(sharp));
    g.bot.push(cl.with_sharp(sharp));
    g.poly = g.top[..g.top.len() - 1].to_vec();
    g.poly.extend_from_slice(&g.bot[..g.bot.len() - 1]);
    // Whether a lid, and not the eye's own edge, closes the eye at the top or the bottom.
    for &u in &us[i0..=i1] {
        if lid(u) > top0(u) + 0.35 {
            g.lidded = true;
        }
        if low_lid(u) < bot0(u) - 0.35 {
            g.raised = true;
        }
    }
    g
}

/// One kind of brow: the stroke's profile and its width range in frame units.
#[derive(Clone, Copy, Debug)]
pub struct BrowKind {
    pub name: &'static str,
    wt: u32,
    pub press: Profile,
    w: [f64; 2],
    bow: f64,
    len: f64,
}

pub const BROW_KINDS: [BrowKind; 7] = [
    BrowKind {
        name: "taper",
        wt: 4,
        press: Profile::Out,
        w: [3.6, 6.2],
        bow: 0.0,
        len: 1.0,
    },
    BrowKind {
        name: "line",
        wt: 2,
        press: Profile::Full,
        w: [2.3, 3.2],
        bow: 0.0,
        len: 1.0,
    },
    BrowKind {
        name: "block",
        wt: 2,
        press: Profile::Even,
        w: [4.4, 7.0],
        bow: 0.0,
        len: 1.0,
    },
    BrowKind {
        name: "arch",
        wt: 2,
        press: Profile::Leaf,
        w: [2.8, 4.2],
        bow: 0.16,
        len: 1.0,
    },
    BrowKind {
        name: "bushy",
        wt: 2,
        press: Profile::Full,
        w: [6.0, 8.6],
        bow: 0.0,
        len: 1.0,
    },
    BrowKind {
        name: "short",
        wt: 2,
        press: Profile::Full,
        w: [5.0, 7.4],
        bow: 0.0,
        len: 0.56,
    },
    BrowKind {
        name: "wedge",
        wt: 2,
        press: Profile::Into,
        w: [3.8, 6.4],
        bow: 0.0,
        len: 1.0,
    },
];

#[derive(Clone, Debug)]
pub struct Brows {
    pub kind: &'static str,
    pub k: BrowKind,
    pub w: f64,
    pub tilt: f64,
    pub gap: f64,
    pub half: f64,
    pub bow: f64,
    pub odd: f64,
    pub y: f64,
}

pub fn make_brows(g: &Genes, e: &Eyes, size: f64) -> Brows {
    let o = dna::BROWS;
    let k = BROW_KINDS[g.pick(o, 0, &BROW_KINDS.map(|k| k.wt))];
    let w = lerp(k.w[0], k.w[1], g.unit(o, 4, 2)) * size;
    let tilt = g.signed(o, 6, 2) * 0.11;
    let gap = lerp(4.2, 9.5, g.unit(o + 1, 0, 2)) * size;
    let half = max(e.a, e.r * 0.82) * k.len * g.lin(o + 1, 2, 2, 0.98, 1.36);
    let bow = k.bow + g.lin(o + 1, 4, 2, 0.04, 0.24);
    let odd = if g.flag(o + 1, 6) {
        g.side(o + 1, 7)
    } else {
        0.0
    };
    // The brow at rest clears the top of the eye.
    let y = e.y - e.bu - gap - w / 2.0;
    Brows {
        kind: k.name,
        k,
        w,
        tilt,
        gap,
        half,
        bow,
        odd,
        y,
    }
}

/// A brow in a mood: raised, slanted (positive: the inner end lower), bent,
/// and drawn in towards the nose.
#[derive(Clone, Copy, Debug, Default)]
pub struct BrowPose {
    pub raise: f64,
    pub tilt: f64,
    pub bow: f64,
    pub pinch: f64,
}

pub fn brow_pts(b: &Brows, e: &Eyes, sd: f64, st: &BrowPose) -> [Pt; 3] {
    let h = b.half;
    let slope = b.tilt + st.tilt;
    let bow = max(-0.12, b.bow + st.bow);
    let cx = sd * (e.x + h * 0.06 - st.pinch);
    let cy = b.y - st.raise - if b.odd == sd { 2.6 } else { 0.0 };
    [
        pt(cx - sd * h, cy + slope * h),
        pt(cx + sd * h * 0.1, cy - bow * h - slope * h * 0.1),
        pt(cx + sd * h, cy - slope * h + h * 0.1),
    ]
}

pub const NOSE_KINDS: [(&str, u32); 14] = [
    ("hook", 4),
    ("u", 3),
    ("dots", 2),
    ("c", 3),
    ("angle", 2),
    ("bulb", 2),
    ("wide", 3),
    ("long", 2),
    ("dash", 2),
    ("potato", 2),
    ("v", 2),
    ("snub", 2),
    ("button", 1),
    ("bracket", 2),
];

#[derive(Clone, Debug)]
pub struct Nose {
    pub kind: &'static str,
    pub w: f64,
    /// Which way it turns, for the kinds that turn.
    pub side: f64,
    /// Where its bridge starts, for the kinds with a bridge.
    pub high: f64,
    /// The size of its tip, for the kinds with a tip.
    pub tip: f64,
}

pub fn make_nose(g: &Genes, size: f64) -> Nose {
    let o = dna::NOSE;
    let kind = NOSE_KINDS[g.pick(o, 0, &NOSE_KINDS.map(|k| k.1))].0;
    let w = g.lin(o, 5, 3, 5.5, 12.5) * size;
    let side = if matches!(kind, "hook" | "long" | "c" | "angle" | "dash") {
        g.side(o + 1, 0)
    } else {
        0.0
    };
    let high = if matches!(kind, "hook" | "long" | "c" | "angle" | "dash" | "potato") {
        g.unit(o + 1, 1, 2)
    } else {
        0.0
    };
    let tip = if matches!(kind, "dots" | "bulb" | "button") {
        g.unit(o + 1, 3, 2)
    } else {
        0.0
    };
    Nose {
        kind,
        w,
        side,
        high,
        tip,
    }
}

pub const MOUTH_KINDS: [(&str, u32); 7] = [
    ("line", 5),
    ("smirk", 2),
    ("lips", 3),
    ("pout", 2),
    ("wavy", 1),
    ("teeth", 1),
    ("wide", 2),
];

#[derive(Clone, Debug)]
pub struct Mouth {
    pub kind: &'static str,
    pub y: f64,
    pub w: f64,
    pub curve: f64,
    pub skew: f64,
    /// Which corner is the higher one.
    pub side: f64,
    pub dimples: bool,
    pub full: f64,
}

pub fn make_mouth(g: &Genes, hd: &Head, f: &FaceLayout) -> Mouth {
    let o = dna::MOUTH;
    let y = f.mouth_y;
    let kind = MOUTH_KINDS[g.pick(o, 0, &MOUTH_KINDS.map(|k| k.1))].0;
    let room = min(hd.hw(y), hd.hw(y + 5.0)) - 9.0;
    let w = clamp(
        hd.hw(y) * g.lin(o, 4, 3, 0.26, 0.56) * if kind == "wide" { 1.2 } else { 1.0 },
        9.0,
        max(9.0, room),
    );
    let curve = g.lin(o + 1, 0, 3, -0.3, 0.5);
    let skew = (if kind == "smirk" { 0.5 } else { 0.0 }) + g.unit(o + 1, 3, 2) * 0.3;
    let side = g.side(o, 7);
    let dimples = kind != "smirk" && g.flag(o + 1, 5);
    let full = if kind == "lips" {
        g.unit(o + 1, 6, 2)
    } else {
        0.0
    };
    Mouth {
        kind,
        y,
        w,
        curve,
        skew,
        side,
        dimples,
        full,
    }
}

/// The shape a mouth takes in a mood, beside the line it rests on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouthForm {
    Line,
    /// A small round mouth.
    O,
    /// A line pressed into a zigzag.
    Zig,
    /// Clenched teeth.
    Grit,
    /// A line with a wave in it.
    Wavy,
}

/// A mouth in a mood.
#[derive(Clone, Copy, Debug)]
pub struct MouthPose {
    pub form: MouthForm,
    /// Its curve: positive is a smile.
    pub c: f64,
    /// Its width, as a share of its width at rest.
    pub w: f64,
    pub open: f64,
    /// How far a round mouth sits off the middle, as a share of its width.
    pub off: f64,
    pub dx: f64,
    pub dy: f64,
    /// How far one corner is lifted, or `None` for the mouth's own lift.
    pub skew: Option<f64>,
    pub teeth: bool,
    pub tongue: bool,
    pub ticks: bool,
    pub plain: bool,
    /// The corner the tongue's tip shows at, or 0.
    pub tongue_out: f64,
    /// The corner a drop falls from, or 0.
    pub drool: f64,
}

impl Default for MouthPose {
    fn default() -> Self {
        Self {
            form: MouthForm::Line,
            c: 0.0,
            w: 1.0,
            open: 0.0,
            off: 0.0,
            dx: 0.0,
            dy: 0.0,
            skew: None,
            teeth: false,
            tongue: true,
            ticks: false,
            plain: false,
            tongue_out: 0.0,
            drool: 0.0,
        }
    }
}

/// The line of a mouth in a pose: its half-width, middle, and height at u
/// (-1 at the left corner, 1 at the right one).
pub struct MouthLine {
    pub w: f64,
    pub mx: f64,
    pub my: f64,
    c: f64,
    skew: f64,
    wavy: bool,
}

impl MouthLine {
    pub fn new(m: &Mouth, pose: &MouthPose) -> Self {
        Self {
            w: m.w * pose.w,
            mx: pose.dx,
            my: m.y + pose.dy,
            c: pose.c,
            skew: pose.skew.unwrap_or(m.skew) * m.side,
            wavy: pose.form == MouthForm::Wavy,
        }
    }

    pub fn y(&self, u: f64) -> f64 {
        let w = self.w;
        self.my + self.c * w * (0.17 - 0.5 * u * u) - self.skew * u * w * 0.2
            + if self.wavy {
                sin(u * PI * 1.5) * w * 0.09
            } else {
                0.0
            }
    }
}
