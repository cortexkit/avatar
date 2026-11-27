//! The actor: one robot's events, and the pose they give at any time.

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

/// One robot's events, each with its start time in seconds, and the pose
/// they give at any time.
///
/// A pose depends only on the robot, the events and the time asked for:
/// the actor keeps no history, so times may be evaluated in any order and
/// give the same poses. Events of each kind are given in time order; a
/// later query sees an ended reaction contribute nothing and a status loop
/// still running.
#[derive(Clone, Debug)]
pub struct Actor<'a> {
    robot: &'a Robot,
    base: Mood,
    idle_base: bool,
    moods: Vec<(f64, Mood)>,
    loops: Vec<(f64, Option<StatusLoop>)>,
    reactions: Vec<(f64, Reaction)>,
    idles: Vec<(f64, bool)>,
    floor: Floor,
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
            base: mood,
            idle_base: true,
            moods: Vec::new(),
            loops: Vec::new(),
            reactions: Vec::new(),
            idles: Vec::new(),
            floor: Floor::new(robot),
        }
    }

    /// The same actor with the idle layer off from the start.
    pub fn without_idle(mut self) -> Self {
        self.idle_base = false;
        self
    }

    /// The robot the actor poses.
    pub fn robot(&self) -> &'a Robot {
        self.robot
    }

    fn last<T>(events: &[(f64, T)], t: f64) {
        if let Some(e) = events.last() {
            assert!(t >= e.0, "events of one kind are given in time order");
        }
    }

    /// Changes the mood at `t`; the posture blends over [`MOOD_BLEND`]
    /// and the face swaps on a blink [`SWAP_LEAD`] later. A change to the
    /// mood already requested is ignored.
    pub fn set_mood(&mut self, t: f64, mood: Mood) {
        Self::last(&self.moods, t);
        let last = self.moods.last().map_or(self.base, |e| e.1);
        if last != mood {
            self.moods.push((t, mood));
        }
    }

    /// Starts a status loop at `t`, or stops the current one with `None`;
    /// each fades over [`LOOP_BLEND`].
    pub fn set_loop(&mut self, t: f64, status: Option<StatusLoop>) {
        Self::last(&self.loops, t);
        let current = self.loops.last().and_then(|e| e.1);
        if current != status {
            self.loops.push((t, status));
        }
    }

    /// Plays a reaction from `t`. One already playing fades out over
    /// [`REACT_CUT`].
    pub fn play(&mut self, t: f64, reaction: Reaction) {
        Self::last(&self.reactions, t);
        self.reactions.push((t, reaction));
    }

    /// Pauses or resumes the idle layer at `t`, blending over [`IDLE_BLEND`].
    pub fn set_idle(&mut self, t: f64, on: bool) {
        Self::last(&self.idles, t);
        self.idles.push((t, on));
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

    fn playing_reactions(&self, t: f64) -> Vec<Playing<'a>> {
        let motion = &self.robot.motion;
        let mut out = Vec::new();
        for (i, &(at, r)) in self.reactions.iter().enumerate() {
            let clip = motion.reaction(r);
            let tt = t - at;
            if tt < 0.0 || tt > clip.duration {
                continue;
            }
            let mut w = 1.0;
            if let Some(&(next, _)) = self.reactions.get(i + 1) {
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

    fn playing_loops(&self, t: f64) -> Vec<Playing<'a>> {
        let motion = &self.robot.motion;
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

    /// The spans over which reactions' faces replace the mood's. A face
    /// swaps in at the envelope's rising midpoint (at least [`SWAP_LEAD`]
    /// after the start) and out at its falling midpoint; when the next
    /// reaction starts first, the span ends when that one swaps its own
    /// face in, or [`SWAP_LEAD`] after it starts if it has none.
    fn face_spans(&self) -> Vec<Span> {
        let motion = &self.robot.motion;
        let swap_in = |c: &Clip, at: f64| at + SWAP_LEAD.max(c.envelope[0] / 2.0);
        let mut spans = Vec::new();
        for (i, &(at, r)) in self.reactions.iter().enumerate() {
            let c = motion.reaction(r);
            let Some(face) = c.face else { continue };
            let start = swap_in(c, at);
            let mut end = at + c.duration - c.envelope[1] / 2.0;
            if let Some(&(next, nr)) = self.reactions.get(i + 1) {
                let nc = motion.reaction(nr);
                end = end.min(if nc.face.is_some() {
                    swap_in(nc, next)
                } else {
                    next + SWAP_LEAD
                });
            }
            if end > start {
                spans.push(Span { start, end, face });
            }
        }
        spans
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
    pub fn face(&self, t: f64) -> Mood {
        self.face_shown(t, &self.face_spans())
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
    pub fn evaluate(&self, t: f64) -> Pose {
        let robot = self.robot;
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
        let loops = self.playing_loops(t);
        for l in &loops {
            b.add_clip(l.clip, l.tt, l.w, Mode::Add);
        }
        let reacts = self.playing_reactions(t);
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

        let spans = self.face_spans();
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
