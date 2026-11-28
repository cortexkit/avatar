//! The scene core: the cast and its robots, the order of steps in a render
//! call, and what a successful frame publishes. Everything here runs on
//! the thread holding the stage's frame lock: a render call, or a command
//! thread serving a pause while no render call runs.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use arc_swap::ArcSwap;
use avatar_3d::motion::{ActorState, Pose, Reaction, StatusLoop};
use avatar_3d::robot::Robot;
use avatar_small_robots::{Mood, GENOME_LEN};
use crossbeam_queue::SegQueue;

use crate::build::{Builds, Job};
use crate::clock::{Clock, Tick};
use crate::events::{CommandKind, Event, EventQueue};
use crate::renderer::{DrawItem, Failure, Frame, Image, Renderer, Target};
use crate::spec::Cast;
use crate::stats::{FrameStats, Window};
use crate::{RenderStatus, StageView};

/// How long an actor keeps the events whose effects are over, in seconds
/// of stage time, before they are forgotten.
const ACTOR_MEMORY: f64 = 10.0;

/// A command as the host gave it, waiting in the command queue.
pub(crate) enum Command {
    /// Applied at the start of every render call, drawn or not.
    Lifecycle(Lifecycle),
    /// Applied when the next drawn frame commits.
    Scene(SceneCommand),
}

pub(crate) enum Lifecycle {
    Pause,
    Resume,
    Resize {
        width_px: u32,
        height_px: u32,
        scale: f64,
    },
    SetMaxFps(u32),
}

pub(crate) enum SceneCommand {
    SetCreatures(Cast),
    SetState {
        key: String,
        mood: Mood,
        status_loop: Option<StatusLoop>,
    },
    Play {
        key: String,
        reaction: Reaction,
    },
    SetView(StageView),
}

/// What the latest successful frame published, which reads load without a
/// lock. With the stage camera it also carries the frame's creature
/// positions on screen and the shapes a tap is tested against.
#[derive(Debug, Default)]
pub(crate) struct Snapshot {
    pub stats: FrameStats,
}

/// What command and read threads share with the frame thread, none of it
/// behind a lock.
pub(crate) struct Shared {
    pub commands: SegQueue<Command>,
    pub events: EventQueue,
    pub snapshot: ArcSwap<Snapshot>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            commands: SegQueue::new(),
            events: EventQueue::new(),
            snapshot: ArcSwap::from_pointee(Snapshot::default()),
        }
    }
}

/// One key of the cast.
struct Creature<G> {
    /// The genome of the key's latest accepted spec.
    genome: Arc<[u8; GENOME_LEN]>,
    /// The latest mood and status loop asked for, which a robot that
    /// appears starts in.
    mood: Mood,
    status_loop: Option<StatusLoop>,
    /// The generation of the latest build asked for the current spec, or
    /// `None` when a build is still to be asked for.
    wanted: Option<u64>,
    /// Cancels that build while it waits for a worker.
    cancel: Option<Arc<AtomicBool>>,
    /// That build once uploaded, until it takes over.
    landed: Option<Landed<G>>,
    /// The robot frames draw for the key.
    shown: Option<Shown<G>>,
}

impl<G> Creature<G> {
    fn new(genome: Arc<[u8; GENOME_LEN]>) -> Self {
        Self {
            genome,
            mood: Mood::Neutral,
            status_loop: None,
            wanted: None,
            cancel: None,
            landed: None,
            shown: None,
        }
    }

    /// Stops wanting the outstanding build: a worker that has not started
    /// it skips it, and a result that lands anyway is discarded.
    fn cancel_build(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Release);
        }
        self.wanted = None;
    }
}

struct Landed<G> {
    generation: u64,
    robot: Arc<Robot>,
    gpu: G,
}

struct Shown<G> {
    generation: u64,
    robot: Arc<Robot>,
    gpu: G,
    actor: ActorState,
    /// Whether a successful frame has shown this generation, and so its
    /// `Ready` event has gone out.
    announced: bool,
}

/// The frame thread's state.
pub(crate) struct Core<R: Renderer> {
    renderer: R,
    clock: Clock,
    width_px: u32,
    height_px: u32,
    scale: f64,
    view: StageView,
    paused: bool,
    /// Whether the renderer holds nothing since the last release: set by a
    /// release, cleared when a frame begins.
    released: bool,
    /// Set when the renderer reported a lost device; the next render call
    /// that gets past the early returns recovers it.
    device_lost: bool,
    next_generation: u64,
    /// The cast's keys in the order the host listed them.
    order: Vec<String>,
    creatures: HashMap<String, Creature<R::Robot>>,
    /// Scene commands taken from the queue, waiting for a drawn frame.
    pending: VecDeque<SceneCommand>,
    cpu: Window,
    gpu: Window,
    readback: Window,
    frames_dropped: u64,
    /// Dropped last, so destroying the stage waits for running builds
    /// after everything else is gone.
    builds: Builds,
}

impl<R: Renderer> Core<R> {
    pub fn new(renderer: R, width_px: u32, height_px: u32, scale: f64, builds: Builds) -> Self {
        Self {
            renderer,
            clock: Clock::default(),
            width_px,
            height_px,
            scale,
            view: StageView::Plaza,
            paused: false,
            released: false,
            device_lost: false,
            next_generation: 0,
            order: Vec::new(),
            creatures: HashMap::new(),
            pending: VecDeque::new(),
            cpu: Window::default(),
            gpu: Window::default(),
            readback: Window::default(),
            frames_dropped: 0,
            builds,
        }
    }

    /// One render call past the checks that consume nothing (busy, target,
    /// a non-finite time), in the documented order.
    pub fn render(
        &mut self,
        shared: &Shared,
        target_time: f64,
        entry: Instant,
    ) -> Result<Option<Image>, RenderStatus> {
        self.take_commands(shared);
        if self.paused {
            if !self.released {
                self.release(shared);
            }
            // Builds that land while paused are discarded.
            self.builds.discard_landed();
            return Err(RenderStatus::Paused);
        }
        if self.width_px == 0 || self.height_px == 0 || !self.clock.due(target_time) {
            return Err(RenderStatus::NotDue);
        }
        if self.device_lost {
            self.renderer.recover().map_err(|f| self.failed(f))?;
            self.device_lost = false;
            self.drop_robots(false);
        }
        self.renderer
            .begin(self.width_px, self.height_px, self.scale)
            .map_err(|f| self.failed(f))?;
        self.released = false;

        // The commit point: from here the frame's commands, stage time,
        // actor states and clock reference stand, whatever happens to the
        // frame on the GPU, so a retry at the same time advances nothing.
        let tick = self.clock.commit(target_time);
        let t = self.clock.time();
        self.apply_commands(shared, t, target_time);
        self.request_builds();
        self.land_builds()?;
        self.take_over(t);
        for creature in self.creatures.values_mut() {
            if let Some(shown) = &mut creature.shown {
                shown.actor.forget_before(&shown.robot, t - ACTOR_MEMORY);
            }
        }

        let drawn = self.drawn();
        let poses: Vec<Pose> = drawn
            .iter()
            .map(|key| {
                let shown = self.shown(key);
                shown.actor.evaluate(&shown.robot, t)
            })
            .collect();
        let items: Vec<DrawItem<'_, R::Robot>> = drawn
            .iter()
            .zip(&poses)
            .map(|(key, pose)| {
                let shown = self.creatures[key]
                    .shown
                    .as_ref()
                    .expect("drawn keys are shown");
                DrawItem {
                    key,
                    generation: shown.generation,
                    robot: &shown.robot,
                    gpu: &shown.gpu,
                    pose,
                    offset: [0.0; 3],
                }
            })
            .collect();
        let frame = Frame {
            width_px: self.width_px,
            height_px: self.height_px,
            scale: self.scale,
            target_time,
            stage_time: t,
            items: &items,
        };
        let submitted = self.renderer.submit(&frame);
        drop(items);
        submitted.map_err(|f| self.failed(f))?;
        let cpu = entry.elapsed().as_secs_f64();

        let (image, readback) = match self.renderer.target() {
            Target::Surface => {
                self.renderer.present().map_err(|f| self.failed(f))?;
                (None, None)
            }
            Target::Image => {
                let start = Instant::now();
                let image = self.renderer.read_back().map_err(|f| self.failed(f))?;
                (Some(image), Some(start.elapsed().as_secs_f64()))
            }
        };
        self.succeeded(shared, &drawn, tick, cpu, readback, target_time);
        Ok(image)
    }

    /// Takes every queued command: lifecycle commands are applied now,
    /// scene commands wait for the next drawn frame.
    pub fn take_commands(&mut self, shared: &Shared) {
        while let Some(command) = shared.commands.pop() {
            match command {
                Command::Scene(command) => self.pending.push_back(command),
                Command::Lifecycle(Lifecycle::Pause) => {
                    self.paused = true;
                    if !self.released {
                        self.release(shared);
                    }
                }
                Command::Lifecycle(Lifecycle::Resume) => {
                    if self.paused {
                        self.paused = false;
                        self.clock.restart();
                    }
                }
                Command::Lifecycle(Lifecycle::Resize {
                    width_px,
                    height_px,
                    scale,
                }) => {
                    self.width_px = width_px;
                    self.height_px = height_px;
                    self.scale = scale;
                }
                Command::Lifecycle(Lifecycle::SetMaxFps(fps)) => self.clock.set_max_fps(fps),
            }
        }
    }

    /// Records what a renderer failure means for the stage and maps it to
    /// the call's status.
    fn failed(&mut self, failure: Failure) -> RenderStatus {
        match failure {
            Failure::SurfaceLost => RenderStatus::SurfaceLost,
            Failure::DeviceLost => {
                self.device_lost = true;
                RenderStatus::DeviceLost
            }
            Failure::Render(e) => RenderStatus::Render(e),
        }
    }

    /// Frees every GPU resource but the surface. Each creature keeps its
    /// spec, mood and status loop, and is rebuilt after resume.
    fn release(&mut self, shared: &Shared) {
        self.drop_robots(true);
        self.builds.discard_landed();
        self.renderer.release();
        self.released = true;
        self.publish(shared);
    }

    /// Drops every robot and cancels every build, so each creature is built
    /// again: retired through the renderer, or, after a lost device, simply
    /// dropped.
    fn drop_robots(&mut self, retire: bool) {
        for creature in self.creatures.values_mut() {
            creature.cancel_build();
            let landed = creature.landed.take().map(|l| l.gpu);
            let shown = creature.shown.take().map(|s| s.gpu);
            if retire {
                for gpu in landed.into_iter().chain(shown) {
                    self.renderer.retire(gpu);
                }
            }
        }
    }

    /// Applies the scene commands in the host's order at stage time `t`.
    fn apply_commands(&mut self, shared: &Shared, t: f64, target_time: f64) {
        let dropped = |key, command| Event::CommandDropped {
            key,
            command,
            time: target_time,
        };
        while let Some(command) = self.pending.pop_front() {
            match command {
                SceneCommand::SetCreatures(cast) => self.set_cast(shared, cast, target_time),
                SceneCommand::SetState {
                    key,
                    mood,
                    status_loop,
                } => match self.creatures.get_mut(&key) {
                    None => shared.events.push(dropped(key, CommandKind::SetState)),
                    Some(creature) => {
                        // A key not shown yet keeps it for the robot that
                        // appears; a shown one also changes now.
                        creature.mood = mood;
                        creature.status_loop = status_loop;
                        if let Some(shown) = &mut creature.shown {
                            shown.actor.set_mood(t, mood);
                            shown.actor.set_loop(t, status_loop);
                        }
                    }
                },
                SceneCommand::Play { key, reaction } => {
                    match self.creatures.get_mut(&key).and_then(|c| c.shown.as_mut()) {
                        Some(shown) => shown.actor.play(t, reaction),
                        None => shared.events.push(dropped(key, CommandKind::Play)),
                    }
                }
                SceneCommand::SetView(view) => self.view = view,
            }
        }
    }

    /// Replaces the cast: reports skipped rows, removes keys no longer
    /// listed, and asks for a build of each new key or changed spec. A key
    /// whose spec is unchanged keeps its robot and actor.
    fn set_cast(&mut self, shared: &Shared, cast: Cast, target_time: f64) {
        for (key, reason) in cast.skipped {
            shared.events.push(Event::Skipped {
                key,
                reason,
                time: target_time,
            });
        }
        let listed: HashSet<&str> = cast.rows.iter().map(|r| r.key.as_str()).collect();
        for key in &self.order {
            if listed.contains(key.as_str()) {
                continue;
            }
            if let Some(mut creature) = self.creatures.remove(key) {
                creature.cancel_build();
                let landed = creature.landed.map(|l| l.gpu);
                let shown = creature.shown.map(|s| s.gpu);
                for gpu in landed.into_iter().chain(shown) {
                    self.renderer.retire(gpu);
                }
            }
        }
        self.order.clear();
        for row in cast.rows {
            match self.creatures.get_mut(&row.key) {
                Some(creature) => {
                    if creature.genome != row.genome {
                        // The shown robot stays until the new build can
                        // take over; a build of an older spec never does.
                        creature.genome = row.genome;
                        creature.cancel_build();
                        if let Some(landed) = creature.landed.take() {
                            self.renderer.retire(landed.gpu);
                        }
                    }
                }
                None => {
                    self.creatures
                        .insert(row.key.clone(), Creature::new(row.genome));
                }
            }
            self.order.push(row.key);
        }
    }

    /// Asks for a build of every key that needs one, in cast order.
    fn request_builds(&mut self) {
        for key in &self.order {
            let creature = self
                .creatures
                .get_mut(key)
                .expect("every listed key has a creature");
            if creature.wanted.is_some() {
                continue;
            }
            self.next_generation += 1;
            let generation = self.next_generation;
            let cancel = Arc::new(AtomicBool::new(false));
            creature.wanted = Some(generation);
            creature.cancel = Some(Arc::clone(&cancel));
            self.builds.request(Job {
                key: key.clone(),
                generation,
                genome: Arc::clone(&creature.genome),
                mood: creature.mood,
                cancelled: cancel,
            });
        }
    }

    /// Uploads every build that landed and is still wanted; the rest are
    /// discarded unseen.
    fn land_builds(&mut self) -> Result<(), RenderStatus> {
        while let Some(built) = self.builds.next_landed() {
            let Some(creature) = self.creatures.get_mut(&built.key) else {
                continue;
            };
            if creature.wanted != Some(built.generation) {
                continue;
            }
            creature.cancel = None;
            let mut robot = built.robot;
            match self
                .renderer
                .upload(&built.key, built.generation, &mut robot)
            {
                Ok(gpu) => {
                    let landed = Landed {
                        generation: built.generation,
                        robot: Arc::new(robot),
                        gpu,
                    };
                    if let Some(old) = creature.landed.replace(landed) {
                        self.renderer.retire(old.gpu);
                    }
                }
                Err(failure) => {
                    // Asked for again by the next drawn frame.
                    creature.wanted = None;
                    return Err(self.failed(failure));
                }
            }
        }
        Ok(())
    }

    /// Lets each landed robot take over its key, with an actor in the key's
    /// latest mood and status loop. A robot takes over in the frame it
    /// lands until faces load on demand and the plaza is laid out, which
    /// add their own conditions here.
    fn take_over(&mut self, t: f64) {
        for key in &self.order {
            let creature = self
                .creatures
                .get_mut(key)
                .expect("every listed key has a creature");
            let Some(landed) = creature.landed.take() else {
                continue;
            };
            let mut actor = ActorState::new(&landed.robot, creature.mood);
            if creature.status_loop.is_some() {
                actor.set_loop(t, creature.status_loop);
            }
            let shown = Shown {
                generation: landed.generation,
                robot: landed.robot,
                gpu: landed.gpu,
                actor,
                announced: false,
            };
            if let Some(old) = creature.shown.replace(shown) {
                self.renderer.retire(old.gpu);
            }
        }
    }

    /// The keys this frame draws: every shown robot in cast order in the
    /// plaza, or the staged key's alone.
    fn drawn(&self) -> Vec<String> {
        match &self.view {
            StageView::Plaza => self
                .order
                .iter()
                .filter(|key| self.creatures[*key].shown.is_some())
                .cloned()
                .collect(),
            StageView::Stage { key } => self
                .creatures
                .get(key)
                .filter(|c| c.shown.is_some())
                .map(|_| key.clone())
                .into_iter()
                .collect(),
        }
    }

    fn shown(&self, key: &str) -> &Shown<R::Robot> {
        self.creatures[key]
            .shown
            .as_ref()
            .expect("drawn keys are shown")
    }

    /// Publishes a frame that was presented or read back: its timings and
    /// dropped frames, the snapshot, and `Ready` for each robot it showed
    /// for the first time.
    fn succeeded(
        &mut self,
        shared: &Shared,
        drawn: &[String],
        tick: Tick,
        cpu: f64,
        readback: Option<f64>,
        target_time: f64,
    ) {
        self.cpu.push(cpu);
        if let Some(seconds) = readback {
            self.readback.push(seconds);
        }
        for seconds in self.renderer.take_gpu_times() {
            self.gpu.push(seconds);
        }
        self.frames_dropped += tick.dropped;
        self.publish(shared);
        for key in drawn {
            let shown = self
                .creatures
                .get_mut(key)
                .and_then(|c| c.shown.as_mut())
                .expect("drawn keys are shown");
            if !shown.announced {
                shown.announced = true;
                shared.events.push(Event::Ready {
                    key: key.clone(),
                    time: target_time,
                });
            }
        }
    }

    fn publish(&self, shared: &Shared) {
        shared.snapshot.store(Arc::new(Snapshot {
            stats: FrameStats {
                cpu: self.cpu.series(),
                gpu: self.gpu.series(),
                gpu_timestamps: self.renderer.timestamps(),
                readback: self.readback.series(),
                frames_dropped: self.frames_dropped,
                held_face_commands: 0,
                late_face_switches: 0,
                gpu_memory_bytes: self.renderer.memory_bytes(),
            },
        }));
    }
}
