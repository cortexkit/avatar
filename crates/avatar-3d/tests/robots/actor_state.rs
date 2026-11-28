//! The owned actor state: face moments list every change of face exactly,
//! and forgetting old events changes nothing at or after the cutoff.

use std::time::{Duration, Instant};

use avatar_3d::motion::{
    pose_bytes, Actor, ActorState, Clip, Reaction, StatusLoop, IDLE_BLEND, LOOP_BLEND, MOOD_BLEND,
    REACTIONS, REACT_CUT, STATUS_LOOPS, SWAP_LEAD, SWAP_TAIL,
};
use avatar_3d::robot::{build_with, BuildOptions, Robot};
use avatar_small_robots::{Mood, MOODS};

use crate::cases;

/// The first corpus robot, untextured.
fn robot() -> Robot {
    let (_, genome) = cases::corpus()
        .into_iter()
        .next()
        .expect("the corpus has a genome");
    build_with(&genome, BuildOptions::untextured())
}

/// The float just below `t`, for a positive `t`.
fn just_before(t: f64) -> f64 {
    assert!(t > 0.0);
    f64::from_bits(t.to_bits() - 1)
}

/// `actor`'s face moments after `from`, checked against its faces: each
/// listed time is later than the one before, the face changes there to
/// the listed mood (from the float just below it, and from 1e-6 s before
/// it to 1e-6 s after it), and the face at every 1/120 s from `from` to
/// `end` is the one the list gives.
fn checked_moments(label: &str, actor: &Actor, from: f64, end: f64) -> Vec<(f64, Mood)> {
    let moments = actor.face_moments(from);
    let mut last = from;
    for &(m, mood) in &moments {
        assert!(m > last, "{label}: {moments:?} come in order after {from}");
        last = m;
        assert_eq!(actor.face(m), mood, "{label}: the face at {m}");
        assert_ne!(
            actor.face(just_before(m)),
            mood,
            "{label}: the face just before {m}"
        );
        assert_ne!(
            actor.face(m - 1e-6),
            actor.face(m + 1e-6),
            "{label}: the face changes at {m}"
        );
        assert_eq!(actor.face(m + 1e-6), mood, "{label}: the face after {m}");
    }
    let start = actor.face(from);
    for k in 0.. {
        let t = from + f64::from(k) / 120.0;
        if t > end {
            break;
        }
        let listed = moments
            .iter()
            .rev()
            .find(|e| e.0 <= t)
            .map_or(start, |e| e.1);
        assert_eq!(actor.face(t), listed, "{label}: the face at {t}");
    }
    moments
}

fn moods_of(moments: &[(f64, Mood)]) -> Vec<Mood> {
    moments.iter().map(|e| e.1).collect()
}

#[test]
fn face_moments_list_every_change_of_face_exactly() {
    let robot = robot();
    let motion = &robot.motion;
    let hello = motion.reaction(Reaction::Hello);
    let ask = motion.reaction(Reaction::Ask);
    // The cases below rest on these faces.
    assert_eq!(hello.face, Some(Mood::Happy));
    assert_eq!(ask.face, Some(Mood::Waiting));
    assert_eq!(motion.reaction(Reaction::Done).face, None);
    let end = 6.0;

    // A mood change swaps the face on a blink after it.
    let mut a = Actor::new(&robot, Mood::Neutral);
    a.set_mood(1.0, Mood::Happy);
    let m = checked_moments("mood change", &a, 0.0, end);
    assert_eq!(m, vec![(1.0 + SWAP_LEAD, Mood::Happy)]);
    assert!(m[0].0 - 1.0 >= SWAP_LEAD);

    // A reaction with another mood's face swaps it in and out.
    let mut a = Actor::new(&robot, Mood::Neutral);
    a.play(1.0, Reaction::Hello);
    let m = checked_moments("hello from neutral", &a, 0.0, end);
    assert_eq!(moods_of(&m), [Mood::Happy, Mood::Neutral]);
    assert!(m[0].0 - 1.0 >= SWAP_LEAD, "{m:?}");
    assert!(m[1].0 < 1.0 + hello.duration, "{m:?}");

    // A reaction without a face, or with the face already shown, changes
    // nothing.
    let mut a = Actor::new(&robot, Mood::Neutral);
    a.play(1.0, Reaction::Done);
    assert_eq!(checked_moments("done", &a, 0.0, end), []);
    let mut a = Actor::new(&robot, Mood::Happy);
    a.play(1.0, Reaction::Hello);
    assert_eq!(checked_moments("hello from happy", &a, 0.0, end), []);

    // A status loop changes no face.
    for l in STATUS_LOOPS {
        let mut a = Actor::new(&robot, Mood::Neutral);
        a.set_loop(0.5, Some(l));
        assert_eq!(checked_moments(l.name(), &a, 0.0, end), []);
    }

    // A reaction interrupted by one with its own face hands over at that
    // one's swap-in, which comes a blink after it starts.
    let mut a = Actor::new(&robot, Mood::Neutral);
    a.play(1.0, Reaction::Hello);
    a.play(1.5, Reaction::Ask);
    let m = checked_moments("hello cut by ask", &a, 0.0, end);
    assert_eq!(moods_of(&m), [Mood::Happy, Mood::Waiting, Mood::Neutral]);
    assert!(m[1].0 - 1.5 >= SWAP_LEAD, "{m:?}");
    // Moments are strictly after `from`: from one of them, it is left out.
    assert_eq!(checked_moments("from a moment", &a, m[0].0, end), m[1..]);
    assert_eq!(checked_moments("from between", &a, 1.3, end), m[1..]);
    assert_eq!(checked_moments("from the end", &a, m[2].0, end), []);

    // Interrupted by one without a face, the face goes back to the mood a
    // blink after the interruption.
    let mut a = Actor::new(&robot, Mood::Neutral);
    a.play(1.0, Reaction::Hello);
    a.play(1.5, Reaction::Done);
    let m = checked_moments("hello cut by done", &a, 0.0, end);
    assert_eq!(moods_of(&m), [Mood::Happy, Mood::Neutral]);
    assert_eq!(m[1].0, 1.5 + SWAP_LEAD);

    // A mood swap under a reaction showing the same face is no change, nor
    // is the face going back to that mood when the reaction ends.
    let mut a = Actor::new(&robot, Mood::Neutral);
    a.play(1.0, Reaction::Hello);
    a.set_mood(1.2, Mood::Happy);
    let m = checked_moments("mood under hello", &a, 0.0, end);
    assert_eq!(moods_of(&m), [Mood::Happy]);

    // Two mood changes at one time that end where they began are no change.
    let mut a = Actor::new(&robot, Mood::Neutral);
    a.set_mood(1.0, Mood::Happy);
    a.set_mood(1.0, Mood::Neutral);
    assert_eq!(checked_moments("there and back", &a, 0.0, end), []);
}

/// One call to an actor.
#[derive(Clone, Copy, Debug)]
enum Event {
    Mood(Mood),
    Loop(Option<StatusLoop>),
    Play(Reaction),
    Idle(bool),
}

fn send(state: &mut ActorState, at: f64, event: Event) {
    match event {
        Event::Mood(m) => state.set_mood(at, m),
        Event::Loop(l) => state.set_loop(at, l),
        Event::Play(r) => state.play(at, r),
        Event::Idle(on) => state.set_idle(at, on),
    }
}

/// SplitMix64: a small deterministic random sequence.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> usize {
        usize::try_from(self.next() % n).expect("a small number")
    }

    /// A time within a second, on a 1 ms grid.
    fn within(&mut self) -> f64 {
        f64::from(u32::try_from(self.next() % 1000).expect("below 1000")) / 1000.0
    }
}

/// How many events the pruning script sends.
const EVENTS: usize = 10_000;
/// How far behind the stage time the pruned actor forgets.
const KEEP: f64 = 10.0;

/// The pruning script: [`EVENTS`] calls in time order, every one a real
/// change, over about 1,100 s. Seconds alternate in runs of five between
/// calm (up to two reactions and one mood change) and busy (6 to 15
/// reactions and 3 to 9 mood changes); the idle layer toggles every six
/// seconds. The working loop runs from 2.25 s to 400.7 s, where it stops;
/// the waiting loop replaces it at 400.9 s, inside the stop's fade, and
/// the blocked loop replaces that at 600.4 s; that stops at 800.6 s, and
/// the sleepy loop starts at 850.5 s. Every 40 s, four mood changes come
/// in the last 0.05 s before a whole second, and 10 s later a mood change
/// 0.2 s before a whole second blends across it. Returns the script and
/// those whole seconds: the burst cutoffs and the blend cutoffs.
fn pruning_script() -> (Vec<(f64, Event)>, Vec<f64>, Vec<f64>) {
    let mut rng = Rng(0x0a17_5eed);
    let mut out: Vec<(f64, Event)> = Vec::new();
    let (mut bursts, mut blends) = (Vec::new(), Vec::new());
    let mut mood = Mood::Neutral;
    let mut idle = true;
    let mut s = 0_u32;
    while out.len() < EVENTS {
        let t0 = f64::from(s);
        let busy = s % 10 >= 5;
        let mut second: Vec<(f64, Event)> = Vec::new();
        let reactions = if busy {
            6 + rng.below(10)
        } else {
            rng.below(3)
        };
        for _ in 0..reactions {
            let r = REACTIONS[rng.below(REACTIONS.len() as u64)];
            second.push((t0 + rng.within(), Event::Play(r)));
        }
        // Mood changes get their moods once in time order, below.
        let moods = if busy { 3 + rng.below(7) } else { rng.below(2) };
        for _ in 0..moods {
            second.push((t0 + rng.within(), Event::Mood(mood)));
        }
        if s % 6 == 3 {
            idle = !idle;
            second.push((t0 + rng.within(), Event::Idle(idle)));
        }
        if s % 40 == 19 {
            for before in [0.045, 0.03, 0.015, 0.001] {
                second.push((t0 + 1.0 - before, Event::Mood(mood)));
            }
            bursts.push(t0 + 1.0);
        }
        if s % 40 == 29 {
            second.push((t0 + 0.8, Event::Mood(mood)));
            blends.push(t0 + 1.0);
        }
        let status = match s {
            2 => Some((0.25, Some(StatusLoop::Working))),
            400 => Some((0.7, None)),
            600 => Some((0.4, Some(StatusLoop::Blocked))),
            800 => Some((0.6, None)),
            850 => Some((0.5, Some(StatusLoop::Sleepy))),
            _ => None,
        };
        if let Some((at, l)) = status {
            second.push((t0 + at, Event::Loop(l)));
        }
        if s == 400 {
            second.push((t0 + 0.9, Event::Loop(Some(StatusLoop::Waiting))));
        }
        second.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut last_play = f64::NEG_INFINITY;
        for (at, event) in second {
            let event = match event {
                Event::Mood(_) => {
                    let mut m = MOODS[rng.below(MOODS.len() as u64)];
                    if m == mood {
                        let i = MOODS.iter().position(|x| *x == m).expect("a listed mood");
                        m = MOODS[(i + 1) % MOODS.len()];
                    }
                    mood = m;
                    Event::Mood(m)
                }
                // Reactions keep distinct times, so each is found by its
                // time below.
                Event::Play(_) if at == last_play => continue,
                Event::Play(r) => {
                    last_play = at;
                    Event::Play(r)
                }
                e => e,
            };
            out.push((at, event));
        }
        s += 1;
    }
    out.truncate(EVENTS);
    (out, bursts, blends)
}

/// Reaction `i`'s face span in the full list of reactions sent, `(start,
/// end)`, as the motion module defines it: the face swaps in at the
/// envelope's rising midpoint, at least [`SWAP_LEAD`] after the start,
/// and out at its falling midpoint, or earlier when the next reaction
/// swaps its own face in, or starts without one ([`SWAP_LEAD`] after it).
fn span(robot: &Robot, sent: &[(f64, Reaction)], i: usize) -> Option<(f64, f64)> {
    let motion = &robot.motion;
    let swap_in = |c: &Clip, at: f64| at + SWAP_LEAD.max(c.envelope[0] / 2.0);
    let (at, r) = sent[i];
    let c = motion.reaction(r);
    c.face?;
    let start = swap_in(c, at);
    let mut end = at + c.duration - c.envelope[1] / 2.0;
    if let Some(&(next, nr)) = sent.get(i + 1) {
        let nc = motion.reaction(nr);
        end = end.min(if nc.face.is_some() {
            swap_in(nc, next)
        } else {
            next + SWAP_LEAD
        });
    }
    (end > start).then_some((start, end))
}

/// Panics unless every event `pruned` holds from before `cutoff` is the
/// current status loop, the idle setting, or an event whose blend, swap or
/// reaction ends after `cutoff`, judged against every event sent, which
/// `full` holds. A swap ends [`SWAP_TAIL`] after it.
fn check_retained(robot: &Robot, full: &ActorState, pruned: &ActorState, cutoff: f64) {
    for &(at, m) in pruned.mood_changes() {
        if at < cutoff {
            let blending = (cutoff - at) / MOOD_BLEND < 1.0;
            let swapping = cutoff - (at + SWAP_LEAD) < SWAP_TAIL;
            assert!(
                blending || swapping,
                "cutoff {cutoff}: the mood change to {m:?} at {at} is over"
            );
        }
    }

    let loops = full.loop_changes();
    for &(at, l) in pruned.loop_changes() {
        if at >= cutoff {
            continue;
        }
        let i = loops
            .iter()
            .position(|e| e.0 == at)
            .expect("a held loop change was sent");
        let fading = |next: f64| (cutoff - next) / LOOP_BLEND < 1.0;
        let in_effect = match l {
            // The loop is the current one, or fades out under the next.
            Some(_) => i + 1 == loops.len() || fading(loops[i + 1].0),
            // A stop, while the loop it stops fades out.
            None => fading(at),
        };
        assert!(in_effect, "cutoff {cutoff}: the loop {l:?} at {at} is over");
    }

    let setting = full
        .idle_changes()
        .iter()
        .rev()
        .find(|e| e.0 < cutoff && (cutoff - e.0) / IDLE_BLEND >= 1.0)
        .map(|e| e.0);
    for &(at, on) in pruned.idle_changes() {
        if at < cutoff {
            let blending = (cutoff - at) / IDLE_BLEND < 1.0;
            assert!(
                blending || Some(at) == setting,
                "cutoff {cutoff}: the idle change to {on} at {at} is over"
            );
        }
    }

    let sent: Vec<(f64, Reaction)> = full.reactions().collect();
    for (at, r) in pruned.reactions() {
        if at >= cutoff {
            continue;
        }
        let i = sent.partition_point(|e| e.0 < at);
        assert_eq!(sent[i], (at, r), "a held reaction was sent");
        let clip = robot.motion.reaction(r);
        let cut = sent
            .get(i + 1)
            .is_some_and(|&(next, _)| (cutoff - next) / REACT_CUT >= 1.0);
        let playing = !(cutoff - at > clip.duration || cut);
        let swapping = span(robot, &sent, i).is_some_and(|(_, end)| cutoff - end < SWAP_TAIL);
        assert!(
            playing || swapping,
            "cutoff {cutoff}: the reaction {r:?} at {at} is over"
        );
    }
}

/// The times compared after forgetting before `cutoff`: every 1/120 s for
/// the first 0.25 s, every 1/30 s to the next cutoff, and a few later ones
/// up to and past the stage time.
fn samples(cutoff: f64) -> Vec<f64> {
    let mut out: Vec<f64> = (0..30).map(|j| cutoff + f64::from(j) / 120.0).collect();
    out.extend((0..23).map(|j| cutoff + 0.25 + f64::from(j) / 30.0));
    out.extend([1.5, 4.25, 9.99, KEEP, KEEP + 0.5].map(|d| cutoff + d));
    out
}

fn median_evaluate(state: &ActorState, robot: &Robot, from: f64) -> Duration {
    let mut times: Vec<Duration> = (0..101)
        .map(|k| {
            let t = from + f64::from(k) / 120.0;
            let start = Instant::now();
            let pose = state.evaluate(robot, t);
            let took = start.elapsed();
            std::hint::black_box(pose);
            took
        })
        .collect();
    times.sort();
    times[50]
}

#[test]
fn forgetting_old_events_changes_nothing_at_or_after_the_cutoff() {
    let robot = robot();
    let (script, bursts, blends) = pruning_script();
    assert_eq!(script.len(), EVENTS);
    let last = script.last().expect("a script").0;
    assert!(last > 851.0, "the script reaches the last loop change");

    let mut full = ActorState::new(&robot, Mood::Neutral);
    let mut pruned = ActorState::new(&robot, Mood::Neutral);
    let mut sent = 0;
    let measure_at = last.ceil();
    let mut report = None;
    let mut tick = 1.0;
    // The last cutoffs come after every reaction has ended.
    while tick <= measure_at + KEEP + 3.0 {
        while sent < script.len() && script[sent].0 <= tick {
            let (at, event) = script[sent];
            send(&mut full, at, event);
            send(&mut pruned, at, event);
            sent += 1;
        }
        let cutoff = tick - KEEP;
        pruned.forget_before(&robot, cutoff);
        check_retained(&robot, &full, &pruned, cutoff);
        let held = |c: f64| pruned.mood_changes().iter().filter(|e| e.0 < c).count();
        if bursts.contains(&cutoff) {
            assert!(held(cutoff) >= 4, "cutoff {cutoff}: the burst is held");
        }
        if blends.contains(&cutoff) {
            assert!(held(cutoff) >= 1, "cutoff {cutoff}: the blend is held");
        }

        for t in samples(cutoff) {
            let want = pose_bytes(&full.evaluate(&robot, t)).expect("a pose is finite");
            let got = pose_bytes(&pruned.evaluate(&robot, t)).expect("a pose is finite");
            assert!(got == want, "cutoff {cutoff}: the pose at {t} differs");
            assert_eq!(
                pruned.face_moments(&robot, t),
                full.face_moments(&robot, t),
                "cutoff {cutoff}: the face moments after {t}"
            );
            assert_eq!(pruned.face(&robot, t), full.face(&robot, t));
        }

        if tick == measure_at {
            let mood = pruned.mood_changes().last().map_or(pruned.base(), |e| e.1);
            let mut fresh = ActorState::new(&robot, mood);
            if let Some(&(at, Some(l))) = pruned.loop_changes().last() {
                fresh.set_loop(at, Some(l));
            }
            let count = |s: &ActorState| {
                s.mood_changes().len()
                    + s.loop_changes().len()
                    + s.reactions().count()
                    + s.idle_changes().len()
            };
            report = Some(format!(
                "evaluate at {tick} s, median of 101: pruned {:?} ({} events held), \
                 fresh {:?} ({} events), unpruned {:?} ({} events)",
                median_evaluate(&pruned, &robot, tick),
                count(&pruned),
                median_evaluate(&fresh, &robot, tick),
                count(&fresh),
                median_evaluate(&full, &robot, tick),
                count(&full),
            ));
        }
        tick += 1.0;
    }
    assert_eq!(sent, EVENTS);
    // Once every event has ended, only the current loop and the idle
    // setting are left.
    assert_eq!(pruned.mood_changes(), []);
    assert_eq!(pruned.reactions().count(), 0);
    assert_eq!(pruned.loop_changes(), [(850.5, Some(StatusLoop::Sleepy))]);
    assert_eq!(pruned.idle_changes().len(), 1);
    println!("{}", report.expect("the measurement ran"));
}

#[test]
fn an_actor_is_its_robot_and_its_state() {
    fn shared<T: Send + Sync + 'static>() {}
    shared::<ActorState>();

    let robot = robot();
    let mut actor = Actor::new(&robot, Mood::Neutral);
    actor.set_loop(0.0, Some(StatusLoop::Working));
    actor.play(0.5, Reaction::Hello);
    actor.set_mood(1.0, Mood::Happy);
    let state = actor.clone().into_state();
    for t in [0.0, 0.7, 1.2, 2.5] {
        assert_eq!(state.evaluate(&robot, t), actor.evaluate(t));
        assert_eq!(state.face_moments(&robot, t), actor.face_moments(t));
    }
    let again = Actor::from_state(&robot, state);
    assert_eq!(again.evaluate(1.2), actor.evaluate(1.2));
}
