//! Faces on the GPU: which face sets each robot keeps resident, the edge
//! its texture layers are drawn at, and the mood its face shows.
//!
//! A robot is built with one mood's face set. Every other set is
//! rasterized from the genome on a worker when it is first needed, and
//! released once unused for [`RELEASE_AFTER`] seconds of stage time. Its
//! face changes only to a resident set, and only under a blink, so a frame
//! never shows a missing face, two faces, or lights from another mood.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use avatar_3d::motion::math::{cos, sin};
use avatar_3d::motion::{mood_index, Pose};
use avatar_3d::robot::camera::Camera;
use avatar_3d::robot::{MoodSet, Robot, TextureContent, TEXTURE_PX_MAX, TEXTURE_PX_MIN, UNIT};
use avatar_small_robots::{Mood, MOODS};

use crate::renderer::LayerChange;

/// The edge in pixels a robot is built at before any frame has worked out
/// the edge it is drawn at.
pub const FIRST_EDGE: u32 = 128;

/// Seconds of stage time a face set stays resident after it was last drawn
/// or needed. The drawn mood's set is never released.
pub const RELEASE_AFTER: f64 = 10.0;

/// Seconds of stage time after which a face set that is resident and asked
/// for switches in without a blink.
pub const LATE_AFTER: f64 = 1.0;

/// Seconds of stage time a held command waits for its face set at most.
pub const HOLD_AT_MOST: f64 = 0.5;

/// Seconds of stage time a robot's edge target must stay at or below half
/// its edge before its layers are drawn again smaller.
pub const SHRINK_AFTER: f64 = 2.0;

/// The blink at and above which the blinking textures show, and a face may
/// switch.
pub const SHUT: f64 = 0.5;

/// The texture edge a robot drawn through `camera` with its model origin at
/// `offset` needs, on a frame `height_px` device pixels high.
///
/// It is the smallest power of two from 64 to 512 at least as large as the
/// largest canvas of the robot's texture bindings, projected: the canvas
/// edge (robot units) times [`UNIT`] times the view's device pixels per
/// model unit at the depth of the nearest corner of the robot's
/// [`reach`](Robot::reach) box placed at `offset`. A box reaching behind
/// the camera needs the largest edge. The first call on a robot works out
/// its reach box, which takes a while, so the stage asks only for robots
/// whose builds already worked it out.
pub fn edge_target(robot: &Robot, camera: &Camera, offset: [f64; 3], height_px: u32) -> u32 {
    let reach = robot.reach();
    let depth = (0..8)
        .map(|i| {
            let corner: [f64; 3] = std::array::from_fn(|k| {
                offset[k]
                    + if i >> k & 1 == 0 {
                        reach.lo[k]
                    } else {
                        reach.hi[k]
                    }
            });
            camera.project(corner)[2]
        })
        .fold(f64::INFINITY, f64::min);
    let extent = robot
        .textures
        .iter()
        .map(|t| t.binding.extent)
        .max()
        .unwrap_or(0);
    if extent == 0 {
        return TEXTURE_PX_MIN;
    }
    if depth.is_nan() || depth <= 0.0 {
        return TEXTURE_PX_MAX;
    }
    let half = 0.5 * camera.fov_y_degrees.to_radians();
    let per_unit = f64::from(height_px) * cos(half) / (2.0 * depth * sin(half));
    edge_for(f64::from(extent) * UNIT * per_unit)
}

/// The smallest power of two from 64 to 512 at least `projected`.
fn edge_for(projected: f64) -> u32 {
    let mut edge = TEXTURE_PX_MIN;
    while edge < TEXTURE_PX_MAX && f64::from(edge) < projected {
        edge *= 2;
    }
    edge
}

/// The mood of a face or stalk-eye texture; `None` for marks and chests.
fn mood_of(content: TextureContent) -> Option<Mood> {
    match content {
        TextureContent::Face { mood, .. } | TextureContent::StalkEye { mood, .. } => Some(mood),
        _ => None,
    }
}

/// The moods of a set of seven flags in `MOODS` order.
type Moods = [bool; 7];

fn moods_of(set: &Moods) -> impl Iterator<Item = Mood> + '_ {
    MOODS
        .iter()
        .zip(set)
        .filter(|(_, on)| **on)
        .map(|(m, _)| *m)
}

/// Every binding resident when the face sets of `moods` are: the marks,
/// the chest and those sets', in binding order.
fn resident_bindings(robot: &Robot, moods: &Moods) -> Vec<usize> {
    robot
        .textures
        .iter()
        .enumerate()
        .filter(|(_, t)| match mood_of(t.binding.content) {
            Some(m) => moods[mood_index(m)],
            None => true,
        })
        .map(|(i, _)| i)
        .collect()
}

/// Work on a robot's textures that a frame asks a worker for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Ask {
    /// One mood's face set at an edge.
    Faces { mood: Mood, edge: u32 },
    /// The marks, chest and these face sets at a new edge.
    Layers { moods: MoodSet, edge: u32 },
}

/// Pixels of some bindings.
type Pixels = Vec<(usize, Vec<u8>)>;

/// A move to a new edge: the layers rasterized at it so far.
struct EdgeChange {
    edge: u32,
    /// The marks, chest and the face sets of `moods`, once they land.
    base: Option<Pixels>,
    moods: Moods,
    /// Face sets rasterized at `edge` since, by mood.
    extra: Vec<(Mood, Pixels)>,
}

impl EdgeChange {
    /// Whether this change holds `mood`'s set, or has asked for it.
    fn covers(&self, mood: Mood) -> bool {
        self.moods[mood_index(mood)] || self.extra.iter().any(|(m, _)| *m == mood)
    }
}

/// The layer change a frame makes, until the renderer has made it.
pub(crate) struct Plan {
    change: LayerChange,
    kept: Kept,
}

/// What a [`Plan`] leaves resident, for [`Faces::commit`].
pub(crate) struct Kept {
    resident: Moods,
    edge: u32,
}

impl Plan {
    /// The change for the renderer, and what to commit once it is made.
    pub fn split(self) -> (LayerChange, Kept) {
        (self.change, self.kept)
    }
}

/// One robot's face sets and layers.
pub(crate) struct Faces {
    /// The edge every layer has.
    edge: u32,
    /// The moods whose face sets are resident.
    resident: Moods,
    /// When each resident set became drawable, in stage time.
    drawable_at: [f64; 7],
    /// When each mood was last drawn or needed, in stage time.
    last_used: [f64; 7],
    /// Face sets asked of a worker and not landed yet, with their edge.
    in_flight: Vec<(Mood, u32)>,
    /// Face sets landed at the current edge, installed at the next frame's
    /// start.
    landed: Vec<(Mood, Pixels)>,
    /// The mood whose face set the head and stalk eyes show.
    drawn: Mood,
    /// The face the pose asked for on the latest drawn frame, and since
    /// when it has.
    requested: Mood,
    requested_at: f64,
    /// The edge the robot's layers should have.
    want: u32,
    /// Since when the edge target has stayed at or below half the edge.
    low_since: Option<f64>,
    /// The one move to a new edge in progress.
    change: Option<EdgeChange>,
    /// Cancels the robot's work still queued once it is retired.
    pub cancel: Arc<AtomicBool>,
}

impl Faces {
    /// The faces of a robot just uploaded at `t`, built in `mood` at `edge`,
    /// whose resident sets are those of its textures that had pixels.
    pub fn new(robot: &Robot, edge: u32, mood: Mood, t: f64) -> Self {
        let mut resident = [true; 7];
        for texture in &robot.textures {
            if let Some(m) = mood_of(texture.binding.content) {
                resident[mood_index(m)] &= texture.pixels.is_some();
            }
        }
        Self {
            edge,
            resident,
            drawable_at: [t; 7],
            last_used: [t; 7],
            in_flight: Vec::new(),
            landed: Vec::new(),
            drawn: mood,
            requested: mood,
            requested_at: t,
            want: edge,
            low_since: None,
            change: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The edge every layer has.
    pub fn edge(&self) -> u32 {
        self.edge
    }

    /// The mood the head and stalk eyes show.
    pub fn drawn(&self) -> Mood {
        self.drawn
    }

    /// Whether `mood`'s face set is resident, and so drawable.
    pub fn drawable(&self, mood: Mood) -> bool {
        self.resident[mood_index(mood)]
    }

    /// Starts drawing the robot at `t`, in `mood`, whose set is resident.
    pub fn show(&mut self, mood: Mood, t: f64) {
        self.drawn = mood;
        self.requested = mood;
        self.requested_at = t;
    }

    /// Takes a face set a worker rasterized.
    pub fn land_faces(&mut self, mood: Mood, edge: u32, pixels: Pixels) {
        if let Some(i) = self.in_flight.iter().position(|a| *a == (mood, edge)) {
            self.in_flight.swap_remove(i);
        }
        if edge == self.edge {
            if !self.drawable(mood) && self.landed.iter().all(|(m, _)| *m != mood) {
                self.landed.push((mood, pixels));
            }
        } else if let Some(change) = self.change.as_mut().filter(|c| c.edge == edge) {
            if !change.covers(mood) {
                change.extra.push((mood, pixels));
            }
        }
    }

    /// Takes the layers a worker rasterized at a new edge. A change whose
    /// edge is no longer the one wanted is dropped, so a slow change never
    /// replaces a later one.
    pub fn land_layers(&mut self, edge: u32, pixels: Pixels) {
        let Some(change) = self
            .change
            .as_mut()
            .filter(|c| c.edge == edge && c.base.is_none())
        else {
            return;
        };
        if edge == self.want {
            change.base = Some(pixels);
        } else {
            self.change = None;
        }
    }

    /// The layer change to make at the start of the frame at `t`, if any:
    /// a move to a new edge once every set kept is rasterized at it, or
    /// else the sets landed since and the release of each set neither drawn
    /// nor needed for [`RELEASE_AFTER`] s.
    pub fn plan(&mut self, robot: &Robot, t: f64) -> Option<Plan> {
        let kept: Moods = std::array::from_fn(|i| {
            self.resident[i] && (MOODS[i] == self.drawn || t - self.last_used[i] < RELEASE_AFTER)
        });
        if let Some(change) = self.change.as_mut() {
            if change.base.is_some() && change.edge != self.want {
                self.change = None;
            } else if change.base.is_some() && moods_of(&kept).all(|m| change.covers(m)) {
                let change = self.change.take().expect("checked above");
                // Sets needed lately that the new edge already holds come
                // along, though not resident at the old edge yet.
                let resident: Moods = std::array::from_fn(|i| {
                    kept[i] || (change.covers(MOODS[i]) && t - self.last_used[i] < RELEASE_AFTER)
                });
                return Some(Self::move_to(robot, change, resident));
            }
        }
        let mut resident = kept;
        for (mood, _) in &self.landed {
            resident[mood_index(*mood)] = true;
        }
        if resident == self.resident {
            return None;
        }
        let mut fresh: Pixels = self.landed.drain(..).flat_map(|(_, p)| p).collect();
        fresh.sort_by_key(|(i, _)| *i);
        Some(Plan {
            change: LayerChange {
                edge: self.edge,
                resident: resident_bindings(robot, &resident),
                fresh,
            },
            kept: Kept {
                resident,
                edge: self.edge,
            },
        })
    }

    /// The change that moves the layers of the marks, the chest and the
    /// face sets of `resident` to a new edge.
    fn move_to(robot: &Robot, change: EdgeChange, resident: Moods) -> Plan {
        let bindings = resident_bindings(robot, &resident);
        let mut fresh: Pixels = change
            .base
            .into_iter()
            .flatten()
            .chain(change.extra.into_iter().flat_map(|(_, p)| p))
            .filter(|(i, _)| bindings.binary_search(i).is_ok())
            .collect();
        fresh.sort_by_key(|(i, _)| *i);
        fresh.dedup_by_key(|(i, _)| *i);
        Plan {
            change: LayerChange {
                edge: change.edge,
                resident: bindings,
                fresh,
            },
            kept: Kept {
                resident,
                edge: change.edge,
            },
        }
    }

    /// Records that the renderer made a plan's change at the frame at `t`.
    pub fn commit(&mut self, kept: Kept, t: f64) {
        for (i, on) in kept.resident.iter().enumerate() {
            if *on && !self.resident[i] {
                self.drawable_at[i] = t;
                self.last_used[i] = t;
            }
        }
        self.resident = kept.resident;
        if kept.edge != self.edge {
            self.edge = kept.edge;
            self.landed.clear();
        }
    }

    /// Forgets the sets landed for a change the renderer refused; they are
    /// asked for again while needed.
    pub fn refused(&mut self) {
        self.landed.clear();
    }

    /// Applies the drawn-mood rule on a drawn frame at `t` whose pose is
    /// `pose`: the drawn mood becomes the pose's face once its set is
    /// resident, on a frame whose blink is at least [`SHUT`], or
    /// [`LATE_AFTER`] s after the later of the pose asking for it and its
    /// set becoming drawable. Returns whether it switched late, off a
    /// blink.
    pub fn choose(&mut self, pose: &Pose, t: f64) -> bool {
        if pose.face != self.requested {
            self.requested = pose.face;
            self.requested_at = t;
        }
        if pose.face == self.drawn || !self.drawable(pose.face) {
            return false;
        }
        if pose.blink >= SHUT {
            self.drawn = pose.face;
            return false;
        }
        let since = self
            .requested_at
            .max(self.drawable_at[mood_index(pose.face)]);
        if t - since >= LATE_AFTER {
            self.drawn = pose.face;
            return true;
        }
        false
    }

    /// Updates the edge the layers should have from this frame's target at
    /// `t`: `None` for a robot not drawn, or drawn where no target is worked
    /// out, which keeps its edge. A larger target is wanted at once, and a
    /// target at or below half the edge once it has stayed there for
    /// [`SHRINK_AFTER`] s.
    pub fn aim(&mut self, target: Option<u32>, t: f64) {
        match target {
            Some(target) if target > self.edge => {
                self.want = target;
                self.low_since = None;
            }
            Some(target) if 2 * target <= self.edge => {
                let since = *self.low_since.get_or_insert(t);
                self.want = if t - since >= SHRINK_AFTER {
                    target
                } else {
                    self.edge
                };
            }
            _ => {
                self.want = self.edge;
                self.low_since = None;
            }
        }
    }

    /// Marks `needed` as used at `t`, and returns the work to ask for: each
    /// needed set not resident or on its way, and, while the wanted edge
    /// differs from the edge, the move to it, one at a time, with each set
    /// it must keep.
    pub fn asks(&mut self, needed: &[Mood], t: f64) -> Vec<Ask> {
        let mut asks = Vec::new();
        for &mood in needed {
            let i = mood_index(mood);
            self.last_used[i] = t;
            let coming = self.landed.iter().any(|(m, _)| *m == mood)
                || self.in_flight.contains(&(mood, self.edge));
            if !self.resident[i] && !coming {
                self.in_flight.push((mood, self.edge));
                asks.push(Ask::Faces {
                    mood,
                    edge: self.edge,
                });
            }
        }
        // The sets a new edge must hold: those kept resident, and those
        // needed now.
        let wanted: Moods = std::array::from_fn(|i| {
            let kept = self.resident[i]
                && (MOODS[i] == self.drawn || t - self.last_used[i] < RELEASE_AFTER);
            kept || needed.iter().any(|m| mood_index(*m) == i)
        });
        if self.change.is_none() && self.want != self.edge {
            let moods = wanted;
            self.change = Some(EdgeChange {
                edge: self.want,
                base: None,
                moods,
                extra: Vec::new(),
            });
            asks.push(Ask::Layers {
                moods: moods_of(&moods).collect(),
                edge: self.want,
            });
        }
        if let Some(change) = &self.change {
            for mood in moods_of(&wanted) {
                let edge = change.edge;
                if !change.covers(mood) && !self.in_flight.contains(&(mood, edge)) {
                    self.in_flight.push((mood, edge));
                    asks.push(Ask::Faces { mood, edge });
                }
            }
        }
        asks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_powers_of_two_from_64_to_512() {
        assert_eq!(edge_for(0.0), 64);
        assert_eq!(edge_for(64.0), 64);
        assert_eq!(edge_for(64.5), 128);
        assert_eq!(edge_for(300.0), 512);
        assert_eq!(edge_for(5000.0), 512);
        assert_eq!(edge_for(f64::NAN), 64);
    }
}
