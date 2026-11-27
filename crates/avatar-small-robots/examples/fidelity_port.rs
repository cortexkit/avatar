//! The port side of the fidelity harness for `creature.small-robots`.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-small-robots --example fidelity_port -- \
//!     --manifest FILE --out DIR [--size PX]
//! ```
//!
//! For every case in the manifest (see `scripts/fidelity/README.md`) this
//! writes `<case>.svg`, the generic SVG of the case's render tree drawn at
//! the detail of a `--size` pixel drawing (256 by default), and
//! `<case>.choices.json`, the robot's discrete traits under the keys the
//! reference shim writes: each field the reference's summary reports, and
//! the palette recipe, plus `body_rgb`. A key the robot does not have (a
//! pod's chassis and neck, a lens face's screen) is left out, as the shim
//! leaves it out.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use avatar_small_robots::{decode, Creature, Mood, Tech, GENOME_LEN, MOODS};
use avatar_svg::serialize_tree;
use serde_json::{json, Map, Value};

fn usage() -> ExitCode {
    eprintln!("usage: fidelity_port --manifest FILE [--manifest FILE ...] --out DIR [--size PX]");
    ExitCode::from(2)
}

/// The choices file of one robot, keyed as the reference shim keys it.
fn choices(c: &Creature) -> Map<String, Value> {
    let mut out = Map::new();
    let mut put = |k: &str, v: Value| {
        out.insert(k.to_owned(), v);
    };
    put("plan", json!(c.plan));
    put("hull", json!(c.hull));
    put("face.tech", json!(c.face.name()));
    put("face.mouth", json!(c.mouth.name()));
    put("top", json!(c.top));
    put("side", json!(c.side));
    put("arms.style", json!(c.arms));
    put("arms.hand", json!(c.hand));
    put("arms.handL", json!(c.hand_left));
    put("gear", json!(c.gear));
    put("chest", json!(c.chest));
    put("back", json!(c.back));
    put("metal", json!(c.metal));
    put("palette.recipe", json!(c.palette_recipe));
    put("body_rgb", json!([c.body.r, c.body.g, c.body.b]));
    if let Some(chassis) = c.chassis {
        put("chassis", json!(chassis));
    }
    if let Some(neck) = c.neck {
        put("neck", json!(neck));
    }
    if let (Tech::Screen, Some((lcd, style))) = (c.face, c.screen) {
        put("face.lcd", json!(lcd));
        put("face.eyeStyle", json!(style));
    }
    out
}

fn mood_named(name: &str) -> Option<Mood> {
    MOODS
        .into_iter()
        .find(|m| avatar_small_robots::mood_name(*m) == name)
}

fn genome_of(hex: &str) -> Option<Box<[u8; GENOME_LEN]>> {
    if hex.len() != GENOME_LEN * 2 {
        return None;
    }
    let mut out = Box::new([0_u8; GENOME_LEN]);
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

fn run(manifests: &[PathBuf], out_dir: &PathBuf, size: u32) -> Result<usize, String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let mut written = 0;
    for path in manifests {
        let text =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let manifest: Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut genomes = BTreeMap::new();
        for g in manifest["genomes"]
            .as_array()
            .ok_or("the manifest lists no genomes")?
        {
            let name = g["name"].as_str().ok_or("a genome has no name")?;
            let hex = g["hex"].as_str().ok_or("a genome has no hex")?;
            let bytes = genome_of(hex)
                .ok_or_else(|| format!("genome {name} is not {GENOME_LEN} hex bytes"))?;
            genomes.insert(name.to_owned(), bytes);
        }
        let mut robots = BTreeMap::new();
        for case in manifest["cases"]
            .as_array()
            .ok_or("the manifest lists no cases")?
        {
            let id = case["id"].as_str().ok_or("a case has no id")?;
            let genome = case["genome"].as_str().ok_or("a case names no genome")?;
            let mood_text = case["mood"].as_str().ok_or("a case names no mood")?;
            let mood = mood_named(mood_text)
                .ok_or_else(|| format!("case {id} has unknown mood {mood_text}"))?;
            let bytes = genomes.get(genome).ok_or_else(|| {
                format!("case {id} names genome {genome}, which the manifest does not list")
            })?;
            let robot = robots
                .entry(genome.to_owned())
                .or_insert_with(|| decode(bytes));
            let tree = robot.render_tree(mood, Some(size));
            fs::write(out_dir.join(format!("{id}.svg")), serialize_tree(&tree))
                .map_err(|e| format!("cannot write case {id}: {e}"))?;
            let choices =
                serde_json::to_string(&Value::Object(choices(robot))).map_err(|e| e.to_string())?;
            fs::write(out_dir.join(format!("{id}.choices.json")), choices)
                .map_err(|e| format!("cannot write case {id}: {e}"))?;
            written += 1;
        }
    }
    Ok(written)
}

fn main() -> ExitCode {
    let mut manifests = Vec::new();
    let mut out = None;
    let mut size = avatar_small_robots::FULL_DETAIL_PX;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--manifest" => manifests.push(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--size" => match value.parse() {
                Ok(px) if (1..=2048).contains(&px) => size = px,
                _ => return usage(),
            },
            _ => return usage(),
        }
    }
    let (Some(out), false) = (out, manifests.is_empty()) else {
        return usage();
    };
    match run(&manifests, &out, size) {
        Ok(n) => {
            println!("wrote {n} cases at {size} px to {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fidelity_port: {e}");
            ExitCode::from(2)
        }
    }
}
