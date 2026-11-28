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
use avatar_3d::robot::camera::{frame_box, AZIMUTH, ELEVATION};
use avatar_3d::robot::Robot;
use avatar_small_robots::{Mood, GENOME_LEN};
use crossbeam_queue::SegQueue;

use crate::build::{Builds, Job, Output, Task};
use crate::clock::{Clock, Tick};
use crate::events::{CommandKind, Event, EventQueue};
use crate::faces::{edge_target, Ask, Faces, FIRST_EDGE};
use crate::held::{FaceCommand, Held};
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
    /// The edge the key's next build is rasterized at: the latest edge of
    /// its robot, or [`FIRST_EDGE`] before it had one.
    edge: u32,
    /// Face commands waiting for their face sets, in the host's order.
    held: Held,
    /// That build once uploaded, until it takes over.
    landed: Option<Body<G>>,
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
            edge: FIRST_EDGE,
            held: Held::default(),
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

    /// The uploaded robot of `generation`, shown or waiting to take over.
    fn body(&mut self, generation: u64) -> Option<&mut Body<G>> {
        let shown = self.shown.as_mut().map(|s| &mut s.body);
        shown
            .into_iter()
            .chain(self.landed.as_mut())
            .find(|b| b.generation == generation)
    }
}

/// A robot uploaded for a key.
struct Body<G> {
    generation: u64,
    robot: Arc<Robot>,
    /// The genome it was built from, which its other face sets and new
    /// edges are rasterized from.
    genome: Arc<[u8; GENOME_LEN]>,
    gpu: G,
    faces: Faces,
}

impl<G> Body<G> {
    /// Asks the workers for the texture work `asks` names.
    fn ask(&self, builds: &Builds, key: &str, asks: Vec<Ask>) {
        for ask in asks {
            let (texture_px, task) = match ask {
                Ask::Faces { mood, edge } => (edge, Task::Faces { mood }),
                Ask::Layers { moods, edge } => (edge, Task::Layers { moods }),
            };
            builds.request(Job {
                key: key.to_owned(),
                generation: self.generation,
                genome: Arc::clone(&self.genome),
                texture_px,
                task,
                cancelled: Arc::clone(&self.faces.cancel),
            });
        }
    }
}

struct Shown<G> {
    body: Body<G>,
    actor: ActorState,
    /// Whether a successful frame has shown this generation, and so its
    /// `Ready` event has gone out.
    announced: bool,
}

/// Gives a robot back to the renderer and cancels its queued work.
fn retire<R: Renderer>(renderer: &mut R, body: Body<R::Robot>) {
    body.faces.cancel.store(true, Ordering::Release);
    renderer.retire(body.gpu);
}

/// `pose` as drawn in `drawn`: the face shows that mood's set, and the
/// status lights, LEDs, `lit` and `rays` are that mood's own.
fn as_drawn(robot: &Robot, mut pose: Pose, drawn: Mood) -> Pose {
    let lights = robot.motion.mood(drawn);
    pose.face = drawn;
    pose.status = lights.status;
    pose.leds = lights.leds;
    pose.lit = lights.lit;
    pose.rays = lights.rays;
    pose
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
    /// Face commands held since creation.
    held_face_commands: u64,
    /// Drawn moods switched off a blink since creation.
    late_face_switches: u64,
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
            held_face_commands: 0,
            late_face_switches: 0,
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
        self.land(t)?;
        // A robot waiting to take over needs the face set of the key's
        // latest mood first; built inline, it lands at once.
        self.ask_for_landed(t);
        self.land(t)?;
        self.change_layers(t)?;
        self.take_over(t);
        self.serve_held(t);
        for creature in self.creatures.values_mut() {
            if let Some(shown) = &mut creature.shown {
                shown
                    .actor
                    .forget_before(&shown.body.robot, t - ACTOR_MEMORY);
            }
        }

        let drawn = self.drawn();
        let mut poses = Vec::with_capacity(drawn.len());
        for key in &drawn {
            let shown = self
                .creatures
                .get_mut(key)
                .and_then(|c| c.shown.as_mut())
                .expect("drawn keys are shown");
            let robot = &shown.body.robot;
            let pose = shown.actor.evaluate(robot, t);
            if shown.body.faces.choose(&pose, t) {
                self.late_face_switches += 1;
            }
            let posed_face = pose.face;
            poses.push((as_drawn(robot, pose, shown.body.faces.drawn()), posed_face));
        }
        self.look_after(&drawn, &poses, t);

        let items: Vec<DrawItem<'_, R::Robot>> = drawn
            .iter()
            .zip(&poses)
            .map(|(key, (pose, posed_face))| {
                let body = &self.creatures[key]
                    .shown
                    .as_ref()
                    .expect("drawn keys are shown")
                    .body;
                DrawItem {
                    key,
                    generation: body.generation,
                    robot: &body.robot,
                    gpu: &body.gpu,
                    pose,
                    posed_face: *posed_face,
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
    /// dropped. Each key keeps its held commands and its edge.
    fn drop_robots(&mut self, retire_them: bool) {
        for creature in self.creatures.values_mut() {
            creature.cancel_build();
            let landed = creature.landed.take();
            let shown = creature.shown.take().map(|s| s.body);
            for body in landed.into_iter().chain(shown) {
                if retire_them {
                    retire(&mut self.renderer, body);
                } else {
                    body.faces.cancel.store(true, Ordering::Release);
                }
            }
        }
    }

    /// Applies the scene commands in the host's order at stage time `t`.
    fn apply_commands(&mut self, shared: &Shared, t: f64, target_time: f64) {
        while let Some(command) = self.pending.pop_front() {
            match command {
                SceneCommand::SetCreatures(cast) => self.set_cast(shared, cast, target_time),
                SceneCommand::SetState {
                    key,
                    mood,
                    status_loop,
                } => {
                    let command = FaceCommand::SetState { mood, status_loop };
                    self.face_command(shared, key, command, t, target_time);
                }
                SceneCommand::Play { key, reaction } => {
                    let command = FaceCommand::Play(reaction);
                    self.face_command(shared, key, command, t, target_time);
                }
                SceneCommand::SetView(view) => self.view = view,
            }
        }
    }

    /// Gives a `set_state` or `play` to the key's actor at `t`, or holds it
    /// while the face set it needs is not drawable or an earlier command of
    /// the key waits; a held command's set is asked for at once.
    ///
    /// A key not shown yet keeps a `set_state` for the robot that appears
    /// and drops a `play`, unless commands of the key wait from before a
    /// pause or a lost device, which it then waits behind. A key not in the
    /// cast drops both. Dropped commands are reported.
    fn face_command(
        &mut self,
        shared: &Shared,
        key: String,
        command: FaceCommand,
        t: f64,
        target_time: f64,
    ) {
        let kind = match command {
            FaceCommand::SetState { .. } => CommandKind::SetState,
            FaceCommand::Play(_) => CommandKind::Play,
        };
        let Some(creature) = self.creatures.get_mut(&key) else {
            shared.events.push(Event::CommandDropped {
                key,
                command: kind,
                time: target_time,
            });
            return;
        };
        if let FaceCommand::SetState { mood, status_loop } = command {
            // The robot that appears next starts in the latest state.
            creature.mood = mood;
            creature.status_loop = status_loop;
        }
        let Some(shown) = &mut creature.shown else {
            if !creature.held.is_empty() {
                creature.held.hold(command, t);
                self.held_face_commands += 1;
            } else if matches!(command, FaceCommand::Play(_)) {
                shared.events.push(Event::CommandDropped {
                    key,
                    command: kind,
                    time: target_time,
                });
            }
            return;
        };
        let needs = command.needs(&shown.body.robot, &shown.actor);
        let drawable = match needs {
            Some(mood) => shown.body.faces.drawable(mood),
            None => true,
        };
        if creature.held.is_empty() && drawable {
            command.apply(&mut shown.actor, t);
            return;
        }
        creature.held.hold(command, t);
        self.held_face_commands += 1;
        if let Some(mood) = needs {
            let asks = shown.body.faces.asks(&[mood], t);
            shown.body.ask(&self.builds, &key, asks);
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
                let shown = creature.shown.map(|s| s.body);
                for body in creature.landed.into_iter().chain(shown) {
                    retire(&mut self.renderer, body);
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
                            retire(&mut self.renderer, landed);
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

    /// Asks for a build of every key that needs one, in cast order, in the
    /// key's latest mood and at its edge.
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
                texture_px: creature.edge,
                task: Task::Build {
                    mood: creature.mood,
                },
                cancelled: cancel,
            });
        }
    }

    /// Takes every job that landed: uploads each build still wanted, and
    /// hands each face set or new edge to the robot it was made for. The
    /// rest are discarded unseen.
    fn land(&mut self, t: f64) -> Result<(), RenderStatus> {
        while let Some(built) = self.builds.next_landed() {
            let Some(creature) = self.creatures.get_mut(&built.key) else {
                continue;
            };
            let mut robot = match built.output {
                Output::Robot(robot) => *robot,
                Output::Faces { mood, pixels } => {
                    if let Some(body) = creature.body(built.generation) {
                        body.faces.land_faces(mood, built.texture_px, pixels);
                    }
                    continue;
                }
                Output::Layers { pixels } => {
                    if let Some(body) = creature.body(built.generation) {
                        body.faces.land_layers(built.texture_px, pixels);
                    }
                    continue;
                }
            };
            if creature.wanted != Some(built.generation) {
                continue;
            }
            creature.cancel = None;
            // Read before the upload takes the pixels.
            let edge = robot.textures.first().map_or(built.texture_px, |t| t.px);
            let faces = Faces::new(&robot, edge, creature.mood, t);
            match self
                .renderer
                .upload(&built.key, built.generation, &mut robot)
            {
                Ok(gpu) => {
                    let landed = Body {
                        generation: built.generation,
                        robot: Arc::new(robot),
                        genome: Arc::clone(&creature.genome),
                        gpu,
                        faces,
                    };
                    if let Some(old) = creature.landed.replace(landed) {
                        retire(&mut self.renderer, old);
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

    /// Asks for the face set of its key's latest mood for each robot
    /// waiting to take over, unless it has it or it is on its way.
    fn ask_for_landed(&mut self, t: f64) {
        for (key, creature) in &mut self.creatures {
            if let Some(landed) = &mut creature.landed {
                let asks = landed.faces.asks(&[creature.mood], t);
                landed.ask(&self.builds, key, asks);
            }
        }
    }

    /// Makes, at the start of the frame at `t`, each robot's change to its
    /// resident layers: face sets landed since the last frame, sets
    /// released, or a move to a new edge.
    fn change_layers(&mut self, t: f64) -> Result<(), RenderStatus> {
        let mut failure = None;
        'creatures: for creature in self.creatures.values_mut() {
            let shown = creature.shown.as_mut().map(|s| &mut s.body);
            for body in shown.into_iter().chain(creature.landed.as_mut()) {
                let Some(plan) = body.faces.plan(&body.robot, t) else {
                    continue;
                };
                let (change, kept) = plan.split();
                match self.renderer.set_layers(&mut body.gpu, change) {
                    Ok(()) => body.faces.commit(kept, t),
                    Err(f) => {
                        body.faces.refused();
                        failure = Some(f);
                        break 'creatures;
                    }
                }
            }
        }
        match failure {
            Some(f) => Err(self.failed(f)),
            None => Ok(()),
        }
    }

    /// Lets each landed robot take over its key once the face set of the
    /// key's latest mood is drawable, with an actor in that mood and the
    /// key's status loop, so it first draws the latest mood's face.
    fn take_over(&mut self, t: f64) {
        for key in &self.order {
            let creature = self
                .creatures
                .get_mut(key)
                .expect("every listed key has a creature");
            if !creature
                .landed
                .as_ref()
                .is_some_and(|l| l.faces.drawable(creature.mood))
            {
                continue;
            }
            let mut body = creature.landed.take().expect("checked above");
            let mut actor = ActorState::new(&body.robot, creature.mood);
            if creature.status_loop.is_some() {
                actor.set_loop(t, creature.status_loop);
            }
            body.faces.show(creature.mood, t);
            let shown = Shown {
                body,
                actor,
                announced: false,
            };
            if let Some(old) = creature.shown.replace(shown) {
                retire(&mut self.renderer, old.body);
            }
        }
    }

    /// Gives each key's held commands that are due on the frame at `t` to
    /// its actor, in order.
    fn serve_held(&mut self, t: f64) {
        for creature in self.creatures.values_mut() {
            let Some(shown) = &mut creature.shown else {
                continue;
            };
            let (robot, faces) = (&shown.body.robot, &shown.body.faces);
            let actor = &shown.actor;
            let due = creature
                .held
                .due(t, |command| match command.needs(robot, actor) {
                    Some(mood) => faces.drawable(mood),
                    None => true,
                });
            for command in due {
                command.apply(&mut shown.actor, t);
            }
        }
    }

    /// After the drawn frame at `t` has chosen its faces: keeps resident,
    /// and asks for, each robot's drawn set, its pose's set, the sets its
    /// coming face changes show and those its held commands need; and
    /// aims each robot's edge at the size it is drawn at. `drawn` are the
    /// keys drawn and `poses` their poses with the face each pose asks for.
    fn look_after(&mut self, drawn: &[String], poses: &[(Pose, Mood)], t: f64) {
        let staged = match &self.view {
            StageView::Stage { key } => Some(key.as_str()),
            StageView::Plaza => None,
        };
        let (width_px, height_px) = (self.width_px, self.height_px);
        for (key, creature) in &mut self.creatures {
            let Some(shown) = &mut creature.shown else {
                continue;
            };
            let robot = &shown.body.robot;
            let drawn_at = drawn.iter().position(|k| k == key);
            let face = match drawn_at {
                Some(i) => poses[i].1,
                None => shown.actor.face(robot, t),
            };
            let mut needed = vec![shown.body.faces.drawn(), face];
            needed.extend(
                shown
                    .actor
                    .face_moments(robot, t)
                    .into_iter()
                    .map(|(_, m)| m),
            );
            needed.extend(
                creature
                    .held
                    .commands()
                    .filter_map(|c| c.needs(robot, &shown.actor)),
            );
            let target = match (drawn_at, staged) {
                (Some(_), Some(staged)) if staged == key => {
                    let reach = robot.reach();
                    let aspect = f64::from(width_px) / f64::from(height_px);
                    let camera = frame_box(reach.lo, reach.hi, AZIMUTH, ELEVATION, aspect, 1.0);
                    Some(edge_target(robot, &camera, [0.0; 3], height_px))
                }
                _ => None,
            };
            shown.body.faces.aim(target, t);
            let asks = shown.body.faces.asks(&needed, t);
            shown.body.ask(&self.builds, key, asks);
            creature.edge = shown.body.faces.edge();
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
                held_face_commands: self.held_face_commands,
                late_face_switches: self.late_face_switches,
                gpu_memory_bytes: self.renderer.memory_bytes(),
            },
        }));
    }
}

#[cfg(test)]
mod tests {
    use avatar_3d::robot::{build_with, BuildOptions, TextureContent};

    use super::*;
    use crate::build::{BuildMode, BuildRequest, Builder, FaceRequest, StageOptions};
    use crate::fake::FakeRenderer;
    use crate::spec::{self, CreatureSpec};

    /// Builds robots untextured, with zeroed pixels standing in for the
    /// textures a build or a face set would rasterize.
    struct StandIn;

    fn stand_in(robot: &Robot, mood: Mood, px: u32, marks: bool) -> Vec<(usize, Vec<u8>)> {
        robot
            .textures
            .iter()
            .enumerate()
            .filter(|(_, t)| match t.binding.content {
                TextureContent::Face { mood: m, .. } | TextureContent::StalkEye { mood: m, .. } => {
                    m == mood
                }
                _ => marks,
            })
            .map(|(i, _)| (i, vec![0; 4 * px as usize * px as usize]))
            .collect()
    }

    impl Builder for StandIn {
        fn build(&self, request: &BuildRequest<'_>) -> Robot {
            let mut robot = build_with(request.genome, BuildOptions::untextured());
            let px = request.texture_px;
            for (i, pixels) in stand_in(&robot, request.mood, px, true) {
                robot.textures[i].pixels = Some(pixels);
            }
            for texture in &mut robot.textures {
                texture.px = px;
            }
            robot
        }

        fn face_set(&self, request: &FaceRequest<'_>) -> Vec<(usize, Vec<u8>)> {
            let robot = build_with(request.genome, BuildOptions::untextured());
            stand_in(&robot, request.mood, request.texture_px, false)
        }
    }

    #[test]
    fn each_frame_forgets_what_ended_ten_seconds_of_stage_time_before() {
        let options = StageOptions {
            build: BuildMode::Inline,
            build_threads: 1,
        };
        let builds = Builds::new(options, Arc::new(StandIn));
        let (renderer, _) = FakeRenderer::new(Target::Image, false);
        let mut core = Core::new(renderer, 64, 48, 1.0, builds);
        let shared = Shared::new();
        let cast = spec::check(&[CreatureSpec::small_robot("a", vec![7; GENOME_LEN])]);
        shared
            .commands
            .push(Command::Scene(SceneCommand::SetCreatures(cast)));
        let set_state = |mood| {
            shared.commands.push(Command::Scene(SceneCommand::SetState {
                key: "a".into(),
                mood,
                status_loop: None,
            }));
        };
        let mut at = Vec::new();
        // Frames 0.1 s apart; frame n is at about n / 10 s of stage time.
        for n in 0..=200 {
            match n {
                10 => set_state(Mood::Happy),
                95 => set_state(Mood::Sad),
                102 => set_state(Mood::Blocked),
                _ => {}
            }
            core.render(&shared, 100.0 + 0.1 * f64::from(n), Instant::now())
                .expect("the frame draws");
            if [10, 95, 102].contains(&n) {
                at.push(core.clock.time());
            }
            let actor = &core.creatures["a"].shown.as_ref().expect("shown").actor;
            let changes: Vec<Mood> = actor.mood_changes().iter().map(|c| c.1).collect();
            match n {
                // At 12 s the cutoff is 2 s: happy, settled by 1.5 s, is the
                // base now.
                120 => {
                    assert_eq!(actor.base(), Mood::Happy);
                    assert_eq!(changes, [Mood::Sad, Mood::Blocked]);
                }
                // At 20 s the cutoff is 10 s: sad, from 9.5 s, settled by
                // then and is the base; blocked, from 10.2 s, had not.
                200 => {
                    assert_eq!(actor.base(), Mood::Sad);
                    assert_eq!(actor.mood_changes(), [(at[2], Mood::Blocked)]);
                }
                _ => {}
            }
        }
    }
}
