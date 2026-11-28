//! Building robots: on worker threads off the frame thread, or inside the
//! render call that first needs them.

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
    /// The key's latest mood when the build was asked for, whose face set
    /// the robot is drawn with first.
    pub mood: Mood,
}

/// Builds robots for a stage. [`RobotBuilder`] is the one hosts use; tests
/// put another in front of it to hold or count builds.
pub trait Builder: Send + Sync + 'static {
    /// Builds one robot. It may block; it runs on a worker thread unless
    /// the stage builds inline.
    fn build(&self, request: &BuildRequest<'_>) -> Robot;
}

/// Builds each robot from its genome with the face set of its mood drawn,
/// and works out its reach box and stage camera on the same thread, so
/// nothing that reads the stage ever waits for them.
///
/// Faces are drawn at the default 256 px edge until texture edges follow
/// the size robots are shown at.
#[derive(Clone, Copy, Debug, Default)]
pub struct RobotBuilder;

impl Builder for RobotBuilder {
    fn build(&self, request: &BuildRequest<'_>) -> Robot {
        let options = BuildOptions::default().with_moods(MoodSet::NONE.with(request.mood));
        let robot = build_with(request.genome, options);
        // The first call samples every reaction and loop; later calls on
        // the frame thread return the kept result.
        let _ = robot.camera();
        robot
    }
}

/// A build waiting for a worker.
pub(crate) struct Job {
    pub key: String,
    pub generation: u64,
    pub genome: Arc<[u8; GENOME_LEN]>,
    pub mood: Mood,
    /// Set when the build is no longer wanted, so a worker skips it if it
    /// has not started.
    pub cancelled: Arc<AtomicBool>,
}

/// A finished build.
pub(crate) struct Built {
    pub key: String,
    pub generation: u64,
    pub robot: Robot,
}

/// The builds of one stage: the workers, the queue they take jobs from and
/// the queue finished robots land in.
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

    /// Starts a build: queues it for a worker, or builds it now when
    /// building inline. Either way the robot lands in [`Builds::landed`].
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

    /// The next finished build, if one has landed.
    pub fn next_landed(&self) -> Option<Built> {
        self.landed.try_recv().ok()
    }

    /// Drops every finished build that has landed.
    pub fn discard_landed(&self) {
        while self.next_landed().is_some() {}
    }

    /// Cancels every queued build and waits for the running ones to end.
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

/// Builds one job. A build that panics yields nothing: its creature stays
/// unbuilt rather than taking the thread down.
fn run(builder: &dyn Builder, job: &Job) -> Option<Built> {
    let request = BuildRequest {
        key: &job.key,
        generation: job.generation,
        genome: &job.genome,
        mood: job.mood,
    };
    let robot = panic::catch_unwind(AssertUnwindSafe(|| builder.build(&request))).ok()?;
    Some(Built {
        key: job.key.clone(),
        generation: job.generation,
        robot,
    })
}
