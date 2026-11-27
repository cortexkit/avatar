//! Moods: how one robot carries each of the seven, as the genes of its
//! temperament choose.

use avatar_svg::Mood;

use crate::face::{BrowKind, Face, Form, MouthKind};
use crate::kit::{Reader, Status};

/// The seven moods, in the order the reference lists them.
pub const MOODS: [Mood; 7] = [
    Mood::Neutral,
    Mood::Working,
    Mood::Happy,
    Mood::Waiting,
    Mood::Blocked,
    Mood::Sad,
    Mood::Sleepy,
];

pub(crate) fn mood_index(m: Mood) -> usize {
    MOODS
        .iter()
        .position(|x| *x == m)
        .expect("every mood is listed")
}

/// The name the reference gives a mood.
pub fn mood_name(m: Mood) -> &'static str {
    [
        "neutral", "working", "happy", "waiting", "blocked", "sad", "sleepy",
    ][mood_index(m)]
}

/// How this robot carries its moods: which of the variants it uses.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Temper {
    happy_eyes: u32,
    happy_arms: u32,
    work_arms: bool,
    work_mouth: bool,
    wait_side: f64,
    block_eyes: bool,
    wait_arms: u32,
    block_mouth: u32,
    block_arms: bool,
    sad_tear: bool,
    sad_mouth: bool,
    steam: bool,
    sleep_eyes: u32,
    sleep_mouth: bool,
    sleep_side: f64,
    wave_side: f64,
    happy_mouth: bool,
    sad_side: f64,
}

const TEMPER: usize = 1116;

pub(crate) fn build_temper(r: &Reader) -> Temper {
    let o = TEMPER;
    let sign = |byte: usize, bit: u32| if r.flag(byte, bit) { 1.0 } else { -1.0 };
    Temper {
        happy_eyes: r.raw(o, 0, 2),
        happy_arms: r.raw(o, 2, 2),
        work_arms: r.flag(o, 4),
        work_mouth: r.flag(o, 5),
        wait_side: sign(o, 6),
        block_eyes: r.flag(o, 7),
        wait_arms: r.raw(o + 1, 0, 2),
        block_mouth: r.raw(o + 1, 2, 2),
        block_arms: r.flag(o + 1, 4),
        sad_tear: r.flag(o + 1, 5),
        sad_mouth: r.flag(o + 1, 6),
        steam: r.flag(o + 1, 7),
        sleep_eyes: r.raw(o + 2, 0, 2),
        sleep_mouth: r.flag(o + 2, 2),
        sleep_side: sign(o + 2, 3),
        wave_side: sign(o + 2, 4),
        happy_mouth: r.flag(o + 2, 5),
        sad_side: sign(o + 2, 6),
    }
}

/// The stroke that stands for a shut eye.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shut {
    ArcUp,
    ArcDown,
    Chevron,
    Line,
}

/// What one eye does in a mood: shutters, slant, arch, where it looks, its
/// scale and its pupil, or a shut eye.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EyeSpec {
    pub(crate) upper: f64,
    pub(crate) slant: f64,
    pub(crate) lower: f64,
    pub(crate) arch: f64,
    pub(crate) look: [f64; 2],
    pub(crate) sx: f64,
    pub(crate) sy: f64,
    pub(crate) pupil: f64,
    pub(crate) closed: Option<Shut>,
}

const OPEN: EyeSpec = EyeSpec {
    upper: 0.0,
    slant: 0.0,
    lower: 0.0,
    arch: 0.0,
    look: [0.0, 0.0],
    sx: 1.0,
    sy: 1.0,
    pupil: 1.0,
    closed: None,
};

fn shut(form: Shut) -> EyeSpec {
    EyeSpec {
        closed: Some(form),
        ..OPEN
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BrowSpec {
    pub(crate) slant: f64,
    pub(crate) lift: f64,
    pub(crate) skew: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MouthSpec {
    pub(crate) c: f64,
    pub(crate) open: f64,
    pub(crate) w: f64,
    pub(crate) form: Option<Form>,
    pub(crate) lit: f64,
}

fn mouth(form: Option<Form>, c: f64, open: f64, w: f64) -> MouthSpec {
    MouthSpec {
        c,
        open,
        w,
        form,
        lit: 0.5,
    }
}

/// Where a hand goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gesture {
    Hang,
    Out,
    Ready,
    Limp,
    Fists,
    Front,
    FrontHigh,
    Chin,
    Hip,
    Up,
    High,
    Shrug,
}

/// The mark that floats beside the head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Busy,
    Dots,
    Anger,
    Zz,
}

/// One mood: how high the robot stands, how its head is held, and what its
/// eyes, mouth, arms, antennae and lights do. Lengths are in frame units,
/// angles in degrees, clockwise.
#[derive(Clone, Debug)]
pub(crate) struct MoodSpec {
    pub(crate) lift: f64,
    pub(crate) lean: f64,
    pub(crate) drop: f64,
    pub(crate) tilt: f64,
    pub(crate) land: bool,
    pub(crate) eye: EyeSpec,
    pub(crate) eye_l: Option<EyeSpec>,
    pub(crate) eye_r: Option<EyeSpec>,
    pub(crate) light: f64,
    pub(crate) screen: f64,
    pub(crate) brows: Option<BrowSpec>,
    pub(crate) mouth: MouthSpec,
    pub(crate) arms: [Gesture; 2],
    pub(crate) grip: f64,
    pub(crate) ant: f64,
    pub(crate) ant_tilt: f64,
    pub(crate) tail: f64,
    pub(crate) status: Status,
    pub(crate) rays: bool,
    pub(crate) lit: bool,
    pub(crate) leds: [Status; 4],
    pub(crate) needle: f64,
    pub(crate) thrust: f64,
    pub(crate) mark: Option<Mark>,
    pub(crate) mark_side: f64,
    pub(crate) blush: bool,
    pub(crate) tear: bool,
    pub(crate) tear_side: f64,
    pub(crate) steam: bool,
    pub(crate) puff: f64,
}

impl MoodSpec {
    /// The eye spec for an eye on `side`.
    pub(crate) fn eye_for(&self, side: f64) -> EyeSpec {
        if side < 0.0 {
            if let Some(e) = self.eye_l {
                return e;
            }
        }
        if side > 0.0 {
            if let Some(e) = self.eye_r {
                return e;
            }
        }
        self.eye
    }

    pub(crate) fn gesture(&self, side: f64) -> Gesture {
        if side < 0.0 {
            self.arms[0]
        } else {
            self.arms[1]
        }
    }
}

pub(crate) fn mood_of(f: &Face, t: &Temper, hover: bool, rest: Gesture, name: Mood) -> MoodSpec {
    use Gesture::*;
    use Status as S;
    let screen = f.screen.is_some();
    let mut m = MoodSpec {
        lift: 0.0,
        lean: 0.0,
        drop: 0.0,
        tilt: 0.0,
        land: false,
        eye: EyeSpec {
            upper: f.rest_lid,
            ..OPEN
        },
        eye_l: None,
        eye_r: None,
        light: 0.9,
        screen: 0.94,
        brows: if f.brow_kind == BrowKind::Mood {
            None
        } else {
            Some(BrowSpec {
                slant: 0.0,
                lift: 0.0,
                skew: 0.0,
            })
        },
        mouth: mouth(None, f.mouth.rest, 0.0, 1.0),
        arms: [rest, rest],
        grip: 0.45,
        ant: 0.0,
        ant_tilt: 0.0,
        tail: 0.0,
        status: S::Idle,
        rays: false,
        lit: false,
        leds: [S::Idle, S::Idle, S::Off, S::Off],
        needle: 0.1,
        thrust: 1.0,
        mark: None,
        mark_side: 1.0,
        blush: false,
        tear: false,
        tear_side: 1.0,
        steam: false,
        puff: 0.0,
    };
    let brow = |slant, lift| {
        Some(BrowSpec {
            slant,
            lift,
            skew: 0.0,
        })
    };
    match name {
        Mood::Working => {
            m.lift = -2.0;
            m.drop = 3.0;
            m.eye = EyeSpec {
                upper: 0.3,
                slant: 0.1,
                lower: 0.2,
                look: [0.0, 0.55],
                sy: 0.96,
                pupil: 0.8,
                ..OPEN
            };
            m.brows = brow(0.14, -1.0);
            // A mouth that is a display shows a progress bar; any other is pressed small.
            m.mouth = if screen || f.mouth.kind == MouthKind::Strip {
                MouthSpec {
                    lit: if t.work_mouth { 0.7 } else { 0.45 },
                    ..mouth(Some(Form::Bar), 0.0, 0.0, 0.9)
                }
            } else if t.work_mouth {
                mouth(Some(Form::O), 0.0, 0.3, 0.42)
            } else {
                mouth(Some(Form::Arc), 0.04, 0.0, 0.55)
            };
            m.arms = if t.work_arms {
                [Front, Chin]
            } else {
                [Front, FrontHigh]
            };
            m.grip = 0.85;
            m.ant = 0.5;
            m.status = S::Warn;
            m.rays = true;
            m.leds = [S::Warn, S::Warn, S::Warn, S::Off];
            m.needle = 0.6;
            m.mark = Some(Mark::Busy);
            m.mark_side = t.wave_side;
            m.thrust = 1.1;
            m.tail = 0.3;
            m.puff = 1.0;
        }
        Mood::Happy => {
            m.lift = if hover { 12.0 } else { 7.0 };
            m.light = 1.0;
            m.screen = 1.0;
            m.eye = match t.happy_eyes {
                0 => shut(Shut::ArcUp),
                2 => EyeSpec {
                    lower: 0.2,
                    arch: 0.8,
                    sx: 1.08,
                    sy: 1.12,
                    pupil: 1.2,
                    ..OPEN
                },
                _ => EyeSpec {
                    lower: 0.42,
                    arch: 1.0,
                    sy: 1.04,
                    pupil: 1.15,
                    ..OPEN
                },
            };
            if t.happy_eyes == 3 {
                if t.wave_side > 0.0 {
                    m.eye_r = Some(shut(Shut::ArcUp));
                } else {
                    m.eye_l = Some(shut(Shut::ArcUp));
                }
            }
            m.brows = if f.brow_kind == BrowKind::Mood {
                None
            } else {
                brow(-0.1, 2.0)
            };
            m.mouth = if t.happy_mouth {
                mouth(None, 1.0, 0.0, 1.15)
            } else {
                mouth(None, 1.0, 0.8, 1.08)
            };
            m.arms = match t.happy_arms {
                0 => [Up, Up],
                1 => [High, High],
                2 => {
                    if t.wave_side > 0.0 {
                        [Out, High]
                    } else {
                        [High, Out]
                    }
                }
                _ => [Shrug, Shrug],
            };
            m.grip = 0.0;
            m.ant = 1.0;
            m.status = S::Bright;
            m.rays = true;
            m.lit = true;
            m.leds = [S::Ok; 4];
            m.needle = 0.9;
            m.blush = true;
            m.thrust = 1.35;
            m.tail = 1.0;
        }
        Mood::Waiting => {
            let s = t.wait_side;
            m.tilt = 7.0 * s;
            m.lean = 1.2 * s;
            m.eye = EyeSpec {
                look: [-s * 0.85, -0.75],
                sy: 1.05,
                ..OPEN
            };
            m.brows = Some(BrowSpec {
                slant: -0.1,
                lift: 2.5,
                skew: -s,
            });
            m.mouth = if t.work_mouth {
                mouth(Some(Form::Arc), 0.0, 0.0, 0.5)
            } else {
                mouth(Some(Form::O), 0.0, 0.2, 0.36)
            };
            m.arms = match t.wait_arms {
                0 => [Hang, Hang],
                1 => {
                    if s > 0.0 {
                        [Hang, Hip]
                    } else {
                        [Hip, Hang]
                    }
                }
                2 => {
                    if s > 0.0 {
                        [Chin, Hang]
                    } else {
                        [Hang, Chin]
                    }
                }
                _ => [Ready, Ready],
            };
            m.ant = 0.2;
            m.ant_tilt = -s * 12.0;
            m.status = S::Warn;
            m.leds = [S::Warn, S::Off, S::Off, S::Off];
            m.needle = -0.2;
            m.mark = Some(Mark::Dots);
            m.mark_side = -s;
        }
        Mood::Blocked => {
            m.lift = -4.0;
            m.drop = 2.0;
            m.eye = if t.block_eyes && screen {
                shut(Shut::Chevron)
            } else {
                EyeSpec {
                    upper: 0.42,
                    slant: 0.42,
                    lower: 0.12,
                    look: [0.0, 0.15],
                    pupil: 0.7,
                    ..OPEN
                }
            };
            m.brows = brow(0.5, -2.0);
            m.mouth = match t.block_mouth {
                0 => mouth(Some(Form::Zig), -0.3, 0.0, 0.9),
                1 => mouth(Some(Form::Arc), -0.75, 0.0, 0.9),
                2 => mouth(Some(Form::Arc), -0.25, 0.0, 0.8),
                _ => mouth(Some(Form::Wave), -0.3, 0.0, 0.95),
            };
            m.arms = if t.block_arms {
                [Shrug, Shrug]
            } else {
                [Fists, Fists]
            };
            m.grip = 1.0;
            m.ant = -0.3;
            m.status = S::Err;
            m.rays = true;
            m.leds = [S::Err; 4];
            m.needle = 1.0;
            m.mark = Some(Mark::Anger);
            m.mark_side = t.wait_side;
            m.steam = t.steam;
            m.thrust = 0.8;
            m.tail = 0.6;
            m.puff = 2.0;
        }
        Mood::Sad => {
            let s = t.sad_side;
            m.lift = -5.0;
            m.drop = 6.0;
            m.tilt = 3.0 * s;
            m.screen = 0.72;
            m.light = 0.62;
            m.eye = EyeSpec {
                upper: 0.34,
                slant: -0.4,
                look: [0.0, 0.5],
                sy: 0.97,
                pupil: 1.1,
                ..OPEN
            };
            m.brows = brow(-0.45, 1.0);
            m.mouth = if t.sad_mouth {
                mouth(Some(Form::Wave), -0.4, 0.0, 0.85)
            } else {
                mouth(Some(Form::Arc), -0.85, 0.0, 0.82)
            };
            m.arms = [Limp, Limp];
            m.grip = 0.5;
            m.ant = -1.0;
            m.status = S::Cold;
            m.leds = [S::Cold, S::Off, S::Off, S::Off];
            m.needle = -0.7;
            m.tear = t.sad_tear;
            m.tear_side = s;
            m.thrust = 0.55;
            m.tail = -1.0;
        }
        Mood::Sleepy => {
            let s = t.sleep_side;
            m.lift = -8.0;
            m.land = true;
            m.drop = 5.0;
            m.tilt = 8.0 * s;
            m.screen = 0.4;
            m.light = 0.28;
            m.eye = match t.sleep_eyes {
                0 | 3 => shut(Shut::ArcDown),
                1 => shut(Shut::Line),
                _ => EyeSpec {
                    upper: 0.76,
                    slant: -0.06,
                    lower: 0.08,
                    look: [0.0, 0.3],
                    ..OPEN
                },
            };
            m.brows = None;
            m.mouth = if t.sleep_mouth {
                mouth(Some(Form::O), 0.0, 0.25, 0.4)
            } else {
                mouth(Some(Form::Arc), 0.05, 0.0, 0.34)
            };
            m.arms = [Limp, Limp];
            m.grip = 0.5;
            m.ant = -1.0;
            m.status = S::Off;
            m.leds = [S::Off; 4];
            m.needle = -1.0;
            m.mark = Some(Mark::Zz);
            m.mark_side = -s;
            m.thrust = 0.0;
            m.tail = -1.0;
        }
        Mood::Neutral => {}
    }
    m
}
