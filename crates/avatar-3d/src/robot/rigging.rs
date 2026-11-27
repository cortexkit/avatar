//! The robot's motion data: mood postures from the 2D layout's resolved
//! poses, the idle formulas' bones, and the reactions and status loops
//! mapped onto this robot's bones.

use avatar_small_robots::layout::{ArmPlacement, BackLayout, Frame, MastPlacement};
use avatar_small_robots::{Mood, GENOME_LEN, MOODS};

use super::build::Builder;
use super::{Bone, UNIT};
use crate::motion::math::{
    distance, lerp3, normalize, q_from_axis_angle, q_from_unit_vectors, q_invert, q_mul, q_rotate,
    q_to_euler, scale, sub, DEG, V3,
};
use crate::motion::{
    ArmRig, BoneOffset, Channel, Clip, Idle, IdleExtra, Keys, Mode, MoodPose, Motion, MotionParams,
    Track, TrackPath,
};

/// How far a 2D grip of 1 closes the jaws across the hand, as a fraction of
/// their rest spread, relative to the neutral grip.
const GRIP_CLOSE: f64 = 0.3;

/// The degrees a gauge needle turns per unit of the 2D needle setting.
const NEEDLE_DEGREES: f64 = 62.0;

/// Builds the motion data from the finished skeleton.
pub(super) fn motion(b: &Builder, genome: &[u8; GENOME_LEN], bones: &[Bone]) -> Motion {
    let r = Rigging::new(b, genome, bones);
    let moods = MOODS.iter().map(|&m| r.mood_pose(b, m)).collect();
    let (reactions, loops) = r.clips();
    Motion {
        params: r.params,
        height: r.height,
        idle: r.idle(),
        moods,
        reactions,
        loops,
        arms: r.arms.clone(),
        exhaust: r.find("exhaust.0"),
    }
}

/// What the motion data is built from.
struct Rigging<'a> {
    bones: &'a [Bone],
    params: MotionParams,
    /// The rest height, in model units.
    height: f64,
    pod: bool,
    root: usize,
    body: usize,
    /// The bone head gestures, glances and nods turn: the head, or on a pod,
    /// whose only hull is its head, the body, so the arms turn with it.
    head: usize,
    arms: Vec<ArmRig>,
    /// Each antenna's bone and the side it sways toward.
    antennae: Vec<(usize, f64)>,
    /// The head's top, the chassis's top (a pod's head top) and a hand's
    /// size, in model units.
    head_top: f64,
    chassis_top: f64,
    hand: f64,
    stub: bool,
}

/// The XYZ Euler angles, in radians, of the shoulder and elbow turns that
/// point an arm's upper segment along `du` and its forearm along `df`, from
/// its rest pose; the elbow turn is relative to the turned upper arm, and
/// zero for an arm without an elbow.
fn arm_to(arm: &ArmRig, du: V3, df: V3) -> (V3, V3) {
    let [s, e, w] = arm.joints;
    let qu = q_from_unit_vectors(normalize(sub(e, s)), normalize(du));
    let f1 = q_rotate(qu, sub(w, e));
    let qw = q_from_unit_vectors(normalize(f1), normalize(df));
    let qf = q_mul(q_mul(q_invert(qu), qw), qu);
    let f = if arm.elbow { q_to_euler(qf) } else { [0.0; 3] };
    (q_to_euler(qu), f)
}

/// The clockwise angle in radians, in the 2D drawing (y down), that turns
/// direction `a` to direction `b`.
fn turn_2d(a: [f64; 2], b: [f64; 2]) -> f64 {
    let cross = a[0] * b[1] - a[1] * b[0];
    let dot = a[0] * b[0] + a[1] * b[1];
    avatar_svg::wild::math::atan2(cross, dot)
}

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// A 2D offset (y down) in robot units as a model-space offset.
fn offset_2d(d: [f64; 2]) -> V3 {
    [d[0] * UNIT, -d[1] * UNIT, 0.0].map(|v| v + 0.0)
}

/// A unit direction.
fn dir(x: f64, y: f64, z: f64) -> V3 {
    normalize([x, y, z])
}

/// `d` turned about +z by `degrees`.
fn swing(d: V3, degrees: f64) -> V3 {
    q_rotate(q_from_axis_angle([0.0, 0.0, 1.0], degrees * DEG), d)
}

/// A track from keys whose rotations are in degrees.
fn track(bone: usize, path: TrackPath, mode: Mode, keys: &[(f64, V3)]) -> Track {
    let k = if path == TrackPath::Rotation {
        DEG
    } else {
        1.0
    };
    let keys: Vec<(f64, V3)> = keys.iter().map(|(t, v)| (*t, v.map(|x| x * k))).collect();
    Track::new(bone, path, mode, &keys)
}

/// Scalar keys.
fn keys(k: &[(f64, f64)]) -> Keys {
    k.iter().map(|(t, v)| [*t, *v]).collect()
}

/// A volume-keeping squash key: `y` up, half as much in across.
fn sq(t: f64, y: f64) -> (f64, V3) {
    (t, [-0.5 * y, y, -0.5 * y])
}

/// A key of a translation along y.
fn ty(t: f64, y: f64) -> (f64, V3) {
    (t, [0.0, y, 0.0])
}

const Z: V3 = [0.0; 3];

impl<'a> Rigging<'a> {
    fn new(b: &Builder, genome: &[u8; GENOME_LEN], bones: &'a [Bone]) -> Self {
        let find = |name: &str| {
            bones
                .iter()
                .position(|x| x.name == name)
                .unwrap_or_else(|| panic!("the skeleton has {name}"))
        };
        let params = MotionParams::new(genome);
        let height = b
            .parts
            .iter()
            .flat_map(|p| p.mesh.positions.iter())
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let head_top = b
            .parts
            .iter()
            .filter(|p| p.role == "head")
            .flat_map(|p| p.mesh.positions.iter())
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let layout = b.layout;
        let chassis_top = layout.chassis_top.map_or(head_top, |y| {
            b.at(Frame::UpperBody, [0.0, y], 0.0)[1] * UNIT
        });
        let mut arms = Vec::new();
        let mut hand = 0.0;
        let mut stub = false;
        for (i, x) in ["L", "R"].iter().enumerate() {
            let Some(arm) = layout.arms[i] else { continue };
            let bone = |p: &str| find(&format!("{p}.{x}"));
            let (upper, fore, wrist, grip) = (
                bone("upperarm"),
                bone("forearm"),
                bone("wrist"),
                bone("grip"),
            );
            hand = arm.hand_scale * UNIT;
            stub = arm.kind == "stub";
            arms.push(ArmRig {
                side: if i == 0 { -1.0 } else { 1.0 },
                upper,
                fore,
                wrist,
                grip,
                joints: [upper, fore, wrist].map(|j| bones[j].joint),
                elbow: arm.kind != "stub" && arm.kind != "telescope",
            });
        }
        let antennae = layout
            .top
            .masts
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let side = if m.side == 0 {
                    params.lead
                } else {
                    f64::from(m.side)
                };
                (find(&format!("antenna.{i}")), side)
            })
            .collect();
        let pod = layout.chassis.is_none();
        Self {
            bones,
            params,
            height,
            pod,
            root: find("root"),
            body: find("body"),
            head: if pod { find("body") } else { find("head") },
            arms,
            antennae,
            head_top,
            chassis_top,
            hand,
            stub,
        }
    }

    fn find(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|x| x.name == name)
    }

    /// A mood's posture as offsets from neutral, and its light fields.
    ///
    /// - The body rises by the 2D lift and turns about z by the 2D lean (a
    ///   2D clockwise turn is a turn about −z seen from the front).
    /// - The head drops and tips by the 2D drop and tilt, and pitches by a
    ///   small per-mood nod the 2D drawing cannot show; a pod's head is its
    ///   whole hull, whose drop and tilt the 2D type folds into the body's
    ///   lift and lean, so its body only nods, at 30%.
    /// - Each arm's shoulder and elbow turn to carry the rest arm onto the
    ///   mood's arm; the jaws close across the hand with the grip.
    /// - Antennae, stalks and a trailing cable turn about their mounts to
    ///   the mood's stems, and a gauge needle by the needle setting.
    fn mood_pose(&self, b: &Builder, m: Mood) -> MoodPose {
        let layout = b.layout;
        let n = layout.mood(Mood::Neutral);
        let p = layout.mood(m);
        let mut offsets = Vec::new();
        let mut put = |bone: usize, rotation: V3, translation: V3, scale: V3| {
            let o = BoneOffset {
                bone,
                rotation: rotation.map(|v| v + 0.0),
                translation: translation.map(|v| v + 0.0),
                scale: scale.map(|v| v + 0.0),
            };
            if o.rotation != Z || o.translation != Z || o.scale != Z {
                offsets.push(o);
            }
        };
        let lean = p.lean - n.lean;
        put(
            self.body,
            [0.0, 0.0, -lean * DEG],
            [0.0, (p.lift - n.lift) * UNIT, 0.0],
            Z,
        );
        let nod = match m {
            Mood::Working => 6.0,
            Mood::Happy => -4.0,
            Mood::Waiting => -2.0,
            Mood::Blocked => 3.0,
            Mood::Sad | Mood::Sleepy => 8.0,
            _ => 0.0,
        };
        if self.pod {
            put(self.head, [nod * 0.3 * DEG, 0.0, 0.0], Z, Z);
        } else {
            put(
                self.head,
                [nod * DEG, 0.0, -(p.tilt - n.tilt) * DEG],
                [0.0, -(p.drop - n.drop) * UNIT, 0.0],
                Z,
            );
        }
        for (arm, rig) in layout.arms.iter().zip([-1.0, 1.0]) {
            let Some(arm) = arm else { continue };
            let Some(rig) = self.arms.iter().find(|a| a.side == rig) else {
                continue;
            };
            let Some(placed): Option<ArmPlacement> = p.arms[usize::from(rig.side > 0.0)] else {
                continue;
            };
            let point = |q: [f64; 2]| scale(super::limbs::arm_point(b, arm, q), UNIT);
            let (s, e, w) = (
                point(placed.shoulder),
                point(placed.elbow),
                point(placed.wrist),
            );
            let (u, f) = arm_to(rig, sub(e, s), sub(w, e));
            put(rig.upper, u, sub(s, rig.joints[0]), Z);
            put(rig.fore, f, Z, Z);
            put(rig.grip, Z, Z, [-GRIP_CLOSE * (p.grip - n.grip), 0.0, 0.0]);
        }
        let stem_dir = |kind: &str, g: &MastPlacement| {
            if kind == "zigzag" || kind == "coil" {
                g.base_dir
            } else {
                sub2(g.end, g.base)
            }
        };
        for (i, mast) in layout.top.masts.iter().enumerate() {
            let (rest, now) = (&mast.rest, &p.masts[i]);
            let turn = turn_2d(stem_dir(mast.kind, rest), stem_dir(mast.kind, now));
            put(
                self.antennae[i].0,
                [0.0, 0.0, -turn],
                offset_2d(sub2(now.base, rest.base)),
                Z,
            );
        }
        for (i, stalk) in layout.top.stalks.iter().enumerate() {
            let name = if stalk.side < 0 { "stalk.L" } else { "stalk.R" };
            let (Some(bone), Some(now)) = (self.find(name), p.stalks.get(i)) else {
                continue;
            };
            let rest = &stalk.rest;
            let turn = turn_2d(
                sub2(rest.eye_centre, rest.stem.base),
                sub2(now.eye_centre, now.stem.base),
            );
            put(
                bone,
                [0.0, 0.0, -turn],
                offset_2d(sub2(now.stem.base, rest.stem.base)),
                Z,
            );
        }
        if let Some(bone) = self.find("needle") {
            put(
                bone,
                [0.0, 0.0, -(p.needle - n.needle) * NEEDLE_DEGREES * DEG],
                Z,
                Z,
            );
        }
        if let (Some(bone), Some(BackLayout::Cable { rest, .. }), Some(now)) =
            (self.find("cable"), layout.back, p.cable)
        {
            // The cable's bone hangs on the body, which already turns by the
            // lean; the turn here is what is left of the cable's own swing.
            let turn = turn_2d(sub2(rest.control, rest.base), sub2(now.control, now.base));
            put(bone, [0.0, 0.0, -turn + lean * DEG], Z, Z);
        }
        MoodPose {
            mood: m,
            offsets,
            status: p.status,
            leds: p.leds,
            lit: p.lit,
            rays: p.rays,
            thrust: p.thrust,
            landed: p.landed,
        }
    }

    /// The idle layer: the body breathes and shifts its weight, the head
    /// glances (a pod's whole hull, by less), each antenna sways, and hover
    /// gear bobs while it hovers.
    fn idle(&self) -> Idle {
        let h = self.height;
        let mut extra: Vec<IdleExtra> = self
            .antennae
            .iter()
            .enumerate()
            .map(|(i, &(bone, side))| IdleExtra {
                bone,
                path: TrackPath::Rotation,
                amp: [0.0, 0.0, 3.0 * side],
                period: 2.7,
                phase: 0.23 * i as f64,
                hover_only: false,
            })
            .collect();
        if self.find("jet.0").is_some() {
            extra.push(IdleExtra {
                bone: self.body,
                path: TrackPath::Translation,
                amp: [0.0, 0.02 * h, 0.0],
                period: 2.2,
                phase: 0.0,
                hover_only: true,
            });
        }
        Idle {
            breathe_bone: self.body,
            breathe_amp: 0.006,
            breathe_lift: 0.004 * h,
            sway_bone: self.body,
            sway_degrees: 0.8,
            glance_bone: self.head,
            glance_yaw: if self.pod { 4.0 } else { 9.0 },
            glance_pitch: if self.pod { 1.5 } else { 4.0 },
            extra,
        }
    }

    fn arm_length(a: &ArmRig) -> f64 {
        let [s, e, w] = a.joints;
        distance(e, s) + distance(w, e)
    }

    /// How far a raised arm's shoulder slides up the body so its hand
    /// clears the top of the head: at most to the chassis top plus a third
    /// of the arm's length (a stub arm: 0.8 of it).
    fn shoulder_lift(&self, a: &ArmRig) -> f64 {
        let len = Self::arm_length(a);
        let s = a.joints[0][1];
        let clear = self.head_top + 0.06 * self.height - (s + 0.95 * len + 0.8 * self.hand);
        let reach = (self.chassis_top - s).max(0.0) + len * if self.stub { 0.8 } else { 1.0 / 3.0 };
        clear.min(reach).max(0.0)
    }

    /// Two set tracks turning an arm's shoulder and elbow so its segments
    /// point along each key's directions `(t, upper, fore)`.
    fn arm_keys(a: &ArmRig, k: &[(f64, V3, V3)]) -> [Track; 2] {
        let turns: Vec<(f64, (V3, V3))> = k
            .iter()
            .map(|(t, du, df)| (*t, arm_to(a, *du, *df)))
            .collect();
        let upper: Vec<(f64, V3)> = turns.iter().map(|(t, r)| (*t, r.0)).collect();
        let fore: Vec<(f64, V3)> = turns.iter().map(|(t, r)| (*t, r.1)).collect();
        [
            Track::new(a.upper, TrackPath::Rotation, Mode::Set, &upper),
            Track::new(a.fore, TrackPath::Rotation, Mode::Set, &fore),
        ]
    }

    /// Additive turns of every antenna about z, toward its own side.
    fn ant(&self, k: &[(f64, f64)]) -> Vec<Track> {
        self.antennae
            .iter()
            .map(|&(bone, side)| {
                let keys: Vec<(f64, V3)> =
                    k.iter().map(|(t, v)| (*t, [0.0, 0.0, v * side])).collect();
                track(bone, TrackPath::Rotation, Mode::Add, &keys)
            })
            .collect()
    }

    /// Additive head turns in degrees; a pod's head is its whole hull, so
    /// its gestures are scaled to 35%.
    fn head_turn(&self, k: &[(f64, V3)]) -> Track {
        let hk = if self.pod { 0.35 } else { 1.0 };
        let keys: Vec<(f64, V3)> = k.iter().map(|(t, v)| (*t, v.map(|x| x * hk))).collect();
        track(self.head, TrackPath::Rotation, Mode::Add, &keys)
    }

    fn clip(name: &'static str, duration: f64, tracks: Vec<Track>, envelope: [f64; 2]) -> Clip {
        Clip {
            name,
            duration,
            looping: false,
            envelope,
            tracks,
            blink: Vec::new(),
            glow: Vec::new(),
            wide: Vec::new(),
            face: None,
            peak: None,
            channel: None,
        }
    }

    /// The five reactions and the four status loops.
    ///
    /// Arms are posed by direction (model space: +x right, +y up, +z toward
    /// the viewer) as set tracks, so a reaction takes an arm over from
    /// whatever the mood had it doing. The side nearer the stage camera is
    /// +x.
    fn clips(&self) -> (Vec<Clip>, Vec<Clip>) {
        let h = self.height;
        let s = self.params.lead;
        let arms = &self.arms;
        let cam = arms.iter().find(|a| a.side == 1.0).or(arms.first());
        let lead = arms.iter().find(|a| a.side == s).or(arms.first());
        let (root, body) = (self.root, self.body);
        let rest_upper = |a: &ArmRig| normalize(sub(a.joints[1], a.joints[0]));
        let rest_fore = |a: &ArmRig| sub(a.joints[2], a.joints[1]);
        let mut reactions = Vec::new();

        // hello: the camera-side arm swings up above the shoulder (sliding
        // the shoulder up as for ask) and waves through 50 degrees, two full
        // swings held 0.32-1.2 s; the head tips toward it, the body bobs,
        // the happy face, the antennae bounce. Its channel is the shoulder's
        // lift, held over the wave.
        {
            let mut tracks = Vec::new();
            let mut channel = None;
            if let Some(a) = cam {
                let sd = a.side;
                let up = dir(sd * 0.45, 1.0, 0.2);
                let (wa, wb) = (swing(up, 25.0 * sd), swing(up, -25.0 * sd));
                let lift = self.shoulder_lift(a);
                tracks.extend(Self::arm_keys(
                    a,
                    &[
                        (0.32, up, up),
                        (0.48, wa, wa),
                        (0.66, wb, wb),
                        (0.84, wa, wa),
                        (1.02, wb, wb),
                        (1.2, up, up),
                    ],
                ));
                channel = Some(Channel::Track {
                    track: tracks.len(),
                    component: 1,
                    sign: 1.0,
                });
                tracks.push(track(
                    a.upper,
                    TrackPath::Translation,
                    Mode::Add,
                    &[ty(0.0, 0.0), ty(0.3, lift), ty(1.2, lift), ty(1.6, 0.0)],
                ));
            }
            let sd = cam.map_or(1.0, |a| a.side);
            tracks.push(track(
                body,
                TrackPath::Translation,
                Mode::Add,
                &[
                    ty(0.0, 0.0),
                    ty(0.12, -0.012 * h),
                    ty(0.3, 0.012 * h),
                    ty(0.5, 0.0),
                    ty(1.6, 0.0),
                ],
            ));
            tracks.push(track(
                body,
                TrackPath::Rotation,
                Mode::Add,
                &[
                    (0.0, Z),
                    (0.48, [0.0, 0.0, -2.0 * sd]),
                    (0.66, [0.0, 0.0, 2.0 * sd]),
                    (0.84, [0.0, 0.0, -2.0 * sd]),
                    (1.02, [0.0, 0.0, 2.0 * sd]),
                    (1.3, Z),
                    (1.6, Z),
                ],
            ));
            tracks.push(self.head_turn(&[
                (0.0, Z),
                (0.35, [-4.0, 10.0 * sd, -8.0 * sd]),
                (1.15, [-4.0, 10.0 * sd, -8.0 * sd]),
                (1.6, Z),
            ]));
            tracks.extend(self.ant(&[
                (0.0, 0.0),
                (0.3, -8.0),
                (0.5, 6.0),
                (0.7, -6.0),
                (0.9, 6.0),
                (1.1, -3.0),
                (1.3, 0.0),
                (1.6, 0.0),
            ]));
            let mut c = Self::clip("hello", 1.6, tracks, [0.3, 0.35]);
            c.face = Some(Mood::Happy);
            c.peak = Some(0.66);
            c.channel = channel;
            reactions.push(c);
        }

        // ask: the camera-side arm goes straight up (its shoulder sliding up
        // if the arm is short) and holds from 0.35 s to 1.3 s with a small
        // wave at the top; a hop at the start; the body rises and leans a
        // little toward that side, the head cocks toward the camera, the
        // waiting face with the eyes opened wide, and the lights pulse. Its
        // channel is the eyes' widening, held over the question.
        {
            let mut tracks = Vec::new();
            let sd = cam.map_or(1.0, |a| a.side);
            if let Some(a) = cam {
                let up = dir(sd * 0.12, 1.0, 0.12);
                let lift = self.shoulder_lift(a);
                tracks.extend(Self::arm_keys(a, &[(0.0, up, up)]));
                tracks.push(track(
                    a.upper,
                    TrackPath::Translation,
                    Mode::Add,
                    &[ty(0.0, 0.0), ty(0.3, lift), ty(1.3, lift), ty(1.7, 0.0)],
                ));
                tracks.push(track(
                    a.wrist,
                    TrackPath::Rotation,
                    Mode::Add,
                    &[
                        (0.0, Z),
                        (0.45, Z),
                        (0.6, [0.0, 0.0, 18.0 * sd]),
                        (0.75, [0.0, 0.0, -14.0 * sd]),
                        (0.9, [0.0, 0.0, 18.0 * sd]),
                        (1.05, [0.0, 0.0, -10.0 * sd]),
                        (1.2, Z),
                        (1.7, Z),
                    ],
                ));
            }
            tracks.push(track(
                root,
                TrackPath::Translation,
                Mode::Add,
                &[
                    ty(0.0, 0.0),
                    ty(0.08, 0.0),
                    ty(0.2, 0.05 * h),
                    ty(0.32, 0.0),
                    ty(1.7, 0.0),
                ],
            ));
            tracks.push(track(
                body,
                TrackPath::Translation,
                Mode::Add,
                &[
                    ty(0.0, 0.0),
                    ty(0.35, 0.02 * h),
                    ty(1.3, 0.02 * h),
                    ty(1.7, 0.0),
                ],
            ));
            tracks.push(track(
                body,
                TrackPath::Rotation,
                Mode::Add,
                &[
                    (0.0, Z),
                    (0.35, [4.0, 0.0, -3.0 * sd]),
                    (1.3, [4.0, 0.0, -3.0 * sd]),
                    (1.7, Z),
                ],
            ));
            tracks.push(self.head_turn(&[
                (0.0, Z),
                (0.35, [-5.0, 12.0, -9.0]),
                (1.3, [-5.0, 12.0, -9.0]),
                (1.7, Z),
            ]));
            tracks.extend(self.ant(&[
                (0.0, 0.0),
                (0.25, 10.0),
                (0.4, -4.0),
                (0.55, 6.0),
                (0.7, 0.0),
                (1.7, 0.0),
            ]));
            let mut c = Self::clip("ask", 1.7, tracks, [0.3, 0.4]);
            c.face = Some(Mood::Waiting);
            c.wide = keys(&[(0.0, 0.0), (0.3, 1.0), (1.3, 1.0), (1.6, 0.0)]);
            c.peak = Some(0.7);
            c.glow = keys(&[
                (0.0, 0.0),
                (0.3, 1.0),
                (0.55, 0.5),
                (0.8, 1.0),
                (1.05, 0.5),
                (1.3, 1.0),
                (1.7, 0.0),
            ]);
            c.channel = Some(Channel::Wide);
            reactions.push(c);
        }

        // done: a celebration jump with both arms thrown up and a squash on
        // landing; the lights flash. Its channel is the root's height, at
        // its apex at 0.42 s.
        {
            let jump = [0.0, 0.16, 0.24, 0.32, 0.42, 0.52, 0.6, 0.68, 1.3];
            let hj = [0.0, 0.0, 0.52, 0.85, 1.0, 0.85, 0.52, 0.0, 0.0];
            let rise: Vec<(f64, V3)> = jump
                .iter()
                .zip(hj)
                .map(|(t, k)| ty(*t, k * 0.32 * h))
                .collect();
            let mut tracks = vec![track(root, TrackPath::Translation, Mode::Add, &rise)];
            tracks.push(track(
                body,
                TrackPath::Scale,
                Mode::Add,
                &[
                    sq(0.0, 0.0),
                    sq(0.14, -0.1),
                    sq(0.24, 0.06),
                    sq(0.62, 0.03),
                    sq(0.71, -0.12),
                    sq(0.84, 0.03),
                    sq(1.0, 0.0),
                    sq(1.3, 0.0),
                ],
            ));
            for a in arms {
                let cheer = dir(a.side * 0.55, 1.0, 0.1);
                tracks.extend(Self::arm_keys(
                    a,
                    &[
                        (0.22, cheer, cheer),
                        (0.62, cheer, cheer),
                        (
                            0.8,
                            dir(a.side * 0.9, 0.5, 0.2),
                            dir(a.side * 0.3, 1.0, 0.1),
                        ),
                    ],
                ));
            }
            tracks.push(self.head_turn(&[
                (0.0, Z),
                (0.3, [-8.0, 0.0, 0.0]),
                (0.6, [-6.0, 0.0, 0.0]),
                (0.74, [6.0, 0.0, 0.0]),
                (0.95, [-2.0, 0.0, 0.0]),
                (1.3, Z),
            ]));
            tracks.extend(self.ant(&[
                (0.0, 0.0),
                (0.24, 10.0),
                (0.6, 8.0),
                (0.72, -14.0),
                (0.86, 8.0),
                (1.0, -3.0),
                (1.15, 0.0),
                (1.3, 0.0),
            ]));
            let mut c = Self::clip("done", 1.3, tracks, [0.2, 0.4]);
            c.blink = keys(&[
                (0.0, 0.0),
                (0.66, 0.0),
                (0.71, 1.0),
                (0.82, 1.0),
                (0.92, 0.0),
            ]);
            c.peak = Some(0.42);
            c.glow = keys(&[(0.0, 0.0), (0.3, 1.0), (0.6, 1.0), (0.9, 0.0)]);
            c.channel = Some(Channel::Track {
                track: 0,
                component: 1,
                sign: 1.0,
            });
            reactions.push(c);
        }

        // blocked: two frustrated stomps with fists pumped down and the head
        // shaking "no", then a small slump with the antennae drooping. Its
        // channel is the body's squash, deepest at each stomp; the second,
        // at 0.42 s, is the peak.
        {
            let rise = [
                ty(0.0, 0.0),
                ty(0.08, 0.03 * h),
                ty(0.16, 0.0),
                ty(0.34, 0.03 * h),
                ty(0.42, 0.0),
                ty(1.7, 0.0),
            ];
            let mut tracks = vec![
                track(root, TrackPath::Translation, Mode::Add, &rise),
                track(
                    body,
                    TrackPath::Scale,
                    Mode::Add,
                    &[
                        sq(0.0, 0.0),
                        sq(0.08, 0.03),
                        sq(0.16, -0.08),
                        sq(0.26, 0.0),
                        sq(0.34, 0.03),
                        sq(0.42, -0.08),
                        sq(0.55, 0.0),
                        sq(1.7, 0.0),
                    ],
                ),
            ];
            for a in arms {
                let fist = dir(a.side * 0.3, -1.0, 0.45);
                let fore = dir(a.side * 0.15, -0.2, 1.0);
                tracks.extend(Self::arm_keys(
                    a,
                    &[
                        (0.0, fist, fore),
                        (0.8, fist, fore),
                        (
                            1.1,
                            lerp3(rest_upper(a), fist, 0.3),
                            normalize(rest_fore(a)),
                        ),
                    ],
                ));
            }
            tracks.push(self.head_turn(&[
                (0.0, Z),
                (0.1, [4.0, 18.0, 0.0]),
                (0.22, [4.0, -18.0, 0.0]),
                (0.34, [4.0, 16.0, 0.0]),
                (0.46, [4.0, -14.0, 0.0]),
                (0.58, [4.0, 8.0, 0.0]),
                (0.75, [5.0, 0.0, 0.0]),
                (1.1, [10.0, 0.0, 0.0]),
                (1.4, [10.0, 0.0, 0.0]),
                (1.7, Z),
            ]));
            tracks.push(track(
                body,
                TrackPath::Translation,
                Mode::Add,
                &[
                    ty(0.0, 0.0),
                    ty(0.75, 0.0),
                    ty(1.1, -0.02 * h),
                    ty(1.4, -0.02 * h),
                    ty(1.7, 0.0),
                ],
            ));
            tracks.push(track(
                body,
                TrackPath::Rotation,
                Mode::Add,
                &[
                    (0.0, Z),
                    (0.75, Z),
                    (1.1, [5.0, 0.0, 0.0]),
                    (1.4, [5.0, 0.0, 0.0]),
                    (1.7, Z),
                ],
            ));
            tracks.extend(self.ant(&[
                (0.0, 0.0),
                (0.16, 14.0),
                (0.28, -8.0),
                (0.42, 14.0),
                (0.55, -6.0),
                (0.8, 0.0),
                (1.1, -16.0),
                (1.4, -16.0),
                (1.7, 0.0),
            ]));
            let mut c = Self::clip("blocked", 1.7, tracks, [0.15, 0.35]);
            c.blink = keys(&[(0.0, 0.0), (1.05, 0.0), (1.11, 0.7), (1.21, 0.0)]);
            c.peak = Some(0.42);
            c.channel = Some(Channel::Track {
                track: 1,
                component: 1,
                sign: -1.0,
            });
            reactions.push(c);
        }

        // poke: startled by the click. The body jolts back and up, the arms
        // jerk out, the eyes shut, the antennae spring, then it wobbles back
        // to rest. Its channel is the startled blink.
        {
            let mut tracks = vec![
                track(
                    body,
                    TrackPath::Rotation,
                    Mode::Add,
                    &[
                        (0.0, Z),
                        (0.07, [-8.0, 0.0, 0.0]),
                        (0.22, [3.0, 0.0, 3.0 * s]),
                        (0.38, [-1.5, 0.0, -2.0 * s]),
                        (0.54, [0.5, 0.0, s]),
                        (0.75, Z),
                        (1.0, Z),
                    ],
                ),
                track(
                    body,
                    TrackPath::Scale,
                    Mode::Add,
                    &[
                        sq(0.0, 0.0),
                        sq(0.06, -0.06),
                        sq(0.16, 0.05),
                        sq(0.3, -0.02),
                        sq(0.45, 0.0),
                        sq(1.0, 0.0),
                    ],
                ),
                track(
                    body,
                    TrackPath::Translation,
                    Mode::Add,
                    &[ty(0.0, 0.0), ty(0.08, 0.02 * h), ty(0.2, 0.0), ty(1.0, 0.0)],
                ),
            ];
            for a in arms {
                let jerk = dir(a.side * 0.75, -0.4, 0.3);
                tracks.extend(Self::arm_keys(
                    a,
                    &[
                        (0.1, jerk, jerk),
                        (0.34, lerp3(rest_upper(a), jerk, 0.35), rest_fore(a)),
                    ],
                ));
            }
            tracks.push(self.head_turn(&[
                (0.0, Z),
                (0.08, [-6.0, 0.0, 5.0 * s]),
                (0.3, [2.0, 0.0, -3.0 * s]),
                (0.55, [0.0, 0.0, s]),
                (0.8, Z),
                (1.0, Z),
            ]));
            tracks.extend(self.ant(&[
                (0.0, 0.0),
                (0.07, 18.0),
                (0.2, -14.0),
                (0.34, 9.0),
                (0.48, -5.0),
                (0.62, 2.0),
                (0.8, 0.0),
                (1.0, 0.0),
            ]));
            let mut c = Self::clip("poke", 1.0, tracks, [0.06, 0.55]);
            c.blink = keys(&[(0.0, 0.0), (0.03, 1.0), (0.12, 1.0), (0.24, 0.0)]);
            c.peak = Some(0.1);
            c.channel = Some(Channel::Blink);
            reactions.push(c);
        }

        (reactions, self.loops(lead))
    }

    /// The four status loops.
    fn loops(&self, lead: Option<&ArmRig>) -> Vec<Clip> {
        let h = self.height;
        let body = self.body;
        let looping = |name, duration, tracks| {
            let mut c = Self::clip(name, duration, tracks, [0.01, 0.01]);
            c.looping = true;
            c
        };
        // A forearm's tap; an arm without an elbow taps at the shoulder, by
        // half as much.
        let fore = |a: &ArmRig, k: &[(f64, V3)]| {
            if a.elbow {
                track(a.fore, TrackPath::Rotation, Mode::Add, k)
            } else {
                let half: Vec<(f64, V3)> =
                    k.iter().map(|(t, v)| (*t, v.map(|x| x * 0.5))).collect();
                track(a.upper, TrackPath::Rotation, Mode::Add, &half)
            }
        };
        let tap = [-12.0, 0.0, 0.0];

        // working: the arms tap in turn, the head nods, the body bobs.
        let mut tracks: Vec<Track> = self
            .arms
            .iter()
            .enumerate()
            .map(|(i, a)| {
                if i == 0 {
                    fore(a, &[(0.0, Z), (0.3, tap), (0.6, Z), (1.2, Z)])
                } else {
                    fore(a, &[(0.0, Z), (0.6, Z), (0.9, tap), (1.2, Z)])
                }
            })
            .collect();
        tracks.push(self.head_turn(&[
            (0.0, [3.0, 0.0, 0.0]),
            (0.3, [6.0, 0.0, 0.0]),
            (0.6, [3.0, 0.0, 0.0]),
            (0.9, [6.0, 0.0, 0.0]),
            (1.2, [3.0, 0.0, 0.0]),
        ]));
        tracks.push(track(
            body,
            TrackPath::Translation,
            Mode::Add,
            &[
                ty(0.0, 0.0),
                ty(0.3, -0.006 * h),
                ty(0.6, 0.0),
                ty(0.9, -0.006 * h),
                ty(1.2, 0.0),
            ],
        ));
        let working = looping("loop.working", 1.2, tracks);

        // waiting: slow looks left and right, a slow lean, and the leading
        // hand taps twice.
        let mut tracks = vec![
            self.head_turn(&[
                (0.0, Z),
                (0.8, [-2.0, -14.0, 0.0]),
                (1.6, [-2.0, -14.0, 0.0]),
                (2.4, [-2.0, 12.0, 0.0]),
                (3.2, [-2.0, 12.0, 0.0]),
                (4.0, Z),
            ]),
            track(
                body,
                TrackPath::Rotation,
                Mode::Add,
                &[(0.0, Z), (2.0, [0.0, 0.0, 1.5]), (4.0, Z)],
            ),
        ];
        if let Some(a) = lead {
            tracks.push(fore(
                a,
                &[
                    (0.0, Z),
                    (2.6, Z),
                    (2.75, [-10.0, 0.0, 0.0]),
                    (2.9, Z),
                    (3.05, [-10.0, 0.0, 0.0]),
                    (3.2, Z),
                    (4.0, Z),
                ],
            ));
        }
        tracks.extend(self.ant(&[(0.0, 0.0), (2.0, 5.0), (4.0, 0.0)]));
        let waiting = looping("loop.waiting", 4.0, tracks);

        // blocked: slumped, the head down with a short shake each cycle,
        // the antennae drooping.
        let mut tracks = vec![
            self.head_turn(&[
                (0.0, [6.0, 0.0, 0.0]),
                (1.6, [6.0, 0.0, 0.0]),
                (1.75, [6.0, 12.0, 0.0]),
                (1.9, [6.0, -12.0, 0.0]),
                (2.05, [6.0, 9.0, 0.0]),
                (2.2, [6.0, 0.0, 0.0]),
                (2.8, [6.0, 0.0, 0.0]),
            ]),
            track(
                body,
                TrackPath::Translation,
                Mode::Add,
                &[ty(0.0, -0.01 * h), ty(2.8, -0.01 * h)],
            ),
        ];
        tracks.extend(self.ant(&[
            (0.0, -8.0),
            (1.6, -8.0),
            (1.8, -2.0),
            (2.2, -8.0),
            (2.8, -8.0),
        ]));
        let blocked = looping("loop.blocked", 2.8, tracks);

        // sleepy: slow deep breaths and a drooping nod.
        let mut tracks = vec![
            track(
                body,
                TrackPath::Scale,
                Mode::Add,
                &[sq(0.0, 0.0), sq(2.3, 0.012), sq(4.6, 0.0)],
            ),
            self.head_turn(&[
                (0.0, [3.0, 0.0, 0.0]),
                (3.2, [12.0, 0.0, 0.0]),
                (3.6, [4.0, 0.0, 0.0]),
                (4.6, [3.0, 0.0, 0.0]),
            ]),
        ];
        tracks.extend(self.ant(&[(0.0, -4.0), (3.2, -10.0), (4.6, -4.0)]));
        let sleepy = looping("loop.sleepy", 4.6, tracks);

        vec![working, waiting, blocked, sleepy]
    }
}
