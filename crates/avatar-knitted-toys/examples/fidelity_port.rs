//! Writes the port's side of a fidelity run for `creature.knitted-toys`.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-knitted-toys --example fidelity_port -- \
//!     --manifest BASE.json --out DIR [--size PX]
//! ```
//!
//! For every case in the manifest it writes `<case>.svg`, the generic SVG of
//! the toy's render tree drawn for `--size` pixels (256 by default), and
//! `<case>.choices.json`, the traits `decode` chose under the reference's
//! own keys, plus `body_rgb`, the main yarn's colour. A trait the toy does
//! not have is left out, as the reference leaves it out. See
//! `scripts/fidelity/README.md` for the manifest format and the comparison.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use avatar_knitted_toys::{decode, main_yarn, render_tree, Traits};
use avatar_svg::{serialize_tree, Mood};
use serde_json::{json, Map, Value};

const USAGE: &str = "usage: fidelity_port --manifest FILE --out DIR [--size PX]";

fn mood(name: &str) -> Option<Mood> {
    Some(match name {
        "neutral" => Mood::Neutral,
        "working" => Mood::Working,
        "happy" => Mood::Happy,
        "waiting" => Mood::Waiting,
        "blocked" => Mood::Blocked,
        "sad" => Mood::Sad,
        "sleepy" => Mood::Sleepy,
        _ => return None,
    })
}

fn genome(hex: &str) -> Result<Box<[u8; 16_384]>, String> {
    if hex.len() != 32_768 {
        return Err(format!("a genome is 32,768 hex digits, not {}", hex.len()));
    }
    let mut out = Box::new([0_u8; 16_384]);
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|e| format!("genome hex at byte {i}: {e}"))?;
    }
    Ok(out)
}

/// The reference's choice keys for a toy's traits. Keys whose trait the
/// toy does not have are left out.
fn choices(t: &Traits) -> Map<String, Value> {
    let mut out = Map::new();
    let mut put = |key: &str, value: Value| {
        out.insert(key.to_owned(), value);
    };
    let opt = |v: Option<&str>| v.map(Value::from);
    put("plan", t.plan.into());
    put("tone", t.tone.into());
    put("accentFamily", t.accent_family.into());
    put("ears.kind", t.ears.kind.into());
    put("top.kind", t.top.kind.into());
    put("arms.kind", t.arms.kind.into());
    put("legs.kind", t.legs.kind.into());
    put("tail.kind", t.tail.kind.into());
    put("stripes.kind", t.stripes.kind.into());
    put("patches.belly", t.patches.belly.into());
    put("patches.spots", t.patches.spots.into());
    put("patches.mend", t.patches.mend.into());
    put("eyes.kind", t.eyes.kind.into());
    put("eyes.count", t.eyes.count.into());
    put("eyes.mount", t.eyes.mount.into());
    put("eyes.lid", t.eyes.lid.into());
    put("eyes.wonk", t.eyes.wonk.into());
    put("snout.kind", t.snout.kind.into());
    put("nose.kind", t.nose.kind.into());
    put("mouth.kind", t.mouth.kind.into());
    put("brows.rest", t.brows.rest.into());
    put("cheeks.kind", t.cheeks.kind.into());
    put("colour.accent", t.colour.accent.into());
    put("temper.happy", t.temper.happy.into());
    put("temper.working", t.temper.working.into());
    put("temper.blocked", t.temper.blocked.into());
    put("temper.sad", t.temper.sad.into());
    let optional = [
        ("ears.inner", opt(t.ears.inner)),
        ("ears.tip", opt(t.ears.tip)),
        ("top.yarn", opt(t.top.yarn)),
        ("top.count", t.top.count.map(Value::from)),
        ("top.tines", t.top.tines.map(Value::from)),
        ("arms.paw", opt(t.arms.paw)),
        ("legs.pads", opt(t.legs.pads)),
        ("legs.count", t.legs.count.map(Value::from)),
        ("tail.tip", opt(t.tail.tip)),
        ("eyes.iris", opt(t.eyes.iris)),
        ("eyes.pupil", opt(t.eyes.pupil)),
        ("eyes.felt", opt(t.eyes.felt)),
        ("eyes.look", opt(t.eyes.look)),
        ("snout.yarn", opt(t.snout.yarn)),
        ("nose.shape", opt(t.nose.shape)),
        ("colour.second", opt(t.colour.second)),
    ];
    for (key, value) in optional {
        if let Some(value) = value {
            put(key, value);
        }
    }
    let rgb = main_yarn(t);
    put("body_rgb", json!([rgb.r, rgb.g, rgb.b]));
    out
}

fn run() -> Result<(), String> {
    let mut manifest = None;
    let mut out = None;
    let mut size = 256_u32;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--size" => {
                let text = value()?;
                size = text
                    .parse()
                    .ok()
                    .filter(|s| (1..=2048).contains(s))
                    .ok_or_else(|| format!("--size must be 1 to 2048 pixels, not {text:?}"))?;
            }
            _ => return Err(format!("unknown argument {arg:?}\n{USAGE}")),
        }
    }
    let manifest = manifest.ok_or_else(|| format!("--manifest is required\n{USAGE}"))?;
    let out = out.ok_or_else(|| format!("--out is required\n{USAGE}"))?;
    let text =
        fs::read_to_string(&manifest).map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", manifest.display()))?;
    let genomes = doc["genomes"]
        .as_array()
        .ok_or("the manifest has no genomes list")?;
    let cases = doc["cases"]
        .as_array()
        .ok_or("the manifest has no cases list")?;
    let mut by_name = std::collections::BTreeMap::new();
    for g in genomes {
        let name = g["name"].as_str().ok_or("a genome has no name")?;
        let hex = g["hex"].as_str().ok_or("a genome has no hex")?;
        by_name.insert(
            name.to_owned(),
            genome(hex).map_err(|e| format!("{name}: {e}"))?,
        );
    }
    fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
    for case in cases {
        let id = case["id"].as_str().ok_or("a case has no id")?;
        let name = case["genome"].as_str().ok_or("a case has no genome")?;
        let mood_name = case["mood"].as_str().ok_or("a case has no mood")?;
        let bytes = by_name
            .get(name)
            .ok_or_else(|| format!("case {id}: no genome {name}"))?;
        let m = mood(mood_name).ok_or_else(|| format!("case {id}: unknown mood {mood_name}"))?;
        let tree = render_tree(bytes, m, Some(size));
        fs::write(out.join(format!("{id}.svg")), serialize_tree(&tree))
            .map_err(|e| format!("write {id}.svg: {e}"))?;
        let choices = Value::Object(choices(&decode(bytes)));
        fs::write(
            out.join(format!("{id}.choices.json")),
            serde_json::to_string(&choices).expect("choices serialize"),
        )
        .map_err(|e| format!("write {id}.choices.json: {e}"))?;
    }
    println!(
        "wrote {} cases at {size} px to {}",
        cases.len(),
        out.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("fidelity_port: {message}");
            ExitCode::from(2)
        }
    }
}
