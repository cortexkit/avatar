//! The first numbers, printed without thresholds: for the stage (the
//! stated robot alone) and the plaza (the plaza sheet's 33 robots, placed
//! by `avatar_3d_wgpu::plaza::layout` from their neutral rest footprints),
//! both animating, the frame size, the warmup
//! and measured frame counts, the CPU and GPU frame times, the time to
//! build and to upload one robot, and what the robots hold in memory.
//!
//! - Animation is stepped 1/30 s a frame whatever a frame takes: the step
//!   sets the poses, not a frame rate. Every frame is drawn with
//!   [`Renderer::draw`], which reads nothing back, and the GPU is waited
//!   for before the next frame, so frames never overlap.
//! - CPU frame time runs from the start of posing (every actor evaluated at
//!   the frame's time) until the frame is submitted: the per-frame buffers
//!   written and the pass recorded and submitted. It leaves out the wait
//!   for the GPU; nothing is read back or encoded to PNG.
//! - GPU frame time is the scene pass from its first timestamp to its
//!   last, or "unavailable" when the device has no timestamp queries.
//! - Build time runs until `build` returns: the geometry and every texture
//!   rasterized. Upload time runs from [`Renderer::upload`] until the GPU
//!   has finished the copies; the time until `upload` returns is printed
//!   beside it. One robot is built and uploaded untimed first, so no
//!   one-time setup lands in a sample.
//! - Memory is what the code allocates, in decimal megabytes (10^6 bytes);
//!   a driver may pad or compress it. GPU textures count every layer's mip
//!   chain; the CPU side counts the pixel buffers a `Robot` keeps.
//! - The plaza's texture memory is broken down by kind, with what stalk
//!   eyes add, and with what 128 px faces, or only the moods on screen,
//!   would hold instead. A census of [`CENSUS`] genomes of the plaza's seed,
//!   built without textures, shows how often robots carry stalk eyes.

use std::time::Instant;

use avatar_3d::motion::{Actor, Pose, REACTIONS, STATUS_LOOPS};
use avatar_3d::robot::{build, build_with, BuildOptions, Robot, Skin, TextureContent, TEXTURE_PX};
use avatar_3d_wgpu::upload::mip_levels;
use avatar_3d_wgpu::view::View;
use avatar_3d_wgpu::{target_bytes, Background, GpuRobot, Item, RenderError, Renderer, Scene};
use avatar_small_robots::{Mood, GENOME_LEN, MOODS};

use crate::{genome, place_plaza, BACKGROUND, FRAME, PLAZA_ROBOTS, PLAZA_SEED, PLAZA_SIZE, STATED};

/// Seconds of animation a frame advances.
const STEP: f64 = 1.0 / 30.0;
/// The stage's warmup and measured frames.
const STAGE_FRAMES: (usize, usize) = (60, 600);
/// The plaza's warmup and measured frames.
const PLAZA_FRAMES: (usize, usize) = (30, 300);
/// How many times the stage robot is built, and uploaded, for its times.
const STAGE_RUNS: usize = 15;
/// Genomes of the plaza's seed counted, built without textures, for how
/// many textures robots hold.
const CENSUS: u64 = 1000;
/// Bytes in a decimal megabyte.
const MB: f64 = 1e6;
/// The face pairs a robot holds: one per mood.
const MOOD_SETS: u64 = MOODS.len() as u64;

/// The median and the 95th percentile (nearest rank) of samples.
fn median_p95(samples: &[f64]) -> (f64, f64) {
    assert!(!samples.is_empty(), "a statistic needs samples");
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    let median = if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    };
    let rank = (95 * n).div_ceil(100).max(1);
    (median, s[rank - 1])
}

/// Seconds as milliseconds.
fn ms(seconds: f64) -> String {
    format!("{:.3} ms", seconds * 1e3)
}

/// Bytes as decimal megabytes, or kilobytes when small.
fn mb(bytes: u64) -> String {
    if bytes < 100_000 {
        format!("{:.1} kB", bytes as f64 / 1e3)
    } else {
        format!("{:.2} MB", bytes as f64 / MB)
    }
}

/// `part` as a percentage of `whole`.
fn share(part: u64, whole: u64) -> String {
    format!("{:.1}%", 100.0 * part as f64 / whole.max(1) as f64)
}

/// One labelled line of the report.
fn line(label: &str, value: impl std::fmt::Display) {
    println!("{label:<28}{value}");
}

/// The GPU bytes of one square RGBA8 texture layer of edge `px` with its
/// whole mip chain, as [`Renderer::upload`] stores it.
fn layer_bytes(px: u32) -> u64 {
    (0..mip_levels(px))
        .map(|level| {
            let edge = u64::from((px >> level).max(1));
            4 * edge * edge
        })
        .sum()
}

/// The bytes of one square RGBA8 texture of edge `px` without mips, as a
/// `Robot` keeps its pixels.
fn pixel_bytes(px: u32) -> u64 {
    4 * u64::from(px) * u64::from(px)
}

/// A robot built and uploaded, with what each step took, in seconds.
struct Timed {
    robot: Robot,
    gpu: GpuRobot,
    build: f64,
    /// Until [`Renderer::upload`] returned.
    upload_call: f64,
    /// Until the GPU had finished the upload's copies.
    upload: f64,
}

fn build_and_upload(renderer: &Renderer, genome: &[u8; GENOME_LEN]) -> Result<Timed, RenderError> {
    let start = Instant::now();
    let robot = build(genome);
    let build = start.elapsed().as_secs_f64();
    renderer.finish_uploads()?;
    let start = Instant::now();
    let gpu = renderer.upload(&robot);
    let upload_call = start.elapsed().as_secs_f64();
    renderer.finish_uploads()?;
    let upload = start.elapsed().as_secs_f64();
    assert_eq!(
        gpu.texture_bytes,
        robot.textures.len().max(1) as u64 * layer_bytes(TEXTURE_PX),
        "every texture is uploaded as one layer with its mip chain"
    );
    Ok(Timed {
        robot,
        gpu,
        build,
        upload_call,
        upload,
    })
}

/// Seconds `build_with` takes with textures off: the geometry alone.
fn untextured_build(genome: &[u8; GENOME_LEN]) -> (Robot, f64) {
    let start = Instant::now();
    let robot = build_with(genome, BuildOptions::untextured());
    (robot, start.elapsed().as_secs_f64())
}

/// Times `start`, `start + period`, … up to `until`, with their count.
pub(crate) fn every(start: f64, period: f64, until: f64) -> impl Iterator<Item = (usize, f64)> {
    (0..)
        .map(move |k| (k, start + period * k as f64))
        .take_while(move |(_, t)| *t <= until)
}

/// The stage's events up to `until` seconds, over the idle layer: from
/// 0.5 s a reaction every 3 s in `REACTIONS` order; from 2 s the mood
/// moving on to the next in `MOODS` order every 7 s, which swaps the face;
/// from 4 s the status loops in `STATUS_LOOPS` order, 9 s each.
fn stage_actor(robot: &Robot, until: f64) -> Actor<'_> {
    let mut actor = Actor::new(robot, Mood::Neutral);
    for (k, t) in every(0.5, 3.0, until) {
        actor.play(t, REACTIONS[k % REACTIONS.len()]);
    }
    for (k, t) in every(2.0, 7.0, until) {
        actor.set_mood(t, MOODS[(k + 1) % MOODS.len()]);
    }
    for (k, t) in every(4.0, 9.0, until) {
        actor.set_loop(t, Some(STATUS_LOOPS[k % STATUS_LOOPS.len()]));
    }
    actor
}

/// Plaza robot `i`'s events up to `until` seconds, over the idle layer, in
/// neutral throughout: a reaction every 4 s from `0.5 + 0.12 i` s, robot
/// `i` starting at the `i`-th of `REACTIONS`, so the robots move out of
/// step.
fn plaza_actor(robot: &Robot, i: usize, until: f64) -> Actor<'_> {
    let mut actor = Actor::new(robot, Mood::Neutral);
    for (k, t) in every(0.5 + 0.12 * i as f64, 4.0, until) {
        actor.play(t, REACTIONS[(i + k) % REACTIONS.len()]);
    }
    actor
}

/// Frame times of one scene's measured frames, in seconds.
#[derive(Default)]
struct Times {
    /// From the start of posing until the frame was submitted.
    cpu: Vec<f64>,
    /// Posing alone.
    posing: Vec<f64>,
    /// The scene pass on the GPU, for the frames that were timed.
    gpu: Vec<f64>,
}

/// What one scene draws: its robots, where they stand, the camera and the
/// frame size.
pub(crate) struct Stage<'a> {
    pub(crate) actors: Vec<Actor<'a>>,
    pub(crate) gpus: Vec<&'a GpuRobot>,
    pub(crate) offsets: Vec<[f64; 3]>,
    pub(crate) view: View,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Stage<'_> {
    /// Every actor's pose at `t` seconds, in actor order.
    pub(crate) fn poses(&self, t: f64) -> Vec<Pose> {
        self.actors.iter().map(|a| a.evaluate(t)).collect()
    }

    /// The robots in `poses`, one per actor in actor order, each where it
    /// stands.
    pub(crate) fn items<'s>(&'s self, poses: &'s [Pose]) -> Vec<Item<'s>> {
        self.actors
            .iter()
            .zip(&self.gpus)
            .zip(poses)
            .zip(&self.offsets)
            .map(|(((actor, gpu), pose), offset)| Item {
                robot: actor.robot(),
                gpu,
                pose,
                offset: *offset,
            })
            .collect()
    }

    /// The frame that draws `items` from the stage's camera at its size.
    pub(crate) fn scene<'s>(&self, items: &'s [Item<'s>]) -> Scene<'s> {
        Scene {
            view: self.view,
            width: self.width,
            height: self.height,
            background: Background::Opaque(BACKGROUND),
            shadows: true,
            items,
        }
    }
}

/// Draws `warmup + measured` frames, each `STEP` seconds of animation on
/// from the last, and keeps the measured frames' times.
fn measure(
    renderer: &Renderer,
    stage: &Stage<'_>,
    (warmup, measured): (usize, usize),
) -> Result<Times, RenderError> {
    let mut times = Times::default();
    for f in 0..warmup + measured {
        let t = f as f64 * STEP;
        let start = Instant::now();
        let poses = stage.poses(t);
        let posing = start.elapsed().as_secs_f64();
        let items = stage.items(&poses);
        let submitted = renderer.draw(&stage.scene(&items));
        let cpu = start.elapsed().as_secs_f64();
        let gpu = submitted.wait()?;
        if f >= warmup {
            times.cpu.push(cpu);
            times.posing.push(posing);
            times.gpu.extend(gpu);
        }
    }
    Ok(times)
}

/// What robots hold, in bytes, and their triangles.
#[derive(Clone, Copy, Debug, Default)]
struct Memory {
    triangles: u64,
    /// Texture layers with their mip chains.
    gpu_textures: u64,
    /// Vertex and index buffers.
    gpu_geometry: u64,
    /// The buffers rewritten every frame, and the texture bindings.
    gpu_frame: u64,
    /// The pixel buffers the `Robot` keeps.
    cpu_textures: u64,
    /// The `Robot`'s meshes: positions, normals, projection coordinates,
    /// triangles, tags and skin weights.
    cpu_meshes: u64,
}

impl Memory {
    fn of(robot: &Robot, gpu: &GpuRobot) -> Self {
        let cpu_meshes = robot
            .parts
            .iter()
            .map(|p| {
                let m = &p.mesh;
                let skin = match &p.skin {
                    Skin::Skinned { joints, weights } => 16 * joints.len() + 32 * weights.len(),
                    _ => 0,
                };
                24 * m.positions.len()
                    + 24 * m.normals.len()
                    + 16 * m.projection.len()
                    + 12 * m.triangles.len()
                    + m.textured_front.len()
                    + skin
            })
            .sum::<usize>() as u64;
        Self {
            triangles: u64::from(gpu.triangles),
            gpu_textures: gpu.texture_bytes,
            gpu_geometry: gpu.geometry_bytes,
            gpu_frame: gpu.frame_bytes,
            cpu_textures: robot
                .textures
                .iter()
                .filter_map(|t| t.pixels.as_ref())
                .map(|p| p.len() as u64)
                .sum(),
            cpu_meshes,
        }
    }

    fn sum<'a>(all: impl IntoIterator<Item = &'a Memory>) -> Self {
        all.into_iter().fold(Self::default(), |a, b| Self {
            triangles: a.triangles + b.triangles,
            gpu_textures: a.gpu_textures + b.gpu_textures,
            gpu_geometry: a.gpu_geometry + b.gpu_geometry,
            gpu_frame: a.gpu_frame + b.gpu_frame,
            cpu_textures: a.cpu_textures + b.cpu_textures,
            cpu_meshes: a.cpu_meshes + b.cpu_meshes,
        })
    }
}

/// A robot's textures by what they show.
#[derive(Clone, Copy, Debug, Default)]
struct Kinds {
    /// Face textures: a pair per mood.
    faces: u64,
    /// Stalk-eye textures: a pair per mood per eye.
    stalk_eyes: u64,
    /// Marks and chest drawings, one each, whatever the mood.
    other: u64,
}

impl Kinds {
    fn of(robot: &Robot) -> Self {
        let mut k = Self::default();
        for t in &robot.textures {
            match t.binding.content {
                TextureContent::Face { .. } => k.faces += 1,
                TextureContent::StalkEye { .. } => k.stalk_eyes += 1,
                _ => k.other += 1,
            }
        }
        assert!(
            k.faces % MOOD_SETS == 0 && k.stalk_eyes % MOOD_SETS == 0,
            "faces and stalk eyes come in the same number of textures per mood"
        );
        k
    }

    fn count(&self) -> u64 {
        self.faces + self.stalk_eyes + self.other
    }

    fn sum<'a>(all: impl IntoIterator<Item = &'a Kinds>) -> Self {
        all.into_iter().fold(Self::default(), |a, b| Self {
            faces: a.faces + b.faces,
            stalk_eyes: a.stalk_eyes + b.stalk_eyes,
            other: a.other + b.other,
        })
    }

    /// The bytes these textures would take with `face_px` faces and stalk
    /// eyes, keeping `moods` of the seven moods' face sets, and every other
    /// texture at [`TEXTURE_PX`]; `bytes` gives one texture's bytes at an
    /// edge.
    fn bytes(&self, face_px: u32, moods: u64, bytes: fn(u32) -> u64) -> u64 {
        (self.faces + self.stalk_eyes) * moods / MOOD_SETS * bytes(face_px)
            + self.other * bytes(TEXTURE_PX)
    }
}

/// Prints one scene's frame size, frame counts and frame times.
fn print_frames(renderer: &Renderer, stage: &Stage<'_>, frames: (usize, usize), times: &Times) {
    line(
        "target size",
        format!("{} x {} px", stage.width, stage.height),
    );
    line(
        "frames",
        format!(
            "{} warmup, {} measured, animation stepped {:.4} s a frame",
            frames.0, frames.1, STEP
        ),
    );
    let (median, p95) = median_p95(&times.cpu);
    let (pose_median, pose_p95) = median_p95(&times.posing);
    line(
        "CPU frame time",
        format!(
            "median {}, p95 {} (posing alone: median {}, p95 {})",
            ms(median),
            ms(p95),
            ms(pose_median),
            ms(pose_p95)
        ),
    );
    if times.gpu.is_empty() {
        let why = if renderer.timestamps() {
            "the timestamps read back as no time"
        } else {
            "the device has no timestamp queries"
        };
        line("GPU frame time", format!("unavailable ({why})"));
    } else {
        let (median, p95) = median_p95(&times.gpu);
        line(
            "GPU frame time",
            format!(
                "median {}, p95 {} ({} of {} frames timed)",
                ms(median),
                ms(p95),
                times.gpu.len(),
                times.cpu.len()
            ),
        );
    }
}

/// Prints the build and upload times of robots; `whose` says which robots
/// the samples come from.
fn print_build(robots: &[Timed], untextured: &[f64], whose: &str) {
    let n = robots.len();
    let builds: Vec<f64> = robots.iter().map(|r| r.build).collect();
    let calls: Vec<f64> = robots.iter().map(|r| r.upload_call).collect();
    let uploads: Vec<f64> = robots.iter().map(|r| r.upload).collect();
    let (build, build_p95) = median_p95(&builds);
    let (bare, _) = median_p95(untextured);
    line(
        "build one robot (CPU)",
        format!(
            "median {}, p95 {} over {n} builds {whose}, until build returns with every texture (geometry alone, untextured: median {})",
            ms(build),
            ms(build_p95),
            ms(bare)
        ),
    );
    let (upload, upload_p95) = median_p95(&uploads);
    let (call, _) = median_p95(&calls);
    line(
        "upload one robot",
        format!(
            "median {}, p95 {} over {n} uploads {whose}, until the GPU has it (until upload returns: median {})",
            ms(upload),
            ms(upload_p95),
            ms(call)
        ),
    );
}

/// Prints what robots hold.
fn print_memory(memory: &Memory, layers: u64, width: u32, height: u32) {
    line("resident triangles", memory.triangles);
    line(
        "GPU textures",
        format!(
            "{} ({layers} layers of {TEXTURE_PX} px RGBA8 with mip chains)",
            mb(memory.gpu_textures)
        ),
    );
    line(
        "GPU meshes",
        format!("{} (vertices and indices)", mb(memory.gpu_geometry)),
    );
    line(
        "GPU per-frame buffers",
        format!(
            "{} (placement, skin matrices, materials, texture bindings)",
            mb(memory.gpu_frame)
        ),
    );
    line(
        "GPU frame targets",
        format!(
            "{} (4x multisampled colour and depth, resolve, readback)",
            mb(target_bytes(width, height))
        ),
    );
    line(
        "CPU textures",
        format!(
            "{} (the pixel buffers the robots keep, no mips)",
            mb(memory.cpu_textures)
        ),
    );
    line(
        "CPU meshes",
        format!("{} (f64 vertex data as built)", mb(memory.cpu_meshes)),
    );
}

/// Prints the plaza's texture memory by kind, what stalk eyes add, and
/// what smaller or fewer face textures would hold instead.
fn print_textures(kinds: &[Kinds]) {
    let all = Kinds::sum(kinds);
    let now = all.bytes(TEXTURE_PX, MOOD_SETS, layer_bytes);
    let half = TEXTURE_PX / 2;
    line(
        "texture layers",
        format!(
            "{} = {} face, {} stalk-eye, {} marks and chest",
            all.count(),
            all.faces,
            all.stalk_eyes,
            all.other
        ),
    );
    line(
        "GPU textures by kind",
        format!(
            "faces {}, stalk eyes {}, marks and chest {}",
            mb(all.faces * layer_bytes(TEXTURE_PX)),
            mb(all.stalk_eyes * layer_bytes(TEXTURE_PX)),
            mb(all.other * layer_bytes(TEXTURE_PX))
        ),
    );
    let stalked: Vec<&Kinds> = kinds.iter().filter(|k| k.stalk_eyes > 0).collect();
    let stalk_bytes = all.stalk_eyes * layer_bytes(TEXTURE_PX);
    let stalked_bytes: u64 = stalked
        .iter()
        .map(|k| k.bytes(TEXTURE_PX, MOOD_SETS, layer_bytes))
        .sum();
    line(
        "stalk-eyed robots",
        format!(
            "{} of {}: their stalk-eye layers {} ({} of the plaza's GPU textures); their textures in all {} ({})",
            stalked.len(),
            kinds.len(),
            mb(stalk_bytes),
            share(stalk_bytes, now),
            mb(stalked_bytes),
            share(stalked_bytes, now)
        ),
    );
    for (label, bytes) in [("GPU", layer_bytes as fn(u32) -> u64), ("CPU", pixel_bytes)] {
        let now = all.bytes(TEXTURE_PX, MOOD_SETS, bytes);
        let saving = |held: u64| {
            format!(
                "{} (saves {}, {})",
                mb(held),
                mb(now - held),
                share(now - held, now)
            )
        };
        line(&format!("{label} textures now"), mb(now));
        line(
            &format!("{label} with {half} px faces"),
            saving(all.bytes(half, MOOD_SETS, bytes)),
        );
        line(
            &format!("{label} with one mood"),
            saving(all.bytes(TEXTURE_PX, 1, bytes)),
        );
        line(
            &format!("{label} with two moods"),
            saving(all.bytes(TEXTURE_PX, 2, bytes)),
        );
        line(
            &format!("{label} with one mood, {half} px"),
            saving(all.bytes(half, 1, bytes)),
        );
    }
}

/// Prints how many textures robots of the plaza's seed hold, from a
/// census of untextured builds.
fn print_census(kinds: &[Kinds]) {
    let n = kinds.len() as u64;
    let stalked = kinds.iter().filter(|k| k.stalk_eyes > 0).count() as u64;
    let counts: Vec<f64> = kinds.iter().map(|k| k.count() as f64).collect();
    let (median, _) = median_p95(&counts);
    let min = kinds.iter().map(Kinds::count).min().unwrap_or(0);
    let max = kinds.iter().map(Kinds::count).max().unwrap_or(0);
    let all = Kinds::sum(kinds);
    let bytes = all.bytes(TEXTURE_PX, MOOD_SETS, layer_bytes);
    let mean = bytes / n.max(1);
    let most = max * layer_bytes(TEXTURE_PX);
    line(
        "genomes",
        format!(
            "0 to {} of seed {PLAZA_SEED}, built without textures",
            n - 1
        ),
    );
    line(
        "stalk-eyed robots",
        format!("{stalked} of {n} ({})", share(stalked, n)),
    );
    line(
        "textures per robot",
        format!(
            "min {min}, median {median}, max {max}, mean {:.2}",
            all.count() as f64 / n.max(1) as f64
        ),
    );
    line(
        "GPU textures per robot",
        format!(
            "mean {}, max {}; {PLAZA_ROBOTS} robots at the mean {}, at the max {}",
            mb(mean),
            mb(most),
            mb(PLAZA_ROBOTS * mean),
            mb(PLAZA_ROBOTS * most)
        ),
    );
    line(
        "stalk-eye share",
        format!(
            "{} of all GPU texture bytes",
            share(all.stalk_eyes * layer_bytes(TEXTURE_PX), bytes)
        ),
    );
}

/// Measures and prints the stage's and the plaza's numbers.
pub fn run(renderer: &Renderer) -> Result<(), RenderError> {
    let info = renderer.adapter();
    println!(
        "numbers: {} on {:?}; timestamp queries {}",
        info.name,
        info.backend,
        if renderer.timestamps() { "on" } else { "off" }
    );
    println!("numbers: medians, and 95th percentiles by nearest rank; MB are 10^6 bytes");

    // One untimed build and upload, so no one-time setup lands in a sample.
    let stated = genome(STATED.0, STATED.1);
    drop(build_and_upload(renderer, &stated)?);

    eprintln!("numbers: stage");
    let mut stage_robots = Vec::with_capacity(STAGE_RUNS);
    let mut stage_bare = Vec::with_capacity(STAGE_RUNS);
    for _ in 0..STAGE_RUNS {
        stage_robots.push(build_and_upload(renderer, &stated)?);
        stage_bare.push(untextured_build(&stated).1);
    }
    let shown = stage_robots.last().expect("the stage robot was built");
    let until = (STAGE_FRAMES.0 + STAGE_FRAMES.1) as f64 * STEP;
    let stage = Stage {
        actors: vec![stage_actor(&shown.robot, until)],
        gpus: vec![&shown.gpu],
        offsets: vec![[0.0; 3]],
        view: View::from(&shown.robot.camera()),
        width: FRAME,
        height: FRAME,
    };
    let times = measure(renderer, &stage, STAGE_FRAMES)?;
    println!();
    println!(
        "[stage] seed {} genome {} alone, from its stage camera; a reaction every 3 s, a mood change every 7 s, a status loop every 9 s",
        STATED.0, STATED.1
    );
    print_frames(renderer, &stage, STAGE_FRAMES, &times);
    print_build(&stage_robots, &stage_bare, "of this robot");
    print_memory(
        &Memory::of(&shown.robot, &shown.gpu),
        shown.robot.textures.len() as u64,
        stage.width,
        stage.height,
    );

    eprintln!("numbers: plaza");
    let mut built = Vec::with_capacity(PLAZA_ROBOTS as usize);
    let mut bare = Vec::with_capacity(PLAZA_ROBOTS as usize);
    for i in 0..PLAZA_ROBOTS {
        let g = genome(PLAZA_SEED, i);
        built.push(build_and_upload(renderer, &g)?);
        bare.push(untextured_build(&g).1);
    }
    // Placed as the plaza review sheet places them.
    let (_, placed) = place_plaza(built.iter().map(|b| &b.robot));
    let (width, height) = PLAZA_SIZE;
    let until = (PLAZA_FRAMES.0 + PLAZA_FRAMES.1) as f64 * STEP;
    let stage = Stage {
        actors: built
            .iter()
            .enumerate()
            .map(|(i, b)| plaza_actor(&b.robot, i, until))
            .collect(),
        gpus: built.iter().map(|b| &b.gpu).collect(),
        offsets: placed.offsets.clone(),
        view: placed.view,
        width,
        height,
    };
    let times = measure(renderer, &stage, PLAZA_FRAMES)?;
    println!();
    println!(
        "[plaza] genomes 0 to {} of seed {PLAZA_SEED}, placed as the plaza sheet places them; neutral, a reaction every 4 s each, out of step",
        PLAZA_ROBOTS - 1
    );
    print_frames(renderer, &stage, PLAZA_FRAMES, &times);
    print_build(&built, &bare, "of the plaza's robots, one each");
    let memory: Vec<Memory> = built.iter().map(|b| Memory::of(&b.robot, &b.gpu)).collect();
    let kinds: Vec<Kinds> = built.iter().map(|b| Kinds::of(&b.robot)).collect();
    print_memory(
        &Memory::sum(&memory),
        Kinds::sum(&kinds).count(),
        width,
        height,
    );
    println!();
    println!(
        "[plaza textures] by kind, and what {} px faces and stalk eyes, or only the moods on screen (one, or two while a mood changes), would hold instead",
        TEXTURE_PX / 2
    );
    print_textures(&kinds);

    eprintln!("numbers: census");
    let census: Vec<Kinds> = (0..CENSUS)
        .map(|i| Kinds::of(&untextured_build(&genome(PLAZA_SEED, i)).0))
        .collect();
    println!();
    println!("[census] how many textures robots hold");
    print_census(&census);
    Ok(())
}
