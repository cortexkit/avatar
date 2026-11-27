//! Building a toy: its pieces, its face and its sewn-on parts, in toy space.
//!
//! A toy is built once from its traits. Every mood draws the same toy in a
//! different pose, and the fit into the frame covers all seven poses, so the
//! toy keeps its size from mood to mood.

use avatar_svg::Rgb;

use crate::decode::{plan, Traits};
use crate::geom::{
    add, atan2, dir_of, ellipse_pts, flip_x, lerp, max, max_of, min, scl, sdist, sub, Pt, PI,
};
use crate::knit::{Row, Stitch};
use crate::palette::{palette, Palette, Yarn};
use crate::piece::{arc_spine, make_piece, make_tube, mirror_tube, Piece, Tube};

/// The seven moods. Poses and drawings index them in this order.
pub const MOODS: [&str; 7] = [
    "neutral", "working", "happy", "waiting", "blocked", "sad", "sleepy",
];

/// How a part's open end is sewn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Seam {
    #[default]
    None,
    /// Across a tube's base.
    Tube,
    /// Round a flat round's rim.
    Ring,
}

/// One sewn-on or stuffed part, in toy space.
#[derive(Clone, Debug, Default)]
pub struct Part {
    pub id: String,
    pub cls: &'static str,
    pub group: &'static str,
    pub kind: &'static str,
    pub rig: &'static str,
    pub layer: i32,
    pub piece: Option<Piece>,
    pub outline: Vec<Pt>,
    pub yarn: Option<Yarn>,
    pub seam: Seam,
    pub tube: Option<Tube>,
    pub c: Pt,
    pub rx: f64,
    pub ry: f64,
    pub r: f64,
    pub felt: Option<Rgb>,
    pub felt_edge: Option<Rgb>,
    pub vein: bool,
    pub stitched: bool,
    pub cross_stitch: bool,
    pub thread_on: Option<Rgb>,
    pub ringed: Option<Yarn>,
    pub tip_yarn: Option<&'static str>,
    pub tip_rows: f64,
    pub loops: Vec<Vec<Pt>>,
    pub width: f64,
    pub rows: Vec<Row>,
    pub rows_y: Vec<f64>,
    pub yarns: Vec<Yarn>,
    pub stitches: Vec<Stitch>,
    pub edge: Vec<Pt>,
}

impl Part {
    pub fn yarn(&self) -> &Yarn {
        self.yarn.as_ref().expect("a knitted part has a yarn")
    }
}

/// A rig's pose in one mood. Angles are degrees.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pose {
    pub rot: f64,
    pub dx: f64,
    pub dy: f64,
    pub scale: Option<(f64, f64)>,
}

/// A joint the parts on it turn and move about.
#[derive(Clone, Debug)]
pub struct Rig {
    pub name: &'static str,
    pub parent: Option<&'static str>,
    pub pivot: Pt,
    pub pose: [Option<Pose>; 7],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EyeSpot {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub side: f64,
    pub rig: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub struct SnoutSpot {
    pub kind: &'static str,
    pub c: Pt,
    pub rx: f64,
    pub ry: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct NoseSpot {
    pub kind: &'static str,
    pub shape: &'static str,
    pub c: Pt,
    pub s: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct MouthSpot {
    pub y: f64,
    pub w: f64,
    pub kind: &'static str,
    pub curve: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct CheekSpot {
    pub x: f64,
    pub y: f64,
    pub rx: f64,
    pub ry: f64,
}

/// Where the face's features sit.
#[derive(Clone, Debug)]
pub struct Face {
    pub r: f64,
    pub ex: f64,
    pub fw: f64,
    pub eyes: Vec<EyeSpot>,
    pub off_face: bool,
    pub snout: Option<SnoutSpot>,
    pub nose: Option<NoseSpot>,
    pub mouth: MouthSpot,
    pub cheeks: Option<CheekSpot>,
    pub brow_y: f64,
    pub brow_len: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct MountSpec {
    pub kind: &'static str,
    pub th: f64,
    pub rot_k: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct EarSpec {
    pub kind: &'static str,
    pub th: f64,
    pub rot_k: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct ArmSpec {
    pub kind: &'static str,
    pub shoulder: Pt,
}

#[derive(Clone, Copy, Debug)]
pub struct Belly {
    pub c: Pt,
    pub rx: f64,
    pub ry: f64,
}

/// How the toy sits in the 200-unit frame: frame = (ox, oy) + toy * s.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fit {
    pub s: f64,
    pub ox: f64,
    pub oy: f64,
    /// The box round every mood's pose, in toy units.
    pub bbox: [f64; 4],
}

/// A built toy.
#[derive(Clone, Debug)]
pub struct Toy {
    pub t: Traits,
    pub pal: Palette,
    pub parts: Vec<Part>,
    pub rigs: Vec<Rig>,
    pub body: Piece,
    pub head: Option<Piece>,
    pub neck: Option<Pt>,
    pub ground_y: f64,
    pub face: Option<Face>,
    pub mount_spec: Option<MountSpec>,
    pub ear_spec: Option<EarSpec>,
    pub arm_spec: Option<ArmSpec>,
    pub belly: Option<Belly>,
    pub joint_buttons: Vec<(Pt, f64)>,
    pub happy_arm: f64,
    pub fit: Fit,
    /// Row height and stitch width in toy units.
    pub rh: f64,
    pub sw: f64,
}

impl Toy {
    /// The piece the face sits on: the head of a two-piece toy, or the one
    /// piece.
    pub fn face_piece(&self) -> &Piece {
        self.head.as_ref().unwrap_or(&self.body)
    }

    pub fn face(&self) -> &Face {
        self.face.as_ref().expect("the face is laid out")
    }

    pub fn rig(&self, name: &str) -> Option<&Rig> {
        self.rigs.iter().find(|r| r.name == name)
    }

    pub fn rig_mut(&mut self, name: &str) -> Option<&mut Rig> {
        self.rigs.iter_mut().find(|r| r.name == name)
    }

    pub fn add_rig(&mut self, name: &'static str, parent: Option<&'static str>, pivot: Pt) {
        let rig = Rig {
            name,
            parent,
            pivot,
            pose: [None; 7],
        };
        if let Some(existing) = self.rig_mut(name) {
            *existing = rig;
        } else {
            self.rigs.push(rig);
        }
    }

    /// The rig parts on the head hang from: the head on a two-piece toy, the
    /// whole toy otherwise.
    pub fn head_rig(&self) -> &'static str {
        if self.head.is_some() {
            "head"
        } else {
            "root"
        }
    }
}

/// Builds a toy from its traits.
pub fn build(t: Traits) -> Toy {
    let pal = palette(&t);
    let mut g = build_pieces(t, pal);
    // Legs first: they set the ground the toy stands on, which no hanging
    // arm, tentacle or tail may reach below.
    crate::parts::build_legs(&mut g);
    g.ground_y = max(
        g.body.y_bot,
        max_of(
            g.parts
                .iter()
                .filter(|p| p.group == "legs")
                .map(|p| max_of(p.outline.iter().map(|q| q[1]))),
        ),
    );
    layout_face(&mut g);
    build_eye_mounts(&mut g);
    crate::parts::build_ears(&mut g);
    crate::parts::build_top(&mut g);
    crate::parts::build_arms(&mut g);
    crate::parts::build_tail(&mut g);
    crate::parts::build_snout(&mut g);
    crate::parts::build_patches(&mut g);
    crate::pose::build_poses(&mut g);
    crate::pose::fit_frame(&mut g);
    crate::knit::knit_all(&mut g);
    g
}

fn build_pieces(t: Traits, pal: Palette) -> Toy {
    let main_yarn = pal.main.clone();
    let mut parts = Vec::new();
    let (body, head, neck) = if !t.stacked {
        let body = make_piece(&t.main, [0.0, 0.0], true);
        (body, None, None)
    } else {
        let bp = t.body_p.as_ref().expect("a two-piece toy has a body");
        let body = make_piece(bp, [0.0, 0.0], true);
        // The head's half-width is the plan's share of the body's, measured
        // no narrower than 0.45 of the body's height.
        let measure = max(bp.a, 0.45 * (bp.b_t + bp.b_b));
        let k = t.head_ratio * measure / t.main.a;
        let mut hp = t.main.clone();
        hp.a = t.main.a * k;
        hp.b_t = t.main.b_t * k;
        hp.b_b = t.main.b_b * k;
        let probe = make_piece(&hp, [0.0, 0.0], false);
        // The head sits on the body, its lower edge reaching `overlap` of
        // the body's height down over it.
        let hy = body.y_top + t.overlap * body.h - probe.y_bot;
        let head = make_piece(&hp, [0.0, hy], false);
        let neck = [
            0.0,
            min(head.y_bot - head.h * 0.12, body.y_top + body.h * 0.5),
        ];
        (body, Some(head), Some(neck))
    };
    parts.push(Part {
        id: "body".into(),
        cls: "body piece",
        kind: "piece",
        rig: "root",
        layer: 30,
        piece: Some(body.clone()),
        outline: body.outline.clone(),
        yarn: Some(main_yarn.clone()),
        ..Part::default()
    });
    if let Some(head) = &head {
        parts.push(Part {
            id: "head-piece".into(),
            cls: "head piece",
            kind: "piece",
            rig: "head",
            layer: 60,
            piece: Some(head.clone()),
            outline: head.outline.clone(),
            yarn: Some(main_yarn),
            ..Part::default()
        });
    }
    let mut g = Toy {
        t,
        pal,
        parts,
        rigs: Vec::new(),
        body,
        head,
        neck,
        ground_y: 0.0,
        face: None,
        mount_spec: None,
        ear_spec: None,
        arm_spec: None,
        belly: None,
        joint_buttons: Vec::new(),
        happy_arm: 0.0,
        fit: Fit::default(),
        rh: 0.0,
        sw: 0.0,
    };
    g.add_rig("root", None, [0.0, 0.0]);
    let head_pivot = g.neck.unwrap_or([0.0, g.body.y_bot]);
    g.add_rig("head", Some("root"), head_pivot);
    g
}

/// The face sits on the head (or the upper part of a one-piece toy): eyes on
/// a line a little below the middle, the snout or nose between and below
/// them, cheeks under the outer corners of the eyes, brows above. Each part
/// is tested against the piece's outline at a few points and moved in or
/// shrunk, for a bounded number of tries.
fn layout_face(g: &mut Toy) {
    let t = &g.t;
    let p = g.face_piece();
    let e = &t.eyes;
    let pl = plan(t.plan);
    let mut ey = if !t.stacked {
        p.y_top + p.h * (pl.face + 0.06 * t.face.y)
    } else {
        p.c[1] + p.p.b_b * (0.13 + 0.13 * t.face.y)
    };
    let fw0 = p.half_w(ey);
    let k = t.face.scale * pl.face_scale.unwrap_or(1.0);
    // A dark toy's eyes sit on light felt discs, which need more room. So
    // does a pair of dark eyes resting under lids.
    let backed = g.pal.dark && e.kind != "felt";
    let reach = if backed { 1.5 } else { 1.3 };
    let apart = max(
        if backed { 1.55 } else { 1.35 },
        if e.lid != "none" && e.kind != "felt" && e.mount == "face" {
            1.6
        } else {
            0.0
        },
    );
    // Three eyes sit in a row with more than an eye's radius between each
    // two, the middle one a little higher.
    let three = e.count == 3;
    let gap3 = if backed { 3.5 } else { 3.1 };
    let mut r =
        fw0 * e.size * k * if e.count == 1 { 1.65 } else { 1.0 } * if backed { 0.88 } else { 1.0 };
    let mut ex = if e.count == 1 {
        0.0
    } else if three {
        max(fw0 * (0.42 + 0.3 * (e.spacing - 0.3)), r * gap3)
    } else {
        max(fw0 * e.spacing, r * apart)
    };
    let outline = &p.outline;
    let inside = |q: Pt, m: f64| sdist(q, outline) < -m;
    // Fit the eyes: each eye's outer edge and its brow inside the outline.
    for it in 0..60 {
        let side_ok = inside([ex + r * reach, ey], 2.0);
        let top_ok = inside([ex, ey - r * 1.9 - 3.0], 2.0) && inside([ex + r, ey - r * 1.5], 2.0);
        if side_ok && top_ok {
            break;
        }
        if !top_ok && it % 3 == 2 && it < 30 {
            ey += p.h * 0.012;
        } else if ex > r * if three { gap3 } else { apart } && e.count != 1 {
            ex *= 0.97;
        } else {
            r *= 0.97;
        }
    }
    let mut eyes: Vec<EyeSpot> = if e.count == 1 {
        vec![EyeSpot {
            x: 0.0,
            y: ey,
            r,
            side: 0.0,
            ..EyeSpot::default()
        }]
    } else {
        [-1.0, 1.0]
            .iter()
            .map(|&side| EyeSpot {
                x: side * ex,
                y: ey,
                r: r * (1.0 + 0.14 * f64::from(e.wonk) * -side),
                side,
                ..EyeSpot::default()
            })
            .collect()
    };
    if three {
        eyes.insert(
            1,
            EyeSpot {
                x: 0.0,
                y: ey - r * 0.5,
                r,
                side: 0.0,
                rig: None,
            },
        );
    }
    // Eyes on stalks or bumps leave the face, and the rest of the face moves
    // up into their place.
    let off_face = e.mount != "face";
    let lift = if off_face { r * 1.15 } else { 0.0 };
    let s = &t.snout;
    let mut snout = None;
    let mut below = ey + r * 0.9 - lift;
    if s.kind != "none" {
        let (mut sa, mut sb, sy0): (f64, f64, Box<dyn Fn(f64) -> f64>) = match s.kind {
            "muzzle" => {
                let sa = fw0 * s.size * k * 1.08;
                let sy = s.y;
                (
                    sa,
                    sa * s.aspect,
                    Box::new(move |sb2| ey + r * 0.25 + sb2 * (0.62 + 0.45 * sy)),
                )
            }
            "pig" => {
                let sa = fw0 * s.size * k * 0.78;
                let sy = s.y;
                (
                    sa,
                    sa * (0.62 + 0.26 * s.aspect),
                    Box::new(move |sb2| ey + r * 0.45 + sb2 * (0.9 + 0.35 * sy)),
                )
            }
            "bill" => {
                let sa = fw0 * s.size * k * 0.95;
                let sy = s.y;
                (
                    sa,
                    sa * (0.34 + 0.12 * s.aspect),
                    Box::new(move |sb2| ey + r * 0.7 + sb2 * (0.9 + 0.4 * sy)),
                )
            }
            // A beak is a small felt triangle, wider than it is tall.
            _ => {
                let sa = fw0 * s.size * k * 0.55;
                let sy = s.y;
                (
                    sa,
                    sa * (0.6 + 0.3 * s.aspect),
                    Box::new(move |sb2| ey + r * 0.35 + sb2 * (0.7 + 0.3 * sy)),
                )
            }
        };
        // A snout stays a part of the face.
        let widest =
            fw0 * match s.kind {
                "muzzle" => 0.6,
                "pig" => 0.46,
                "bill" => 0.56,
                _ => 0.34,
            } * if off_face { 0.9 } else { 1.0 };
        if sa > widest {
            sb *= widest / sa;
            sa = widest;
        }
        // A snout wider than the gap between the eyes starts below them; a
        // narrow one may rise between them (never under a middle eye).
        let eye_r = max_of(eyes.iter().map(|e| e.r)) * if backed { 1.42 } else { 1.0 };
        let mut clear = if off_face {
            -lift - eye_r
        } else if three || sa > ex - eye_r * 1.1 {
            eye_r * 0.85
        } else {
            -r * 0.2
        };
        let place = |sb: f64, clear: f64| max(sy0(sb) - lift, ey + clear + sb);
        let mut sy = place(sb, clear);
        let fits = |sa: f64, sb: f64, sy: f64| {
            inside([sa, sy], 3.0)
                && inside([0.0, sy + sb], 3.0)
                && inside([sa * 0.7, sy + sb * 0.7], 3.0)
        };
        let mut it = 0;
        while it < 60 && !fits(sa, sb, sy) {
            sa *= 0.97;
            sb *= 0.97;
            if it > 20 {
                clear -= r * 0.05;
            }
            sy = place(sb, clear);
            it += 1;
        }
        snout = Some(SnoutSpot {
            kind: s.kind,
            c: [0.0, sy],
            rx: sa,
            ry: sb,
        });
        below = sy + sb;
    }
    let mut nose = None;
    let mw_base = match snout {
        Some(sn) if sn.kind == "muzzle" => sn.rx * 0.62,
        _ => fw0 * 0.27 * min(1.25, pl.face_scale.unwrap_or(1.0)),
    };
    let mouth_under = |n: &NoseSpot| n.c[1] + n.s * 1.25 + 2.0;
    if t.nose.kind != "none" {
        let ns = fw0 * 0.085 * t.nose.size * k;
        // Under three eyes the nose starts below the raised middle one, and
        // under a single eye below the eye's rim.
        let rim = r * if backed {
            1.42
        } else if e.kind == "felt" {
            1.3
        } else {
            1.14
        };
        let ny = if let Some(sn) = snout {
            sn.c[1] - sn.ry * 0.3
        } else if e.count == 1 {
            ey + rim + ns * 0.95
        } else {
            ey + r * if three { 1.0 } else { 0.55 } + ns * 0.3 - lift
        };
        let n = NoseSpot {
            kind: t.nose.kind,
            shape: t.nose.shape.unwrap_or("triangle"),
            c: [0.0, ny],
            s: ns,
        };
        nose = Some(n);
        // A face too short for an eye, a nose below it and a mouth below
        // that has no nose.
        if e.count == 1
            && snout.is_none()
            && !inside(
                [
                    mw_base * t.mouth.width,
                    mouth_under(&n) + mw_base * t.mouth.width * 0.5,
                ],
                2.0,
            )
        {
            nose = None;
        }
    }
    let my = match (snout, nose) {
        (Some(sn), _) if sn.kind == "muzzle" => sn.c[1] + sn.ry * 0.28,
        (Some(_), _) => below + max(3.0, r * 0.35),
        (None, Some(n)) => mouth_under(&n),
        (None, None) => ey + r * 1.25 - lift,
    };
    // A face whose eyes are elsewhere is carried by its mouth, so it is
    // wider.
    let mut mouth = MouthSpot {
        y: my,
        w: mw_base * t.mouth.width * if off_face { 1.3 } else { 1.0 },
        kind: t.mouth.kind,
        curve: t.mouth.curve.unwrap_or(0.6),
    };
    // Keep the mouth above the face piece's lower edge.
    let mut it = 0;
    while it < 20 && !inside([mouth.w, mouth.y + mouth.w * 0.5], 2.0) {
        mouth.w *= 0.94;
        mouth.y -= 0.6;
        it += 1;
    }
    let ck = &t.cheeks;
    let mut cheeks = None;
    if e.count != 1 || ck.kind != "none" {
        // Cheeks stay small: large ones read as goggles.
        let crx = min(max(r * 0.85, fw0 * 0.13) * ck.size, fw0 * 0.17);
        let cry = crx * 0.62;
        let mut cx = if e.count != 1 {
            ex + r * 0.7
        } else {
            fw0 * 0.5
        };
        let cy = ey + r * 1.25 - lift;
        // Under a single eye the cheeks stand beside the snout, clear of it.
        let beside = match snout {
            Some(sn) if e.count == 1 => sn.rx + crx * 1.1,
            _ => 0.0,
        };
        cx = max(cx, beside);
        let mut it = 0;
        while it < 20 && !inside([cx + crx, cy], 2.0) {
            cx -= crx * 0.08;
            it += 1;
        }
        // A face with no room beside the snout has no cheeks.
        if cx >= beside - crx * 0.15 {
            cheeks = Some(CheekSpot {
                x: cx,
                y: cy,
                rx: crx,
                ry: cry,
            });
        }
    }
    let face = Face {
        r,
        ex,
        fw: fw0,
        eyes,
        off_face,
        snout,
        nose,
        mouth,
        cheeks,
        brow_y: ey - r * 1.5 - 2.2,
        brow_len: r * 1.45 * t.brows.length,
    };
    g.face = Some(face);
}

pub fn ang_delta(a: f64, b: f64) -> f64 {
    let mut d = (b - a) % crate::geom::TAU;
    if d > PI {
        d -= crate::geom::TAU;
    }
    if d < -PI {
        d += crate::geom::TAU;
    }
    d
}

/// A pair of eyes on knitted stalks rising from the head, each ending in a
/// ball that holds the eye, or on two bumps on the crown. Each mount has its
/// own pivot, so a mood can swing it.
fn build_eye_mounts(g: &mut Toy) {
    if !g.face().off_face {
        return;
    }
    let e = g.t.eyes.clone();
    let p = g.face_piece().clone();
    let stalks = e.mount == "stalks";
    let backed = g.pal.dark && e.kind != "felt";
    let r = g.face().r * if stalks { 0.92 } else { 1.0 };
    // The ball or bump must hold the eye, and a dark toy's eye its disc.
    let hold = r * if backed { 1.75 } else { 1.42 };
    let th = (if stalks {
        lerp(0.1, 0.17, e.stalk_lean)
    } else {
        0.17
    }) * PI;
    let (pt, n) = p.at(th);
    let n_ang = atan2(n[1], n[0]);
    let ang = n_ang + ang_delta(n_ang, -PI / 2.0) * if stalks { 0.72 } else { 0.35 };
    let base = min(p.w, p.h * 0.55);
    let mut tb = None;
    let mut eye_c;
    if stalks {
        let w = max(hold * 0.4, base * 0.07);
        let l = base * 0.5 * e.stalk_len;
        let t = make_tube(
            arc_spine(sub(pt, scl(dir_of(ang), w * 0.7)), ang, l, 0.12, None, 0.0),
            &|t| 1.0 - 0.12 * t,
            w,
            false,
        );
        eye_c = add(
            t.spine[t.spine.len() - 1],
            scl(dir_of(ang + 0.12), hold * 0.55),
        );
        tb = Some(t);
    } else {
        // A bump is a ball sewn on behind the head, more than half of it
        // standing above the head's edge.
        eye_c = add(pt, scl(dir_of(ang), hold * 0.42));
    }
    let ball_r = hold;
    // The two mounts must not meet in the middle.
    if eye_c[0] < hold * 1.08 {
        eye_c = [hold * 1.08, eye_c[1]];
    }
    let parent = g.head_rig();
    for side in [1.0, -1.0] {
        let sfx = if side > 0.0 { "r" } else { "l" };
        let rname = if side > 0.0 { "stalkR" } else { "stalkL" };
        g.add_rig(
            rname,
            Some(parent),
            if side > 0.0 { pt } else { flip_x(pt) },
        );
        let c = if side > 0.0 { eye_c } else { flip_x(eye_c) };
        if let Some(t) = &tb {
            let t2 = if side > 0.0 {
                t.clone()
            } else {
                mirror_tube(t)
            };
            g.parts.push(Part {
                id: format!("eye-stalk-{sfx}"),
                cls: "eye-stalk",
                group: "mounts",
                kind: "tube",
                rig: rname,
                layer: 66,
                outline: t2.outline.clone(),
                tube: Some(t2),
                yarn: Some(g.pal.main.clone()),
                seam: Seam::Tube,
                ..Part::default()
            });
        }
        g.parts.push(Part {
            id: format!("{}{sfx}", if stalks { "eye-ball-" } else { "eye-bump-" }),
            cls: if stalks { "eye-ball" } else { "eye-bump" },
            group: "mounts",
            kind: "ring",
            rig: rname,
            layer: if stalks { 67 } else { 28 },
            c,
            rx: ball_r,
            ry: ball_r,
            outline: ellipse_pts(c, ball_r, ball_r, 36, 0.0),
            yarn: Some(g.pal.main.clone()),
            ..Part::default()
        });
    }
    let face = g.face.as_mut().expect("the face is laid out");
    face.eyes = [-1.0, 1.0]
        .iter()
        .map(|&side| EyeSpot {
            x: side * eye_c[0],
            y: eye_c[1],
            r,
            side,
            rig: Some(if side > 0.0 { "stalkR" } else { "stalkL" }),
        })
        .collect();
    g.mount_spec = Some(MountSpec {
        kind: e.mount,
        th,
        rot_k: if stalks { 1.0 } else { 0.25 },
    });
}
