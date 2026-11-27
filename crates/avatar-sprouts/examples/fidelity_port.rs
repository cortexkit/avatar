//! Writes the port's side of a fidelity run: for every case of a manifest,
//! `<case>.svg` from `serialize_tree` and `<case>.choices.json` from
//! `decode`, mapped onto the reference's own choice keys.
//!
//! ```sh
//! cargo run --release -p cortexkit-avatar-sprouts --example fidelity_port -- \
//!     --manifest target/fidelity/sprouts/base.json --out target/fidelity/sprouts/port
//! ```
//!
//! See `scripts/fidelity/README.md` for the manifest format and the rest of a
//! fidelity run. The choice keys and their values follow
//! `scripts/fidelity/sprouts.js`.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::{env, fs, process, thread};

use avatar_sprouts::{decode, render_tree, Sprout, GENOME_LEN};
use avatar_svg::{serialize_tree, Mood};
use serde_json::{json, Value};

fn fail(message: &str) -> ! {
    eprintln!("fidelity_port: {message}");
    process::exit(2);
}

fn mood(name: &str) -> Mood {
    match name {
        "neutral" => Mood::Neutral,
        "working" => Mood::Working,
        "happy" => Mood::Happy,
        "waiting" => Mood::Waiting,
        "blocked" => Mood::Blocked,
        "sad" => Mood::Sad,
        "sleepy" => Mood::Sleepy,
        other => fail(&format!("unknown mood {other:?}")),
    }
}

fn genome(name: &str, hex: &str) -> Box<[u8; GENOME_LEN]> {
    if hex.len() != GENOME_LEN * 2 {
        fail(&format!(
            "genome {name} has {} hex digits, not {}",
            hex.len(),
            GENOME_LEN * 2
        ));
    }
    let mut bytes = Box::new([0_u8; GENOME_LEN]);
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .unwrap_or_else(|_| fail(&format!("genome {name} is not hex")));
    }
    bytes
}

/// The reference's choice keys for one decoded genome.
fn choices(s: &Sprout) -> BTreeMap<&'static str, Value> {
    let mut out = BTreeMap::new();
    out.insert("body", json!(s.body));
    out.insert("palette.recipe", json!(s.recipe));
    out.insert("palette.foliage", json!(s.foliage));
    out.insert("roots", json!(s.roots));
    out.insert("coat", json!(s.coat));
    out.insert("skin", json!(s.skin));
    out.insert("eyes.plan", json!(s.eye_plan));
    out.insert("eyes.style", json!(s.eye_style));
    out.insert("mouth", json!(s.mouth));
    out.insert(
        "body_rgb",
        json!([s.body_rgb.r, s.body_rgb.g, s.body_rgb.b]),
    );
    for (key, place) in [
        ("shoots.apex", "apex"),
        ("shoots.ear", "ear"),
        ("shoots.arm", "arm"),
    ] {
        out.insert(key, json!(s.grows(place)));
    }
    let b = s.bears;
    for (key, value) in [
        ("has.L", b.leaves),
        ("has.U", b.flowers),
        ("has.K", b.fruit),
        ("has.T", b.thorns),
        ("has.C", b.tendrils),
    ] {
        out.insert(key, json!(value));
    }
    if let Some(n) = s.lobes {
        out.insert("lobes", json!(n));
    }
    if let Some(form) = s.leaf_form {
        out.insert("leaf.form", json!(form));
    }
    if let Some(form) = s.flower_form {
        out.insert("flower.form", json!(form));
    }
    if let Some(form) = s.fruit_form {
        out.insert("fruit.form", json!(form));
    }
    out
}

fn main() {
    let mut manifest = None;
    let mut out = None;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .unwrap_or_else(|| fail(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            other => fail(&format!("unknown argument {other:?}")),
        }
    }
    let manifest = manifest.unwrap_or_else(|| fail("--manifest is required"));
    let out = out.unwrap_or_else(|| fail("--out is required"));
    let text = fs::read_to_string(&manifest)
        .unwrap_or_else(|e| fail(&format!("read {}: {e}", manifest.display())));
    let m: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| fail(&format!("parse manifest: {e}")));
    if m["type"] != "creature.sprouts" {
        fail(&format!(
            "the manifest is for {}, not creature.sprouts",
            m["type"]
        ));
    }
    let mut genomes: HashMap<String, Box<[u8; GENOME_LEN]>> = HashMap::new();
    for g in m["genomes"]
        .as_array()
        .unwrap_or_else(|| fail("the manifest has no genomes"))
    {
        let name = g["name"]
            .as_str()
            .unwrap_or_else(|| fail("a genome has no name"));
        let hex = g["hex"]
            .as_str()
            .unwrap_or_else(|| fail("a genome has no hex"));
        genomes.insert(name.to_owned(), genome(name, hex));
    }
    let mut cases: Vec<(String, String, Mood)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in m["cases"]
        .as_array()
        .unwrap_or_else(|| fail("the manifest has no cases"))
    {
        let id = c["id"].as_str().unwrap_or_else(|| fail("a case has no id"));
        let genome = c["genome"]
            .as_str()
            .unwrap_or_else(|| fail("a case has no genome"));
        if !genomes.contains_key(genome) {
            fail(&format!("case {id} names an unknown genome {genome}"));
        }
        if !seen.insert(id.to_owned()) {
            fail(&format!("case {id} is listed twice"));
        }
        let mood = mood(
            c["mood"]
                .as_str()
                .unwrap_or_else(|| fail("a case has no mood")),
        );
        cases.push((id.to_owned(), genome.to_owned(), mood));
    }
    fs::create_dir_all(&out).unwrap_or_else(|e| fail(&format!("create {}: {e}", out.display())));
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = cases.len().div_ceil(workers).max(1);
    thread::scope(|scope| {
        for part in cases.chunks(chunk) {
            let (genomes, out) = (&genomes, &out);
            scope.spawn(move || {
                let mut decoded: HashMap<&str, String> = HashMap::new();
                for (id, genome, mood) in part {
                    let bytes = &genomes[genome];
                    let svg = serialize_tree(&render_tree(bytes, *mood));
                    let choice = decoded.entry(genome.as_str()).or_insert_with(|| {
                        serde_json::to_string(&choices(&decode(bytes))).expect("choices serialize")
                            + "\n"
                    });
                    fs::write(out.join(format!("{id}.svg")), svg)
                        .unwrap_or_else(|e| fail(&format!("write {id}.svg: {e}")));
                    fs::write(out.join(format!("{id}.choices.json")), choice.as_bytes())
                        .unwrap_or_else(|e| fail(&format!("write {id}.choices.json: {e}")));
                }
            });
        }
    });
    println!(
        "fidelity_port: wrote {} cases from {} genomes into {}",
        cases.len(),
        genomes.len(),
        out.display()
    );
}
