//! Draws the review sheets of the 3D small robots to PNG, each with a TSV
//! of the same name that lists every cell's seed, genome, mood, view,
//! reaction and time.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-3d-wgpu --example review_sheets -- \
//!     --out DIR [--only random,moods,reactions,views,plaza]
//! ```
//!
//! The sheets:
//!
//! - `random`: sixteen robots, genomes 0 to 15 of [`RANDOM_SEED`], neutral,
//!   from the stage's three-quarter view;
//! - `moods`: the stated robot, genome [`STATED`] of its seed, in the seven
//!   moods in `MOODS` order;
//! - `reactions`: a film strip per reaction, one row each in `REACTIONS`
//!   order, the stated robot neutral, with frames at a quarter, a half and
//!   all of the way to the reaction's peak, then a quarter, a half and
//!   three quarters of the way from the peak to the end;
//! - `views`: the stated robot neutral from the three-quarter, front, side
//!   and back views, all at the stage camera's distance;
//! - `plaza`: 33 robots, genomes 0 to 32 of [`PLAZA_SEED`], neutral, in
//!   rows of six from the front row back, labelled "01" to "33" at their
//!   `headTop` anchors.
//!
//! Genome `g` of seed `s` is derived as the `random_sheet` example derives
//! it. Every frame is posed with the idle layer off, so a still shows the
//! posture and the face exactly, never a passing blink or glance.
//! Single-robot frames are 288 px square.

mod sheet;

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use avatar_3d::motion::math::{cos, sin};
use avatar_3d::motion::{Actor, Pose, REACTIONS};
use avatar_3d::robot::{build, Robot};
use avatar_3d_wgpu::view::{framing, View};
use avatar_3d_wgpu::{frame, GpuRobot, Image, Item, Renderer, Scene};
use avatar_small_robots::{mood_name, Mood, Rgb, GENOME_LEN, MOODS};
use sheet::Sheet;

/// The seed of the sixteen random robots.
const RANDOM_SEED: u64 = 501;
/// The stated robot of the moods, reactions and views sheets: a seed and
/// a genome index.
const STATED: (u64, u64) = (501, 14);
/// The seed of the plaza's 33 robots.
const PLAZA_SEED: u64 = 503;
/// The edge of a single-robot frame.
const FRAME: u32 = 288;
/// Behind every frame: a light grey, so white parts still show.
const BACKGROUND: Rgb = Rgb {
    r: 0xf2,
    g: 0xf3,
    b: 0xf5,
};
/// The plaza's frame.
const PLAZA_SIZE: (u32, u32) = (1600, 1000);
/// The plaza's view, azimuth and elevation in degrees: straight on, so
/// rows stay level across the frame, and from high enough that each row
/// shows over the one in front.
const PLAZA_VIEW: (f64, f64) = (0.0, 35.0);
/// Robots in each plaza row.
const PLAZA_COLUMNS: usize = 6;

/// Genome `index` of the sheet drawn with `seed`, as the `random_sheet`
/// example derives it: the BLAKE3 extendable output of the text `avatar
/// random_sheet genome`, keyed by the seed and the index as little-endian
/// u64 values followed by 16 zero bytes.
fn genome(seed: u64, index: u64) -> Box<[u8; GENOME_LEN]> {
    let mut key = [0_u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&index.to_le_bytes());
    let mut hasher = blake3::Hasher::new_keyed(&key);
    hasher.update(b"avatar random_sheet genome");
    let mut genome = Box::new([0_u8; GENOME_LEN]);
    hasher.finalize_xof().fill(&mut genome[..]);
    genome
}

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

fn random_sheet(renderer: &Renderer) -> (Sheet, Vec<Row>) {
    let mut sheet = Sheet::grid(4, 4, FRAME, FRAME);
    let mut rows = Vec::new();
    for i in 0..16_u32 {
        let b = Built::new(renderer, RANDOM_SEED, u64::from(i));
        let pose = Actor::new(&b.robot, Mood::Neutral)
            .without_idle()
            .evaluate(0.0);
        let image = b.render(renderer, &pose, View::from(&b.robot.camera));
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
        let image = b.render(renderer, &pose, View::from(&b.robot.camera));
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
            let image = b.render(renderer, &pose, View::from(&b.robot.camera));
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
    let stage = View::from(&b.robot.camera);
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

/// The rest bounds of a robot's parts.
fn bounds(robot: &Robot) -> ([f64; 3], [f64; 3]) {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in robot.parts.iter().flat_map(|p| p.mesh.positions.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

fn plaza_sheet(renderer: &Renderer) -> (Sheet, Vec<Row>) {
    let built: Vec<Built> = (0..33)
        .map(|i| Built::new(renderer, PLAZA_SEED, i))
        .collect();
    let poses: Vec<Pose> = built
        .iter()
        .map(|b| {
            Actor::new(&b.robot, Mood::Neutral)
                .without_idle()
                .evaluate(0.0)
        })
        .collect();
    // Every robot gets the same cell: as wide as the widest robot, and deep
    // enough that a robot of the median height clears the row in front of
    // it in the view, and never less than the deepest robot plus a gap. A
    // short last row is centred.
    let boxes: Vec<_> = built.iter().map(|b| bounds(&b.robot)).collect();
    let pitch_x = boxes
        .iter()
        .map(|(lo, hi)| hi[0] - lo[0])
        .fold(0.0, f64::max)
        + 0.25;
    let mut heights: Vec<f64> = boxes.iter().map(|(_, hi)| hi[1]).collect();
    heights.sort_by(f64::total_cmp);
    let median = heights[heights.len() / 2];
    let el = PLAZA_VIEW.1 * std::f64::consts::PI / 180.0;
    let pitch_z = (boxes
        .iter()
        .map(|(lo, hi)| hi[2] - lo[2])
        .fold(0.0, f64::max)
        + 0.6)
        .max(0.9 * median * cos(el) / sin(el));
    let offsets: Vec<[f64; 3]> = (0..built.len())
        .map(|i| {
            let row = i / PLAZA_COLUMNS;
            let in_row = PLAZA_COLUMNS.min(built.len() - row * PLAZA_COLUMNS);
            let column = (i % PLAZA_COLUMNS) as f64 - 0.5 * (in_row as f64 - 1.0);
            let (lo, hi) = boxes[i];
            [
                column * pitch_x - 0.5 * (lo[0] + hi[0]),
                0.0,
                -(row as f64) * pitch_z - 0.5 * (lo[2] + hi[2]),
            ]
        })
        .collect();
    let anchors: Vec<[f64; 3]> = built
        .iter()
        .zip(&poses)
        .zip(&offsets)
        .map(|((b, pose), o)| {
            let a = frame::anchor(&b.robot, pose, "headTop").expect("every robot has headTop");
            [a[0] + o[0], a[1] + o[1], a[2] + o[2]]
        })
        .collect();
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for (i, (blo, bhi)) in boxes.iter().enumerate() {
        for k in 0..3 {
            lo[k] = lo[k].min(blo[k] + offsets[i][k]).min(anchors[i][k]);
            hi[k] = hi[k].max(bhi[k] + offsets[i][k]).max(anchors[i][k]);
        }
    }
    let (width, height) = PLAZA_SIZE;
    let view = framing(
        lo,
        hi,
        PLAZA_VIEW.0,
        PLAZA_VIEW.1,
        20.0,
        f64::from(width) / f64::from(height),
        0.9,
    );
    let items: Vec<Item> = built
        .iter()
        .zip(&poses)
        .zip(&offsets)
        .map(|((b, pose), o)| Item {
            robot: &b.robot,
            gpu: &b.gpu,
            pose,
            offset: *o,
        })
        .collect();
    let image = renderer
        .render(&Scene {
            view,
            width,
            height,
            background: BACKGROUND,
            items: &items,
        })
        .unwrap_or_else(|e| panic!("render the plaza: {e}"));
    let mut sheet = Sheet::of(&image);
    let mut rows = Vec::new();
    for (i, b) in built.iter().enumerate() {
        let text = format!("{:02}", i + 1);
        let [x, y] = view
            .to_screen(anchors[i], width, height)
            .expect("every anchor is in front of the camera");
        let (w, h) = Sheet::label_size(&text, 3);
        sheet.label(
            &text,
            x.round() as i64 - i64::from(w / 2),
            y.round() as i64 - i64::from(h),
            3,
        );
        rows.push(Row {
            cell: text,
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
    let mut only: Option<Vec<String>> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--only" => {
                only = args
                    .next()
                    .map(|s| s.split(',').map(str::to_owned).collect())
            }
            other => {
                eprintln!("review_sheets: unknown argument {other:?}");
                return ExitCode::FAILURE;
            }
        }
    }
    let Some(out) = out else {
        eprintln!("usage: review_sheets --out DIR [--only random,moods,reactions,views,plaza]");
        return ExitCode::FAILURE;
    };
    if let Err(e) = fs::create_dir_all(&out) {
        eprintln!("review_sheets: create {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    let wanted = |name: &str| only.as_ref().is_none_or(|o| o.iter().any(|s| s == name));
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
        sheets.push(("random", random_sheet(&renderer)));
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
