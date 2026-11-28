//! The stage: commands and reads from any thread, render calls from one
//! frame thread at a time.

use std::fmt;
use std::sync::atomic::{fence, AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, TryLockError};
use std::time::Instant;

use avatar_3d::motion::{Reaction, StatusLoop};
use avatar_small_robots::Mood;

use crate::build::{Builder, Builds, RobotBuilder, StageOptions};
use crate::events::Event;
use crate::renderer::{Image, RenderError, Renderer, Target};
use crate::scene::{Command, Core, Lifecycle, SceneCommand, Shared};
use crate::spec::{self, CreatureSpec};
use crate::stats::FrameStats;

/// What a stage view shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum StageView {
    /// One creature alone. A key that is absent or not built yet draws the
    /// background only.
    Stage {
        /// The creature's key.
        key: String,
    },
    /// Every creature of the cast. The default.
    #[default]
    Plaza,
}

/// Why a render call drew nothing, or did not finish its frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderStatus {
    /// Too soon after the last frame for the fps cap, or the frame has no
    /// area. Nothing was drawn or advanced, and no frame counts as dropped.
    NotDue,
    /// The stage is paused. Nothing was drawn.
    Paused,
    /// Another render call is running on another thread. Nothing changed.
    Busy,
    /// `target_time` is not finite. Nothing changed and no command was
    /// taken.
    InvalidArgument,
    /// The surface was lost or out of date and has been reconfigured. If
    /// the frame had committed, its commands and time stand.
    SurfaceLost,
    /// The GPU device was lost. If the frame had committed, its commands
    /// and time stand. The next call that is not `Busy`, `Paused` or
    /// `NotDue` makes a new device and rebuilds every creature from its
    /// spec in its latest mood and status loop.
    DeviceLost,
    /// A surface call on a stage that renders images, or the reverse.
    /// Nothing changed.
    WrongTarget,
    /// The renderer failed otherwise.
    Render(RenderError),
}

impl fmt::Display for RenderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDue => f.write_str("no frame is due"),
            Self::Paused => f.write_str("the stage is paused"),
            Self::Busy => f.write_str("another render call is running"),
            Self::InvalidArgument => f.write_str("the target time is not finite"),
            Self::SurfaceLost => f.write_str("the surface was lost"),
            Self::DeviceLost => f.write_str("the GPU device was lost"),
            Self::WrongTarget => f.write_str("the stage renders to the other target"),
            Self::Render(e) => write!(f, "rendering failed: {e}"),
        }
    }
}

impl std::error::Error for RenderStatus {}

/// A command's argument was refused; nothing changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidArgument;

impl fmt::Display for InvalidArgument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid argument")
    }
}

impl std::error::Error for InvalidArgument {}

/// Why a stage could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StageError {
    /// The scale is not finite and above 0.
    InvalidArgument,
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument => f.write_str("invalid argument"),
        }
    }
}

impl std::error::Error for StageError {}

/// A scene of creatures keyed by the caller, their motion and a clock,
/// drawn through a [`Renderer`].
///
/// # Threads
///
/// `Stage` is `Send + Sync`. Commands ([`Stage::set_creatures`],
/// [`Stage::set_state`], [`Stage::play`], [`Stage::set_view`],
/// [`Stage::resize`], [`Stage::set_max_fps`], [`Stage::pause`],
/// [`Stage::resume`]) and reads ([`Stage::events`], [`Stage::frame_stats`])
/// may come from any thread at any time, also while a render call runs on
/// another. They return at once: a command goes into a lock-free queue, a
/// read loads data a frame published, and none of them waits on a frame,
/// the GPU, a build or a lock a frame or build thread holds.
///
/// Render calls ([`Stage::render_to_surface`], [`Stage::render_to_image`])
/// run on one thread at a time, the host's frame thread; a call made while
/// another runs returns [`RenderStatus::Busy`] at once. They may block, for
/// a drawable, a read-back, a device recovery or an inline build, so they
/// belong off the main thread.
///
/// # Time
///
/// A render call's `target_time` is in seconds on any host clock that
/// never goes back, such as a display link's target timestamp; it is the
/// stage's only time base. The first frame after creation or resume sets
/// the origin. Each frame after it advances stage time by its
/// `target_time` minus the previous committed frame's, clamped to
/// 0..max(0.25, 1.5 / max fps) s, or 0..0.25 s uncapped, so motion keeps
/// step with presentation and a long gap does not jump. A time earlier
/// than the previous frame's advances nothing.
///
/// A command takes effect at the first frame drawn after it: scene
/// commands at that frame's stage time, each exactly once, and
/// [`Stage::pause`], [`Stage::resume`], [`Stage::resize`] and
/// [`Stage::set_max_fps`] at the start of the next render call, drawn or
/// not. Events carry that frame's `target_time`.
///
/// # A render call, in order
///
/// 1. Another call running: [`RenderStatus::Busy`]. The wrong target:
///    [`RenderStatus::WrongTarget`]. A non-finite `target_time`:
///    [`RenderStatus::InvalidArgument`]. None of these takes a command.
/// 2. Pause, resume, resize and fps commands apply. A paused stage frees
///    its GPU resources if it still holds any and returns
///    [`RenderStatus::Paused`].
/// 3. A frame with no area, or a call less than a period less 2 ms after
///    the previous frame when capped: [`RenderStatus::NotDue`], with
///    nothing drawn or advanced.
/// 4. After a lost device, a new device; every creature is rebuilt.
/// 5. The frame begins: a surface frame acquires its drawable. A failure
///    here commits nothing.
/// 6. The commit point: the queued scene commands, stage time, actor
///    states and clock reference change together and are never replayed,
///    even if the rest of the frame fails, so retrying the same
///    `target_time` advances nothing.
/// 7. Builds are asked for, landed builds are uploaded and take over, and
///    the frame is drawn, submitted, then presented or read back.
/// 8. Only a presented or read-back frame publishes its statistics and the
///    [`Event::Ready`] of each robot it showed for the first time.
///
/// # Creatures
///
/// Each build of a key is a new generation. A robot appears in the first
/// frame after its build lands; a changed spec keeps the old robot drawn
/// until the new one takes over, in the key's latest mood and status loop.
/// A build whose spec has since changed, or whose key was removed, is
/// discarded unseen. Robots build on worker threads, or inside the render
/// call that first needs them ([`crate::BuildMode::Inline`]).
///
/// # Pause
///
/// [`Stage::pause`] stops stage time and frees every GPU resource but the
/// surface (robots, faces, frame and readback targets) as soon as no
/// render call is running: at once, or at the end of the running one. The
/// host makes no further call for it. Until [`Stage::resume`], render
/// calls return [`RenderStatus::Paused`] without drawing, builds that land
/// are discarded, and commands queue. Each key keeps its spec, mood and
/// status loop, and the view, size and fps cap stay. After resume the
/// next frames rebuild what the scene needs: each creature reappears once
/// rebuilt, in its latest mood with its status loop started at that frame
/// and no reaction playing.
///
/// # Destroy
///
/// Dropping the stage cancels queued builds and waits only for builds
/// already running, so it blocks for at most about one build. Drop it
/// once, after the host's last other call.
pub struct Stage<R: Renderer> {
    shared: Shared,
    core: Mutex<Core<R>>,
    target: Target,
    /// Set while a render call runs, so a second one returns `Busy`.
    rendering: AtomicBool,
    /// Set by `pause` until the thread that next holds the frame lock has
    /// taken the pause and freed the GPU resources.
    pause_waiting: AtomicBool,
}

/// A render call's turn: clears the running flag when the call ends,
/// however it ends.
struct Turn<'a>(&'a AtomicBool);

impl<'a> Turn<'a> {
    fn take(flag: &'a AtomicBool) -> Option<Self> {
        (!flag.swap(true, Ordering::Acquire)).then_some(Self(flag))
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl<R: Renderer> Stage<R> {
    /// A stage drawing through `renderer` into a frame of
    /// `width_px` × `height_px` device pixels at `scale` device pixels per
    /// point, building robots with [`RobotBuilder`].
    pub fn new(
        renderer: R,
        width_px: u32,
        height_px: u32,
        scale: f64,
        options: StageOptions,
    ) -> Result<Self, StageError> {
        Self::with_builder(
            renderer,
            width_px,
            height_px,
            scale,
            options,
            Arc::new(RobotBuilder),
        )
    }

    /// A stage as [`Stage::new`] makes, building robots with `builder`.
    pub fn with_builder(
        renderer: R,
        width_px: u32,
        height_px: u32,
        scale: f64,
        options: StageOptions,
        builder: Arc<dyn Builder>,
    ) -> Result<Self, StageError> {
        if !valid_scale(scale) {
            return Err(StageError::InvalidArgument);
        }
        let target = renderer.target();
        let builds = Builds::new(options, builder);
        Ok(Self {
            shared: Shared::new(),
            core: Mutex::new(Core::new(renderer, width_px, height_px, scale, builds)),
            target,
            rendering: AtomicBool::new(false),
            pause_waiting: AtomicBool::new(false),
        })
    }

    /// Where this stage's frames go.
    pub fn target(&self) -> Target {
        self.target
    }

    /// Replaces the cast. Rows are checked in order for their type, then
    /// version, then genome length; a row failing one is skipped with that
    /// reason, and a valid row whose key already has a valid row earlier in
    /// the batch is skipped as a duplicate. The new cast is the first valid
    /// row of each key, in batch order; a key with no valid row is removed
    /// and vanishes from the next frame. Each skipped row is reported once
    /// as an [`Event::Skipped`] and never drawn as a default.
    pub fn set_creatures(&self, specs: &[CreatureSpec]) {
        let cast = spec::check(specs);
        self.scene(SceneCommand::SetCreatures(cast));
    }

    /// Sets a creature's mood and status loop. For a key in the cast whose
    /// robot is not built yet, the latest call is kept and the robot starts
    /// in it. For a key not in the cast, the call is dropped and reported
    /// as an [`Event::CommandDropped`].
    pub fn set_state(&self, key: &str, mood: Mood, status_loop: Option<StatusLoop>) {
        self.scene(SceneCommand::SetState {
            key: key.to_owned(),
            mood,
            status_loop,
        });
    }

    /// Plays a reaction. For a key whose robot is not built yet, or not in
    /// the cast, the call is dropped and reported as an
    /// [`Event::CommandDropped`].
    pub fn play(&self, key: &str, reaction: Reaction) {
        self.scene(SceneCommand::Play {
            key: key.to_owned(),
            reaction,
        });
    }

    /// Shows one creature alone, or the whole cast.
    pub fn set_view(&self, view: StageView) {
        self.scene(SceneCommand::SetView(view));
    }

    /// Sets the frame's size in device pixels and the device pixels per
    /// point. The next render call reconfigures the surface or targets, so
    /// no frame is drawn at another size than the last resize. A size of 0
    /// draws nothing until resized. A scale that is not finite and above 0
    /// is refused and changes nothing.
    pub fn resize(&self, width_px: u32, height_px: u32, scale: f64) -> Result<(), InvalidArgument> {
        if !valid_scale(scale) {
            return Err(InvalidArgument);
        }
        self.lifecycle(Lifecycle::Resize {
            width_px,
            height_px,
            scale,
        });
        Ok(())
    }

    /// Caps the frame rate: a render call less than 1 / `fps` s, less
    /// 2 ms, after the previous frame returns [`RenderStatus::NotDue`]. 0
    /// means uncapped, the default.
    pub fn set_max_fps(&self, fps: u32) {
        self.lifecycle(Lifecycle::SetMaxFps(fps));
    }

    /// Pauses the stage and frees its GPU resources as soon as no render
    /// call runs: here, unless one is running, which frees them when it
    /// ends. Never waits.
    pub fn pause(&self) {
        self.lifecycle(Lifecycle::Pause);
        self.pause_waiting.store(true, Ordering::SeqCst);
        // Pairs with the fence after a render call lets go of the frame
        // lock: either this thread sees the lock free, or that call sees
        // the pause waiting, so the release is never missed.
        fence(Ordering::SeqCst);
        self.serve_pause();
    }

    /// Resumes a paused stage. The next frame sets the clock's origin and
    /// the following frames rebuild the creatures.
    pub fn resume(&self) {
        self.lifecycle(Lifecycle::Resume);
    }

    /// Takes every event waiting. Each event goes to exactly one caller,
    /// whichever threads call. At most [`crate::EVENT_CAPACITY`] wait; when
    /// more come, the oldest are dropped and their number is reported first
    /// in an [`Event::Overflow`].
    pub fn events(&self) -> Vec<Event> {
        self.shared.events.drain()
    }

    /// The statistics the latest presented or read-back frame published.
    pub fn frame_stats(&self) -> FrameStats {
        self.shared.snapshot.load().stats
    }

    /// Draws a frame at `target_time` and presents it on the surface.
    pub fn render_to_surface(&self, target_time: f64) -> Result<(), RenderStatus> {
        self.render(target_time, Target::Surface).map(|_| ())
    }

    /// Draws a frame at `target_time` and reads it back.
    pub fn render_to_image(&self, target_time: f64) -> Result<Image, RenderStatus> {
        self.render(target_time, Target::Image)
            .map(|image| image.expect("an image stage reads every frame back"))
    }

    fn render(&self, target_time: f64, target: Target) -> Result<Option<Image>, RenderStatus> {
        let entry = Instant::now();
        let Some(turn) = Turn::take(&self.rendering) else {
            return Err(RenderStatus::Busy);
        };
        if target != self.target {
            return Err(RenderStatus::WrongTarget);
        }
        if !target_time.is_finite() {
            return Err(RenderStatus::InvalidArgument);
        }
        let result = self
            .core
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .render(&self.shared, target_time, entry);
        drop(turn);
        // Pairs with the fence in `pause`.
        fence(Ordering::SeqCst);
        self.serve_pause();
        result
    }

    /// Takes a waiting pause if the frame lock is free; never waits for it.
    fn serve_pause(&self) {
        while self.pause_waiting.load(Ordering::SeqCst) {
            let mut core = match self.core.try_lock() {
                Ok(core) => core,
                Err(TryLockError::Poisoned(e)) => e.into_inner(),
                Err(TryLockError::WouldBlock) => return,
            };
            self.pause_waiting.store(false, Ordering::SeqCst);
            core.take_commands(&self.shared);
        }
    }

    fn scene(&self, command: SceneCommand) {
        self.shared.commands.push(Command::Scene(command));
    }

    fn lifecycle(&self, command: Lifecycle) {
        self.shared.commands.push(Command::Lifecycle(command));
    }
}

fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && scale > 0.0
}
