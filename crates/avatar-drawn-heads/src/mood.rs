//! The marks a face collects with age, and how a head carries its moods.

use crate::beard::Beard;
use crate::face::{BrowPose, Brows, EyePose, Eyes, Mouth, MouthForm, MouthPose, Shut};
use crate::genes::{dna, Genes};
use crate::geom::{max, min, Op};
use crate::head::Head;
use avatar_svg::Mood;

/// Lines, cheeks and a mole.
#[derive(Clone, Debug, Default)]
pub struct Marks {
    /// How old the face is, from 0 (young) to 3 (old).
    pub age: u32,
    /// How many lines run across the forehead.
    pub brow: u32,
    /// Room for lines between the brows and the hairline.
    pub rows: f64,
    /// Lines from the nose past the mouth.
    pub fold: u32,
    /// Lines under the eyes.
    pub bag: u32,
    /// Lines at the corners of the eyes.
    pub crow: bool,
    /// Lines between the brows.
    pub frown: bool,
    /// Lines under the cheekbones.
    pub gaunt: bool,
    /// 1 a cleft chin, 2 a crease, 0 neither.
    pub chin: u32,
    /// Bit 1 a blush, bit 2 freckles.
    pub cheek: u32,
    /// Where a mole sits (0 to 2), or -1 for none.
    pub mole: i32,
}

pub fn make_marks(
    g: &Genes,
    b: &Brows,
    e: &Eyes,
    hair_fore: f64,
    beard: &Beard,
    m: &Mouth,
    hd: &Head,
) -> Marks {
    let o = dna::MARKS;
    let mut k = Marks {
        age: g.pick(o, 0, &[3, 2, 2, 1]) as u32,
        ..Marks::default()
    };
    // Lines across the forehead need room between the brows and the hairline.
    k.rows = ((b.y - b.w / 2.0 - 4.0 - hair_fore) / 5.6).floor();
    if k.age >= 1 {
        if k.rows >= 1.0 {
            let most = if k.age == 1 { 1.0 } else { 2.0 };
            let wanted = f64::from([0, 1, 1, 2][g.raw(o, 3, 2) as usize]);
            k.brow = min(min(k.rows, most), wanted) as u32;
        }
        k.frown = e.x - b.half >= 6.5 && g.flag(o, 5);
    }
    if k.age >= 2 {
        k.fold = [0, 1, 1, 2][g.raw(o + 1, 0, 2) as usize];
        k.bag = [0, 0, 1, 2][g.raw(o + 1, 2, 2) as usize];
        k.crow = g.flag(o + 1, 4);
    }
    if k.age >= 3 {
        k.gaunt = g.flag(o + 1, 5);
    }
    // A young face may blush or be freckled; an older one may still be flushed.
    k.cheek = if k.age <= 1 {
        g.raw(o + 1, 6, 2)
    } else {
        u32::from(g.flag(o + 1, 6))
    };
    let mole = g.raw(o + 2, 0, 3) as i32;
    k.mole = if mole < 5 { -1 } else { mole - 5 };
    if !(beard.covers || m.y > -22.0 * hd.size) {
        k.chin = [0, 0, 1, 2][g.raw(o + 2, 3, 2) as usize];
    }
    k
}

/// How a head carries its moods: which way it tips, and which of the ways
/// to smile, to frown, to wait and to doze it takes.
#[derive(Clone, Copy, Debug)]
pub struct Temper {
    pub tip: f64,
    pub happy_eyes: u32,
    pub happy_mouth: u32,
    pub tear: bool,
    pub blocked: u32,
    pub working: u32,
    pub waiting: u32,
    pub sleepy: u32,
    pub steam: bool,
    pub mark_side: f64,
    pub tip_k: f64,
    pub blush: bool,
    pub sad: bool,
    pub look: f64,
}

pub fn make_temper(g: &Genes) -> Temper {
    let o = dna::TEMPER;
    Temper {
        tip: g.side(o, 0),
        happy_eyes: g.raw(o, 1, 2),
        happy_mouth: g.raw(o, 3, 2),
        tear: g.flag(o, 5),
        blocked: g.raw(o, 6, 2),
        working: g.raw(o + 1, 0, 2),
        waiting: g.raw(o + 1, 2, 2),
        sleepy: g.raw(o + 1, 4, 2),
        steam: g.flag(o + 1, 6),
        mark_side: g.side(o + 1, 7),
        tip_k: g.lin(o + 2, 0, 2, 0.8, 1.25),
        blush: g.flag(o + 2, 2),
        sad: g.flag(o + 2, 3),
        look: g.side(o + 2, 4),
    }
}

/// The mark that floats beside a head in a mood.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    /// Two cogs.
    Busy,
    /// Three dots.
    Dots,
    /// The mark comics draw on a cross forehead.
    Anger,
    /// Zz.
    Zz,
}

/// One mood as numbers.
#[derive(Clone, Debug)]
pub struct Pose {
    pub mood: Mood,
    /// How the head sits on its neck: moved, and tipped clockwise in degrees.
    pub head: [f64; 3],
    /// How far the features slide on the face.
    pub nod: (f64, f64),
    pub eye_l: EyePose,
    pub eye_r: EyePose,
    pub brow_l: BrowPose,
    pub brow_r: BrowPose,
    pub mouth: MouthPose,
    pub mark: Option<MarkKind>,
    pub mark_side: f64,
    /// The side beads of sweat fly off to, or 0.
    pub sweat: f64,
    pub steam: bool,
    pub flush: bool,
    pub blush: bool,
    /// The side a tear runs down, or 0.
    pub tear: f64,
}

pub fn pose_of(t: &Temper, e: &Eyes, mm: &Mouth, trim: f64, mood: Mood) -> Pose {
    let sd = t.tip;
    let k = t.tip_k * trim;
    let eye = EyePose {
        open: e.rest,
        low: 0.0,
        slant: e.slant,
        look: (0.0, 0.0),
        closed: None,
        pupil: 1.0,
    };
    let brow = BrowPose::default();
    let line = MouthPose::default();
    let mut p = Pose {
        mood,
        head: [0.0, 0.0, 0.0],
        nod: (0.0, 0.0),
        eye_l: eye,
        eye_r: eye,
        brow_l: brow,
        brow_r: brow,
        mouth: MouthPose {
            c: mm.curve,
            ..line
        },
        mark: None,
        mark_side: t.mark_side,
        sweat: 0.0,
        steam: false,
        flush: false,
        blush: false,
        tear: 0.0,
    };
    let still = Some(0.0);
    match mood {
        Mood::Neutral => {}
        Mood::Working => {
            // Bent over the work: the head down and to one side, the eyes nearly
            // shut and on the work, the brows drawn together, the tongue out.
            p.head = [0.0, 5.5, -sd * 4.5 * k];
            p.nod = (0.0, 4.5);
            let ey = EyePose {
                open: e.rest * 0.55,
                low: 0.1,
                slant: e.slant + 0.15,
                look: (-sd * 0.25, 0.95),
                ..eye
            };
            p.eye_l = ey;
            p.eye_r = ey;
            p.brow_l = BrowPose {
                raise: if sd < 0.0 { -3.6 } else { -1.4 },
                tilt: 0.16,
                pinch: 2.5,
                bow: 0.0,
            };
            p.brow_r = BrowPose {
                raise: if sd > 0.0 { -3.6 } else { -1.4 },
                tilt: 0.16,
                pinch: 2.5,
                bow: 0.0,
            };
            p.mouth = match t.working {
                0 => MouthPose {
                    c: 0.05,
                    w: 0.72,
                    tongue_out: sd,
                    skew: still,
                    ..line
                },
                1 => MouthPose {
                    c: -0.06,
                    w: 0.5,
                    ticks: true,
                    skew: still,
                    plain: true,
                    dx: sd * mm.w * 0.18,
                    ..line
                },
                2 => MouthPose {
                    c: 0.05,
                    w: 0.72,
                    tongue_out: -sd,
                    skew: still,
                    ..line
                },
                _ => MouthPose {
                    c: 0.12,
                    w: 0.62,
                    tongue_out: sd,
                    skew: still,
                    ..line
                },
            };
            p.sweat = -t.mark_side;
            p.mark = Some(MarkKind::Busy);
        }
        Mood::Happy => {
            p.head = [0.0, -5.0, sd * 3.5 * k];
            p.nod = (0.0, -2.0);
            let shut = EyePose {
                closed: Some(Shut::Happy),
                ..eye
            };
            let wide = |low: f64| EyePose {
                open: 1.0,
                low,
                slant: 0.0,
                ..eye
            };
            match t.happy_eyes {
                0 => {
                    p.eye_l = shut;
                    p.eye_r = shut;
                }
                1 => {
                    p.eye_l = wide(0.5);
                    p.eye_r = wide(0.5);
                }
                2 => {
                    p.eye_l = if sd < 0.0 { shut } else { wide(0.4) };
                    p.eye_r = if sd < 0.0 { wide(0.4) } else { shut };
                }
                _ => {
                    let ey = EyePose {
                        open: max(e.rest, 0.9),
                        low: 0.72,
                        slant: 0.0,
                        ..eye
                    };
                    p.eye_l = ey;
                    p.eye_r = ey;
                }
            }
            let br = BrowPose {
                raise: 3.5,
                bow: 0.08,
                ..brow
            };
            p.brow_l = br;
            p.brow_r = br;
            p.mouth = match t.happy_mouth {
                0 => MouthPose {
                    c: 1.0,
                    w: 1.15,
                    ticks: true,
                    skew: still,
                    ..line
                },
                1 => MouthPose {
                    c: 0.75,
                    w: 1.1,
                    open: 0.62,
                    skew: still,
                    ..line
                },
                2 => MouthPose {
                    c: 0.8,
                    w: 1.15,
                    open: 0.7,
                    teeth: true,
                    skew: still,
                    ..line
                },
                _ => MouthPose {
                    c: 1.15,
                    w: 1.25,
                    skew: Some(0.2),
                    ..line
                },
            };
            p.blush = t.blush;
        }
        Mood::Waiting => {
            let way = t.look;
            p.head = [0.0, 0.0, -way * 6.5 * k];
            p.nod = (way * 1.5, -1.5);
            let ey = EyePose {
                look: (way * 0.85, -0.75),
                ..eye
            };
            p.eye_l = ey;
            p.eye_r = ey;
            p.brow_l = BrowPose {
                raise: if way < 0.0 { 5.0 } else { -0.5 },
                bow: if way < 0.0 { 0.06 } else { 0.0 },
                ..brow
            };
            p.brow_r = BrowPose {
                raise: if way > 0.0 { 5.0 } else { -0.5 },
                bow: if way > 0.0 { 0.06 } else { 0.0 },
                ..brow
            };
            p.mouth = match t.waiting {
                0 => MouthPose {
                    c: 0.0,
                    w: 0.6,
                    dx: way * mm.w * 0.3,
                    skew: Some(0.7 * way * mm.side),
                    ..line
                },
                1 => MouthPose {
                    form: MouthForm::O,
                    open: 0.12,
                    w: 0.5,
                    off: way * 0.2,
                    ..line
                },
                2 => MouthPose {
                    form: MouthForm::Wavy,
                    c: 0.0,
                    w: 0.8,
                    skew: still,
                    ..line
                },
                _ => MouthPose {
                    c: -0.08,
                    w: 0.62,
                    skew: still,
                    plain: true,
                    ..line
                },
            };
            p.mark = Some(MarkKind::Dots);
            p.mark_side = way;
        }
        Mood::Blocked => {
            p.head = [0.0, 4.0, sd * 1.5 * k];
            p.nod = (0.0, 1.5);
            let ey = EyePose {
                open: e.rest * 0.8,
                slant: 0.85,
                low: 0.2,
                look: (0.0, 0.15),
                pupil: 0.86,
                ..eye
            };
            p.eye_l = ey;
            p.eye_r = ey;
            let br = BrowPose {
                raise: -3.5,
                tilt: 0.3,
                pinch: 3.0,
                bow: -0.1,
            };
            p.brow_l = br;
            p.brow_r = br;
            p.mouth = match t.blocked {
                0 => MouthPose {
                    form: MouthForm::Grit,
                    c: -0.5,
                    w: 0.95,
                    skew: still,
                    ..line
                },
                1 => MouthPose {
                    form: MouthForm::Zig,
                    c: -0.3,
                    w: 0.9,
                    skew: still,
                    ..line
                },
                2 => MouthPose {
                    c: -0.95,
                    w: 0.9,
                    ticks: true,
                    skew: still,
                    plain: true,
                    ..line
                },
                _ => MouthPose {
                    c: -0.7,
                    w: 0.85,
                    open: 0.5,
                    tongue: false,
                    skew: still,
                    ..line
                },
            };
            p.mark = Some(MarkKind::Anger);
            p.steam = t.steam;
            p.flush = true;
        }
        Mood::Sad => {
            p.head = [0.0, 7.0, sd * 5.5 * k];
            p.nod = (0.0, 3.5);
            let ey = EyePose {
                open: e.rest * 0.74,
                slant: -0.75,
                look: (0.0, 0.7),
                ..eye
            };
            p.eye_l = ey;
            p.eye_r = ey;
            let br = BrowPose {
                raise: 1.5,
                tilt: -0.3,
                bow: -0.04,
                ..brow
            };
            p.brow_l = br;
            p.brow_r = br;
            p.mouth = if t.sad {
                MouthPose {
                    form: MouthForm::Wavy,
                    c: -0.45,
                    w: 0.8,
                    skew: still,
                    plain: true,
                    ..line
                }
            } else {
                MouthPose {
                    c: -0.8,
                    w: 0.82,
                    skew: still,
                    plain: true,
                    ..line
                }
            };
            p.tear = if t.tear { -sd } else { 0.0 };
        }
        Mood::Sleepy => {
            p.head = [0.0, 9.0, sd * 10.0 * k];
            p.nod = (0.0, 2.5);
            let ey = if t.sleepy == 2 {
                EyePose {
                    open: 0.24,
                    slant: -0.1,
                    look: (0.0, 0.8),
                    ..eye
                }
            } else {
                EyePose {
                    closed: Some(Shut::Sleep),
                    ..eye
                }
            };
            p.eye_l = ey;
            p.eye_r = ey;
            let br = BrowPose {
                raise: -1.0,
                tilt: -0.08,
                ..brow
            };
            p.brow_l = br;
            p.brow_r = br;
            let o = |open: f64, w: f64| MouthPose {
                form: MouthForm::O,
                open,
                w,
                ..line
            };
            p.mouth = match t.sleepy {
                0 => o(0.45, 0.5),
                1 => MouthPose {
                    c: 0.1,
                    w: 0.7,
                    drool: -sd,
                    skew: still,
                    plain: true,
                    ..line
                },
                2 => o(0.62, 0.56),
                _ => o(0.2, 0.4),
            };
            p.mark = Some(MarkKind::Zz);
        }
    }
    p
}

/// The head turns about a point a little above its chin, where it sits on the neck.
pub fn head_ops(hd: &Head, p: &Pose) -> [Op; 3] {
    let pv = hd.h * 0.14;
    [
        Op::T(p.head[0], p.head[1] - pv),
        Op::R(p.head[2]),
        Op::T(0.0, pv),
    ]
}

/// The seven moods, in the order the poses of a [`crate::Creature`] are stored.
pub const MOODS: [Mood; 7] = [
    Mood::Neutral,
    Mood::Working,
    Mood::Happy,
    Mood::Waiting,
    Mood::Blocked,
    Mood::Sad,
    Mood::Sleepy,
];

pub fn mood_name(mood: Mood) -> &'static str {
    match mood {
        Mood::Neutral => "neutral",
        Mood::Working => "working",
        Mood::Happy => "happy",
        Mood::Waiting => "waiting",
        Mood::Blocked => "blocked",
        Mood::Sad => "sad",
        Mood::Sleepy => "sleepy",
    }
}
