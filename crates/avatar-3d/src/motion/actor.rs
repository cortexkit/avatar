//! The actor: one robot's events, and the pose they give at any time.

use std::collections::BTreeSet;

use avatar_small_robots::{Mood, MOODS};

use super::clip::{sample_keys, Clip, Mode, TrackPath};
use super::floor::Floor;
use super::idle::{swap_blink, SWAP_LEAD};
use super::math::{
    add, clamp, compose, lerp, m_mul, q_from_euler, q_mul, q_slerp, smooth, Quat, IDENTITY, M4, V3,
};
use super::rig::{MoodPose, Reaction, StatusLoop};
use super::Pose;
use crate::robot::Robot;

/// Seconds a mood change takes to blend the posture.
pub const MOOD_BLEND: f64 = 0.4;
/// Seconds a status loop takes to fade in or out.
pub const LOOP_BLEND: f64 = 0.5;
/// Seconds the idle layer takes to pause or resume.
pub const IDLE_BLEND: f64 = 0.3;
/// Seconds a reaction fades out when another replaces it.
pub const REACT_CUT: f64 = 0.15;
/// Seconds a face swap stays in effect after it: its blink has opened
/// 0.09 s after the swap ([`swap_blink`]), and the blink of a later swap
/// looks 1e-6 s before that swap to see whether the face really changes
/// there. [`ActorState::forget_before`] keeps a swap until this long after
/// it.
pub const SWAP_TAIL: f64 = 0.1;

/// One robot's events, each with its start time in seconds, and the pose
/// they give at any time: a robot reference plus an [`ActorState`].
///
/// A pose depends only on the robot, the events and the time asked for:
/// the actor keeps no history, so times may be evaluated in any order and
/// give the same poses. Events of each kind are given in time order; a
/// later query sees an ended reaction contribute nothing and a status loop
/// still running.
#[derive(Clone, Debug)]
pub struct Actor<'a> {
    robot: &'a Robot,
    state: ActorState,
}

/// Everything an [`Actor`] holds but the robot: its events and the floor
/// data measured from the robot. It owns all of it, so it can live beside
/// a shared robot; each call that reads the robot takes it as an argument,
/// and that must be the robot the state was made for.
///
/// The calls behave as [`Actor`]'s do. [`ActorState::forget_before`]
/// drops the events whose effects are over, so a state that lives as long
/// as its robot stays small.
#[derive(Clone, Debug)]
pub struct ActorState {
    base: Mood,
    idle_base: bool,
    moods: Vec<(f64, Mood)>,
    loops: Vec<(f64, Option<StatusLoop>)>,
    reactions: Vec<Played>,
    idles: Vec<(f64, bool)>,
    latest: Latest,
    floor: Floor,
}

/// A reaction played at a time.
#[derive(Clone, Copy, Debug)]
struct Played {
    at: f64,
    reaction: Reaction,
    /// The reaction played next, with its start time, once
    /// [`ActorState::forget_before`] has dropped it from the list: it still
    /// decides when this one fades out and when this one's face span ends.
    /// `None` while that reaction is the next entry of the list, or there
    /// is none.
    next: Option<(f64, Reaction)>,
}

/// The start time of the latest event of each kind the state took. Events
/// of a kind are checked against it, so the time-order check still holds
/// after the events it names are forgotten.
#[derive(Clone, Copy, Debug, Default)]
struct Latest {
    mood: Option<f64>,
    status_loop: Option<f64>,
    reaction: Option<f64>,
    idle: Option<f64>,
}

/// Panics unless `t` comes no earlier than `latest`, the latest event of
/// its kind.
fn in_order(latest: Option<f64>, t: f64) {
    if let Some(at) = latest {
        assert!(t >= at, "events of one kind are given in time order");
    }
}

/// A clip playing at a time: how far in, and its weight.
struct Playing<'c> {
    clip: &'c Clip,
    tt: f64,
    w: f64,
}

/// A span over which a reaction's face replaces the mood's.
struct Span {
    start: f64,
    end: f64,
    face: Mood,
}

/// One bone's offset from rest while a pose is built.
#[derive(Clone, Copy)]
pub(super) struct Slot {
    pub q: Quat,
    pub t: V3,
    pub s: V3,
}

const REST: Slot = Slot {
    q: IDENTITY,
    t: [0.0; 3],
    s: [1.0; 3],
};

/// The pose being built: one slot per bone, and the bones' matrices once
/// the layers are applied.
pub(super) struct Build<'r> {
    pub robot: &'r Robot,
    pub slots: Vec<Slot>,
    pub world: Vec<M4>,
}

impl Build<'_> {
    fn rot(&mut self, bone: usize, e: V3, w: f64) {
        if w == 0.0 || e == [0.0; 3] {
            return;
        }
        let s = &mut self.slots[bone];
        s.q = q_mul(s.q, q_from_euler([e[0] * w, e[1] * w, e[2] * w]));
    }

    fn set_rot(&mut self, bone: usize, e: V3, w: f64) {
        if w == 0.0 {
            return;
        }
        let s = &mut self.slots[bone];
        s.q = q_slerp(s.q, q_from_euler(e), clamp(w, 0.0, 1.0));
    }

    fn pos(&mut self, bone: usize, v: V3, w: f64) {
        let s = &mut self.slots[bone];
        s.t = [s.t[0] + v[0] * w, s.t[1] + v[1] * w, s.t[2] + v[2] * w];
    }

    fn set_pos(&mut self, bone: usize, v: V3, w: f64) {
        let s = &mut self.slots[bone];
        s.t = [0, 1, 2].map(|k| lerp(s.t[k], v[k], w));
    }

    fn scl(&mut self, bone: usize, v: V3, w: f64) {
        let s = &mut self.slots[bone];
        s.s = [0, 1, 2].map(|k| s.s[k] * (1.0 + v[k] * w));
    }

    fn set_scl(&mut self, bone: usize, v: V3, w: f64) {
        let s = &mut self.slots[bone];
        s.s = [0, 1, 2].map(|k| lerp(s.s[k], 1.0 + v[k], w));
    }

    fn add_static(&mut self, pose: &MoodPose, w: f64) {
        if w <= 0.0 {
            return;
        }
        for o in &pose.offsets {
            self.rot(o.bone, o.rotation, w);
            self.pos(o.bone, o.translation, w);
            self.scl(o.bone, o.scale, w);
        }
    }

    fn add_clip(&mut self, c: &Clip, t: f64, w: f64, mode: Mode) {
        if w <= 0.0 {
            return;
        }
        for tr in c.tracks.iter().filter(|tr| tr.mode == mode) {
            let v = tr.sample(t, c.looping, c.duration);
            match (mode, tr.path) {
                (Mode::Add, TrackPath::Rotation) => self.rot(tr.bone, v, w),
                (Mode::Add, TrackPath::Translation) => self.pos(tr.bone, v, w),
                (Mode::Add, TrackPath::Scale) => self.scl(tr.bone, v, w),
                (Mode::Set, TrackPath::Rotation) => self.set_rot(tr.bone, v, w),
                (Mode::Set, TrackPath::Translation) => self.set_pos(tr.bone, v, w),
                (Mode::Set, TrackPath::Scale) => self.set_scl(tr.bone, v, w),
            }
        }
    }

    /// The idle layer at `t` with weight `w`, its swings calmed to `calm`
    /// (breathing is never calmed); a swing only a hovering robot makes is
    /// further weighted by `hovering`.
    fn idle(&mut self, t: f64, w: f64, calm: f64, hovering: f64) {
        if w <= 0.0 {
            return;
        }
        let m = self.robot.motion.params;
        let idle = &self.robot.motion.idle;
        let (tp, en) = (m.tempo, m.energy);
        let deg = super::math::DEG;
        let breath = m.breath(t);
        let a = idle.breathe_amp * en;
        self.scl(
            idle.breathe_bone,
            [-0.5 * a * breath, a * breath, -0.5 * a * breath],
            w,
        );
        if idle.breathe_lift != 0.0 {
            self.pos(
                idle.breathe_bone,
                [0.0, idle.breathe_lift * en * breath, 0.0],
                w,
            );
        }
        let sway = m.sway(t);
        self.rot(
            idle.sway_bone,
            [0.0, 0.0, idle.sway_degrees * deg * en * sway],
            w * calm,
        );
        let g = m.glance(t);
        self.rot(
            idle.glance_bone,
            [
                -g[1] * idle.glance_pitch * deg,
                g[0] * idle.glance_yaw * deg,
                0.0,
            ],
            w * calm,
        );
        for x in &idle.extra {
            let s = super::math::sin(std::f64::consts::TAU * (t * tp / x.period + x.phase));
            let wx = if x.hover_only {
                w * calm * hovering
            } else {
                w * calm
            };
            match x.path {
                TrackPath::Translation => self.pos(
                    x.bone,
                    [x.amp[0] * s * en, x.amp[1] * s * en, x.amp[2] * s * en],
                    wx,
                ),
                _ => self.rot(
                    x.bone,
                    [
                        x.amp[0] * s * deg * en,
                        x.amp[1] * s * deg * en,
                        x.amp[2] * s * deg * en,
                    ],
                    wx,
                ),
            }
        }
    }

    /// Each bone's local matrix: its rest offset from its parent plus its
    /// slot's translation, then the slot's rotation and scale.
    fn local(&self, j: usize) -> M4 {
        let bones = &self.robot.bones;
        let parent = bones[j].parent.map_or([0.0; 3], |p| bones[p].joint);
        let rest = [0, 1, 2].map(|k| bones[j].joint[k] - parent[k]);
        let s = &self.slots[j];
        compose(add(rest, s.t), s.q, s.s)
    }

    /// Recomputes the model-space matrix of `from` and every bone below it.
    pub fn update_from(&mut self, from: usize) {
        let n = self.robot.bones.len();
        let mut dirty = vec![false; n];
        dirty[from] = true;
        for j in from..n {
            let parent = self.robot.bones[j].parent;
            if !(dirty[j] || parent.is_some_and(|p| dirty[p])) {
                continue;
            }
            dirty[j] = true;
            let local = self.local(j);
            self.world[j] = match parent {
                Some(p) => m_mul(&self.world[p], &local),
                None => local,
            };
        }
    }
}

impl<'a> Actor<'a> {
    /// An actor for `robot`, in `mood` from the start, with the idle layer
    /// on.
    pub fn new(robot: &'a Robot, mood: Mood) -> Self {
        Self {
            robot,
            state: ActorState::new(robot, mood),
        }
    }

    /// An actor for `robot` holding `state`, which must have been made for
    /// `robot`.
    pub fn from_state(robot: &'a Robot, state: ActorState) -> Self {
        Self { robot, state }
    }

    /// The same actor with the idle layer off from the start.
    pub fn without_idle(mut self) -> Self {
        self.state = self.state.without_idle();
        self
    }

    /// The robot the actor poses.
    pub fn robot(&self) -> &'a Robot {
        self.robot
    }

    /// The actor's events and floor data.
    pub fn state(&self) -> &ActorState {
        &self.state
    }

    /// The actor's events and floor data, without the robot.
    pub fn into_state(self) -> ActorState {
        self.state
    }

    /// Changes the mood at `t`, as [`ActorState::set_mood`] does.
    pub fn set_mood(&mut self, t: f64, mood: Mood) {
        self.state.set_mood(t, mood);
    }

    /// Starts or stops a status loop at `t`, as [`ActorState::set_loop`]
    /// does.
    pub fn set_loop(&mut self, t: f64, status: Option<StatusLoop>) {
        self.state.set_loop(t, status);
    }

    /// Plays a reaction from `t`, as [`ActorState::play`] does.
    pub fn play(&mut self, t: f64, reaction: Reaction) {
        self.state.play(t, reaction);
    }

    /// Pauses or resumes the idle layer at `t`, as [`ActorState::set_idle`]
    /// does.
    pub fn set_idle(&mut self, t: f64, on: bool) {
        self.state.set_idle(t, on);
    }

    /// The mood weights at `t` ([`ActorState::mood_weights`]).
    pub fn mood_weights(&self, t: f64) -> [f64; 7] {
        self.state.mood_weights(t)
    }

    /// The face set shown at `t`.
    pub fn face(&self, t: f64) -> Mood {
        self.state.face(self.robot, t)
    }

    /// Every time after `from` at which [`Actor::face`] changes, with the
    /// new face set ([`ActorState::face_moments`]).
    pub fn face_moments(&self, from: f64) -> Vec<(f64, Mood)> {
        self.state.face_moments(self.robot, from)
    }

    /// Forgets the events whose effects are over by `t`
    /// ([`ActorState::forget_before`]).
    pub fn forget_before(&mut self, t: f64) {
        self.state.forget_before(self.robot, t);
    }

    /// The pose at `t` seconds ([`ActorState::evaluate`]).
    pub fn evaluate(&self, t: f64) -> Pose {
        self.state.evaluate(self.robot, t)
    }
}

impl ActorState {
    /// The state of an actor for `robot`, in `mood` from the start, with
    /// the idle layer on.
    pub fn new(robot: &Robot, mood: Mood) -> Self {
        Self {
            base: mood,
            idle_base: true,
            moods: Vec::new(),
            loops: Vec::new(),
            reactions: Vec::new(),
            idles: Vec::new(),
            latest: Latest::default(),
            floor: Floor::new(robot),
        }
    }

    /// The same state with the idle layer off from the start.
    pub fn without_idle(mut self) -> Self {
        self.idle_base = false;
        self
    }

    /// The mood the moods blend from: the one the actor started in, or the
    /// latest one [`ActorState::forget_before`] found settled.
    pub fn base(&self) -> Mood {
        self.base
    }

    /// The mood changes held, in time order.
    pub fn mood_changes(&self) -> &[(f64, Mood)] {
        &self.moods
    }

    /// The status loop starts and stops held, in time order.
    pub fn loop_changes(&self) -> &[(f64, Option<StatusLoop>)] {
        &self.loops
    }

    /// The reactions held, in time order.
    pub fn reactions(&self) -> impl Iterator<Item = (f64, Reaction)> + '_ {
        self.reactions.iter().map(|p| (p.at, p.reaction))
    }

    /// The idle layer's pauses and resumes held, in time order.
    pub fn idle_changes(&self) -> &[(f64, bool)] {
        &self.idles
    }

    /// Changes the mood at `t`; the posture blends over [`MOOD_BLEND`]
    /// and the face swaps on a blink [`SWAP_LEAD`] later. A change to the
    /// mood already requested is ignored.
    pub fn set_mood(&mut self, t: f64, mood: Mood) {
        in_order(self.latest.mood, t);
        let last = self.moods.last().map_or(self.base, |e| e.1);
        if last != mood {
            self.moods.push((t, mood));
            self.latest.mood = Some(t);
        }
    }

    /// Starts a status loop at `t`, or stops the current one with `None`;
    /// each fades over [`LOOP_BLEND`].
    pub fn set_loop(&mut self, t: f64, status: Option<StatusLoop>) {
        in_order(self.latest.status_loop, t);
        let current = self.loops.last().and_then(|e| e.1);
        if current != status {
            self.loops.push((t, status));
            self.latest.status_loop = Some(t);
        }
    }

    /// Plays a reaction from `t`. One already playing fades out over
    /// [`REACT_CUT`].
    pub fn play(&mut self, t: f64, reaction: Reaction) {
        in_order(self.latest.reaction, t);
        self.reactions.push(Played {
            at: t,
            reaction,
            next: None,
        });
        self.latest.reaction = Some(t);
    }

    /// Pauses or resumes the idle layer at `t`, blending over [`IDLE_BLEND`].
    pub fn set_idle(&mut self, t: f64, on: bool) {
        in_order(self.latest.idle, t);
        self.idles.push((t, on));
        self.latest.idle = Some(t);
    }

    /// The mood weights at `t`, in `MOODS` order: start from the base mood,
    /// and for each change `W ← (1 − a) W + a e(mood)` with
    /// `a = smooth((t − t_k) / MOOD_BLEND)`.
    pub fn mood_weights(&self, t: f64) -> [f64; 7] {
        let mut w = [0.0; 7];
        w[mood_index(self.base)] = 1.0;
        for &(at, mood) in &self.moods {
            if t < at {
                break;
            }
            let a = smooth((t - at) / MOOD_BLEND);
            for v in &mut w {
                *v *= 1.0 - a;
            }
            w[mood_index(mood)] += a;
        }
        w
    }

    fn idle_weight(&self, t: f64) -> f64 {
        let mut w = if self.idle_base { 1.0 } else { 0.0 };
        for &(at, on) in &self.idles {
            if t < at {
                break;
            }
            let a = smooth((t - at) / IDLE_BLEND);
            w = lerp(w, if on { 1.0 } else { 0.0 }, a);
        }
        w
    }

    /// The reaction played after reaction `i`, with its start time.
    fn next_reaction(&self, i: usize) -> Option<(f64, Reaction)> {
        self.reactions[i]
            .next
            .or_else(|| self.reactions.get(i + 1).map(|n| (n.at, n.reaction)))
    }

    fn playing_reactions<'r>(&self, robot: &'r Robot, t: f64) -> Vec<Playing<'r>> {
        let motion = &robot.motion;
        let mut out = Vec::new();
        for (i, p) in self.reactions.iter().enumerate() {
            let clip = motion.reaction(p.reaction);
            let tt = t - p.at;
            if tt < 0.0 || tt > clip.duration {
                continue;
            }
            let mut w = 1.0;
            if let Some((next, _)) = self.next_reaction(i) {
                if t >= next {
                    w = 1.0 - smooth((t - next) / REACT_CUT);
                }
            }
            if w > 0.0 {
                out.push(Playing { clip, tt, w });
            }
        }
        out
    }

    fn playing_loops<'r>(&self, robot: &'r Robot, t: f64) -> Vec<Playing<'r>> {
        let motion = &robot.motion;
        let mut out = Vec::new();
        for (i, &(at, l)) in self.loops.iter().enumerate() {
            let Some(l) = l else { continue };
            if t < at {
                continue;
            }
            let clip = motion.status_loop(l);
            let mut w = smooth((t - at) / LOOP_BLEND);
            if let Some(&(next, _)) = self.loops.get(i + 1) {
                if t >= next {
                    w *= 1.0 - smooth((t - next) / LOOP_BLEND);
                }
            }
            if w > 0.0 {
                out.push(Playing {
                    clip,
                    tt: t - at,
                    w,
                });
            }
        }
        out
    }

    /// The span over which reaction `i`'s face replaces the mood's, if it
    /// has a face and the span is not empty. The face swaps in at the
    /// envelope's rising midpoint (at least [`SWAP_LEAD`] after the start)
    /// and out at its falling midpoint; when the next reaction starts
    /// first, the span ends when that one swaps its own face in, or
    /// [`SWAP_LEAD`] after it starts if it has none.
    fn span(&self, robot: &Robot, i: usize) -> Option<Span> {
        let motion = &robot.motion;
        let swap_in = |c: &Clip, at: f64| at + SWAP_LEAD.max(c.envelope[0] / 2.0);
        let p = self.reactions[i];
        let c = motion.reaction(p.reaction);
        let face = c.face?;
        let start = swap_in(c, p.at);
        let mut end = p.at + c.duration - c.envelope[1] / 2.0;
        if let Some((next, nr)) = self.next_reaction(i) {
            let nc = motion.reaction(nr);
            end = end.min(if nc.face.is_some() {
                swap_in(nc, next)
            } else {
                next + SWAP_LEAD
            });
        }
        (end > start).then_some(Span { start, end, face })
    }

    /// The spans over which reactions' faces replace the mood's, in
    /// reaction order.
    fn face_spans(&self, robot: &Robot) -> Vec<Span> {
        (0..self.reactions.len())
            .filter_map(|i| self.span(robot, i))
            .collect()
    }

    /// The face shown at `t`: the latest reaction span holding `t`, else
    /// the mood of the latest mood swap at or before `t`.
    fn face_shown(&self, t: f64, spans: &[Span]) -> Mood {
        if let Some(s) = spans.iter().rev().find(|s| t >= s.start && t < s.end) {
            return s.face;
        }
        let mut m = self.base;
        for &(at, mood) in &self.moods {
            if t >= at + SWAP_LEAD {
                m = mood;
            }
        }
        m
    }

    /// The strongest swap blink at `t` among the swap moments at which the
    /// shown face really changes.
    fn swap_blink_at(&self, t: f64, spans: &[Span]) -> f64 {
        let mut moments: Vec<f64> = self.moods.iter().map(|e| e.0 + SWAP_LEAD).collect();
        for s in spans {
            moments.push(s.start);
            moments.push(s.end);
        }
        let mut b: f64 = 0.0;
        for c in moments {
            let x = t - c;
            if x <= -SWAP_LEAD || x >= 0.09 {
                continue;
            }
            if self.face_shown(c - 1e-6, spans) != self.face_shown(c, spans) {
                b = b.max(swap_blink(x));
            }
        }
        b
    }

    /// The face set shown at `t`.
    pub fn face(&self, robot: &Robot, t: f64) -> Mood {
        self.face_shown(t, &self.face_spans(robot))
    }

    /// Every time after `from` at which [`ActorState::face`] changes, with
    /// the face set shown from then on, in time order.
    ///
    /// A mood's swap and a reaction's face swapping in and out are listed
    /// wherever the face shown really changes: a reaction without a face,
    /// or whose face is the one already shown, lists nothing, and a swap
    /// hidden under a reaction's face or cut short by the next reaction
    /// lists only the changes that remain. The times are exact, not rounded
    /// to any sampling grid: the face is the listed one from each time
    /// until the next, and before the first it is the face at `from`.
    pub fn face_moments(&self, robot: &Robot, from: f64) -> Vec<(f64, Mood)> {
        /// What happens at a time the face may change.
        enum Edge {
            Start(usize),
            End(usize),
            Swap(usize),
        }
        let spans = self.face_spans(robot);
        let mut edges = Vec::new();
        // The spans holding the time reached, by index: the latest one
        // shows its face.
        let mut open = BTreeSet::new();
        for (i, s) in spans.iter().enumerate() {
            if s.start > from {
                edges.push((s.start, Edge::Start(i)));
            } else if s.end > from {
                open.insert(i);
            }
            if s.end > from {
                edges.push((s.end, Edge::End(i)));
            }
        }
        // The latest mood change whose face has swapped in by the time
        // reached.
        let mut swapped = None;
        for (k, &(at, _)) in self.moods.iter().enumerate() {
            let c = at + SWAP_LEAD;
            if c > from {
                edges.push((c, Edge::Swap(k)));
            } else if from >= c {
                swapped = Some(k);
            }
        }
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        let shown = |open: &BTreeSet<usize>, swapped: Option<usize>| match open.last() {
            Some(&i) => spans[i].face,
            None => swapped.map_or(self.base, |k| self.moods[k].1),
        };
        let mut face = shown(&open, swapped);
        let mut out = Vec::new();
        for (k, (at, edge)) in edges.iter().enumerate() {
            match *edge {
                Edge::Start(i) => {
                    open.insert(i);
                }
                Edge::End(i) => {
                    open.remove(&i);
                }
                Edge::Swap(m) => swapped = swapped.max(Some(m)),
            }
            // Read the face once every edge at this time is applied.
            if edges.get(k + 1).is_some_and(|n| n.0 == *at) {
                continue;
            }
            let now = shown(&open, swapped);
            if now != face {
                out.push((*at, now));
                face = now;
            }
        }
        out
    }

    /// Forgets the events whose effects are over by `t`, so a state that
    /// lives long holds only what still matters at `t` and later:
    ///
    /// - the latest mood change whose posture blend is done and whose face
    ///   swapped in at least [`SWAP_TAIL`] before `t` becomes the base mood,
    ///   and it and every change before it are dropped;
    /// - a status loop is dropped once it has faded out under the change
    ///   after it, and a stop once the loop it stopped is dropped;
    /// - every idle change before the latest one whose blend is done is
    ///   dropped; that one stays as the idle setting;
    /// - a reaction is dropped once its clip has ended, or faded out under
    ///   the next reaction, and its face span, if it has one, ended at least
    ///   [`SWAP_TAIL`] before `t`. The reaction after a kept one is
    ///   remembered by it even when dropped, as it decides when the kept
    ///   one fades out and ends its face.
    ///
    /// Every event still in effect at `t` (a playing status loop, an
    /// unfinished blend, swap or reaction, the idle setting) is kept with
    /// its original time, so loops keep their phase. Afterwards
    /// [`ActorState::evaluate`], [`ActorState::face`] and
    /// [`ActorState::face_moments`] at any time at or after `t` give what
    /// they gave before, and so do they once later events are added; times
    /// before `t` are no longer posed as they were. A NaN `t` forgets
    /// nothing.
    pub fn forget_before(&mut self, robot: &Robot, t: f64) {
        // Once a change's blend weight has reached 1, the weights are
        // exactly its mood whatever came before, so that mood can be the
        // base the later changes blend from.
        let settled = |at: f64| (t - at) / MOOD_BLEND >= 1.0 && t - (at + SWAP_LEAD) >= SWAP_TAIL;
        if let Some(k) = self.moods.iter().rposition(|&(at, _)| settled(at)) {
            self.base = self.moods[k].1;
            self.moods.drain(..=k);
        }

        // A change whose blend is done sets the idle weight to exactly its
        // own, whatever came before, so the earlier changes can go.
        if let Some(k) = self
            .idles
            .iter()
            .rposition(|&(at, _)| (t - at) / IDLE_BLEND >= 1.0)
        {
            self.idles.drain(..k);
        }

        // A loop's fade-out is decided by the change after it, so loops go
        // from the front only; the current loop has no change after it and
        // stays.
        let gone = self
            .loops
            .iter()
            .enumerate()
            .take_while(|&(i, &(_, l))| match l {
                None => true,
                Some(_) => self
                    .loops
                    .get(i + 1)
                    .is_some_and(|&(next, _)| (t - next) / LOOP_BLEND >= 1.0),
            })
            .count();
        self.loops.drain(..gone);

        let mut kept: Vec<Played> = Vec::with_capacity(self.reactions.len());
        // Whether the entry just looked at was kept.
        let mut after_kept = false;
        for (i, &p) in self.reactions.iter().enumerate() {
            let clip = robot.motion.reaction(p.reaction);
            let posing = !(t - p.at > clip.duration
                || self
                    .next_reaction(i)
                    .is_some_and(|(next, _)| (t - next) / REACT_CUT >= 1.0));
            let facing = self.span(robot, i).is_some_and(|s| t - s.end < SWAP_TAIL);
            if posing || facing {
                kept.push(p);
                after_kept = true;
                continue;
            }
            if after_kept {
                let before = kept.last_mut().expect("an entry was kept");
                before.next.get_or_insert((p.at, p.reaction));
            }
            after_kept = false;
        }
        self.reactions = kept;
    }

    /// The pose at `t` seconds.
    ///
    /// Layers, from the bottom: each mood's posture by its weight; the
    /// status loop's additive tracks; idle (calmed to 30% under a reaction,
    /// except for breathing); the reactions' additive tracks; then their
    /// set tracks by their envelopes. Rotations compose as quaternions in
    /// that order, translations add and scales multiply. Then the floor
    /// rule: a hand that would come within 2.5 robot units of the floor
    /// turns the arm forward about the shoulder; the hover exhaust shrinks
    /// toward its nozzle with thrust and above the floor; and if any vertex
    /// would still go below the floor, the root rises by the depth.
    pub fn evaluate(&self, robot: &Robot, t: f64) -> Pose {
        let motion = &robot.motion;
        let n = robot.bones.len();
        let mut b = Build {
            robot,
            slots: vec![REST; n],
            world: vec![[0.0; 16]; n],
        };
        let w = self.mood_weights(t);
        for (i, mood) in motion.moods.iter().enumerate() {
            if w[i] > 1e-4 {
                b.add_static(mood, w[i]);
            }
        }
        let loops = self.playing_loops(robot, t);
        for l in &loops {
            b.add_clip(l.clip, l.tt, l.w, Mode::Add);
        }
        let reacts = self.playing_reactions(robot, t);
        let rw = reacts
            .iter()
            .map(|r| r.w * r.clip.envelope_at(r.tt))
            .fold(0.0, f64::max);
        let iw = self.idle_weight(t);
        let hovering: f64 = motion
            .moods
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.landed)
            .map(|(i, _)| w[i])
            .sum();
        b.idle(t, iw, 1.0 - 0.7 * rw, hovering);
        for r in &reacts {
            b.add_clip(r.clip, r.tt, r.w, Mode::Add);
        }
        for r in &reacts {
            b.add_clip(r.clip, r.tt, r.w * r.clip.envelope_at(r.tt), Mode::Set);
        }
        b.update_from(0);
        let thrust: f64 = motion
            .moods
            .iter()
            .enumerate()
            .map(|(i, m)| w[i] * m.thrust)
            .sum();
        self.floor.constrain(&mut b, thrust);

        let spans = self.face_spans(robot);
        let face = self.face_shown(t, &spans);
        let mut blink = self.swap_blink_at(t, &spans);
        if iw > 0.0 {
            blink = blink.max(motion.params.blink(t) * iw);
        }
        let mut glow: f64 = 0.0;
        for r in &reacts {
            if !r.clip.blink.is_empty() {
                blink = blink.max(sample_keys(&r.clip.blink, r.tt) * r.w);
            }
            if !r.clip.glow.is_empty() {
                glow = glow.max(sample_keys(&r.clip.glow, r.tt) * r.w);
            }
        }
        for l in &loops {
            let tt = l.tt % l.clip.duration;
            if !l.clip.blink.is_empty() {
                blink = blink.max(sample_keys(&l.clip.blink, tt) * l.w);
            }
            if !l.clip.glow.is_empty() {
                glow = glow.max(sample_keys(&l.clip.glow, tt) * l.w);
            }
        }
        let lights = motion.mood(face);
        Pose {
            bones: b.world,
            face,
            blink,
            glow,
            status: lights.status,
            leds: lights.leds,
            lit: lights.lit,
            rays: lights.rays,
            thrust,
        }
    }
}

/// A mood's place in `MOODS`.
pub fn mood_index(m: Mood) -> usize {
    MOODS
        .iter()
        .position(|x| *x == m)
        .expect("every mood is listed")
}
