//! Moods: the same grown plant drawn again in another posture.

use crate::maths::{clamp, max, r3, DEG};
use crate::reader::{dna, Reader};

/// The seven moods, in the reference's order.
pub const MOODS: [&str; 7] = [
    "neutral", "working", "happy", "waiting", "blocked", "sad", "sleepy",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Brows {
    Rest,
    None,
    Focus,
    Raised,
    Knit,
    Worry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouthMood {
    Rest,
    Pressed,
    Grin,
    O,
    Grit,
    Down,
    Sleep,
}

/// What a mood does to the plant and to the face.
#[derive(Clone, Copy, Debug)]
struct Base {
    droop: f64,
    spread: f64,
    fold: f64,
    bloom: f64,
    bristle: f64,
    lean: f64,
    tilt: f64,
    shift: f64,
    lift: f64,
    sx: f64,
    sy: f64,
    lid: f64,
    lid_a: f64,
    squint: f64,
    eye: f64,
    brows: Brows,
    mouth: MouthMood,
    gaze: [f64; 2],
}

const fn base(index: usize) -> Base {
    use Brows as B;
    use MouthMood as M;
    #[rustfmt::skip]
    const TABLE: [Base; 7] = [
        Base { droop: 0.0, spread: 1.0, fold: 0.0, bloom: 0.72, bristle: 1.0, lean: 0.0, tilt: 0.0, shift: 0.0, lift: 1.0, sx: 1.0, sy: 1.0, lid: 0.0, lid_a: 0.0, squint: 0.0, eye: 1.0, brows: B::Rest, mouth: M::Rest, gaze: [0.0, 0.0] },
        Base { droop: -0.12, spread: 0.92, fold: 0.1, bloom: 0.62, bristle: 1.0, lean: 0.55, tilt: 3.0, shift: 0.0, lift: 0.93, sx: 1.01, sy: 0.99, lid: 0.22, lid_a: 4.0, squint: 0.1, eye: 0.96, brows: B::Focus, mouth: M::Pressed, gaze: [0.4, 0.5] },
        Base { droop: -0.6, spread: 1.2, fold: 0.0, bloom: 1.08, bristle: 0.8, lean: 0.0, tilt: 0.0, shift: 0.0, lift: 1.12, sx: 0.98, sy: 1.04, lid: 0.0, lid_a: 0.0, squint: 0.45, eye: 1.04, brows: B::None, mouth: M::Grin, gaze: [0.0, -0.1] },
        Base { droop: 0.14, spread: 1.0, fold: 0.1, bloom: 0.62, bristle: 1.0, lean: -0.6, tilt: -6.0, shift: -3.0, lift: 1.0, sx: 1.0, sy: 1.0, lid: 0.0, lid_a: 0.0, squint: 0.0, eye: 1.14, brows: B::Raised, mouth: M::O, gaze: [-0.55, -0.5] },
        Base { droop: -0.2, spread: 1.4, fold: 0.0, bloom: 0.08, bristle: 2.0, lean: 0.0, tilt: 0.0, shift: 0.0, lift: 0.97, sx: 1.04, sy: 0.98, lid: 0.38, lid_a: 14.0, squint: 0.0, eye: 1.0, brows: B::Knit, mouth: M::Grit, gaze: [0.0, 0.0] },
        Base { droop: 1.0, spread: 0.8, fold: 0.3, bloom: 0.3, bristle: 0.7, lean: 0.0, tilt: 0.0, shift: 0.0, lift: 0.8, sx: 1.02, sy: 0.95, lid: 0.3, lid_a: -12.0, squint: 0.0, eye: 0.97, brows: B::Worry, mouth: M::Down, gaze: [0.0, 0.4] },
        Base { droop: 0.4, spread: 0.7, fold: 1.0, bloom: 0.0, bristle: 0.8, lean: 0.0, tilt: 0.0, shift: 0.0, lift: 0.68, sx: 1.02, sy: 0.96, lid: 0.9, lid_a: 0.0, squint: 0.0, eye: 1.0, brows: B::None, mouth: M::Sleep, gaze: [0.0, 0.2] },
    ];
    TABLE[index]
}

/// How one creature carries its moods: no two wilt, lean or smile alike.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Temper {
    pub(crate) wilt: f64,
    pub(crate) side: f64,
    pub(crate) tilt: f64,
    pub(crate) fold_up: bool,
    pub(crate) smile_eyes: bool,
    pub(crate) tongue: bool,
    pub(crate) grit: bool,
    pub(crate) bounce: f64,
    pub(crate) rest: f64,
}

pub(crate) fn build_temper(r: &Reader<'_>) -> Temper {
    let o = dna::TEMPER;
    Temper {
        wilt: r.lin(o, 0.75, 1.2),
        side: if r.chance(o + 1, 0.5) { 1.0 } else { -1.0 },
        tilt: r.lin(o + 2, 0.6, 1.3),
        fold_up: r.chance(o + 3, 0.6),
        smile_eyes: r.chance(o + 4, 0.45),
        tongue: r.chance(o + 5, 0.4),
        grit: r.chance(o + 6, 0.45),
        bounce: r.lin(o + 7, 0.6, 1.4),
        rest: r.lin(o + 8, 0.55, 0.9),
    }
}

/// One mood as this creature carries it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mood {
    /// Index into [`MOODS`].
    pub(crate) index: usize,
    pub(crate) droop: f64,
    pub(crate) spread: f64,
    pub(crate) fold: f64,
    pub(crate) bloom: f64,
    pub(crate) bristle: f64,
    pub(crate) lean: f64,
    pub(crate) tilt: f64,
    pub(crate) shift: f64,
    pub(crate) lift: f64,
    pub(crate) sx: f64,
    pub(crate) sy: f64,
    pub(crate) lid: f64,
    pub(crate) lid_a: f64,
    pub(crate) squint: f64,
    pub(crate) eye: f64,
    pub(crate) brows: Brows,
    pub(crate) mouth: MouthMood,
    pub(crate) gaze: [f64; 2],
    /// Where the ground is, in the body's own coordinates.
    pub(crate) floor: f64,
}

pub(crate) const NEUTRAL: usize = 0;
pub(crate) const WORKING: usize = 1;
pub(crate) const HAPPY: usize = 2;
pub(crate) const BLOCKED: usize = 4;
pub(crate) const SAD: usize = 5;
pub(crate) const SLEEPY: usize = 6;

/// The mood `index` for a creature with this temper, body and roots.
pub(crate) fn mood_of(
    index: usize,
    t: &Temper,
    body_bottom: f64,
    body_w: f64,
    body_h: f64,
    lift: f64,
) -> Mood {
    let b = base(index);
    let droop = if b.droop > 0.0 {
        b.droop * t.wilt
    } else {
        b.droop
    };
    // A body that sits low on short legs has little room to tip.
    let tilt = r3(b.tilt * t.side * t.tilt * clamp(lift / 12.0, 0.45, 1.0));
    let m_lift = 1.0 + (b.lift - 1.0) * t.bounce;
    // Where the ground is: nothing grows below it. A tipped body has one side
    // nearer the ground, and at rest growth also keeps clear of the feet.
    let floor = body_bottom + lift * m_lift
        - (tilt * DEG).abs() * body_w
        - (if b.droop > 0.3 {
            0.0
        } else {
            max(9.0, body_h * 0.1)
        });
    Mood {
        index,
        droop,
        spread: b.spread,
        fold: b.fold,
        bloom: if index == HAPPY {
            b.bloom
        } else {
            b.bloom * t.rest / 0.72
        },
        bristle: b.bristle,
        lean: b.lean * t.side,
        tilt,
        shift: b.shift * t.side,
        lift: m_lift,
        sx: r3(1.0 + (b.sx - 1.0) * t.bounce),
        sy: r3(1.0 + (b.sy - 1.0) * t.bounce),
        lid: b.lid,
        lid_a: b.lid_a,
        squint: b.squint,
        eye: b.eye,
        brows: b.brows,
        mouth: b.mouth,
        gaze: [b.gaze[0] * t.side, b.gaze[1]],
        floor,
    }
}
