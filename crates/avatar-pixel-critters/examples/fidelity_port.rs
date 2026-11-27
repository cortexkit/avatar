//! Draws every case of a fidelity manifest with this crate, for comparison
//! with the reference run by `scripts/fidelity/pixel-critters.js`.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-pixel-critters --example fidelity_port -- \
//!     --manifest BASE.json --out DIR [--size PX]
//! ```
//!
//! For each case it writes `<case>.svg` and `<case>.choices.json` into DIR.
//! The SVG is the tree as `placed_svg` writes it at `--size` (256 by
//! default): the `serialize_tree` output with its root element placing the
//! pixel grid as `avatar_raster::rasterize_tree` does, so a plain SVG
//! rasterizer draws the pixels apps get. The choices are the decoded genes
//! mapped onto the reference's own keys (see `scripts/fidelity/README.md`).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use avatar_pixel_critters::{decode, placed_svg, Creature, GENOME_LEN};
use avatar_svg::Mood;
use serde_json::{json, Map, Value};

struct Options {
    manifests: Vec<PathBuf>,
    out: PathBuf,
    size: u32,
}

fn parse_args() -> Result<Options, String> {
    let mut manifests = Vec::new();
    let mut out = None;
    let mut size = 256;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--manifest" => manifests.push(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--size" => {
                size = value
                    .parse()
                    .ok()
                    .filter(|s| (1..=2_048).contains(s))
                    .ok_or_else(|| format!("--size must be 1 to 2048 pixels, not {value:?}"))?
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if manifests.is_empty() {
        return Err("give at least one --manifest".to_owned());
    }
    Ok(Options {
        manifests,
        out: out.ok_or_else(|| "give --out DIR".to_owned())?,
        size,
    })
}

fn parse_mood(name: &str) -> Result<Mood, String> {
    Ok(match name {
        "neutral" => Mood::Neutral,
        "working" => Mood::Working,
        "happy" => Mood::Happy,
        "waiting" => Mood::Waiting,
        "blocked" => Mood::Blocked,
        "sad" => Mood::Sad,
        "sleepy" => Mood::Sleepy,
        other => return Err(format!("unknown mood {other:?}")),
    })
}

fn genome_of(hex: &str) -> Result<Box<[u8; GENOME_LEN]>, String> {
    if hex.len() != 2 * GENOME_LEN || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("a genome is 32,768 hex digits".to_owned());
    }
    let mut genome = Box::new([0u8; GENOME_LEN]);
    for (i, byte) in genome.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(genome)
}

/// The decoded creature on the reference's keys: the named genes of its
/// gene record, how the neutral sprite draws its eyes and mouth, the palette
/// recipe and the body colour. A gene the creature does not read is left out.
fn choices(creature: &Creature) -> Value {
    let g = &creature.genes;
    let drawn = creature.drawn_face();
    let mut out = Map::new();
    let mut put = |key: &str, value: Value| {
        out.insert(key.to_owned(), value);
    };
    put("body.plan", json!(g.body.plan.name()));
    put("body.top", json!(g.body.top.name()));
    put("body.bot", json!(g.body.bot.name()));
    put("top.kind", json!(g.top.kind.name()));
    if let Some(detail) = g.top.detail {
        put("top.detail", json!(detail.name()));
    }
    put("side.kind", json!(g.side.kind.name()));
    if let Some(wing) = g.side.wing {
        put("side.wing", json!(wing.name()));
    }
    put("legs.kind", json!(g.legs.kind.name()));
    put("tail.kind", json!(g.tail.kind.name()));
    put("coat.kind", json!(g.coat.kind.name()));
    put("pattern.belly", json!(g.pattern.belly.name()));
    put("pattern.marks", json!(g.pattern.marks.name()));
    put("face.plan", json!(g.face.plan.name()));
    put("face.style", json!(g.face.style.name()));
    put("face.lid", json!(g.face.lid.name()));
    put("face.mouth", json!(g.face.mouth.name()));
    put("face.muzzle", json!(g.face.muzzle.name()));
    put("face.cheeks", json!(g.face.cheeks.name()));
    put("face.brows", json!(g.face.brows.name()));
    put("face.quirk", json!(g.face.quirk.name()));
    put("drawn.eyes.plan", json!(drawn.eye_plan.name()));
    put("drawn.eyes.kind", json!(drawn.eye_kind.name()));
    put("drawn.mouth", json!(drawn.mouth.name()));
    put("palette.recipe", json!(creature.palette.recipe));
    let body = creature.palette.body;
    put("body_rgb", json!([body.r, body.g, body.b]));
    Value::Object(out)
}

/// One genome and the moods of its cases, as (case ID, mood).
struct Work {
    hex: String,
    cases: Vec<(String, Mood)>,
}

fn read_manifest(path: &PathBuf, work: &mut BTreeMap<String, Work>) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let manifest: Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let field = |v: &Value, key: &str| -> Result<String, String> {
        v.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{}: an entry has no {key:?}", path.display()))
    };
    let mut genomes = BTreeMap::new();
    for g in manifest["genomes"]
        .as_array()
        .ok_or("manifest has no genomes")?
    {
        genomes.insert(field(g, "name")?, field(g, "hex")?);
    }
    for case in manifest["cases"]
        .as_array()
        .ok_or("manifest has no cases")?
    {
        let (id, genome, mood) = (
            field(case, "id")?,
            field(case, "genome")?,
            field(case, "mood")?,
        );
        let hex = genomes
            .get(&genome)
            .ok_or_else(|| format!("case {id} names an unknown genome {genome}"))?;
        // Cases are grouped by genome, so each genome is decoded once.
        let key = format!("{}#{genome}", path.display());
        let entry = work.entry(key).or_insert_with(|| Work {
            hex: hex.clone(),
            cases: Vec::new(),
        });
        entry.cases.push((id, parse_mood(&mood)?));
    }
    Ok(())
}

fn run(options: &Options) -> Result<usize, String> {
    let mut work = BTreeMap::new();
    for path in &options.manifests {
        read_manifest(path, &mut work)?;
    }
    fs::create_dir_all(&options.out)
        .map_err(|e| format!("create {}: {e}", options.out.display()))?;
    let jobs: Vec<&Work> = work.values().collect();
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let written = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(2, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(i) else {
                    break;
                };
                let result = (|| -> Result<(), String> {
                    let genome = genome_of(&job.hex)?;
                    let creature = decode(&genome);
                    let picked =
                        serde_json::to_vec(&choices(&creature)).map_err(|e| e.to_string())?;
                    for (id, mood) in &job.cases {
                        let tree = creature.render_tree(*mood);
                        let dir = &options.out;
                        fs::write(
                            dir.join(format!("{id}.svg")),
                            placed_svg(&tree, options.size),
                        )
                        .map_err(|e| format!("write {id}.svg: {e}"))?;
                        fs::write(dir.join(format!("{id}.choices.json")), &picked)
                            .map_err(|e| format!("write {id}.choices.json: {e}"))?;
                        written.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    failures.lock().unwrap().push(error);
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    if let Some(first) = failures.first() {
        return Err(first.clone());
    }
    Ok(written.into_inner())
}

fn main() -> ExitCode {
    let options = match parse_args() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("fidelity_port: {error}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(count) => {
            println!(
                "fidelity_port: wrote {count} cases at {} px into {}",
                options.size,
                options.out.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("fidelity_port: {error}");
            ExitCode::from(2)
        }
    }
}
