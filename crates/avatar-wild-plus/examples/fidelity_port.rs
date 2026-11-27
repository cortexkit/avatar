//! The port side of the fidelity comparison for `creature.wild-plus`.
//!
//! Reads a case manifest written by `scripts/fidelity/wild-plus.js` and, for
//! every case, writes `<case>.svg` (the render tree through
//! `serialize_tree`) and `<case>.choices.json` (the decoded creature mapped
//! onto the reference's choice keys) into the output directory.
//!
//! ```sh
//! cargo run --release -p cortexkit-avatar-wild-plus --example fidelity_port -- \
//!     --manifest base.json --out port/ [--size 256]
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use avatar_svg::wild::GENOME_LEN;
use avatar_svg::{serialize_tree, Mood};
use avatar_wild_plus::{
    decode, render_tree, Creature, COATS, CRESTS, EYE_PLANS, EYE_STYLES, LIMB_KINDS, MOUTH_PLANS,
    PLANS, SURFACES, TAILS,
};
use serde_json::{json, Map, Value};

/// The notes the body builder writes for each plan, by plan index. Plan 0,
/// the blob, writes none.
fn body_notes(plan: usize) -> Vec<String> {
    match plan {
        0 => vec!["none".into()],
        1 => vec!["torso".into(), "necked torso".into()],
        2 => (1..=3)
            .flat_map(|n| [format!("{n} segments"), format!("{n} segments with knobs")])
            .collect(),
        3 => vec![
            "1 lobe pair".into(),
            "2 lobe pairs".into(),
            "3 lobe pairs".into(),
        ],
        4 => vec!["long neck".into()],
        5 => vec!["4 tentacles".into(), "6 tentacles".into()],
        _ => vec![
            "two thick legs".into(),
            "bird legs".into(),
            "4 arched legs".into(),
            "6 arched legs".into(),
            "four legs".into(),
        ],
    }
}

const FEET: [&str; 3] = ["feet", "toed feet", "clawed feet"];
const BELLIES: [&str; 3] = ["plated belly", "wide belly", "belly spot"];

/// The anatomy read back from the creature's notes, as
/// `scripts/fidelity/wild-plus.js` reads the reference's notes: the body
/// plan's masses, crest, arms, tail, feet and the number of slots each limb
/// kind takes.
struct Anatomy {
    body: String,
    crest: String,
    arms: String,
    tail: String,
    feet: String,
    limbs: BTreeMap<&'static str, u32>,
}

fn limb_note(note: &str) -> Option<BTreeMap<&'static str, u32>> {
    let mut counts = BTreeMap::new();
    for part in note.split(", ") {
        let (kind, n) = match part.split_once(" pairs of ") {
            Some((n, kind)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                (kind, n.parse().ok()?)
            }
            _ => (part, 1),
        };
        let kind = *LIMB_KINDS.iter().find(|k| **k == kind)?;
        if counts.insert(kind, n).is_some() {
            return None;
        }
    }
    Some(counts)
}

fn anatomy(c: &Creature, plan: usize) -> Result<Anatomy, String> {
    let mut out = Anatomy {
        body: "none".into(),
        crest: "none".into(),
        arms: "none".into(),
        tail: "none".into(),
        feet: "none".into(),
        limbs: LIMB_KINDS.iter().map(|k| (*k, 0)).collect(),
    };
    let fail = |note: &str| {
        Err(format!(
            "unrecognised anatomy note {note:?} in {:?}",
            c.anatomy
        ))
    };
    let bodies = body_notes(plan);
    for note in &c.anatomy {
        let crest = note.strip_suffix(" crest").filter(|m| CRESTS.contains(m));
        let claw_arms = note
            .strip_suffix(" arms")
            .filter(|m| *m == "claw" || *m == "flipper");
        let tail = note.strip_suffix(" tail").filter(|m| TAILS.contains(m));
        if bodies.contains(note) && out.body == "none" {
            out.body = note.clone();
        } else if let (Some(m), true) = (crest, out.crest == "none") {
            out.crest = m.into();
        } else if note == "arms" && out.arms == "none" {
            out.arms = "nub".into();
        } else if let (Some(m), true) = (claw_arms, out.arms == "none") {
            out.arms = m.into();
        } else if let (Some(m), true) = (tail, out.tail == "none") {
            out.tail = m.into();
        } else if FEET.contains(&note.as_str()) && out.feet == "none" {
            out.feet = note.clone();
        } else if let (Some(limbs), true) = (limb_note(note), out.limbs.values().all(|n| *n == 0)) {
            out.limbs.extend(limbs);
        } else {
            return fail(note);
        }
    }
    if plan != 0 && out.body == "none" {
        return fail(&format!("(no note for body plan {plan})"));
    }
    Ok(out)
}

fn belly(c: &Creature) -> Result<String, String> {
    let found: Vec<&String> = c
        .surface_notes
        .iter()
        .filter(|n| BELLIES.contains(&n.as_str()))
        .collect();
    match found.as_slice() {
        [] => Ok("none".into()),
        [one] => Ok((*one).clone()),
        _ => Err(format!("two belly notes in {:?}", c.surface_notes)),
    }
}

/// The decoded creature on the reference's choice keys.
fn choices(c: &Creature) -> Result<Map<String, Value>, String> {
    let plan = PLANS
        .iter()
        .position(|p| *p == c.plan)
        .expect("a known plan");
    let a = anatomy(c, plan)?;
    let mut out = Map::new();
    let mut put = |k: &str, v: Value| {
        out.insert(k.to_owned(), v);
    };
    put("plan", json!(c.plan));
    put("body", json!(a.body));
    put("crest", json!(a.crest));
    put("arms", json!(a.arms));
    put("tail", json!(a.tail));
    put("feet", json!(a.feet));
    put("coat", json!(c.coat));
    put("surface.kind", json!(c.surface));
    put("surface.second", json!(c.surface_second));
    put("belly", json!(belly(c)?));
    put("palette.recipe", json!(c.recipe));
    put("palette.scheme", json!(c.scheme));
    put("eyes.plan", json!(c.eye_plan));
    put("eyes.style", json!(c.eye_style));
    put("mouth", json!(c.mouth));
    put("nose", json!(c.nose));
    put("body_rgb", json!([c.body.r, c.body.g, c.body.b]));
    for k in LIMB_KINDS {
        put(&format!("limbs.{k}"), json!(a.limbs[k]));
    }
    // The tables the keys come from, checked so a renamed entry fails here.
    debug_assert!(COATS.contains(&c.coat) && SURFACES.contains(&c.surface));
    debug_assert!(EYE_PLANS.contains(&c.eye_plan) && EYE_STYLES.contains(&c.eye_style));
    debug_assert!(MOUTH_PLANS.contains(&c.mouth));
    Ok(out)
}

fn mood_of(name: &str) -> Option<Mood> {
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

fn genome(hex: &str) -> Result<[u8; GENOME_LEN], String> {
    if hex.len() != 2 * GENOME_LEN || !hex.is_ascii() {
        return Err(format!("a genome must be {} hex digits", 2 * GENOME_LEN));
    }
    let mut out = [0u8; GENOME_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn run() -> Result<usize, String> {
    let mut manifest: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut size: Option<u32> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--manifest" => manifest = Some(value()?.into()),
            "--out" => out_dir = Some(value()?.into()),
            "--size" => {
                size = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--size takes a whole number")?,
                )
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let manifest = manifest.ok_or("--manifest is required")?;
    let out_dir = out_dir.ok_or("--out is required")?;
    let text =
        std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let m: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    if m["type"] != "creature.wild-plus" {
        return Err(format!(
            "manifest type is {}, not creature.wild-plus",
            m["type"]
        ));
    }
    let mut genomes = BTreeMap::new();
    for g in m["genomes"].as_array().ok_or("manifest has no genomes")? {
        let name = g["name"].as_str().ok_or("a genome has no name")?;
        let hex = g["hex"].as_str().ok_or("a genome has no hex")?;
        genomes.insert(
            name.to_owned(),
            genome(hex).map_err(|e| format!("genome {name}: {e}"))?,
        );
    }
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let cases = m["cases"].as_array().ok_or("manifest has no cases")?;
    let mut decoded: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("a case has no id")?;
        let gname = case["genome"].as_str().ok_or("a case has no genome")?;
        let mood_name = case["mood"].as_str().ok_or("a case has no mood")?;
        let bytes = genomes
            .get(gname)
            .ok_or(format!("case {id}: unknown genome {gname}"))?;
        let mood = mood_of(mood_name).ok_or(format!("case {id}: unknown mood {mood_name}"))?;
        let tree = render_tree(bytes, mood, size);
        std::fs::write(out_dir.join(format!("{id}.svg")), serialize_tree(&tree))
            .map_err(|e| format!("case {id}: {e}"))?;
        if !decoded.contains_key(gname) {
            let c = decode(bytes);
            decoded.insert(
                gname.to_owned(),
                choices(&c).map_err(|e| format!("case {id}: {e}"))?,
            );
        }
        let text = serde_json::to_string(&Value::Object(decoded[gname].clone())).expect("json");
        std::fs::write(out_dir.join(format!("{id}.choices.json")), text)
            .map_err(|e| format!("case {id}: {e}"))?;
    }
    Ok(cases.len())
}

fn main() -> ExitCode {
    match run() {
        Ok(n) => {
            println!("wrote {n} cases");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fidelity_port: {e}");
            ExitCode::from(2)
        }
    }
}
