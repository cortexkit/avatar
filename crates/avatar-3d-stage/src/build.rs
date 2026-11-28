//! Building robots and rasterizing their textures: on worker threads off
//! the frame thread, or inside the render call that first needs them.

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use avatar_3d::robot::{build_with, BuildOptions, MoodSet, Robot};
use avatar_small_robots::{Mood, GENOME_LEN};

/// Where robots are built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildMode {
    /// On worker threads. A robot appears in the first frame after its
    /// build lands, which depends on wall-clock time.
    #[default]
    Workers,
    /// Inside the first render call that needs the robot, on the frame
    /// thread, so a script driving the stage with a virtual clock gets the
    /// same frames every run. That call takes as long as its builds.
    Inline,
}

/// How a stage is run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageOptions {
    /// Where robots are built.
    pub build: BuildMode,
    /// How many worker threads build robots under [`BuildMode::Workers`];
    /// at least one is started. By default one less than the machine's
    /// cores, from one to four.
    pub build_threads: usize,
}

impl Default for StageOptions {
    fn default() -> Self {
        let cores = thread::available_parallelism().map_or(1, |n| n.get());
        Self {
            build: BuildMode::Workers,
            build_threads: cores.saturating_sub(1).clamp(1, 4),
        }
    }
}

/// One robot to build.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct BuildRequest<'a> {
    /// The creature's key.
    pub key: &'a str,
    /// The build's generation: a number no other build of the stage has.
    pub generation: u64,
    /// The genome.
    pub genome: &'a [u8; GENOME_LEN],
    /// The key's latest mood when the build was asked for. Its face set is
    /// the only one the build rasterizes, and the robot is first drawn
    /// with it.
    pub mood: Mood,
    /// The edge in pixels every texture of the robot is rasterized at: a
    /// power of two from 64 to 512.
    pub texture_px: u32,
}

/// One mood's face set to rasterize for a robot already built: the head
/// face and each stalk eye, as drawn and blinking.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct FaceRequest<'a> {
    /// The creature's key.
    pub key: &'a str,
    /// The generation of the robot the set is for.
    pub generation: u64,
    /// The robot's genome.
    pub genome: &'a [u8; GENOME_LEN],
    /// The mood whose face set to rasterize.
    pub mood: Mood,
    /// The edge in pixels: a power of two from 64 to 512.
    pub texture_px: u32,
}

/// Every layer a robot keeps on the GPU, rasterized again at a new edge:
/// its marks, its chest drawing and the face sets of some moods.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct LayersRequest<'a> {
    /// The creature's key.
    pub key: &'a str,
    /// The generation of the robot the layers are for.
    pub generation: u64,
    /// The robot's genome.
    pub genome: &'a [u8; GENOME_LEN],
    /// The moods whose face sets to rasterize with the marks and chest.
    pub moods: MoodSet,
    /// The new edge in pixels: a power of two from 64 to 512.
    pub texture_px: u32,
}

/// Builds robots for a stage, and rasterizes their textures again when a
/// face set or a new edge is needed. [`RobotBuilder`] is the one hosts
/// use; tests put another in front of it to hold or count the work.
///
/// Every call may block; each runs on a worker thread unless the stage
/// builds inline. Textures come as `(binding index, pixels)` pairs: the
/// index into [`Robot::textures`] and `px` × `px` RGBA8 pixels, as
/// [`avatar_3d::robot::Texture::pixels`] holds them.
pub trait Builder: Send + Sync + 'static {
    /// Builds one robot, with the textures of [`BuildRequest::mood`] at
    /// [`BuildRequest::texture_px`].
    fn build(&self, request: &BuildRequest<'_>) -> Robot;

    /// Rasterizes one mood's face set, as [`avatar_3d::robot::face_set`]
    /// does.
    fn face_set(&self, request: &FaceRequest<'_>) -> Vec<(usize, Vec<u8>)> {
        avatar_3d::robot::face_set(request.genome, request.mood, request.texture_px)
    }

    /// Rasterizes the marks, the chest drawing and the face sets of
    /// [`LayersRequest::moods`] at a new edge.
    fn layers(&self, request: &LayersRequest<'_>) -> Vec<(usize, Vec<u8>)> {
        let options = BuildOptions::default()
            .with_texture_px(request.texture_px)
            .with_moods(request.moods);
        build_with(request.genome, options)
            .textures
            .into_iter()
            .enumerate()
            .filter_map(|(i, texture)| texture.pixels.map(|pixels| (i, pixels)))
            .collect()
    }
}

/// Builds each robot from its genome with the face set of its mood drawn at
/// the edge asked for, and works out its reach box and stage camera on the
/// same thread, so nothing that reads the stage ever waits for them. Other
/// face sets and new edges are rasterized from the genome as
/// [`Builder`]'s provided methods do.
#[derive(Clone, Copy, Debug, Default)]
pub struct RobotBuilder;

impl Builder for RobotBuilder {
    fn build(&self, request: &BuildRequest<'_>) -> Robot {
        let options = BuildOptions::default()
            .with_texture_px(request.texture_px)
            .with_moods(MoodSet::NONE.with(request.mood));
        let robot = build_with(request.genome, options);
        // The first call samples every reaction and loop; later calls on
        // the frame thread return the kept result.
        let _ = robot.camera();
        robot
    }
}

/// What a job makes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Task {
    /// A robot, with this mood's face set.
    Build { mood: Mood },
    /// One mood's face set for a robot built before.
    Faces { mood: Mood },
    /// The marks, chest and these face sets at a new edge.
    Layers { moods: MoodSet },
}

/// Work waiting for a worker.
pub(crate) struct Job {
    pub key: String,
    pub generation: u64,
    pub genome: Arc<[u8; GENOME_LEN]>,
    /// The edge the textures are rasterized at.
    pub texture_px: u32,
    pub task: Task,
    /// Set when the work is no longer wanted, so a worker skips it if it
    /// has not started.
    pub cancelled: Arc<AtomicBool>,
}

/// What a finished job made.
pub(crate) enum Output {
    Robot(Robot),
    Faces {
        mood: Mood,
        pixels: Vec<(usize, Vec<u8>)>,
    },
    Layers {
        pixels: Vec<(usize, Vec<u8>)>,
    },
}

/// A finished job.
pub(crate) struct Built {
    pub key: String,
    pub generation: u64,
    pub texture_px: u32,
    pub output: Output,
}

/// The builds of one stage: the workers, the queue they take jobs from and
/// the queue finished jobs land in.
pub(crate) struct Builds {
    builder: Arc<dyn Builder>,
    /// Where jobs go; `None` when building inline or once shut down.
    jobs: Option<mpsc::Sender<Job>>,
    finished: mpsc::Sender<Built>,
    landed: mpsc::Receiver<Built>,
    workers: Vec<JoinHandle<()>>,
    /// Set when the stage is destroyed: workers start no further job.
    closing: Arc<AtomicBool>,
}

impl Builds {
    pub fn new(options: StageOptions, builder: Arc<dyn Builder>) -> Self {
        let (finished, landed) = mpsc::channel();
        let closing = Arc::new(AtomicBool::new(false));
        let mut builds = Self {
            builder,
            jobs: None,
            finished,
            landed,
            workers: Vec::new(),
            closing,
        };
        if options.build == BuildMode::Workers {
            let (jobs, queue) = mpsc::channel::<Job>();
            let queue = Arc::new(Mutex::new(queue));
            for i in 0..options.build_threads.max(1) {
                let builder = Arc::clone(&builds.builder);
                let queue = Arc::clone(&queue);
                let finished = builds.finished.clone();
                let closing = Arc::clone(&builds.closing);
                let worker = thread::Builder::new()
                    .name(format!("avatar-stage-build-{i}"))
                    .spawn(move || work(&*builder, &queue, &finished, &closing))
                    .expect("the system starts a build thread");
                builds.workers.push(worker);
            }
            builds.jobs = Some(jobs);
        }
        builds
    }

    /// Starts a job: queues it for a worker, or runs it now when building
    /// inline. Either way what it makes lands in [`Builds::landed`].
    pub fn request(&self, job: Job) {
        match &self.jobs {
            Some(jobs) => {
                // The workers outlive the sender, so this fails only once
                // shut down, when no build is wanted.
                let _ = jobs.send(job);
            }
            None => {
                if let Some(built) = run(&*self.builder, &job) {
                    let _ = self.finished.send(built);
                }
            }
        }
    }

    /// The next finished job, if one has landed.
    pub fn next_landed(&self) -> Option<Built> {
        self.landed.try_recv().ok()
    }

    /// Drops every finished job that has landed.
    pub fn discard_landed(&self) {
        while self.next_landed().is_some() {}
    }

    /// Cancels every queued job and waits for the running ones to end.
    pub fn shut_down(&mut self) {
        self.closing.store(true, Ordering::Release);
        self.jobs = None;
        for worker in self.workers.drain(..) {
            // A build that panicked was already contained in `run`.
            let _ = worker.join();
        }
    }
}

impl Drop for Builds {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// A worker's loop: take a job, skip it if cancelled or closing, build it.
fn work(
    builder: &dyn Builder,
    queue: &Mutex<mpsc::Receiver<Job>>,
    finished: &mpsc::Sender<Built>,
    closing: &AtomicBool,
) {
    loop {
        let job = queue.lock().unwrap_or_else(PoisonError::into_inner).recv();
        let Ok(job) = job else {
            return;
        };
        if closing.load(Ordering::Acquire) || job.cancelled.load(Ordering::Acquire) {
            continue;
        }
        if let Some(built) = run(builder, &job) {
            if finished.send(built).is_err() {
                return;
            }
        }
    }
}

/// Runs one job. A job that panics yields nothing: a robot stays unbuilt,
/// and a face set or new edge is asked for again while still needed,
/// rather than the panic taking the thread down.
fn run(builder: &dyn Builder, job: &Job) -> Option<Built> {
    let (key, generation, genome, px) = (&job.key, job.generation, &*job.genome, job.texture_px);
    let output = panic::catch_unwind(AssertUnwindSafe(|| match job.task {
        Task::Build { mood } => Output::Robot(builder.build(&BuildRequest {
            key,
            generation,
            genome,
            mood,
            texture_px: px,
        })),
        Task::Faces { mood } => Output::Faces {
            mood,
            pixels: builder.face_set(&FaceRequest {
                key,
                generation,
                genome,
                mood,
                texture_px: px,
            }),
        },
        Task::Layers { moods } => Output::Layers {
            pixels: builder.layers(&LayersRequest {
                key,
                generation,
                genome,
                moods,
                texture_px: px,
            }),
        },
    }))
    .ok()?;
    Some(Built {
        key: job.key.clone(),
        generation: job.generation,
        texture_px: job.texture_px,
        output,
    })
}
