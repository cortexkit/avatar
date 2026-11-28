//! A renderer without a GPU, for tests of the scene core: it records what
//! the stage uploads, retires and draws, can fail at any step on request,
//! and can hold a frame at its present or read-back.

use std::collections::VecDeque;
use std::mem;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use avatar_3d::motion::Pose;
use avatar_3d::robot::Robot;

use crate::renderer::{Failure, Frame, Image, Renderer, Target};

/// The bytes the fake counts for each uploaded robot.
pub const FAKE_ROBOT_BYTES: u64 = 1 << 20;

/// The GPU time the fake gives a frame when no times are scripted, in
/// seconds.
pub const FAKE_GPU_SECONDS: f64 = 0.001;

/// A step of a frame at which the fake can be told to fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// [`Renderer::begin`], before the frame commits.
    Begin,
    /// [`Renderer::upload`].
    Upload,
    /// [`Renderer::submit`].
    Submit,
    /// [`Renderer::present`] or [`Renderer::read_back`], after submit.
    Finish,
    /// [`Renderer::recover`].
    Recover,
}

/// A robot as the fake holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeRobot {
    /// The creature's key.
    pub key: String,
    /// The build's generation.
    pub generation: u64,
}

/// One robot of a submitted frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FakeItem {
    /// The creature's key.
    pub key: String,
    /// The build drawn.
    pub generation: u64,
    /// Its pose.
    pub pose: Pose,
    /// Where the robot stands.
    pub offset: [f64; 3],
}

/// A frame the stage submitted.
#[derive(Clone, Debug, PartialEq)]
pub struct FakeFrame {
    /// The host's time for it.
    pub target_time: f64,
    /// Stage time.
    pub stage_time: f64,
    /// Its size in device pixels.
    pub width_px: u32,
    /// Its size in device pixels.
    pub height_px: u32,
    /// Device pixels per point.
    pub scale: f64,
    /// The robots drawn, in the stage's order.
    pub items: Vec<FakeItem>,
    /// Whether it was then presented or read back.
    pub finished: bool,
}

impl FakeFrame {
    /// The keys drawn, in order.
    pub fn keys(&self) -> Vec<&str> {
        self.items.iter().map(|i| i.key.as_str()).collect()
    }

    /// The item drawn for `key`.
    pub fn item(&self, key: &str) -> Option<&FakeItem> {
        self.items.iter().find(|i| i.key == key)
    }
}

#[derive(Default)]
struct Log {
    frames: Vec<FakeFrame>,
    uploads: Vec<FakeRobot>,
    retired: Vec<FakeRobot>,
    resident: Vec<FakeRobot>,
    /// The frame targets' size, from the first frame after a release.
    targets: Option<(u32, u32)>,
    releases: usize,
    recoveries: usize,
    faults: Vec<(Step, Failure)>,
    gpu_script: VecDeque<f64>,
    /// The GPU time of the latest finished frame, reported with the next.
    gpu_pending: Option<f64>,
    gpu_ready: Vec<f64>,
}

impl Log {
    /// [`FAKE_ROBOT_BYTES`] per resident robot and 8 bytes per pixel of the
    /// frame targets.
    fn memory_bytes(&self) -> u64 {
        let targets = self
            .targets
            .map_or(0, |(w, h)| 8 * u64::from(w) * u64::from(h));
        self.resident.len() as u64 * FAKE_ROBOT_BYTES + targets
    }

    /// Takes the failure set for `step`, if any.
    fn fault(&mut self, step: Step) -> Result<(), Failure> {
        match self.faults.iter().position(|(s, _)| *s == step) {
            Some(i) => Err(self.faults.remove(i).1),
            None => Ok(()),
        }
    }
}

/// Holds frames at their present or read-back while held.
#[derive(Default)]
struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Default)]
struct GateState {
    held: bool,
    waiting: usize,
}

impl Gate {
    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn pass(&self) {
        let mut state = self.lock();
        state.waiting += 1;
        self.changed.notify_all();
        while state.held {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.waiting -= 1;
        self.changed.notify_all();
    }
}

struct FakeShared {
    log: Mutex<Log>,
    finish: Gate,
}

impl FakeShared {
    fn log(&self) -> MutexGuard<'_, Log> {
        self.log.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A [`Renderer`] that records instead of drawing.
pub struct FakeRenderer {
    target: Target,
    timestamps: bool,
    shared: Arc<FakeShared>,
}

/// The test's handle on a [`FakeRenderer`] the stage owns.
#[derive(Clone)]
pub struct FakeControl {
    shared: Arc<FakeShared>,
}

impl FakeRenderer {
    /// A fake drawing to `target`, with or without timestamp queries, and
    /// the handle to watch and steer it.
    pub fn new(target: Target, timestamps: bool) -> (Self, FakeControl) {
        let shared = Arc::new(FakeShared {
            log: Mutex::new(Log::default()),
            finish: Gate::default(),
        });
        let control = FakeControl {
            shared: Arc::clone(&shared),
        };
        (
            Self {
                target,
                timestamps,
                shared,
            },
            control,
        )
    }

    fn finish(&mut self) -> Result<(), Failure> {
        self.shared.finish.pass();
        let mut log = self.shared.log();
        log.fault(Step::Finish)?;
        if let Some(frame) = log.frames.last_mut() {
            frame.finished = true;
        }
        let seconds = log.gpu_script.pop_front().unwrap_or(FAKE_GPU_SECONDS);
        if let Some(earlier) = log.gpu_pending.replace(seconds) {
            log.gpu_ready.push(earlier);
        }
        Ok(())
    }
}

impl Renderer for FakeRenderer {
    type Robot = FakeRobot;

    fn target(&self) -> Target {
        self.target
    }

    fn timestamps(&self) -> bool {
        self.timestamps
    }

    fn begin(&mut self, width_px: u32, height_px: u32, _scale: f64) -> Result<(), Failure> {
        let mut log = self.shared.log();
        log.fault(Step::Begin)?;
        log.targets = Some((width_px, height_px));
        Ok(())
    }

    fn upload(
        &mut self,
        key: &str,
        generation: u64,
        robot: &mut Robot,
    ) -> Result<FakeRobot, Failure> {
        let mut log = self.shared.log();
        log.fault(Step::Upload)?;
        // Take the pixels, as a renderer that frees them on upload does.
        for texture in &mut robot.textures {
            texture.pixels = None;
        }
        let gpu = FakeRobot {
            key: key.to_owned(),
            generation,
        };
        log.uploads.push(gpu.clone());
        log.resident.push(gpu.clone());
        Ok(gpu)
    }

    fn retire(&mut self, robot: FakeRobot) {
        let mut log = self.shared.log();
        log.resident.retain(|r| *r != robot);
        log.retired.push(robot);
    }

    fn submit(&mut self, frame: &Frame<'_, FakeRobot>) -> Result<(), Failure> {
        let mut log = self.shared.log();
        log.fault(Step::Submit)?;
        let items = frame
            .items
            .iter()
            .map(|item| FakeItem {
                key: item.key.to_owned(),
                generation: item.generation,
                pose: item.pose.clone(),
                offset: item.offset,
            })
            .collect();
        log.frames.push(FakeFrame {
            target_time: frame.target_time,
            stage_time: frame.stage_time,
            width_px: frame.width_px,
            height_px: frame.height_px,
            scale: frame.scale,
            items,
            finished: false,
        });
        Ok(())
    }

    fn present(&mut self) -> Result<(), Failure> {
        self.finish()
    }

    fn read_back(&mut self) -> Result<Image, Failure> {
        self.finish()?;
        let (width, height) = self.shared.log().targets.unwrap_or((0, 0));
        Ok(Image {
            width,
            height,
            pixels: vec![0; 4 * width as usize * height as usize],
        })
    }

    fn take_gpu_times(&mut self) -> Vec<f64> {
        let ready = mem::take(&mut self.shared.log().gpu_ready);
        if self.timestamps {
            ready
        } else {
            Vec::new()
        }
    }

    fn release(&mut self) {
        let mut log = self.shared.log();
        log.resident.clear();
        log.targets = None;
        log.releases += 1;
    }

    fn recover(&mut self) -> Result<(), Failure> {
        let mut log = self.shared.log();
        log.fault(Step::Recover)?;
        log.resident.clear();
        log.targets = None;
        log.gpu_pending = None;
        log.recoveries += 1;
        Ok(())
    }

    fn memory_bytes(&self) -> u64 {
        self.shared.log().memory_bytes()
    }
}

impl FakeControl {
    /// Every frame submitted, in order.
    pub fn frames(&self) -> Vec<FakeFrame> {
        self.shared.log().frames.clone()
    }

    /// The latest frame submitted.
    pub fn last_frame(&self) -> Option<FakeFrame> {
        self.shared.log().frames.last().cloned()
    }

    /// How many frames were submitted.
    pub fn frame_count(&self) -> usize {
        self.shared.log().frames.len()
    }

    /// Every upload, in order.
    pub fn uploads(&self) -> Vec<FakeRobot> {
        self.shared.log().uploads.clone()
    }

    /// Every robot retired, in order.
    pub fn retired(&self) -> Vec<FakeRobot> {
        self.shared.log().retired.clone()
    }

    /// The robots uploaded and neither retired nor released since.
    pub fn resident(&self) -> Vec<FakeRobot> {
        self.shared.log().resident.clone()
    }

    /// How many times the stage released everything.
    pub fn releases(&self) -> usize {
        self.shared.log().releases
    }

    /// How many times the stage recovered the device.
    pub fn recoveries(&self) -> usize {
        self.shared.log().recoveries
    }

    /// The memory the fake counts: [`FAKE_ROBOT_BYTES`] per resident robot
    /// and 8 bytes per pixel of the frame targets.
    pub fn memory_bytes(&self) -> u64 {
        self.shared.log().memory_bytes()
    }

    /// Makes the next call at `step` fail with `failure`, once.
    pub fn fail(&self, step: Step, failure: Failure) {
        self.shared.log().faults.push((step, failure));
    }

    /// The GPU times the next finished frames take, in order; after them,
    /// [`FAKE_GPU_SECONDS`]. Each is reported with the frame after.
    pub fn script_gpu_times(&self, seconds: &[f64]) {
        self.shared.log().gpu_script.extend(seconds);
    }

    /// Holds every frame at its present or read-back until
    /// [`FakeControl::release_finish`].
    pub fn hold_finish(&self) {
        self.shared.finish.lock().held = true;
    }

    /// Lets held frames go on.
    pub fn release_finish(&self) {
        self.shared.finish.lock().held = false;
        self.shared.finish.changed.notify_all();
    }

    /// Waits until a frame is held at its present or read-back, for at most
    /// `timeout`; whether one is.
    pub fn wait_held(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.finish.lock();
        while state.waiting == 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self
                .shared
                .finish
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }
}
