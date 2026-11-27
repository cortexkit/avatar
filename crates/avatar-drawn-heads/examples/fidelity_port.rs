//! The port side of the fidelity harness for `creature.drawn-heads`.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-drawn-heads --example fidelity_port -- \
//!     --manifest FILE --out DIR [--size PX]
//! ```
//!
//! For every case of the manifest, writes `<case>.svg` (the render tree
//! through `serialize_tree`) and `<case>.choices.json` (the decoded head's
//! discrete choices, under the keys the reference shim writes). `--size`
//! draws at the detail of that many pixels, as the shim's `--size` asks the
//! reference to; the default is the 256 px profile. `scripts/fidelity/`
//! describes the manifest and the files.

use std::{fs, path::PathBuf, process};

use avatar_drawn_heads::{decode, Creature, HairStyle, GENOME_LEN};
use avatar_svg::{serialize_tree, Mood};
use serde_json::{json, Map, Value};

fn fail(message: &str) -> ! {
    eprintln!("fidelity_port: {message}");
    process::exit(2)
}

fn mood_of(name: &str) -> Mood {
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

fn genome_of(hex: &str) -> [u8; GENOME_LEN] {
    if hex.len() != 2 * GENOME_LEN {
        fail("a genome is not 32,768 hex digits");
    }
    let mut out = [0u8; GENOME_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .unwrap_or_else(|_| fail("a genome holds a character that is not hex"));
    }
    out
}

/// The head's discrete choices under the reference's keys: the kind of head,
/// hair, hair colour, ears, eyes, brows, nose, mouth, facial hair and
/// moustache, whether there is a neck, the age marks, the palette recipe and
/// the body colour.
fn choices(c: &Creature) -> Value {
    let k = &c.marks;
    let body = c.palette.body;
    let mut out = Map::new();
    let mut put = |key: &str, value: Value| {
        out.insert(key.to_owned(), value);
    };
    put("head", json!(c.head.kind));
    put("hair", json!(c.hair.style.name()));
    put("ears", json!(c.ears.kind));
    put("eyes", json!(c.eyes.kind));
    put("brows", json!(c.brows.kind));
    put("nose", json!(c.nose.kind));
    put("mouth", json!(c.mouth.kind));
    put("beard", json!(c.beard.kind.name()));
    put("beard.tash", json!(c.beard.tash));
    put("neck", json!(c.neck.is_some()));
    put("marks.age", json!(k.age));
    put("marks.brow", json!(k.brow));
    put("marks.fold", json!(k.fold));
    put("marks.bag", json!(k.bag));
    put("marks.crow", json!(k.crow));
    put("marks.frown", json!(k.frown));
    put("marks.gaunt", json!(k.gaunt));
    put("marks.chin", json!(k.chin));
    put("marks.cheek", json!(k.cheek));
    put("marks.mole", json!(k.mole));
    put("palette.recipe", json!(c.palette.recipe));
    put("body_rgb", json!([body.r, body.g, body.b]));
    // The hair colour is named only where there is hair.
    if c.hair.style != HairStyle::Bald {
        put("hair.colour", json!(c.palette.hair_name));
    }
    Value::Object(out)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut manifest = None;
    let mut out_dir = None;
    let mut size = None;
    let mut i = 0;
    while i < args.len() {
        let value = args
            .get(i + 1)
            .unwrap_or_else(|| fail(&format!("{} needs a value", args[i])));
        match args[i].as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--out" => out_dir = Some(PathBuf::from(value)),
            "--size" => {
                size = Some(
                    value
                        .parse::<u32>()
                        .ok()
                        .filter(|px| *px >= 1)
                        .unwrap_or_else(|| fail("--size must be a whole number of pixels")),
                )
            }
            other => fail(&format!("unknown flag {other}")),
        }
        i += 2;
    }
    let manifest = manifest.unwrap_or_else(|| fail("--manifest is required"));
    let out_dir = out_dir.unwrap_or_else(|| fail("--out is required"));
    let text = fs::read_to_string(&manifest)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", manifest.display())));
    let doc: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| fail(&format!("bad manifest: {e}")));
    let genomes = doc["genomes"]
        .as_array()
        .unwrap_or_else(|| fail("the manifest has no genomes"));
    let cases = doc["cases"]
        .as_array()
        .unwrap_or_else(|| fail("the manifest has no cases"));
    fs::create_dir_all(&out_dir)
        .unwrap_or_else(|e| fail(&format!("cannot create the output: {e}")));
    let mut decoded = std::collections::BTreeMap::new();
    for g in genomes {
        let name = g["name"]
            .as_str()
            .unwrap_or_else(|| fail("a genome has no name"));
        let hex = g["hex"]
            .as_str()
            .unwrap_or_else(|| fail("a genome has no hex"));
        decoded.insert(name.to_owned(), decode(&genome_of(hex)));
    }
    for case in cases {
        let id = case["id"]
            .as_str()
            .unwrap_or_else(|| fail("a case has no id"));
        let genome = case["genome"]
            .as_str()
            .unwrap_or_else(|| fail("a case has no genome"));
        let mood = mood_of(
            case["mood"]
                .as_str()
                .unwrap_or_else(|| fail("a case has no mood")),
        );
        let creature = decoded
            .get(genome)
            .unwrap_or_else(|| fail(&format!("case {id} names an unknown genome")));
        let tree = creature.render(mood, size);
        let write = |name: String, bytes: &[u8]| {
            fs::write(out_dir.join(&name), bytes)
                .unwrap_or_else(|e| fail(&format!("cannot write {name}: {e}")));
        };
        write(format!("{id}.svg"), &serialize_tree(&tree));
        write(
            format!("{id}.choices.json"),
            serde_json::to_string(&choices(creature))
                .expect("choices serialize")
                .as_bytes(),
        );
    }
    println!(
        "fidelity_port: wrote {} cases for {} genomes to {}",
        cases.len(),
        genomes.len(),
        out_dir.display()
    );
}
