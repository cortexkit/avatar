//! The harness: genomes, a builder tests can hold, and a stage over the
//! fake renderer with a frame clock.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use avatar_3d::motion::{ActorState, Pose};
use avatar_3d::robot::{build_with, BuildOptions, Robot};
use avatar_3d_stage::fake::{FakeControl, FakeFrame, FakeRenderer};
use avatar_3d_stage::{
    BuildMode, BuildRequest, Builder, CreatureSpec, Event, RenderStatus, Stage, StageOptions,
    Target,
};
use avatar_small_robots::{Mood, GENOME_LEN};

/// How long a test waits for a build or a frame before it fails.
pub const WAIT: Duration = Duration::from_secs(60);

/// One frame at 60 Hz.
pub const TICK: f64 = 1.0 / 60.0;

/// A genome from a seed, by xorshift.
pub fn genome(seed: u32) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9).wrapping_add(0x7F4A_7C15) | 1;
    (0..GENOME_LEN)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x >> 24) as u8
        })
        .collect()
}

/// The first bytes of a genome, which tell the tests' genomes apart.
pub fn tag(genome: &[u8]) -> [u8; 8] {
    genome[..8]
        .try_into()
        .expect("a genome is longer than 8 bytes")
}

/// A small robot with the genome of `seed`.
pub fn spec(key: &str, seed: u32) -> CreatureSpec {
    CreatureSpec::small_robot(key, genome(seed))
}

/// The untextured robot a genome builds, built once per genome.
pub fn robot(genome: &[u8]) -> Robot {
    static CACHE: OnceLock<Mutex<HashMap<Vec<u8>, Arc<Robot>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let lock = || cache.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(robot) = lock().get(genome) {
        return (**robot).clone();
    }
    let genome_array: &[u8; GENOME_LEN] = genome.try_into().expect("a full genome");
    let robot = Arc::new(build_with(genome_array, BuildOptions::untextured()));
    lock().insert(genome.to_vec(), Arc::clone(&robot));
    (*robot).clone()
}

/// The pose a fresh actor in `mood`, with `status_loop` started at `t0`,
/// has at `t`, as the stage starts a robot that appears at `t0`.
pub fn fresh_pose(
    seed: u32,
    mood: Mood,
    status_loop: Option<avatar_3d::motion::StatusLoop>,
    t0: f64,
    t: f64,
) -> Pose {
    let robot = robot(&genome(seed));
    let mut actor = ActorState::new(&robot, mood);
    if status_loop.is_some() {
        actor.set_loop(t0, status_loop);
    }
    actor.evaluate(&robot, t)
}

/// One call of the test builder.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildCall {
    pub key: String,
    pub generation: u64,
    pub tag: [u8; 8],
    pub mood: Mood,
    pub finished: bool,
}

/// A builder whose builds of a key can be held, and which logs each call.
#[derive(Default)]
pub struct TestBuilder {
    state: Mutex<BuilderState>,
    changed: Condvar,
}

#[derive(Default)]
struct BuilderState {
    held: HashSet<String>,
    calls: Vec<BuildCall>,
}

impl Builder for TestBuilder {
    fn build(&self, request: &BuildRequest<'_>) -> Robot {
        let index = {
            let mut state = self.lock();
            state.calls.push(BuildCall {
                key: request.key.to_owned(),
                generation: request.generation,
                tag: tag(request.genome),
                mood: request.mood,
                finished: false,
            });
            self.changed.notify_all();
            while state.held.contains(request.key) {
                state = self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            state.calls.len() - 1
        };
        let robot = robot(request.genome);
        self.lock().calls[index].finished = true;
        self.changed.notify_all();
        robot
    }
}

impl TestBuilder {
    fn lock(&self) -> MutexGuard<'_, BuilderState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds every build of `key` until [`TestBuilder::release`].
    pub fn hold(&self, key: &str) {
        self.lock().held.insert(key.to_owned());
    }

    pub fn release(&self, key: &str) {
        self.lock().held.remove(key);
        self.changed.notify_all();
    }

    /// Lets every held build go on.
    pub fn release_all(&self) {
        self.lock().held.clear();
        self.changed.notify_all();
    }

    /// Every call so far.
    pub fn calls(&self) -> Vec<BuildCall> {
        self.lock().calls.clone()
    }

    /// The calls for `key`.
    pub fn calls_for(&self, key: &str) -> Vec<BuildCall> {
        self.calls().into_iter().filter(|c| c.key == key).collect()
    }

    /// Waits until the calls satisfy `done`.
    pub fn wait_for(&self, what: &str, done: impl Fn(&[BuildCall]) -> bool) {
        let deadline = Instant::now() + WAIT;
        let mut state = self.lock();
        while !done(&state.calls) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "timed out waiting for {what}");
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// A stage over the fake renderer, the builder it uses, and the host's
/// clock.
pub struct Harness {
    pub stage: Stage<FakeRenderer>,
    pub fake: FakeControl,
    pub builder: Arc<TestBuilder>,
    pub target: Target,
    /// The `target_time` of the latest call.
    pub t: f64,
    /// Every event drained by [`Harness::frame`] so far.
    pub events: Vec<Event>,
}

/// The frame size every harness starts with.
pub const WIDTH: u32 = 64;
pub const HEIGHT: u32 = 48;

pub fn harness(target: Target, build: BuildMode) -> Harness {
    harness_with(target, build, 1, true)
}

pub fn harness_with(target: Target, build: BuildMode, threads: usize, timestamps: bool) -> Harness {
    let (renderer, fake) = FakeRenderer::new(target, timestamps);
    let builder = Arc::new(TestBuilder::default());
    let options = StageOptions {
        build,
        build_threads: threads,
    };
    let stage = Stage::with_builder(
        renderer,
        WIDTH,
        HEIGHT,
        1.0,
        options,
        Arc::clone(&builder) as Arc<dyn Builder>,
    )
    .expect("a valid scale");
    Harness {
        stage,
        fake,
        builder,
        target,
        t: 100.0,
        events: Vec::new(),
    }
}

impl Drop for Harness {
    /// Lets held builds and frames go, so a failed test's stage can be
    /// dropped instead of waiting for them forever.
    fn drop(&mut self) {
        self.builder.release_all();
        self.fake.release_finish();
    }
}

/// Lets frames held at their finish go when dropped, so a failed
/// assertion inside a thread scope does not leave the scope waiting for a
/// held render call.
pub struct Unblock<'a>(pub &'a FakeControl);

impl Drop for Unblock<'_> {
    fn drop(&mut self) {
        self.0.release_finish();
    }
}

impl Harness {
    /// A render call at `t` to the harness's target, and the events it
    /// left.
    pub fn render_at(&mut self, t: f64) -> Result<(), RenderStatus> {
        self.t = t;
        let result = match self.target {
            Target::Surface => self.stage.render_to_surface(t),
            Target::Image => self.stage.render_to_image(t).map(|_| ()),
        };
        self.events.extend(self.stage.events());
        result
    }

    /// A render call one 60 Hz tick after the last.
    pub fn frame(&mut self) -> Result<(), RenderStatus> {
        self.render_at(self.t + TICK)
    }

    /// Renders frames until `done` holds, sleeping a little between them so
    /// worker builds land.
    pub fn until(&mut self, what: &str, done: impl Fn(&Harness) -> bool) {
        let deadline = Instant::now() + WAIT;
        loop {
            self.frame().expect("the frame draws");
            if done(self) {
                return;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Renders until the latest frame draws every one of `keys`.
    pub fn until_drawn(&mut self, keys: &[&str]) {
        self.until(&format!("{keys:?} drawn"), |h| {
            h.last()
                .is_some_and(|f| keys.iter().all(|k| f.item(k).is_some()))
        });
    }

    /// The latest frame submitted.
    pub fn last(&self) -> Option<FakeFrame> {
        self.fake.last_frame()
    }

    /// The generation drawn for `key` in the latest frame.
    pub fn drawn_generation(&self, key: &str) -> Option<u64> {
        self.last()?.item(key).map(|i| i.generation)
    }

    /// The `Ready` events so far, by key.
    pub fn readies(&self, key: &str) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e, Event::Ready { key: k, .. } if k == key))
            .count()
    }
}
