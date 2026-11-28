//! Films of the robots moving: consecutive frames, [`FPS`] a second, of
//! robots posed by their actors with the idle layer on, each frame drawn by
//! the renderer and written to PNG, then encoded to an H.264 MP4 with
//! ffmpeg; the PNGs are deleted once encoded. Beside each video a TSV of
//! the same name lists when each event starts.
//!
//! - `stage`, written as `stage-501-14.mp4`, 576 px square: the stated
//!   robot from its stage camera, in one script ([`stage_script`]).
//! - `variety`, written as `stage-509-variety.mp4`, 576 px square: five
//!   robots of seed 509 with different running gear ([`variety_picks`]),
//!   each from its own stage camera for [`VARIETY_SHOT`] seconds: a mood
//!   change, then hello, then ask ([`variety_script`]).
//! - `plaza`, written as `plaza-503.mp4`, 1600 × 1000: the plaza sheet's 33
//!   robots where the plaza sheet places them, labelled at their posed
//!   `headTop` anchors, each in one mood with its status loop where the
//!   mood has one, reacting out of step ([`plaza_actor`]).
//!
//! ffmpeg is looked up on `PATH` before anything is drawn.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use avatar_3d::motion::{Actor, Reaction, StatusLoop, LOOP_BLEND, REACTIONS};
use avatar_3d::robot::{build_with, BuildOptions, Robot, TextureContent};
use avatar_3d_wgpu::plaza::Plaza;
use avatar_3d_wgpu::view::View;
use avatar_3d_wgpu::{frame, GpuRobot, Renderer};
use avatar_small_robots::{mood_name, Mood, MOODS};

use crate::numbers::{every, Stage};
use crate::sheet::Sheet;
use crate::{
    genome, place_plaza, plaza_label, Built, FRAME, PLAZA_SEED, PLAZA_SIZE, RANDOM_SEEDS, STATED,
};

/// Frames a second.
const FPS: u32 = 30;
/// The edge of a stage film's frame: twice a sheet cell's.
const STAGE_EDGE: u32 = 2 * FRAME;
/// Seconds of rest that open the stage film.
const OPENING_REST: f64 = 3.0;
/// Seconds the stage film holds each mood.
const MOOD_HOLD: f64 = 3.0;
/// Seconds of rest before each of the stage film's reactions and after
/// the last.
const REACTION_REST: f64 = 2.0;
/// The status loops the stage film plays after the reactions.
const STAGE_LOOPS: [StatusLoop; 2] = [StatusLoop::Working, StatusLoop::Sleepy];
/// Seconds the stage film plays each status loop.
const LOOP_HOLD: f64 = 4.0;
/// Seconds the stage film runs on after the last loop stops, while it
/// fades out.
const CLOSING_REST: f64 = 1.0;
/// Seconds each robot of the variety film is on screen.
const VARIETY_SHOT: f64 = 8.0;
/// The variety film's moods, one per robot in pick order.
const VARIETY_MOODS: [Mood; 5] = [
    Mood::Working,
    Mood::Happy,
    Mood::Sad,
    Mood::Blocked,
    Mood::Sleepy,
];
/// Genomes of the variety seed looked through for its robots.
const VARIETY_CANDIDATES: u64 = 200;
/// Gear kinds that roll.
const ROLLING: [&str; 3] = ["treads", "track", "wheels"];
/// Gear kinds that are legs, as `avatar_small_robots` counts them.
const LEGGED: [&str; 7] = [
    "biped", "piston", "spring", "quad", "hexa", "tripod", "hoses",
];
/// The plaza's robots: genomes 0 to 32 of its seed.
const PLAZA_ROBOTS: u64 = 33;
/// The plaza film's length in seconds.
const PLAZA_LENGTH: f64 = 25.0;
/// Seconds between one plaza robot's reactions.
const PLAZA_PERIOD: f64 = 7.0;

/// An event of a film, for its TSV.
struct Cue {
    /// Seconds from the film's start.
    time: f64,
    /// Which robot: seed and genome, or a plaza label.
    robot: String,
    event: String,
}

/// One stretch of a film: a stage played from its own time 0.
struct Shot<'a> {
    stage: Stage<'a>,
    /// Seconds.
    length: f64,
    /// The plaza placement whose labels each frame carries, if any.
    labels: Option<&'a Plaza>,
}

/// The ffmpeg on `PATH`.
fn find_ffmpeg() -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            "--film needs ffmpeg with libx264, and there is no ffmpeg on PATH; install it or add its folder to PATH".to_owned()
        })
}

/// A robot alone on a stage-film frame, from its stage camera.
fn alone<'a>(actor: Actor<'a>, gpu: &'a GpuRobot) -> Stage<'a> {
    let view = View::from(&actor.robot().camera);
    Stage {
        actors: vec![actor],
        gpus: vec![gpu],
        offsets: vec![[0.0; 3]],
        view,
        width: STAGE_EDGE,
        height: STAGE_EDGE,
    }
}

/// The stage film's script: 3 s of rest; each mood held 3 s in `MOODS`
/// order (neutral first, so the rest runs on), then back to neutral; each
/// reaction in `REACTIONS` order with 2 s of rest before it, and 2 s after
/// the last; then the working and sleepy status loops, 4 s each, and 1 s
/// more while the last loop fades out. Returns the actor, the film's
/// length and its cues.
fn stage_script<'r>(robot: &'r Robot, name: &str) -> (Actor<'r>, f64, Vec<Cue>) {
    let mut actor = Actor::new(robot, Mood::Neutral);
    let mut cues = Vec::new();
    let mut cue = |time: f64, event: String| {
        cues.push(Cue {
            time,
            robot: name.to_owned(),
            event,
        })
    };
    cue(0.0, "rest, neutral".to_owned());
    let mut t = OPENING_REST;
    for mood in MOODS {
        actor.set_mood(t, mood);
        cue(t, format!("mood {}", mood_name(mood)));
        t += MOOD_HOLD;
    }
    actor.set_mood(t, Mood::Neutral);
    cue(t, "mood neutral".to_owned());
    for reaction in REACTIONS {
        t += REACTION_REST;
        actor.play(t, reaction);
        cue(t, format!("reaction {}", reaction.name()));
        t += robot.motion.reaction(reaction).duration;
    }
    t += REACTION_REST;
    for status in STAGE_LOOPS {
        actor.set_loop(t, Some(status));
        cue(t, format!("loop {}", status.name()));
        t += LOOP_HOLD;
    }
    actor.set_loop(t, None);
    cue(t, "loop stops".to_owned());
    (actor, t + CLOSING_REST, cues)
}

/// Whether a robot carries stalk eyes.
fn stalk_eyed(robot: &Robot) -> bool {
    robot
        .textures
        .iter()
        .any(|t| matches!(t.binding.content, TextureContent::StalkEye { .. }))
}

/// A kind of robot the variety film wants: what it is called, and the test
/// a robot must pass.
type Want = (&'static str, fn(&Robot) -> bool);

/// Five genomes of `seed` for the variety film, each with why it was
/// picked. In turn: one that hovers, one on treads or wheels, one on legs
/// and one with stalk eyes, each the first not yet picked whose gear kind
/// no earlier pick has (or, failing that, the first not yet picked), and a
/// want no genome meets is skipped; then, up to five, the first genomes
/// whose gear kind no pick has yet.
fn variety_picks(seed: u64) -> Vec<(u64, String)> {
    let robots: Vec<Robot> = (0..VARIETY_CANDIDATES)
        .map(|i| build_with(&genome(seed, i), BuildOptions::untextured()))
        .collect();
    let wants: [Want; 4] = [
        ("hovers", |r| r.gear.hover),
        ("treads or wheels", |r| ROLLING.contains(&r.gear.kind)),
        ("legs", |r| LEGGED.contains(&r.gear.kind)),
        ("stalk eyes", stalk_eyed),
    ];
    let mut picks: Vec<(u64, String)> = Vec::new();
    let pick = |picks: &[(u64, String)], test: &dyn Fn(&Robot) -> bool| {
        let free = |i: &u64| !picks.iter().any(|(p, _)| p == i);
        let new_kind = |i: &u64| {
            let kind = robots[*i as usize].gear.kind;
            !picks
                .iter()
                .any(|(p, _)| robots[*p as usize].gear.kind == kind)
        };
        let fits = |i: &u64| free(i) && test(&robots[*i as usize]);
        (0..VARIETY_CANDIDATES)
            .filter(fits)
            .find(new_kind)
            .or_else(|| (0..VARIETY_CANDIDATES).find(fits))
    };
    for (why, test) in wants {
        if let Some(i) = pick(&picks, &test) {
            picks.push((i, why.to_owned()));
        }
    }
    while picks.len() < VARIETY_MOODS.len() {
        match pick(&picks, &|_| true) {
            Some(i) => picks.push((i, "another gear kind".to_owned())),
            None => break,
        }
    }
    picks
        .into_iter()
        .map(|(i, why)| {
            let r = &robots[i as usize];
            let mut text = format!("{} gear, picked for {why}", r.gear.kind);
            if r.gear.hover {
                text.push_str("; hovers");
            }
            if stalk_eyed(r) {
                text.push_str("; stalk eyes");
            }
            (i, text)
        })
        .collect()
}

/// One variety shot's script: rest, a change to `mood` at 0.8 s, hello
/// at 2.2 s, then ask 0.5 s after hello ends. Returns the actor, the
/// shot's length ([`VARIETY_SHOT`], or longer if ask would not end half a
/// second before it) and its cues, timed from the shot's start.
fn variety_script<'r>(robot: &'r Robot, mood: Mood, name: &str) -> (Actor<'r>, f64, Vec<Cue>) {
    let mut actor = Actor::new(robot, Mood::Neutral);
    let mood_at = 0.8;
    let hello = 2.2;
    let ask = hello + robot.motion.reaction(Reaction::Hello).duration + 0.5;
    actor.set_mood(mood_at, mood);
    actor.play(hello, Reaction::Hello);
    actor.play(ask, Reaction::Ask);
    let end = ask + robot.motion.reaction(Reaction::Ask).duration;
    let cue = |time: f64, event: String| Cue {
        time,
        robot: name.to_owned(),
        event,
    };
    let cues = vec![
        cue(0.0, "rest, neutral".to_owned()),
        cue(mood_at, format!("mood {}", mood_name(mood))),
        cue(hello, "reaction hello".to_owned()),
        cue(ask, "reaction ask".to_owned()),
    ];
    (actor, VARIETY_SHOT.max(end + 0.5), cues)
}

/// The status loop a mood carries, for the moods that have one.
fn mood_loop(mood: Mood) -> Option<StatusLoop> {
    match mood {
        Mood::Working => Some(StatusLoop::Working),
        Mood::Waiting => Some(StatusLoop::Waiting),
        Mood::Blocked => Some(StatusLoop::Blocked),
        Mood::Sleepy => Some(StatusLoop::Sleepy),
        _ => None,
    }
}

/// Plaza robot `i`'s actor and cues. It holds mood `i mod 7` of `MOODS`
/// throughout; when that mood has a status loop the loop runs from before
/// the film starts, already faded in, started `0.37 i` s earlier than
/// robot 0's so the loops are out of step. From `0.4 + 7 k / 33` s, where
/// `k = 13 i mod 33` scatters the starts over the plaza, it plays a
/// reaction every 7 s, starting at the `i`-th of `REACTIONS`, so some robot
/// is always reacting.
fn plaza_actor(robot: &Robot, i: usize) -> (Actor<'_>, Vec<Cue>) {
    let label = format!("{:02}", i + 1);
    let cue = |time: f64, event: String| Cue {
        time,
        robot: label.clone(),
        event,
    };
    let mood = MOODS[i % MOODS.len()];
    let mut actor = Actor::new(robot, mood);
    let mut cues = vec![cue(0.0, format!("mood {}", mood_name(mood)))];
    if let Some(status) = mood_loop(mood) {
        actor.set_loop(-LOOP_BLEND - 0.37 * i as f64, Some(status));
        cues.push(cue(0.0, format!("loop {}", status.name())));
    }
    let start =
        0.4 + PLAZA_PERIOD * ((13 * i) % PLAZA_ROBOTS as usize) as f64 / PLAZA_ROBOTS as f64;
    for (k, t) in every(start, PLAZA_PERIOD, PLAZA_LENGTH) {
        let reaction = REACTIONS[(i + k) % REACTIONS.len()];
        actor.play(t, reaction);
        cues.push(cue(t, format!("reaction {}", reaction.name())));
    }
    (actor, cues)
}

/// Encodes the numbered PNGs in `frames` to an H.264 MP4. The frames'
/// sRGB colours are converted with the BT.709 matrix and the stream is
/// tagged BT.709, so players decode the colours the frames hold rather
/// than guessing a matrix from the frame size.
fn encode(ffmpeg: &Path, frames: &Path, video: &Path) -> Result<(), String> {
    let status = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-framerate"])
        .arg(FPS.to_string())
        .arg("-i")
        .arg(frames.join("%05d.png"))
        .args([
            "-vf",
            "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "18",
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-movflags",
            "+faststart",
        ])
        .arg(video)
        .status()
        .map_err(|e| format!("run {}: {e}", ffmpeg.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} could not encode {} ({status})",
            ffmpeg.display(),
            video.display()
        ))
    }
}

/// Draws `shots` one after another, writes `name.mp4` and `name.tsv` in
/// `out`, and returns the film's length in seconds.
fn film(
    renderer: &Renderer,
    ffmpeg: &Path,
    out: &Path,
    name: &str,
    shots: &[Shot<'_>],
    cues: &[Cue],
) -> Result<f64, String> {
    let frames = out.join(format!("{name}.frames"));
    fs::create_dir_all(&frames).map_err(|e| format!("create {}: {e}", frames.display()))?;
    let drawn = draw_frames(renderer, &frames, name, shots);
    let video = out.join(format!("{name}.mp4"));
    let encoded = drawn.and_then(|n| encode(ffmpeg, &frames, &video).map(|()| n));
    // The frames go whether or not the encode worked: they are only a
    // step on the way to the video.
    let removed =
        fs::remove_dir_all(&frames).map_err(|e| format!("remove {}: {e}", frames.display()));
    let n = encoded?;
    removed?;
    let mut tsv = String::from("time_s\trobot\tevent\n");
    for c in cues {
        tsv.push_str(&format!("{:.3}\t{}\t{}\n", c.time, c.robot, c.event));
    }
    let path = out.join(format!("{name}.tsv"));
    fs::write(&path, tsv).map_err(|e| format!("write {}: {e}", path.display()))?;
    let length = n as f64 / f64::from(FPS);
    eprintln!(
        "review_sheets: wrote {} ({} frames, {length:.2} s)",
        video.display(),
        n
    );
    Ok(length)
}

/// Draws every frame of `shots` into `frames` as `00000.png`, `00001.png`,
/// …, and returns how many it drew.
fn draw_frames(
    renderer: &Renderer,
    frames: &Path,
    name: &str,
    shots: &[Shot<'_>],
) -> Result<usize, String> {
    let mut n = 0;
    for shot in shots {
        let count = (shot.length * f64::from(FPS)).round() as usize;
        for f in 0..count {
            let t = f as f64 / f64::from(FPS);
            let poses = shot.stage.poses(t);
            let items = shot.stage.items(&poses);
            let image = renderer
                .render(&shot.stage.scene(&items))
                .map_err(|e| format!("render {name} frame {n}: {e}"))?;
            let mut sheet = Sheet::of(&image);
            if let Some(placed) = shot.labels {
                for (i, (actor, pose)) in shot.stage.actors.iter().zip(&poses).enumerate() {
                    let top = frame::anchor(actor.robot(), pose, "headTop")
                        .expect("every robot has a headTop anchor");
                    plaza_label(&mut sheet, placed, i, top);
                }
            }
            sheet.write_png(&frames.join(format!("{n:05}.png")))?;
            n += 1;
        }
    }
    Ok(n)
}

/// Films each wanted film into `out`.
pub fn run(out: &Path, wanted: &dyn Fn(&str) -> bool) -> Result<(), String> {
    let ffmpeg = find_ffmpeg()?;
    fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let renderer = Renderer::new().map_err(|e| e.to_string())?;
    eprintln!(
        "review_sheets: {} on {:?}; encoding with {}",
        renderer.adapter().name,
        renderer.adapter().backend,
        ffmpeg.display()
    );

    if wanted("stage") {
        let b = Built::new(&renderer, STATED.0, STATED.1);
        let name = format!("{}/{}", b.seed, b.index);
        let (actor, length, cues) = stage_script(&b.robot, &name);
        let shots = [Shot {
            stage: alone(actor, &b.gpu),
            length,
            labels: None,
        }];
        let file = format!("stage-{}-{}", STATED.0, STATED.1);
        film(&renderer, &ffmpeg, out, &file, &shots, &cues)?;
    }

    if wanted("variety") {
        let seed = RANDOM_SEEDS[1];
        let picks = variety_picks(seed);
        let built: Vec<Built> = picks
            .iter()
            .map(|(i, _)| Built::new(&renderer, seed, *i))
            .collect();
        let mut shots = Vec::new();
        let mut cues = Vec::new();
        let mut start = 0.0;
        for ((b, (_, why)), mood) in built.iter().zip(&picks).zip(VARIETY_MOODS) {
            let name = format!("{}/{}", b.seed, b.index);
            eprintln!("review_sheets: variety {name}: {why}");
            let (actor, length, shot_cues) = variety_script(&b.robot, mood, &name);
            cues.push(Cue {
                time: start,
                robot: name.clone(),
                event: format!("on screen: {why}"),
            });
            cues.extend(shot_cues.into_iter().map(|c| Cue {
                time: start + c.time,
                ..c
            }));
            shots.push(Shot {
                stage: alone(actor, &b.gpu),
                length,
                labels: None,
            });
            start += length;
        }
        film(
            &renderer,
            &ffmpeg,
            out,
            &format!("stage-{seed}-variety"),
            &shots,
            &cues,
        )?;
    }

    if wanted("plaza") {
        let built: Vec<Built> = (0..PLAZA_ROBOTS)
            .map(|i| Built::new(&renderer, PLAZA_SEED, i))
            .collect();
        let (_, placed) = place_plaza(built.iter().map(|b| &b.robot));
        let mut actors = Vec::new();
        let mut cues = Vec::new();
        for (i, b) in built.iter().enumerate() {
            let (actor, robot_cues) = plaza_actor(&b.robot, i);
            actors.push(actor);
            cues.extend(robot_cues);
        }
        cues.sort_by(|a, b| a.time.total_cmp(&b.time));
        let (width, height) = PLAZA_SIZE;
        let shots = [Shot {
            stage: Stage {
                actors,
                gpus: built.iter().map(|b| &b.gpu).collect(),
                offsets: placed.offsets.clone(),
                view: placed.view,
                width,
                height,
            },
            length: PLAZA_LENGTH,
            labels: Some(&placed),
        }];
        film(
            &renderer,
            &ffmpeg,
            out,
            &format!("plaza-{PLAZA_SEED}"),
            &shots,
            &cues,
        )?;
    }
    Ok(())
}
