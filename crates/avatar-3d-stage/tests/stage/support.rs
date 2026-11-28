//! The harness: genomes, a builder tests can hold, and a stage over the
//! fake renderer with a frame clock.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use avatar_3d::motion::{ActorState, Pose};
use avatar_3d::robot::{build_with, BuildOptions, MoodSet, Robot, TextureContent};
use avatar_3d_stage::fake::{FakeControl, FakeFrame, FakeItem, FakeRenderer};
use avatar_3d_stage::{
    BuildMode, BuildRequest, Builder, CreatureSpec, Event, FaceRequest, JobInfo, LayersRequest,
    RenderStatus, Stage, StageOptions, Target, Work,
};
use avatar_small_robots::{Mood, GENOME_LEN};

/// How long a test waits for a build or a frame before it fails.
pub const WAIT: Duration = Duration::from_secs(60);

/// One frame at 60 Hz.
pub const TICK: f64 = 1.0 / 60.0;

/// The most frames [`Harness::until`] renders before it fails. Every job
/// lands in the frame after it is asked for, so waits end in a few frames.
pub const UNTIL_FRAMES: usize = 1000;

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
    (*cached(genome)).clone()
}

fn cached(genome: &[u8]) -> Arc<Robot> {
    static CACHE: OnceLock<Mutex<HashMap<Vec<u8>, Arc<Robot>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let lock = || cache.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(robot) = lock().get(genome) {
        return Arc::clone(robot);
    }
    let genome_array: &[u8; GENOME_LEN] = genome.try_into().expect("a full genome");
    let robot = Arc::new(build_with(genome_array, BuildOptions::untextured()));
    Arc::clone(lock().entry(genome.to_vec()).or_insert(robot))
}

/// Works out the reach box of the robot `genome` builds once, so the
/// robots the test builder makes from it carry it, as the stage's own
/// builder's robots do, and the frame thread never works it out.
pub fn warm_reach(genome: &[u8]) {
    let _ = cached(genome).reach();
}

/// The mood of a face or stalk-eye binding; `None` for marks and chests.
pub fn binding_mood(content: TextureContent) -> Option<Mood> {
    match content {
        TextureContent::Face { mood, .. } | TextureContent::StalkEye { mood, .. } => Some(mood),
        _ => None,
    }
}

/// Stand-in pixels, `px` × `px` RGBA8 of zeros, for each binding of
/// `robot` that is a mark, a chest, or a head face or stalk eye of one of
/// `moods`.
pub fn stand_in_pixels(robot: &Robot, moods: MoodSet, px: u32) -> Vec<(usize, Vec<u8>)> {
    robot
        .textures
        .iter()
        .enumerate()
        .filter(|(_, t)| binding_mood(t.binding.content).is_none_or(|m| moods.contains(m)))
        .map(|(i, _)| (i, vec![0; 4 * px as usize * px as usize]))
        .collect()
}

/// The robot a genome builds with stand-in pixels at `px` for its marks,
/// its chest and the face sets (head face and stalk eyes) of `moods`, as a
/// build rasterizing those moods at that edge has.
pub fn textured(genome: &[u8], moods: MoodSet, px: u32) -> Robot {
    let mut robot = robot(genome);
    let pixels = stand_in_pixels(&robot, moods, px);
    for texture in &mut robot.textures {
        texture.px = px;
    }
    for (i, p) in pixels {
        robot.textures[i].pixels = Some(p);
    }
    robot
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
    pub px: u32,
    /// Whether its robot waits in the stage's queue, or was taken.
    pub finished: bool,
}

/// What a texture call of the test builder rasterized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureWork {
    /// One mood's face set.
    Faces(Mood),
    /// The marks, chest and these face sets at a new edge.
    Layers(MoodSet),
}

/// One texture call of the test builder: a face set or a new edge.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureCall {
    pub key: String,
    pub generation: u64,
    pub work: TextureWork,
    pub px: u32,
    /// Whether its pixels wait in the stage's queue, or were taken.
    pub finished: bool,
}

/// What a job waits behind.
enum Hold {
    /// The hold on one key's builds.
    Build(String),
    /// The hold on every face set and new edge.
    Textures,
}

/// A builder whose builds of a key, and whose texture work, can be held,
/// and which logs each call. Its robots carry stand-in pixels.
///
/// It also counts, through the builder hooks, the jobs the stage asked for
/// that have not ended, and those of them waiting behind a hold, so a test
/// can wait until every other job has landed ([`TestBuilder::settle`])
/// and its frames do not depend on how threads are scheduled.
#[derive(Default)]
pub struct TestBuilder {
    state: Mutex<BuilderState>,
    changed: Condvar,
}

#[derive(Default)]
struct BuilderState {
    held: HashSet<String>,
    calls: Vec<BuildCall>,
    textures_held: bool,
    texture_calls: Vec<TextureCall>,
    /// Jobs asked for that have neither landed nor been dropped.
    pending: usize,
    /// Builds waiting behind a hold, by key, and texture jobs waiting
    /// behind the texture hold. A job counts as blocked only while its hold
    /// stands: the moment a hold is released its jobs count as running,
    /// before their threads wake.
    blocked_builds: HashMap<String, usize>,
    blocked_textures: usize,
}

impl BuilderState {
    /// The jobs still held.
    fn blocked(&self) -> usize {
        let builds: usize = self
            .blocked_builds
            .iter()
            .filter(|(key, _)| self.held.contains(*key))
            .map(|(_, n)| n)
            .sum();
        let textures = if self.textures_held {
            self.blocked_textures
        } else {
            0
        };
        builds + textures
    }

    /// Marks the call that made `job` finished: its result is in the
    /// stage's queue.
    fn finish(&mut self, job: &JobInfo<'_>) {
        let texture = match job.work {
            Work::Faces { mood } => Some(TextureWork::Faces(mood)),
            Work::Layers { moods } => Some(TextureWork::Layers(moods)),
            _ => None,
        };
        let finished = match texture {
            None => self
                .calls
                .iter_mut()
                .find(|c| !c.finished && c.key == job.key && c.generation == job.generation)
                .map(|c| &mut c.finished),
            Some(work) => self
                .texture_calls
                .iter_mut()
                .find(|c| {
                    !c.finished
                        && c.key == job.key
                        && c.generation == job.generation
                        && c.work == work
                        && c.px == job.texture_px
                })
                .map(|c| &mut c.finished),
        };
        *finished.expect("a job that landed was logged when it ran") = true;
    }
}

impl Builder for TestBuilder {
    fn build(&self, request: &BuildRequest<'_>) -> Robot {
        {
            let mut state = self.lock();
            state.calls.push(BuildCall {
                key: request.key.to_owned(),
                generation: request.generation,
                tag: tag(request.genome),
                mood: request.mood,
                px: request.texture_px,
                finished: false,
            });
            self.changed.notify_all();
            let key = request.key.to_owned();
            self.wait_while(state, Hold::Build(key));
        }
        textured(
            request.genome,
            MoodSet::NONE.with(request.mood),
            request.texture_px,
        )
    }

    fn face_set(&self, request: &FaceRequest<'_>) -> Vec<(usize, Vec<u8>)> {
        let work = TextureWork::Faces(request.mood);
        self.texture_work(
            request.key,
            request.generation,
            work,
            request.texture_px,
            || {
                let robot = robot(request.genome);
                stand_in_pixels(&robot, MoodSet::NONE.with(request.mood), request.texture_px)
                    .into_iter()
                    .filter(|(i, _)| binding_mood(robot.textures[*i].binding.content).is_some())
                    .collect()
            },
        )
    }

    fn layers(&self, request: &LayersRequest<'_>) -> Vec<(usize, Vec<u8>)> {
        let work = TextureWork::Layers(request.moods);
        self.texture_work(
            request.key,
            request.generation,
            work,
            request.texture_px,
            || stand_in_pixels(&robot(request.genome), request.moods, request.texture_px),
        )
    }

    fn asked(&self, _job: &JobInfo<'_>) {
        self.lock().pending += 1;
        self.changed.notify_all();
    }

    fn landed(&self, job: &JobInfo<'_>) {
        let mut state = self.lock();
        state.finish(job);
        state.pending -= 1;
        drop(state);
        self.changed.notify_all();
    }

    fn dropped(&self, _job: &JobInfo<'_>) {
        self.lock().pending -= 1;
        self.changed.notify_all();
    }
}

impl TestBuilder {
    fn lock(&self) -> MutexGuard<'_, BuilderState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits, counted as blocked, while `hold` stands.
    fn wait_while(&self, mut state: MutexGuard<'_, BuilderState>, hold: Hold) {
        let held = |s: &BuilderState| match &hold {
            Hold::Build(key) => s.held.contains(key),
            Hold::Textures => s.textures_held,
        };
        if !held(&state) {
            return;
        }
        match &hold {
            Hold::Build(key) => *state.blocked_builds.entry(key.clone()).or_default() += 1,
            Hold::Textures => state.blocked_textures += 1,
        }
        self.changed.notify_all();
        while held(&state) {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        match &hold {
            Hold::Build(key) => {
                *state
                    .blocked_builds
                    .get_mut(key)
                    .expect("counted when it began to wait") -= 1;
            }
            Hold::Textures => state.blocked_textures -= 1,
        }
    }

    /// Waits until every job the stage asked for has landed or been
    /// dropped, but those waiting behind a hold that still stands. The
    /// next render call then takes every result there is to take.
    pub fn settle(&self) {
        let deadline = Instant::now() + WAIT;
        let mut state = self.lock();
        while state.pending > state.blocked() {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "timed out waiting for {} jobs to land",
                state.pending - state.blocked()
            );
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
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
        let mut state = self.lock();
        state.held.clear();
        state.textures_held = false;
        drop(state);
        self.changed.notify_all();
    }

    /// Holds every face set and new edge until
    /// [`TestBuilder::release_textures`].
    pub fn hold_textures(&self) {
        self.lock().textures_held = true;
    }

    pub fn release_textures(&self) {
        self.lock().textures_held = false;
        self.changed.notify_all();
    }

    /// Every texture call so far.
    pub fn texture_calls(&self) -> Vec<TextureCall> {
        self.lock().texture_calls.clone()
    }

    /// Waits until the texture calls satisfy `done`.
    pub fn wait_for_textures(&self, what: &str, done: impl Fn(&[TextureCall]) -> bool) {
        let deadline = Instant::now() + WAIT;
        let mut state = self.lock();
        while !done(&state.texture_calls) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "timed out waiting for {what}");
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Logs a texture call, waits while texture work is held, then makes
    /// its pixels.
    fn texture_work(
        &self,
        key: &str,
        generation: u64,
        work: TextureWork,
        px: u32,
        make: impl FnOnce() -> Vec<(usize, Vec<u8>)>,
    ) -> Vec<(usize, Vec<u8>)> {
        {
            let mut state = self.lock();
            state.texture_calls.push(TextureCall {
                key: key.to_owned(),
                generation,
                work,
                px,
                finished: false,
            });
            self.changed.notify_all();
            self.wait_while(state, Hold::Textures);
        }
        make()
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

/// A harness over `build`. Under [`BuildMode::Workers`] it starts four
/// workers, so a job waiting behind a hold never keeps another job from
/// starting, which [`TestBuilder::settle`] relies on.
pub fn harness(target: Target, build: BuildMode) -> Harness {
    harness_with(target, build, 4, true)
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
    /// left. It first waits until every job asked for has landed, but those
    /// held, so what the call draws never depends on how the workers were
    /// scheduled: a job lands in the first call after it was asked for or
    /// released.
    pub fn render_at(&mut self, t: f64) -> Result<(), RenderStatus> {
        self.builder.settle();
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

    /// A render call `dt` seconds after the last, which must draw.
    pub fn step(&mut self, dt: f64) {
        let t = self.t + dt;
        self.render_at(t).expect("the frame draws");
    }

    /// The item drawn for `key` in the latest frame.
    pub fn item(&self, key: &str) -> FakeItem {
        self.last()
            .and_then(|f| f.item(key).cloned())
            .unwrap_or_else(|| panic!("{key} is drawn"))
    }

    /// Lets held texture work go; the next render call takes what it made.
    pub fn land_textures(&self) {
        self.builder.release_textures();
    }

    /// Renders frames until `done` holds, at most [`UNTIL_FRAMES`].
    pub fn until(&mut self, what: &str, done: impl Fn(&Harness) -> bool) {
        for _ in 0..UNTIL_FRAMES {
            self.frame().expect("the frame draws");
            if done(self) {
                return;
            }
        }
        panic!("{what} not within {UNTIL_FRAMES} frames");
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
