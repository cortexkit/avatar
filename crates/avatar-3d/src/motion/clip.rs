//! Keyframed clips: the five reactions and the four status loops, as data a
//! glTF animation could carry.
//!
//! A track holds keys for one bone's rotation, translation or scale.
//! Rotation values are XYZ Euler angles in radians relative to the rest
//! pose, translations are offsets in model units, and scales are factors
//! minus one. Keys are joined by cubic Hermite curves whose tangents are the
//! slope between the neighbouring keys (Catmull-Rom), set to zero at the first
//! and last key and wherever a key is a local extreme, so the motion never
//! overshoots a key. A looping clip takes its end tangents across the seam.

use avatar_small_robots::Mood;

use super::math::{smooth, V3};

/// Which of a bone's transforms a track drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TrackPath {
    /// XYZ Euler angles in radians.
    Rotation,
    /// An offset in model units.
    Translation,
    /// Scale factors minus one.
    Scale,
}

/// How a track combines with the layers below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// Added on top: rotations multiply on, translations add, scales
    /// multiply as `1 + w·s`.
    Add,
    /// An absolute pose relative to rest that the clip blends toward by its
    /// envelope, taking the bone over from whatever the layers below had it
    /// doing. It holds its first key before it and its last key after it.
    Set,
}

/// The keys of one bone transform.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Track {
    /// Index into the robot's bones.
    pub bone: usize,
    /// What the values drive.
    pub path: TrackPath,
    /// How the track combines.
    pub mode: Mode,
    /// Key times in seconds from the clip's start, increasing.
    pub times: Vec<f64>,
    /// One value per key.
    pub values: Vec<V3>,
}

impl Track {
    /// A track from `(time, value)` keys.
    pub(crate) fn new(bone: usize, path: TrackPath, mode: Mode, keys: &[(f64, V3)]) -> Self {
        Self {
            bone,
            path,
            mode,
            times: keys.iter().map(|k| k.0).collect(),
            values: keys.iter().map(|k| k.1).collect(),
        }
    }

    /// The value at `t` seconds into a clip of `duration`, looping if
    /// `looping`.
    pub fn sample(&self, t: f64, looping: bool, duration: f64) -> V3 {
        let mut out = [0.0; 3];
        for (c, v) in out.iter_mut().enumerate() {
            *v = sample(
                self.times.len(),
                &|i| self.times[i],
                &|i| self.values[i][c],
                t,
                looping,
                duration,
            );
        }
        out
    }
}

/// Scalar keys `(time, value)`, joined like a track's.
pub type Keys = Vec<[f64; 2]>;

/// The value of scalar keys at `t`, on the curve tracks use, not looping.
pub fn sample_keys(keys: &[[f64; 2]], t: f64) -> f64 {
    sample(keys.len(), &|i| keys[i][0], &|i| keys[i][1], t, false, 0.0)
}

/// The slope at key `i` of one component: zero at a local extreme and at
/// the ends of a clip that does not loop.
fn slope(
    n: usize,
    time: &dyn Fn(usize) -> f64,
    value: &dyn Fn(usize) -> f64,
    i: usize,
    looping: bool,
    dur: f64,
) -> f64 {
    let (ta, va, tb, vb);
    if i > 0 && i < n - 1 {
        ta = time(i - 1);
        va = value(i - 1);
        tb = time(i + 1);
        vb = value(i + 1);
    } else if looping && n > 2 {
        ta = if i == n - 1 {
            time(n - 2)
        } else {
            time(n - 2) - dur
        };
        va = value(n - 2);
        tb = time(1) + if i == 0 { 0.0 } else { dur };
        vb = value(1);
    } else {
        return 0.0;
    }
    let v = value(i);
    if (v - va) * (vb - v) <= 0.0 {
        return 0.0;
    }
    (vb - va) / (tb - ta)
}

fn sample(
    n: usize,
    time: &dyn Fn(usize) -> f64,
    value: &dyn Fn(usize) -> f64,
    t: f64,
    looping: bool,
    dur: f64,
) -> f64 {
    let t = if !looping {
        t
    } else if t % dur < 0.0 {
        t % dur + dur
    } else {
        t % dur
    };
    if n == 1 || t <= time(0) {
        return value(0);
    }
    if t >= time(n - 1) {
        return value(n - 1);
    }
    let mut i = 0;
    while i < n - 2 && t >= time(i + 1) {
        i += 1;
    }
    let h = time(i + 1) - time(i);
    let u = (t - time(i)) / h;
    let (u2, u3) = (u * u, u * u * u);
    let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
    let h10 = u3 - 2.0 * u2 + u;
    let h01 = -2.0 * u3 + 3.0 * u2;
    let h11 = u3 - u2;
    h00 * value(i)
        + h10 * h * slope(n, time, value, i, looping, dur)
        + h01 * value(i + 1)
        + h11 * h * slope(n, time, value, i + 1, looping, dur)
}

/// The scalar a clip reads best by: the one value a film or a renderer
/// samples to find the clip's moment, which is largest at [`Clip::peak`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Channel {
    /// One component of one of the clip's tracks, times `sign`.
    Track {
        /// Index into [`Clip::tracks`].
        track: usize,
        /// 0, 1 or 2: x, y or z.
        component: usize,
        /// +1, or −1 for a value that reads best at its most negative, such
        /// as a squash.
        sign: f64,
    },
    /// The clip's [`Clip::blink`] keys.
    Blink,
    /// The clip's [`Clip::wide`] keys.
    Wide,
}

/// A reaction or a status loop.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Clip {
    /// `hello`, `ask`, `done`, `blocked`, `poke`, or `loop.` and a status
    /// loop's name.
    pub name: &'static str,
    /// Length in seconds; a loop's period.
    pub duration: f64,
    /// Whether the clip repeats.
    pub looping: bool,
    /// Seconds the clip's envelope takes to rise and to fall:
    /// `smooth(t / in) · smooth((duration − t) / out)`.
    pub envelope: [f64; 2],
    /// Bone tracks.
    pub tracks: Vec<Track>,
    /// Eyes shut, 0 to 1; empty when the clip does not blink.
    pub blink: Keys,
    /// The light channel: emissive strength of glow- and status-painted
    /// materials, 0 to 1; empty when the clip leaves the lights alone.
    pub glow: Keys,
    /// Eyes opened wider, 0 to 1; empty when unused. The 3D robot's face
    /// textures are drawn at rest size, so no pose output carries it yet.
    pub wide: Keys,
    /// The mood whose face the clip shows while it holds.
    pub face: Option<Mood>,
    /// The time in seconds from the start at which the clip reads best.
    pub peak: Option<f64>,
    /// The value that reads the clip's peak.
    pub channel: Option<Channel>,
}

impl Clip {
    /// The envelope at `t` seconds from the start.
    pub fn envelope_at(&self, t: f64) -> f64 {
        let [a, b] = self.envelope;
        smooth(t / a) * smooth((self.duration - t) / b)
    }

    /// The track driving `path` of `bone` in `mode`, if the clip has one.
    pub fn track(&self, bone: usize, path: TrackPath, mode: Mode) -> Option<&Track> {
        self.tracks
            .iter()
            .find(|t| t.bone == bone && t.path == path && t.mode == mode)
    }

    /// The value of the clip's [`Channel`] at `t` seconds from the start.
    pub fn channel_at(&self, t: f64) -> Option<f64> {
        Some(match self.channel? {
            Channel::Track {
                track,
                component,
                sign,
            } => sign * self.tracks[track].sample(t, self.looping, self.duration)[component],
            Channel::Blink => sample_keys(&self.blink, t),
            Channel::Wide => sample_keys(&self.wide, t),
        })
    }
}
