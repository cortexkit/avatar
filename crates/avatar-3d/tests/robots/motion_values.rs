//! Motion values: face swaps and the lights that follow them, reaction
//! peaks, the working and waiting statuses, idle values worked by hand,
//! and one keyed value of each clip.

use avatar_3d::motion::{Actor, Mode, MotionParams, Reaction, StatusLoop, TrackPath, MOTION_BYTE};
use avatar_3d::robot::{build_with, BuildOptions, Robot};
use avatar_small_robots::layout::{Layout, Status};
use avatar_small_robots::{decode, Mood, GENOME_LEN, MOODS};
use avatar_svg::wild::math::sin;

use crate::cases;
use crate::motion::name;
use crate::scripts::{self, Script};

/// The corpus genomes' robots and layouts.
fn corpus() -> Vec<(String, Robot, Layout)> {
    cases::corpus()
        .into_iter()
        .map(|(id, g)| {
            let robot = build_with(&g, BuildOptions::untextured());
            (id, robot, Layout::new(&decode(&g)))
        })
        .collect()
}

/// Panics unless the pose's lights are the 2D flags of its face set's mood.
fn lights_follow_the_face(layout: &Layout, pose: &avatar_3d::motion::Pose, label: &str) {
    let m = layout.mood(pose.face);
    assert_eq!(
        (pose.status, pose.leds, pose.lit, pose.rays),
        (m.status, m.leds, m.lit, m.rays),
        "{label}: the lights are not the {} face's",
        name(pose.face)
    );
}

#[test]
fn faces_swap_on_a_blink_and_the_lights_follow_them() {
    for (id, robot, layout) in corpus() {
        for mood in MOODS {
            let requested = scripts::next_mood(mood);
            for script in [Script::MoodChange, Script::Composed] {
                let actor = script.actor(&robot, mood);
                let mut shown = Vec::new();
                for t in script.samples(&robot) {
                    let label = format!("{id} {} {} at {t}", name(mood), script.name());
                    let pose = actor.evaluate(t);
                    lights_follow_the_face(&layout, &pose, &label);
                    if let Some(&last) = shown.last() {
                        if last != pose.face {
                            assert!(
                                pose.blink >= 0.5,
                                "{label}: the face changes with the eyes open ({})",
                                pose.blink
                            );
                        }
                    }
                    if shown.last() != Some(&pose.face) {
                        shown.push(pose.face);
                    }
                }
                let label = format!("{id} {} {}", name(mood), script.name());
                assert_eq!(shown[0], mood, "{label}: the first face");
                assert_eq!(
                    shown.iter().filter(|m| **m == requested).count(),
                    1,
                    "{label}: faces {shown:?}"
                );
                assert_eq!(shown.last(), Some(&requested), "{label}: faces {shown:?}");
                if script == Script::MoodChange {
                    assert_eq!(shown.len(), 2, "{label}: faces {shown:?}");
                }
            }
            // An ask's waiting face brings the waiting lights with it, and a
            // mood change brings its own.
            let mut actor = Actor::new(&robot, mood);
            actor.play(0.2, Reaction::Ask);
            actor.set_mood(1.0, requested);
            let mut faces = Vec::new();
            for k in 0..=360 {
                let t = f64::from(k) / 120.0;
                let pose = actor.evaluate(t);
                lights_follow_the_face(&layout, &pose, &format!("{id} {} ask at {t}", name(mood)));
                faces.push(pose.face);
            }
            assert!(
                faces.contains(&Mood::Waiting),
                "{id} {}: no ask face",
                name(mood)
            );
        }
    }
}

#[test]
fn each_reactions_channel_is_largest_at_its_peak() {
    for (id, robot, _) in corpus() {
        for clip in &robot.motion.reactions {
            let peak = clip.peak.expect("every reaction has a peak");
            let at_peak = clip.channel_at(peak).expect("every reaction has a channel");
            let mut times: Vec<f64> = (0..)
                .map(|k| f64::from(k) / 120.0)
                .take_while(|t| *t <= clip.duration)
                .collect();
            times.push(peak);
            // A jump's apex is one instant; the other channels hold their
            // peak, so samples away from it may tie.
            let strict = clip.name == "done";
            for t in times {
                let v = clip.channel_at(t).unwrap();
                if strict && t != peak {
                    assert!(
                        v < at_peak,
                        "{id} {}: {v} at {t} reaches the peak's {at_peak}",
                        clip.name
                    );
                } else {
                    assert!(
                        v <= at_peak + 1e-12 * at_peak.abs().max(1.0),
                        "{id} {}: {v} at {t} exceeds the peak's {at_peak}",
                        clip.name
                    );
                }
            }
        }
    }
}

#[test]
fn working_and_waiting_both_warn_with_their_own_leds() {
    use Status::{Off, Warn};
    for (id, robot, layout) in corpus() {
        for (mood, leds) in [
            (Mood::Working, [Warn, Warn, Warn, Off]),
            (Mood::Waiting, [Warn, Off, Off, Off]),
        ] {
            let actor = Actor::new(&robot, mood);
            for t in [0.0, 1.0, 2.5] {
                let pose = actor.evaluate(t);
                let m = layout.mood(mood);
                assert_eq!(
                    (pose.status, pose.leds, pose.lit, pose.rays),
                    (Warn, leds, m.lit, m.rays),
                    "{id} {} at {t}",
                    name(mood)
                );
            }
        }
    }
}

/// lowbias32 of `a` mixed with `b`, written out for the test.
fn hash(a: u32, b: i64) -> f64 {
    let mut x = a ^ ((b + 0x9e37_79b9) as u32).wrapping_mul(0x85eb_ca6b);
    x = (x ^ (x >> 16)).wrapping_mul(0x7feb_352d);
    x = (x ^ (x >> 15)).wrapping_mul(0x846c_a68b);
    f64::from(x ^ (x >> 16)) / 4_294_967_296.0
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// One idle blink `x` s after it starts: 0.07 s closing, 0.03 s shut,
/// 0.16 s opening.
fn shape(x: f64) -> f64 {
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

/// Ten motion bytes and the parameters worked from them: tempo, breath
/// phase, sway phase and blink period; glance and blink seeds; lead and
/// energy.
type Pick = ([u8; 10], [f64; 4], [u32; 2], [f64; 2]);

#[test]
fn idle_values_equal_hand_worked_values() {
    let (_, base) = cases::corpus().into_iter().next().unwrap();
    // Motion bytes chosen by hand: tempo, breath phase, sway phase, glance
    // seed (two bytes), blink seed (two bytes), blink period, lead, energy.
    // Each with its parameters worked from the stated formulas: tempo
    // 0.85 + 0.3 b0/255, phases b/256, seeds of two bytes, period
    // 3 + 2.5 b7/255, lead by b8's low bit, energy 0.8 + 0.4 b9/255.
    let picks: [Pick; 2] = [
        (
            [0, 0, 64, 0, 0, 0, 0, 0, 1, 255],
            [0.85, 0.0, 0.25, 3.0],
            [0, 0],
            [1.0, 0.8 + 0.4],
        ),
        (
            [255, 128, 0, 0x34, 0x12, 0x78, 0x56, 255, 0, 0],
            [0.85 + 0.3, 0.5, 0.0, 5.5],
            [0x1234, 0x5678],
            [-1.0, 0.8],
        ),
    ];
    let tau = std::f64::consts::TAU;
    for (bytes, [tempo, breath_phase, sway_phase, period], [glance, blink], [lead, energy]) in picks
    {
        let mut genome: Box<[u8; GENOME_LEN]> = base.clone();
        genome[MOTION_BYTE..MOTION_BYTE + 10].copy_from_slice(&bytes);
        let m = MotionParams::new(&genome);
        assert_eq!(
            (m.tempo, m.breath_phase, m.sway_phase, m.blink_period),
            (tempo, breath_phase, sway_phase, period)
        );
        assert_eq!((m.glance_seed, m.blink_seed), (glance, blink));
        assert_eq!((m.lead, m.energy), (lead, energy));
        // The motion bytes change nothing a robot looks like.
        let robot = build_with(&genome, BuildOptions::untextured());
        let plain = build_with(&base, BuildOptions::untextured());
        assert_eq!(
            avatar_3d::robot::mesh_bytes(&robot).unwrap(),
            avatar_3d::robot::mesh_bytes(&plain).unwrap()
        );
        assert_eq!(robot.motion.params, m);
        for t in [0.0, 1.3, 2.75] {
            assert_eq!(m.breath(t), sin(tau * (t * tempo / 3.6 + breath_phase)));
            assert_eq!(m.sway(t), sin(tau * (t * tempo / 7.3 + sway_phase)));
            // The glance slot and its target, turned to over the first 22%.
            let x = t * tempo / 5.5;
            let k = x.floor();
            let target = |j: i64| {
                if hash(glance, j) < 0.35 {
                    [0.0, 0.0]
                } else {
                    [
                        2.0 * hash(glance, j + 7919) - 1.0,
                        0.6 * (2.0 * hash(glance, j + 104_729) - 1.0),
                    ]
                }
            };
            let (a, b) = (target(k as i64 - 1), target(k as i64));
            let s = smooth((x - k) / 0.22);
            assert_eq!(
                m.glance(t),
                [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s]
            );
            // Blinks every period, jittered by up to 30% of it, some doubled.
            let k0 = (t / period).floor() as i64;
            let mut want: f64 = 0.0;
            for k in k0 - 1..=k0 + 1 {
                let tk = k as f64 * period + (hash(blink, k) - 0.5) * 0.6 * period;
                want = want.max(shape(t - tk));
                if hash(blink, k + 50021) < 0.18 {
                    want = want.max(shape(t - tk - 0.32));
                }
            }
            assert_eq!(m.blink(t), want, "blink at {t}");
        }
    }
}

#[test]
fn each_clip_matches_a_reference_value() {
    let deg = std::f64::consts::PI / 180.0;
    for (id, robot, layout) in corpus() {
        let m = &robot.motion;
        let h = robot
            .parts
            .iter()
            .flat_map(|p| p.mesh.positions.iter())
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        assert_eq!(m.height, h, "{id}: the rest height");
        let bone = |n: &str| robot.bone(n).unwrap();
        // A pod's head is its whole hull: head gestures turn its body, at 35%.
        let (head, hk) = if layout.chassis.is_none() {
            (bone("body"), 0.35)
        } else {
            (bone("head"), 1.0)
        };
        let (root, body) = (bone("root"), bone("body"));
        use TrackPath::{Rotation as R, Scale as S, Translation as T};
        let reactions: [(Reaction, usize, TrackPath, f64, [f64; 3]); 5] = [
            (Reaction::Hello, body, T, 0.12, [0.0, -0.012 * h, 0.0]),
            (Reaction::Ask, root, T, 0.2, [0.0, 0.05 * h, 0.0]),
            (Reaction::Done, root, T, 0.42, [0.0, 0.32 * h, 0.0]),
            (Reaction::Blocked, body, S, 0.42, [0.04, -0.08, 0.04]),
            (Reaction::Poke, body, R, 0.07, [-8.0 * deg, 0.0, 0.0]),
        ];
        for (r, b, path, t, want) in reactions {
            let clip = m.reaction(r);
            let track = clip.track(b, path, Mode::Add).unwrap();
            assert_eq!(
                track.sample(t, false, clip.duration),
                want,
                "{id} {} at {t}",
                r.name()
            );
        }
        let loops: [(StatusLoop, usize, TrackPath, f64, [f64; 3]); 4] = [
            (
                StatusLoop::Working,
                head,
                R,
                0.3,
                [6.0 * hk * deg, 0.0, 0.0],
            ),
            (
                StatusLoop::Waiting,
                head,
                R,
                0.8,
                [-2.0 * hk * deg, -14.0 * hk * deg, 0.0],
            ),
            (StatusLoop::Blocked, body, T, 2.8, [0.0, -0.01 * h, 0.0]),
            (
                StatusLoop::Sleepy,
                head,
                R,
                3.2,
                [12.0 * hk * deg, 0.0, 0.0],
            ),
        ];
        for (l, b, path, t, want) in loops {
            let clip = m.status_loop(l);
            let track = clip
                .tracks
                .iter()
                .find(|tr| tr.bone == b && tr.path == path && tr.mode == Mode::Add)
                .unwrap();
            assert_eq!(
                track.sample(t, true, clip.duration),
                want,
                "{id} loop.{} at {t}",
                l.name()
            );
        }
    }
}
