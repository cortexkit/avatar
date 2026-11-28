//! Draws the review sheets of the 3D small robots to PNG, each with a TSV
//! of the same name that lists every cell's seed, genome, mood, view,
//! reaction and time.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-3d-wgpu --example review_sheets -- \
//!     --out DIR [--only random,random-509,moods,reactions,views,plaza]
//! cargo run --release -p cortexkit-avatar-3d-wgpu --example review_sheets -- --numbers
//! cargo run --release -p cortexkit-avatar-3d-wgpu --example review_sheets -- \
//!     --film DIR [--only stage,variety,plaza]
//! ```
//!
//! With `--numbers` it draws no sheet and prints the first numbers of the
//! stage and the plaza instead: frame times, build and upload times and
//! memory (see the `numbers` module). With `--film` it draws no sheet and
//! films the robots moving instead, to MP4 videos encoded with the
//! `ffmpeg` found on `PATH` (see the `film` module).
//!
//! The sheets:
//!
//! - `random` and `random-509`: sixteen robots each, genomes 0 to 15 of
//!   the two [`RANDOM_SEEDS`], neutral,
//!   from the stage's three-quarter view;
//! - `moods`: the stated robot, genome [`STATED`] of its seed, in the seven
//!   moods in `MOODS` order;
//! - `reactions`: a film strip per reaction, one row each in `REACTIONS`
//!   order, the stated robot neutral, with frames at a quarter, a half and
//!   all of the way to the reaction's peak, then a quarter, a half and
//!   three quarters of the way from the peak to the end;
//! - `views`: the stated robot neutral from the three-quarter, front, side
//!   and back views, all at the stage camera's distance;
//! - `plaza`: 33 robots, genomes 0 to 32 of [`PLAZA_SEED`], neutral,
//!   placed by `avatar_3d_wgpu::plaza::layout` in staggered rows so no robot
//!   covers another's face and no label touches another label or robot,
//!   labelled "01" to "33" at their `headTop` anchors.
//!
//! Genome `g` of seed `s` is derived as the `random_sheet` example derives
//! it. Every frame is posed with the idle layer off, so a still shows the
//! posture and the face exactly, never a passing blink or glance.
//! Single-robot frames are 288 px square.

mod film;
mod numbers;
#[path = "../shared/robots.rs"]
mod robots;
mod sheet;

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use avatar_3d::motion::{Actor, Pose, REACTIONS};
use avatar_3d::robot::{build, Robot};
use avatar_3d_wgpu::plaza::Plaza;
use avatar_3d_wgpu::view::View;
use avatar_3d_wgpu::{GpuRobot, Image, Item, Renderer, Scene};
use avatar_small_robots::{mood_name, Mood, MOODS};
use robots::{genome, place_plaza, BACKGROUND, PLAZA_ROBOTS, PLAZA_SEED, PLAZA_SIZE, STATED};
use sheet::Sheet;

/// The seeds of the sixteen random robots: the main sheet and a second
/// one, so a fix is seen on robots it was not tuned on.
const RANDOM_SEEDS: [u64; 2] = [501, 509];
/// The edge of a single-robot frame.
const FRAME: u32 = 288;

/// A robot built and uploaded.
struct Built {
    seed: u64,
    index: u64,
    robot: Robot,
    gpu: GpuRobot,
}

impl Built {
    fn new(renderer: &Renderer, seed: u64, index: u64) -> Self {
        let robot = build(&genome(seed, index));
        let gpu = renderer.upload(&robot);
        Self {
            seed,
            index,
            robot,
            gpu,
        }
    }

    fn render(&self, renderer: &Renderer, pose: &Pose, view: View) -> Image {
        renderer
            .render(&Scene {
                view,
                width: FRAME,
                height: FRAME,
                background: BACKGROUND,
                items: &[Item {
                    robot: &self.robot,
                    gpu: &self.gpu,
                    pose,
                    offset: [0.0; 3],
                }],
            })
            .unwrap_or_else(|e| panic!("render seed {} genome {}: {e}", self.seed, self.index))
    }
}

/// One row of a sheet's TSV.
struct Row {
    cell: String,
    seed: u64,
    index: u64,
    mood: Mood,
    view: &'static str,
    reaction: &'static str,
    time: f64,
}

const TSV_HEADER: &str = "cell\tseed\tgenome\tmood\tview\treaction\ttime_s\tidle";

fn tsv(rows: &[Row]) -> String {
    let mut text = format!("{TSV_HEADER}\n");
    for r in rows {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\toff\n",
            r.cell,
            r.seed,
            r.index,
            mood_name(r.mood),
            r.view,
            r.reaction,
            r.time
        ));
    }
    text
}

/// The stage views: name, azimuth and elevation in degrees.
const VIEWS: [(&str, f64, f64); 4] = [
    ("three-quarter", 11.0, 12.0),
    ("front", 0.0, 4.0),
    ("side", 90.0, 8.0),
    ("back", 180.0, 12.0),
];

fn label(sheet: &mut Sheet, cell: usize, column: u32, row: u32) {
    let (x, y) = Sheet::cell_origin(column, row, FRAME, FRAME);
    sheet.label(&format!("{cell:02}"), i64::from(x) + 2, i64::from(y) + 2, 2);
}

fn random_sheet(renderer: &Renderer, seed: u64) -> (Sheet, Vec<Row>) {
    let mut sheet = Sheet::grid(4, 4, FRAME, FRAME);
    let mut rows = Vec::new();
    for i in 0..16_u32 {
        let b = Built::new(renderer, seed, u64::from(i));
        let pose = Actor::new(&b.robot, Mood::Neutral)
            .without_idle()
            .evaluate(0.0);
        let image = b.render(renderer, &pose, View::from(&b.robot.camera()));
        let (x, y) = Sheet::cell_origin(i % 4, i / 4, FRAME, FRAME);
        sheet.blit(&image, x, y);
        label(&mut sheet, i as usize, i % 4, i / 4);
        rows.push(Row {
            cell: format!("{i:02}"),
            seed: b.seed,
            index: b.index,
            mood: Mood::Neutral,
            view: "three-quarter",
            reaction: "-",
            time: 0.0,
        });
    }
    (sheet, rows)
}

fn moods_sheet(renderer: &Renderer, b: &Built) -> (Sheet, Vec<Row>) {
    let mut sheet = Sheet::grid(MOODS.len() as u32, 1, FRAME, FRAME);
    let mut rows = Vec::new();
    for (i, mood) in MOODS.into_iter().enumerate() {
        let pose = Actor::new(&b.robot, mood).without_idle().evaluate(0.0);
        let image = b.render(renderer, &pose, View::from(&b.robot.camera()));
        let (x, y) = Sheet::cell_origin(i as u32, 0, FRAME, FRAME);
        sheet.blit(&image, x, y);
        label(&mut sheet, i, i as u32, 0);
        rows.push(Row {
            cell: format!("{i:02}"),
            seed: b.seed,
            index: b.index,
            mood,
            view: "three-quarter",
            reaction: "-",
            time: 0.0,
        });
    }
    (sheet, rows)
}

/// A reaction's film-strip times: a quarter, a half and all of the way to
/// the peak, then a quarter, a half and three quarters of the way on to
/// the end.
fn strip_times(peak: f64, end: f64) -> [f64; 6] {
    [
        0.25 * peak,
        0.5 * peak,
        peak,
        peak + 0.25 * (end - peak),
        peak + 0.5 * (end - peak),
        peak + 0.75 * (end - peak),
    ]
}

fn reactions_sheet(renderer: &Renderer, b: &Built) -> (Sheet, Vec<Row>) {
    let mut sheet = Sheet::grid(6, REACTIONS.len() as u32, FRAME, FRAME);
    let mut rows = Vec::new();
    for (r, reaction) in REACTIONS.into_iter().enumerate() {
        let clip = b.robot.motion.reaction(reaction);
        let peak = clip.peak.unwrap_or(0.5 * clip.duration);
        let mut actor = Actor::new(&b.robot, Mood::Neutral).without_idle();
        actor.play(0.0, reaction);
        for (f, t) in strip_times(peak, clip.duration).into_iter().enumerate() {
            let pose = actor.evaluate(t);
            let image = b.render(renderer, &pose, View::from(&b.robot.camera()));
            let (x, y) = Sheet::cell_origin(f as u32, r as u32, FRAME, FRAME);
            sheet.blit(&image, x, y);
            let cell = r * 6 + f;
            label(&mut sheet, cell, f as u32, r as u32);
            rows.push(Row {
                cell: format!("{cell:02}"),
                seed: b.seed,
                index: b.index,
                mood: Mood::Neutral,
                view: "three-quarter",
                reaction: reaction.name(),
                time: t,
            });
        }
    }
    (sheet, rows)
}

fn views_sheet(renderer: &Renderer, b: &Built) -> (Sheet, Vec<Row>) {
    let mut sheet = Sheet::grid(VIEWS.len() as u32, 1, FRAME, FRAME);
    let mut rows = Vec::new();
    let pose = Actor::new(&b.robot, Mood::Neutral)
        .without_idle()
        .evaluate(0.0);
    let stage = View::from(&b.robot.camera());
    for (i, (name, azimuth, elevation)) in VIEWS.into_iter().enumerate() {
        let image = b.render(renderer, &pose, stage.orbit(azimuth, elevation));
        let (x, y) = Sheet::cell_origin(i as u32, 0, FRAME, FRAME);
        sheet.blit(&image, x, y);
        label(&mut sheet, i, i as u32, 0);
        rows.push(Row {
            cell: format!("{i:02}"),
            seed: b.seed,
            index: b.index,
            mood: Mood::Neutral,
            view: name,
            reaction: "-",
            time: 0.0,
        });
    }
    (sheet, rows)
}

/// Labels plaza robot `i` ("01" for the first) centred above `top`, a
/// point in the robot's model space, at the plaza's label size.
fn plaza_label(sheet: &mut Sheet, placed: &Plaza, i: usize, top: [f64; 3]) {
    let text = format!("{:02}", i + 1);
    let o = placed.offsets[i];
    let [x, y] = placed
        .view
        .to_screen(
            [top[0] + o[0], top[1] + o[1], top[2] + o[2]],
            placed.width,
            placed.height,
        )
        .expect("every anchor is in front of the camera");
    let scale = placed.label_scale;
    let (w, h) = Sheet::label_size(&text, scale);
    sheet.label(
        &text,
        x.round() as i64 - i64::from(w / 2),
        y.round() as i64 - i64::from(h),
        scale,
    );
}

fn plaza_sheet(renderer: &Renderer) -> (Sheet, Vec<Row>) {
    let built: Vec<Built> = (0..PLAZA_ROBOTS)
        .map(|i| Built::new(renderer, PLAZA_SEED, i))
        .collect();
    let (prints, placed) = place_plaza(built.iter().map(|b| &b.robot));
    let poses: Vec<Pose> = built
        .iter()
        .map(|b| {
            Actor::new(&b.robot, Mood::Neutral)
                .without_idle()
                .evaluate(0.0)
        })
        .collect();
    let (width, height) = PLAZA_SIZE;
    let items: Vec<Item> = built
        .iter()
        .zip(&poses)
        .zip(&placed.offsets)
        .map(|((b, pose), o)| Item {
            robot: &b.robot,
            gpu: &b.gpu,
            pose,
            offset: *o,
        })
        .collect();
    let image = renderer
        .render(&Scene {
            view: placed.view,
            width,
            height,
            background: BACKGROUND,
            items: &items,
        })
        .unwrap_or_else(|e| panic!("render the plaza: {e}"));
    let mut sheet = Sheet::of(&image);
    let mut rows = Vec::new();
    for (i, b) in built.iter().enumerate() {
        plaza_label(&mut sheet, &placed, i, prints[i].head_top);
        rows.push(Row {
            cell: format!("{:02}", i + 1),
            seed: b.seed,
            index: b.index,
            mood: Mood::Neutral,
            view: "plaza",
            reaction: "-",
            time: 0.0,
        });
    }
    (sheet, rows)
}

fn main() -> ExitCode {
    let mut out: Option<PathBuf> = None;
    let mut film: Option<PathBuf> = None;
    let mut only: Option<Vec<String>> = None;
    let mut numbers = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--film" => film = args.next().map(PathBuf::from),
            "--only" => {
                only = args
                    .next()
                    .map(|s| s.split(',').map(str::to_owned).collect())
            }
            "--numbers" => numbers = true,
            other => {
                eprintln!("review_sheets: unknown argument {other:?}");
                return ExitCode::FAILURE;
            }
        }
    }
    if numbers {
        let result = Renderer::new().and_then(|renderer| numbers::run(&renderer));
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("review_sheets: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let wanted = |name: &str| only.as_ref().is_none_or(|o| o.iter().any(|s| s == name));
    if let Some(dir) = film {
        return match film::run(&dir, &wanted) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("review_sheets: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let Some(out) = out else {
        eprintln!(
            "usage: review_sheets --out DIR [--only random,random-509,moods,reactions,views,plaza]\n       review_sheets --numbers\n       review_sheets --film DIR [--only stage,variety,plaza]"
        );
        return ExitCode::FAILURE;
    };
    if let Err(e) = fs::create_dir_all(&out) {
        eprintln!("review_sheets: create {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    let renderer = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("review_sheets: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "review_sheets: {} on {:?}",
        renderer.adapter().name,
        renderer.adapter().backend
    );
    let stated = (wanted("moods") || wanted("reactions") || wanted("views"))
        .then(|| Built::new(&renderer, STATED.0, STATED.1));
    let mut sheets: Vec<(&str, (Sheet, Vec<Row>))> = Vec::new();
    if wanted("random") {
        sheets.push(("random", random_sheet(&renderer, RANDOM_SEEDS[0])));
    }
    if wanted("random-509") {
        sheets.push(("random-509", random_sheet(&renderer, RANDOM_SEEDS[1])));
    }
    if let Some(b) = &stated {
        if wanted("moods") {
            sheets.push(("moods", moods_sheet(&renderer, b)));
        }
        if wanted("reactions") {
            sheets.push(("reactions", reactions_sheet(&renderer, b)));
        }
        if wanted("views") {
            sheets.push(("views", views_sheet(&renderer, b)));
        }
    }
    if wanted("plaza") {
        sheets.push(("plaza", plaza_sheet(&renderer)));
    }
    for (name, (sheet, rows)) in &sheets {
        let png = out.join(format!("{name}.png"));
        if let Err(e) = sheet.write_png(&png) {
            eprintln!("review_sheets: {e}");
            return ExitCode::FAILURE;
        }
        let path = out.join(format!("{name}.tsv"));
        if let Err(e) = fs::write(&path, tsv(rows)) {
            eprintln!("review_sheets: write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        eprintln!(
            "review_sheets: wrote {} ({}x{})",
            png.display(),
            sheet.width,
            sheet.height
        );
    }
    ExitCode::SUCCESS
}
