//! Motion: an actor turns a robot, the events it is given (each with its
//! start time) and a time into a pose.
//!
//! A pose has these outputs:
//!
//! - model-space bone matrices, joint to model, in bone order: a rigid part
//!   follows `M_b · R_b⁻¹`, a skinned vertex `Σ w_j M_j R_j⁻¹` (see
//!   [`crate::robot`]);
//! - the face set: the mood whose face and stalk-eye textures show;
//! - blink, 0 to 1: the blinking textures show when it is at least 0.5,
//!   on the face and the stalk eyes together, and the face set changes only
//!   while they show;
//! - glow, 0 to 1: the emissive strength of glow- and status-painted
//!   materials, without changing their colour;
//! - the status, read in the status colour table for the status lights, and
//!   the four chest LEDs' statuses;
//! - `lit` and `rays`, which pick the bulb's and the beacon's entries;
//! - thrust: the hover jet's emissive scale.
//!
//! The status, LEDs, `lit` and `rays` are always the face set's 2D mood's
//! own, so they switch with the face on the same blink and every frame
//! shows a combination the 2D robot draws.
//!
//! # Layers
//!
//! A pose is built from the bottom up: each mood's static posture by its
//! blend weight, the status loop, the procedural idle, the reaction's
//! additive tracks and its set tracks, then the floor rule
//! ([`Actor::evaluate`]). Every number comes from the creature, its events
//! and the time asked for, through the exact arithmetic operations and the
//! portable sine and arctangent of [`avatar_svg::wild::math`], so a pose is
//! the same bytes on every platform.

mod actor;
mod clip;
mod floor;
mod idle;
pub mod math;
mod rig;

pub use actor::{
    mood_index, Actor, ActorState, IDLE_BLEND, LOOP_BLEND, MOOD_BLEND, REACT_CUT, SWAP_TAIL,
};
pub use clip::{sample_keys, Channel, Clip, Keys, Mode, Track, TrackPath};
pub use floor::HAND_CLEARANCE;
pub use idle::{
    blink_shape, glance_at, swap_blink, Idle, IdleExtra, MotionParams, MOTION_BYTE, SWAP_LEAD,
};
pub use rig::{
    ArmRig, BoneOffset, MoodPose, Motion, Reaction, StatusLoop, REACTIONS, STATUS_LOOPS,
};

use avatar_small_robots::layout::Status;
use avatar_small_robots::Mood;

use crate::mesh::MeshError;
use math::M4;

/// A robot's pose at one time.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Pose {
    /// Model-space joint matrices, column-major, in bone order.
    pub bones: Vec<M4>,
    /// The face set shown.
    pub face: Mood,
    /// How shut the eyes are, 0 to 1.
    pub blink: f64,
    /// The light channel, 0 to 1.
    pub glow: f64,
    /// The status lights' key.
    pub status: Status,
    /// The chest LEDs' keys.
    pub leds: [Status; 4],
    /// Whether the bulb shows the full status colour.
    pub lit: bool,
    /// Whether the beacon dome shows the full status colour.
    pub rays: bool,
    /// The hover jet's thrust.
    pub thrust: f64,
}

/// A pose as canonical bytes, for hashing: the bone matrices in bone order
/// (16 f64 each, column-major), then the face set (its index in `MOODS`),
/// blink, glow, the status and the four LEDs (each its index in
/// [`Status::ALL`]), `lit`, `rays`, then thrust. Integers and flags are
/// little-endian u32 and floats little-endian f64 with −0.0 written as
/// +0.0; a non-finite value is an error.
pub fn pose_bytes(pose: &Pose) -> Result<Vec<u8>, MeshError> {
    let mut out = Vec::with_capacity(pose.bones.len() * 128 + 64);
    let u32le = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let f64le = |out: &mut Vec<u8>, v: f64| -> Result<(), MeshError> {
        if !v.is_finite() {
            return Err(MeshError::NonFinite);
        }
        let v = if v == 0.0 { 0.0 } else { v };
        out.extend_from_slice(&v.to_le_bytes());
        Ok(())
    };
    let status = |s: Status| {
        Status::ALL
            .iter()
            .position(|x| *x == s)
            .expect("every status is listed") as u32
    };
    for m in &pose.bones {
        for v in m {
            f64le(&mut out, *v)?;
        }
    }
    u32le(&mut out, mood_index(pose.face) as u32);
    f64le(&mut out, pose.blink)?;
    f64le(&mut out, pose.glow)?;
    u32le(&mut out, status(pose.status));
    for s in pose.leds {
        u32le(&mut out, status(s));
    }
    u32le(&mut out, u32::from(pose.lit));
    u32le(&mut out, u32::from(pose.rays));
    f64le(&mut out, pose.thrust)?;
    Ok(out)
}
