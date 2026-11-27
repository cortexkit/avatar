//! The procedural idle layer: breathing, weight shift, glances, blinks and
//! small extra swings, all from the genome's motion bytes.

use avatar_small_robots::GENOME_LEN;

use super::clip::TrackPath;
use super::math::{hash_unit, lerp, sin, smooth, V3};

/// The first of the ten motion bytes, 14336 to 14345. The 2D robot reads
/// none of them, so motion never changes how a robot looks.
pub const MOTION_BYTE: usize = 14336;

/// Seconds from a face change's cause to its swap, when the swap blink has
/// shut the eyes.
pub const SWAP_LEAD: f64 = 0.07;

const TAU: f64 = std::f64::consts::TAU;

/// How one robot moves, read from its motion bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MotionParams {
    /// Scales every idle period: `0.85 + 0.3 · b₀ / 255`.
    pub tempo: f64,
    /// Breathing phase in cycles: `b₁ / 256`.
    pub breath_phase: f64,
    /// Weight-shift phase in cycles: `b₂ / 256`.
    pub sway_phase: f64,
    /// Glance schedule seed: `b₃ | b₄ << 8`.
    pub glance_seed: u32,
    /// Blink schedule seed: `b₅ | b₆ << 8`.
    pub blink_seed: u32,
    /// Seconds between blinks before jitter: `3 + 2.5 · b₇ / 255`.
    pub blink_period: f64,
    /// The side that leads one-sided gestures: +1 (+x) when `b₈` is odd,
    /// else −1.
    pub lead: f64,
    /// Scales idle amplitudes: `0.8 + 0.4 · b₉ / 255`.
    pub energy: f64,
}

impl MotionParams {
    /// The parameters of a genome.
    pub fn new(genome: &[u8; GENOME_LEN]) -> Self {
        let b = |i: usize| f64::from(genome[MOTION_BYTE + i]);
        let u = |i: usize| u32::from(genome[MOTION_BYTE + i]);
        Self {
            tempo: 0.85 + 0.3 * b(0) / 255.0,
            breath_phase: b(1) / 256.0,
            sway_phase: b(2) / 256.0,
            glance_seed: u(3) | (u(4) << 8),
            blink_seed: u(5) | (u(6) << 8),
            blink_period: 3.0 + 2.5 * b(7) / 255.0,
            lead: if genome[MOTION_BYTE + 8] & 1 == 1 {
                1.0
            } else {
                -1.0
            },
            energy: 0.8 + 0.4 * b(9) / 255.0,
        }
    }

    /// The breathing wave at `t`: `sin(2π (t · tempo / 3.6 + breath phase))`.
    pub fn breath(&self, t: f64) -> f64 {
        sin(TAU * (t * self.tempo / 3.6 + self.breath_phase))
    }

    /// The weight-shift wave at `t`: `sin(2π (t · tempo / 7.3 + sway phase))`.
    pub fn sway(&self, t: f64) -> f64 {
        sin(TAU * (t * self.tempo / 7.3 + self.sway_phase))
    }

    /// The glance target at `t`, `(x, y)` each in `[−1, 1]`.
    pub fn glance(&self, t: f64) -> [f64; 2] {
        glance_at(self.glance_seed, t * self.tempo / 5.5)
    }

    /// How shut the idle blinks hold the eyes at `t`, 0 to 1.
    pub fn blink(&self, t: f64) -> f64 {
        let p = self.blink_period;
        let k0 = (t / p).floor() as i64;
        let mut b: f64 = 0.0;
        for k in k0 - 1..=k0 + 1 {
            let tk = k as f64 * p + (hash_unit(self.blink_seed, k) - 0.5) * 0.6 * p;
            b = b.max(blink_shape(t - tk));
            if hash_unit(self.blink_seed, k + 50021) < 0.18 {
                b = b.max(blink_shape(t - tk - 0.32));
            }
        }
        b
    }
}

/// The glance at `x` glance slots: slot `k` looks at a target picked by
/// hashing `k` (35% of slots look straight ahead) and turns to it from the
/// previous slot's over the first 22% of the slot.
pub fn glance_at(seed: u32, x: f64) -> [f64; 2] {
    let k = x.floor();
    let f = x - k;
    let target = |j: i64| -> [f64; 2] {
        if hash_unit(seed, j) < 0.35 {
            return [0.0, 0.0];
        }
        [
            2.0 * hash_unit(seed, j + 7919) - 1.0,
            0.6 * (2.0 * hash_unit(seed, j + 104_729) - 1.0),
        ]
    };
    let k = k as i64;
    let (a, b) = (target(k - 1), target(k));
    let s = smooth(f / 0.22);
    [lerp(a[0], b[0], s), lerp(a[1], b[1], s)]
}

/// One blink `x` seconds after it starts: closing over 0.07 s, shut for
/// 0.03 s, opening over 0.16 s.
pub fn blink_shape(x: f64) -> f64 {
    if !(0.0..=0.26).contains(&x) {
        0.0
    } else if x < 0.07 {
        smooth(x / 0.07)
    } else if x < 0.1 {
        1.0
    } else {
        1.0 - smooth((x - 0.1) / 0.16)
    }
}

/// The blink that hides a face swap, `x` seconds from the swap: the eyes
/// close over 0.06 s, stay shut from 0.01 s before the swap to 0.01 s after
/// it, and open over 0.08 s.
pub fn swap_blink(x: f64) -> f64 {
    if x <= -SWAP_LEAD || x >= 0.09 {
        0.0
    } else if x < -0.01 {
        smooth((x + SWAP_LEAD) / 0.06)
    } else if x <= 0.01 {
        1.0
    } else {
        1.0 - smooth((x - 0.01) / 0.08)
    }
}

/// Which bones the idle formulas move, and how much.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Idle {
    /// The breathing bone. It scales by `(−A b / 2, A b, −A b / 2)` with
    /// `A = breathe_amp · energy` and breath `b`, keeping its volume, and
    /// rises by `breathe_lift · energy · b` model units.
    pub breathe_bone: usize,
    /// Breathing amplitude.
    pub breathe_amp: f64,
    /// Breathing rise in model units.
    pub breathe_lift: f64,
    /// The weight-shift bone: it turns about z by
    /// `sway_degrees · energy · sway` degrees.
    pub sway_bone: usize,
    /// Weight-shift amplitude in degrees.
    pub sway_degrees: f64,
    /// The glancing bone: it turns about y by `yaw · x` degrees and about x
    /// by `−pitch · y` degrees for glance `(x, y)`.
    pub glance_bone: usize,
    /// Glance yaw in degrees.
    pub glance_yaw: f64,
    /// Glance pitch in degrees.
    pub glance_pitch: f64,
    /// Further sine swings.
    pub extra: Vec<IdleExtra>,
}

/// A sine swing `amp · energy · sin(2π (t · tempo / period + phase))`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct IdleExtra {
    /// The bone it moves.
    pub bone: usize,
    /// [`TrackPath::Rotation`] with `amp` in degrees, or
    /// [`TrackPath::Translation`] with `amp` in model units.
    pub path: TrackPath,
    /// Amplitude per axis.
    pub amp: V3,
    /// Period in seconds at tempo 1.
    pub period: f64,
    /// Phase in cycles.
    pub phase: f64,
    /// Whether only a hovering robot makes the swing: it fades out as the
    /// robot lands, by the weight of the moods it lands in.
    pub hover_only: bool,
}
