//! Reading a toy's traits from its genome.
//!
//! The byte map. Every field of the toy's own is the top bits of one byte.
//! The colour fields are read as creature.wild reads them: the low three
//! bits of byte 160, all of bytes 161 and 162, and the low two of byte 163.
//!
//! | bytes | trait |
//! |---|---|
//! | 160 (low 3 bits) | wild's colour recipe: pastel, vivid, muted or dark |
//! | 161, 162 | wild's hue |
//! | 163 (low 2 bits) | accent family, as wild's belly variant |
//! | 1024-1028 | body plan, yarn weight, row curvature, stitch drift, size |
//! | 1032-1054 | main piece: a two-piece toy's head, or the one piece |
//! | 1056-1078 | body piece of a two-piece toy |
//! | 1080-1081 | how far the head sits into the body, and its size |
//! | 1088-1101 | face layout and eyes |
//! | 1104-1114 | snout and nose |
//! | 1116-1118 | mouth |
//! | 1120-1122 | brows |
//! | 1124-1125 | cheeks |
//! | 1127 | whether the head grows something at its sides, its crown, or nothing |
//! | 1128-1136 | the sides of the head: ears, gills, frills, ram's horns, tentacles, tufts |
//! | 1140-1146 | the crown: horns, antlers, antennae, spines, plates, a tuft, a sprout |
//! | 1147 | one eye or three, for a toy that would otherwise be a stock animal |
//! | 1148-1149 | the stalks of eyes on stalks |
//! | 1152-1157 | arms |
//! | 1160-1165 | legs |
//! | 1168-1172 | tail |
//! | 1176-1178 | accent yarn, stripe yarn, felt blush |
//! | 1184-1186 | stripes on ears, tail and wings |
//! | 1188-1193 | patches: belly, spots, joints, mends and hearts |
//! | 1200-1204 | moods: which side marks take, and how four moods show |
//!
//! Bytes 256 to 1023 are kept for traits only a 3D toy would show and are
//! never read.

use crate::dna::Reader;
use crate::geom::PI;

/// A body plan's shape ranges. Every plan is rounded: its pieces are
/// ellipses at heart, made fuller or more pointed at the crown and the base
/// (`k_t`, `k_b`), bent by the aspect (width over height) and a taper that
/// widens the bottom (+) or the top (-), and for the bean a waist. `face` is
/// where the eye line sits down a one-piece toy; `face_scale` how large the
/// features are against the piece's width. Two-piece plans set the head's
/// half-width against the body's (`head`) and how far it sits into the body.
pub struct Plan {
    pub pieces: u8,
    pub aspect: [f64; 2],
    pub taper: f64,
    pub k_t: [f64; 2],
    pub k_b: [f64; 2],
    pub face: f64,
    pub face_scale: Option<f64>,
    pub waist: bool,
    pub head: [f64; 2],
    pub head_aspect: [f64; 2],
    pub overlap: [f64; 2],
}

const fn one(aspect: [f64; 2], taper: f64, k_t: [f64; 2], k_b: [f64; 2], face: f64) -> Plan {
    Plan {
        pieces: 1,
        aspect,
        taper,
        k_t,
        k_b,
        face,
        face_scale: None,
        waist: false,
        head: [0.0; 2],
        head_aspect: [0.0; 2],
        overlap: [0.0; 2],
    }
}

pub fn plan(name: &str) -> Plan {
    match name {
        "ball" => one([0.92, 1.12], 0.0, [-0.024, 0.032], [-0.012, 0.052], 0.45),
        "egg" => Plan {
            face_scale: Some(1.1),
            ..one([0.72, 0.86], -0.24, [-0.062, -0.012], [0.0, 0.042], 0.4)
        },
        "loaf" => one([1.22, 1.5], 0.06, [0.0, 0.052], [0.01, 0.052], 0.46),
        "bean" => Plan {
            waist: true,
            ..one([0.8, 0.98], 0.12, [-0.024, 0.032], [0.0, 0.042], 0.3)
        },
        "pear" => one([0.86, 1.06], 0.38, [-0.05, 0.0], [0.0, 0.052], 0.36),
        "pillar" => Plan {
            face_scale: Some(1.5),
            ..one([0.58, 0.72], 0.0, [-0.024, 0.032], [0.0, 0.052], 0.35)
        },
        "chibi" => Plan {
            pieces: 2,
            head: [1.05, 1.35],
            head_aspect: [1.0, 1.3],
            overlap: [0.18, 0.4],
            ..one([0.9, 1.15], 0.1, [-0.024, 0.032], [0.0, 0.052], 0.0)
        },
        _ => Plan {
            pieces: 2,
            head: [0.7, 0.9],
            head_aspect: [0.95, 1.2],
            overlap: [0.08, 0.24],
            face_scale: Some(1.2),
            ..one([1.0, 1.25], 0.1, [-0.024, 0.032], [0.0, 0.052], 0.0)
        },
    }
}

/// One radial DNA point of a piece's outline.
#[derive(Clone, Copy, Debug)]
pub struct Spoke {
    pub t: f64,
    pub dev: f64,
}

/// One stuffed piece: a one-piece body, a head or a body.
#[derive(Clone, Debug)]
pub struct PieceGenes {
    pub aspect: f64,
    pub k_t: f64,
    pub k_b: f64,
    pub taper: f64,
    pub pinch: f64,
    pub share: f64,
    pub lump: f64,
    pub spokes: Vec<Spoke>,
    pub b_t: f64,
    pub b_b: f64,
    pub a: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Ears {
    pub kind: &'static str,
    pub size: f64,
    pub pos: f64,
    pub tilt: f64,
    pub width: f64,
    pub inner: Option<&'static str>,
    pub tip: Option<&'static str>,
    pub bend: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct Top {
    pub kind: &'static str,
    pub size: f64,
    pub spread: Option<f64>,
    pub curl: Option<f64>,
    pub yarn: Option<&'static str>,
    pub count: Option<u32>,
    pub tines: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Arms {
    pub kind: &'static str,
    pub len: f64,
    pub width: f64,
    pub pos: f64,
    pub angle: f64,
    pub paw: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct Legs {
    pub kind: &'static str,
    pub len: f64,
    pub width: f64,
    pub spread: f64,
    pub pads: Option<&'static str>,
    pub count: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Tail {
    pub kind: &'static str,
    pub size: f64,
    pub side: f64,
    pub curl: Option<f64>,
    pub tip: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct Stripes {
    pub kind: &'static str,
    pub period: u32,
    pub phase: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Patches {
    pub belly: &'static str,
    pub spots: &'static str,
    pub mend: &'static str,
    pub belly_size: f64,
    pub spot_layout: u32,
    pub mend_side: f64,
}

#[derive(Clone, Debug, Default)]
pub struct FaceGenes {
    pub y: f64,
    pub scale: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Eyes {
    pub kind: &'static str,
    pub size: f64,
    pub spacing: f64,
    pub count: u32,
    pub mount: &'static str,
    pub iris: Option<&'static str>,
    pub pupil: Option<&'static str>,
    pub felt: Option<&'static str>,
    pub look: Option<&'static str>,
    pub lid: &'static str,
    pub wonk: i32,
    pub stalk_len: f64,
    pub stalk_lean: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Snout {
    pub kind: &'static str,
    pub size: f64,
    pub aspect: f64,
    pub y: f64,
    pub yarn: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct Nose {
    pub kind: &'static str,
    pub size: f64,
    pub shape: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct Mouth {
    pub kind: &'static str,
    pub width: f64,
    pub curve: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct Brows {
    pub rest: &'static str,
    pub weight: f64,
    pub length: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Cheeks {
    pub kind: &'static str,
    pub size: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Colour {
    pub accent: &'static str,
    pub blush: u32,
    pub second: Option<&'static str>,
}

/// How this toy shows four of the moods.
#[derive(Clone, Debug, Default)]
pub struct Temper {
    pub happy: &'static str,
    pub working: &'static str,
    pub blocked: &'static str,
    pub sad: &'static str,
}

/// Every trait a toy's genome chooses.
#[derive(Clone, Debug)]
pub struct Traits {
    pub tone: &'static str,
    pub hue_wild: f64,
    pub accent_family: u32,
    pub plan: &'static str,
    pub stacked: bool,
    pub gauge: f64,
    pub curve: f64,
    pub drift: f64,
    pub size: f64,
    pub main: PieceGenes,
    pub body_p: Option<PieceGenes>,
    pub overlap: f64,
    pub head_ratio: f64,
    pub ears: Ears,
    pub top: Top,
    pub arms: Arms,
    pub legs: Legs,
    pub tail: Tail,
    pub stripes: Stripes,
    pub patches: Patches,
    pub face: FaceGenes,
    pub eyes: Eyes,
    pub snout: Snout,
    pub nose: Nose,
    pub mouth: Mouth,
    pub brows: Brows,
    pub cheeks: Cheeks,
    pub colour: Colour,
    pub stock_saved: bool,
    pub mood_side: f64,
    pub temper: Temper,
}

/// How far each of six choice genes is turned (see [`Reader::pick`]). Which
/// value of a gene means which body plan or which ears is arbitrary; these
/// turns are the reference's.
struct Turns {
    plan: u32,
    site: u32,
    ears: u32,
    top: u32,
    arms: u32,
    legs: u32,
}

const TURNS: Turns = Turns {
    plan: 4,
    site: 1,
    ears: 2,
    top: 0,
    arms: 2,
    legs: 7,
};

/// Ears a shop toy has.
pub fn familiar_ears(kind: &str) -> bool {
    matches!(kind, "round" | "cat" | "bunny" | "floppy" | "leaf")
}

fn decode_piece(r: &Reader, base: usize, plan: &Plan, role: &str) -> PieceGenes {
    let head = role == "head";
    let asp = if head { plan.head_aspect } else { plan.aspect };
    let aspect = r.lin(base, 4, asp[0], asp[1]);
    let k_t = if head { [-0.024, 0.042] } else { plan.k_t };
    let k_b = if head { [-0.012, 0.042] } else { plan.k_b };
    let k_t = r.lin(base + 1, 3, k_t[0], k_t[1]);
    let k_b = r.lin(base + 2, 3, k_b[0], k_b[1]);
    let taper =
        (if head { 0.0 } else { plan.taper }) + r.s(base + 3, 3) * if head { 0.08 } else { 0.06 };
    let pinch = if role == "one" && plan.waist {
        r.lin(base + 4, 3, 0.45, 1.0)
    } else {
        0.0
    };
    let share = r.lin(base + 5, 3, 0.45, 0.55);
    let k = 3 + r.u(base + 6, 2) as usize;
    let lump = r.lin(base + 7, 3, 0.03, 0.12);
    let mut spokes = Vec::with_capacity(k + 2);
    for i in 0..=k + 1 {
        let t0 = i as f64 / (k + 1) as f64 * PI;
        let jit = if i == 0 || i == k + 1 {
            0.0
        } else {
            r.s(base + 16 + i, 2) * 0.3 * PI / (k + 1) as f64
        };
        spokes.push(Spoke {
            t: t0 + jit,
            dev: r.s(base + 8 + i, 3) * lump,
        });
    }
    // Toy units; the fit scales the toy into its frame, so only proportions
    // matter.
    let h = match role {
        "one" => 110.0,
        "body" => 70.0,
        _ => 60.0,
    };
    let b_t = h * share;
    PieceGenes {
        aspect,
        k_t,
        k_b,
        taper,
        pinch,
        share,
        lump,
        spokes,
        b_t,
        b_b: h - b_t,
        a: aspect * h / 2.0,
    }
}

/// Reads a toy's traits. Only the traits a toy keeps read their details, so
/// a trait that is not drawn leaves its bytes unread.
pub fn decode(bytes: &[u8; 16_384]) -> Traits {
    let r = Reader::new(bytes);

    // Colour, as creature.wild reads it.
    let rsel = r.low(160, 3);
    let tone = match rsel {
        0..=2 => "pastel",
        3..=4 => "vivid",
        5..=6 => "muted",
        _ => "dark",
    };
    let hue_wild = (f64::from(r.u(161, 8)) / 255.0 + f64::from(r.u(162, 8)) / 65535.0) * 360.0;
    let accent_family = r.low(163, 2);

    let plan_name = r.pick(
        1024,
        4,
        &[
            ("chibi", 3),
            ("ball", 2),
            ("egg", 2),
            ("pear", 2),
            ("bean", 2),
            ("loaf", 2),
            ("snowman", 2),
            ("pillar", 1),
        ],
        TURNS.plan,
    );
    let pl = plan(plan_name);
    let stacked = pl.pieces == 2;
    let gauge = r.lin(1025, 3, 0.86, 1.3);
    let curve = r.lin(1026, 2, 0.15, 0.3);
    let drift = r.lin(1027, 2, 0.0, 0.2);
    // Toys differ in visual weight on purpose: after the fit into the frame,
    // a toy is drawn at this share of the size that would fill it.
    let size = r.lin(1028, 3, 0.83, 1.0);

    let (main, body_p, overlap, head_ratio) = if !stacked {
        (decode_piece(&r, 1032, &pl, "one"), None, 0.0, 0.0)
    } else {
        let main = decode_piece(&r, 1032, &pl, "head");
        let body = decode_piece(&r, 1056, &pl, "body");
        let overlap = r.lin(1080, 3, pl.overlap[0], pl.overlap[1]);
        let head_ratio = r.lin(1081, 3, pl.head[0], pl.head[1]);
        (main, Some(body), overlap, head_ratio)
    };

    let mut x = Ears::default();
    let mut hd = Top::default();
    let mut a = Arms::default();
    let mut l = Legs::default();
    let mut tl = Tail::default();
    let mut st = Stripes::default();
    let mut pt = Patches::default();
    // A head grows something at its sides or on its crown, never both.
    let site = r.pick(
        1127,
        4,
        &[("none", 2), ("sides", 8), ("crown", 6)],
        TURNS.site,
    );
    x.kind = if site != "sides" {
        "none"
    } else {
        r.pick(
            1128,
            4,
            &[
                ("round", 1),
                ("cat", 1),
                ("bunny", 1),
                ("floppy", 1),
                ("leaf", 1),
                ("gills", 3),
                ("tentacles", 2),
                ("frill", 2),
                ("ram", 3),
                ("tufts", 1),
            ],
            TURNS.ears,
        )
    };
    hd.kind = if site != "crown" {
        "none"
    } else {
        r.pick(
            1140,
            4,
            &[
                ("nubs", 1),
                ("horns", 3),
                ("unicorn", 1),
                ("antlers", 3),
                ("antennae", 3),
                ("tuft", 1),
                ("sprout", 1),
                ("spines", 2),
                ("plates", 1),
            ],
            TURNS.top,
        )
    };
    a.kind = r.pick(
        1152,
        3,
        &[
            ("none", 1),
            ("nub", 3),
            ("long", 1),
            ("wing", 1),
            ("four", 1),
            ("tentacle", 1),
        ],
        TURNS.arms,
    );
    l.kind = r.pick(
        1160,
        3,
        &[("none", 1), ("feet", 4), ("legs", 2), ("tentacles", 1)],
        TURNS.legs,
    );
    tl.kind = r.pick(
        1168,
        3,
        &[
            ("none", 3),
            ("pompom", 1),
            ("curly", 1),
            ("long", 1),
            ("bushy", 1),
            ("spike", 1),
        ],
        0,
    );
    st.kind = r.pick(1184, 1, &[("none", 1), ("parts", 1)], 0);
    pt.belly = r.pick(1188, 2, &[("none", 2), ("felt", 1), ("knit", 1)], 0);
    pt.spots = r.pick(1190, 2, &[("none", 3), ("spots", 1)], 0);
    pt.mend = r.pick(
        1192,
        3,
        &[("none", 5), ("joints", 1), ("mend", 1), ("heart", 1)],
        0,
    );

    let face = FaceGenes {
        y: r.s(1088, 3),
        scale: r.lin(1089, 3, 0.92, 1.2),
    };
    let mut e = Eyes {
        kind: r.pick(
            1090,
            3,
            &[
                ("safety", 3),
                ("iris", 1),
                ("felt", 2),
                ("stitched", 1),
                ("button", 1),
            ],
            0,
        ),
        size: r.lin(1091, 3, 0.15, 0.215),
        spacing: r.lin(1092, 3, 0.3, 0.6),
        count: r.pick(1093, 3, &[(2, 6), (1, 1), (3, 1)], 0),
        ..Eyes::default()
    };
    // Where a pair of eyes sits: on the face, on stalks, or on two bumps.
    e.mount = if e.count == 2 {
        r.pick(1094, 3, &[("face", 6), ("stalks", 1), ("bumps", 1)], 0)
    } else {
        "face"
    };
    // Eyes on stalks or bumps take the crown's place, and such a toy has no
    // ears, frill or tufts beneath them.
    if e.mount != "face" {
        hd.kind = "none";
        if familiar_ears(x.kind) || x.kind == "frill" || x.kind == "tufts" {
            x.kind = "none";
        }
    }
    // A loaf with nothing on its head reads as a knitted hat, so it always
    // grows something at its sides.
    if plan_name == "loaf" && x.kind == "none" && hd.kind == "none" && e.mount == "face" {
        x.kind = r.pick(
            1136,
            3,
            &[
                ("gills", 1),
                ("tentacles", 1),
                ("frill", 1),
                ("ram", 2),
                ("tufts", 1),
                ("cat", 1),
                ("round", 1),
            ],
            0,
        );
    }
    // Odd anatomy: wings, a second pair of arms, tentacles, gills.
    let odd_body = u32::from(matches!(a.kind, "wing" | "four" | "tentacle"))
        + u32::from(l.kind == "tentacles")
        + u32::from(matches!(x.kind, "gills" | "tentacles"));
    // A toy with a shop toy's ears and nothing odd about it would be a stock
    // animal, so it has one eye or three.
    let mut stock_saved = false;
    if familiar_ears(x.kind) && odd_body == 0 && e.count == 2 {
        e.count = r.pick(1147, 1, &[(3, 1), (1, 1)], 0);
        stock_saved = true;
    }
    let odd = odd_body + u32::from(e.count != 2 || e.mount != "face");
    // A single large button reads as a button sewn on, not an eye.
    if e.count == 1 && e.kind == "button" {
        e.kind = r.pick(1101, 1, &[("safety", 1), ("felt", 1)], 0);
    }
    if e.kind == "iris" {
        e.iris = Some(r.pick(
            1095,
            3,
            &[
                ("amber", 2),
                ("brown", 1),
                ("green", 2),
                ("blue", 2),
                ("violet", 1),
            ],
            0,
        ));
        e.pupil = Some(r.pick(1096, 2, &[("round", 3), ("slit", 1)], 0));
    }
    if e.kind == "felt" {
        e.felt = Some(r.pick(1097, 2, &[("round", 2), ("oval", 1), ("drop", 1)], 0));
        e.look = Some(r.pick(1098, 2, &[("centre", 2), ("out", 1), ("cross", 1)], 0));
    }
    // A button has no lid at rest.
    e.lid = if e.kind == "button" {
        "none"
    } else {
        r.pick(1099, 2, &[("none", 2), ("half", 1), ("heavy", 1)], 0)
    };
    e.wonk = if e.count == 2 && e.mount == "face" {
        r.pick(1100, 3, &[(0, 6), (1, 1), (-1, 1)], 0)
    } else {
        0
    };

    let mut s = Snout {
        kind: r.pick(
            1104,
            3,
            &[
                ("none", 3),
                ("muzzle", 2),
                ("pig", 1),
                ("beak", 1),
                ("bill", 1),
            ],
            0,
        ),
        ..Snout::default()
    };
    // Three eyes in a row fill a face; a snout under them would crowd it.
    if e.count == 3 {
        s.kind = "none";
    }
    if s.kind != "none" {
        s.size = r.lin(1105, 3, 0.3, 0.5);
        s.aspect = r.lin(1106, 3, 0.56, 0.9);
        s.y = r.f(1107, 3);
    }
    if s.kind == "muzzle" || s.kind == "pig" {
        s.yarn = Some(r.pick(1108, 2, &[("accent", 2), ("tint", 1), ("main", 1)], 0));
    }
    let mut n = Nose {
        kind: if matches!(s.kind, "pig" | "beak" | "bill") {
            "none"
        } else {
            r.pick(
                1112,
                2,
                &[("safety", 1), ("stitched", 1), ("felt", 1), ("none", 1)],
                0,
            )
        },
        ..Nose::default()
    };
    if n.kind != "none" {
        n.size = r.lin(1113, 2, 0.75, 1.45);
        n.shape = Some(r.pick(1114, 2, &[("triangle", 2), ("oval", 1), ("heart", 1)], 0));
    }

    let mut m = Mouth {
        kind: r.pick(
            1116,
            3,
            &[
                ("w", 2),
                ("smile", 2),
                ("v", 1),
                ("line", 1),
                ("fangs", 1),
                ("teeth", 1),
            ],
            0,
        ),
        width: r.lin(1117, 3, 0.75, 1.4),
        curve: None,
    };
    if matches!(m.kind, "smile" | "w" | "fangs") {
        m.curve = Some(r.lin(1118, 3, 0.35, 1.0));
    }

    let brows = Brows {
        rest: r.pick(1120, 2, &[("none", 2), ("stitched", 1), ("felt", 1)], 0),
        weight: r.lin(1121, 2, 0.8, 1.35),
        length: r.lin(1122, 2, 0.8, 1.3),
    };
    let cheeks = Cheeks {
        kind: r.pick(1124, 2, &[("none", 1), ("felt", 2), ("stitched", 1)], 0),
        size: r.lin(1125, 2, 0.8, 1.3),
    };

    // A toy carries three standout traits at most. Odd anatomy is always
    // kept and counts first, then a frill, a ram's horns or tufts; what is
    // left is spent in a fixed order: a crown part, a tail, stripes, spots,
    // a patch.
    let mut room = 3_i32 - odd as i32 - i32::from(matches!(x.kind, "frill" | "ram" | "tufts"));
    room = room.max(0);
    let mut spend = |on: bool| {
        if !on {
            return false;
        }
        if room > 0 {
            room -= 1;
            return false;
        }
        true
    };
    if spend(hd.kind != "none") {
        hd.kind = "none";
    }
    if spend(tl.kind != "none") {
        tl.kind = "none";
    }
    if spend(st.kind != "none") {
        st.kind = "none";
    }
    // Stripes and spots together read as pyjamas.
    if st.kind != "none" {
        pt.spots = "none";
    }
    if spend(pt.spots != "none") {
        pt.spots = "none";
    }
    if spend(pt.mend != "none") {
        pt.mend = "none";
    }

    if x.kind != "none" {
        x.size = r.lin(1129, 3, 0.78, 1.28);
        x.pos = r.f(1130, 3);
        x.tilt = r.s(1131, 3);
        x.width = r.lin(1132, 3, 0.8, 1.22);
        if familiar_ears(x.kind) {
            x.inner = Some(r.pick(1133, 2, &[("felt", 2), ("none", 1), ("yarn", 1)], 0));
        }
        if matches!(x.kind, "cat" | "bunny" | "floppy" | "leaf" | "tentacles") {
            x.tip = Some(r.pick(1134, 2, &[("none", 3), ("tipped", 1)], 0));
        }
        if matches!(
            x.kind,
            "bunny" | "floppy" | "cat" | "leaf" | "tentacles" | "ram"
        ) {
            x.bend = Some(r.s(1135, 3));
        }
    }
    if hd.kind != "none" {
        hd.size = r.lin(1141, 3, 0.78, 1.3);
        if matches!(hd.kind, "nubs" | "horns" | "antennae" | "antlers") {
            hd.spread = Some(r.f(1142, 3));
        }
        if matches!(
            hd.kind,
            "horns" | "antennae" | "tuft" | "sprout" | "antlers"
        ) {
            hd.curl = Some(r.s(1143, 3));
        }
        if matches!(hd.kind, "nubs" | "horns" | "unicorn" | "antlers") {
            hd.yarn = Some(r.pick(
                1144,
                2,
                &[("cream", 1), ("gold", 1), ("accent", 1), ("grey", 1)],
                0,
            ));
        }
        if matches!(hd.kind, "spines" | "plates") {
            hd.count = Some(r.pick(1145, 1, &[(3, 1), (5, 1)], 0));
        }
        if hd.kind == "antlers" {
            hd.tines = Some(r.pick(1146, 1, &[(2, 1), (3, 1)], 0));
        }
    }
    if e.mount == "stalks" {
        e.stalk_len = r.lin(1148, 3, 0.75, 1.25);
        e.stalk_lean = r.f(1149, 3);
    }
    if a.kind != "none" {
        a.len = r.lin(1153, 3, 0.78, 1.28);
        a.width = r.lin(1154, 3, 0.8, 1.25);
        a.pos = r.f(1155, 3);
        a.angle = r.f(1156, 3);
        if matches!(a.kind, "long" | "nub" | "four") {
            a.paw = Some(r.pick(1157, 2, &[("none", 3), ("paw", 1)], 0));
        }
    }
    if l.kind != "none" {
        l.len = r.lin(1161, 3, 0.78, 1.28);
        l.width = r.lin(1162, 3, 0.8, 1.25);
        l.spread = r.f(1163, 3);
        if l.kind == "feet" {
            l.pads = Some(r.pick(1164, 2, &[("none", 2), ("pad", 1), ("beans", 1)], 0));
        }
        if l.kind == "tentacles" {
            l.count = Some(r.pick(1165, 1, &[(4, 1), (6, 1)], 0));
        }
    }
    if tl.kind != "none" {
        tl.size = r.lin(1169, 3, 0.78, 1.28);
        tl.side = if r.bit(1170) { 1.0 } else { -1.0 };
        if matches!(tl.kind, "long" | "bushy" | "curly") {
            tl.curl = Some(r.s(1171, 3));
        }
        if matches!(tl.kind, "long" | "bushy") {
            tl.tip = Some(r.pick(1172, 1, &[("none", 1), ("tipped", 1)], 0));
        }
    }

    let mut colour = Colour {
        accent: r.pick(
            1176,
            3,
            &[
                ("cream", 3),
                ("tint", 2),
                ("shade", 1),
                ("contrast", 1),
                ("charcoal", 1),
            ],
            0,
        ),
        blush: r.u(1178, 2),
        second: None,
    };
    // Stripes are narrow and many, and only on ears, tails and wings.
    if st.kind != "none" {
        colour.second = Some(r.pick(
            1177,
            3,
            &[
                ("cream", 2),
                ("accent", 1),
                ("complement", 2),
                ("analog", 2),
                ("dark", 1),
            ],
            0,
        ));
        st.period = 2 + r.u(1185, 1);
        st.phase = r.u(1186, 1);
    }
    if pt.belly != "none" {
        pt.belly_size = r.lin(1189, 3, 0.48, 0.78);
    }
    if pt.spots == "spots" {
        pt.spot_layout = r.u(1191, 3);
    }
    if pt.mend == "mend" || pt.mend == "heart" {
        pt.mend_side = if r.bit(1193) { 1.0 } else { -1.0 };
    }

    // How this toy carries its moods: the side a tear, a tongue, the Zz and
    // a raised brow take, and which of two ways it shows four of the moods.
    let mood_side = if r.bit(1200) { 1.0 } else { -1.0 };
    let temper = Temper {
        happy: r.pick(1201, 1, &[("arches", 1), ("open", 1)], 0),
        working: r.pick(1202, 1, &[("tongue", 1), ("pressed", 1)], 0),
        blocked: r.pick(1203, 1, &[("teeth", 1), ("zigzag", 1)], 0),
        sad: r.pick(1204, 1, &[("frown", 1), ("wobble", 1)], 0),
    };

    Traits {
        tone,
        hue_wild,
        accent_family,
        plan: plan_name,
        stacked,
        gauge,
        curve,
        drift,
        size,
        main,
        body_p,
        overlap,
        head_ratio,
        ears: x,
        top: hd,
        arms: a,
        legs: l,
        tail: tl,
        stripes: st,
        patches: pt,
        face,
        eyes: e,
        snout: s,
        nose: n,
        mouth: m,
        brows,
        cheeks,
        colour,
        stock_saved,
        mood_side,
        temper,
    }
}
