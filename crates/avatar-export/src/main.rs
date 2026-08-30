//! PNG export surface for fleet consumers — first user: ALF's GitHub App
//! minting ceremony (one App per head agent, the agent's face as the App
//! avatar).
//!
//! Contract (mirrors the wire identity contract):
//!   - Input is the STABLE ID alone (`--stable-id pj-…`), never a name —
//!     the same host-authored identity rule as the render wire: names are
//!     presentation, identity is the registry's opaque value.
//!   - Output is deterministic: stableId → genome → phenotype → SVG is the
//!     pinned corpus pipeline, and rasterization is resvg (pure Rust, no
//!     system fontconfig — the SVG contains no text), so the same input
//!     yields the same PNG bytes on every run and host.
//!   - Regeneration on demand IS the model: consumers re-run the export
//!     when what a row renders to changes — a new avatar_version chosen
//!     for that row, a new avatar_type, or a re-rolled genome. Nothing is
//!     cached here.
//!
//! Usage:
//!   avatar-export --stable-id pj-0000000000000001 --out ava.png
//!   avatar-export --manifest agents.json --out-dir avatars/
//!     manifest: JSON array of {"stable_id": "pj-…"} (unknown keys ignored,
//!     so ALF can pass richer records); output files are <stable_id>.png.
//!   Options: --size N (default 400), --mood M (default neutral).

use avatar_genome::{decode, derive_genome, Genome, SeatIdentity};
use avatar_svg::{render_svg, Mood, RenderParams, Q32};

/// The avatar TYPE: which procedural representation renders the genome.
/// Types are different representations OF THE SAME GENOME behind one
/// shared interface — the same bits depicted in another style, not another
/// creature. The type also pins the genome LAYOUT, since a representation
/// reading different byte ranges could not be a depiction of those bits.
///
/// Named for its character rather than a number on purpose: a row reading
/// `type=creature.v1, version=2` invites the reader to assume one of the
/// two is wrong, when they are independent axes. The creature redesign in
/// flight becomes its own type (`creature.wild`), not a version bump.
const AVATAR_TYPE: &str = "creature.classic";

/// The generation VERSION of that type, which exists for one purpose: fixing
/// bugs in procedural generation without silently changing anybody's face.
/// A stored version is authoritative for rendering — a renderer must draw a
/// row at the version the row names, not at its own newest — and moving a
/// row to a newer version is an explicit, consented act. That is what makes
/// old generation paths a permanent obligation rather than dead code.
const AVATAR_VERSION: u32 = 2;

fn mood_from(name: &str) -> Result<Mood, String> {
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

/// Renders a genome that already exists, as opposed to deriving one.
///
/// This is the correct path for anything depicting a LIVE agent, because
/// the registry row is the only truth once genomes can be re-rolled:
/// deriving from an id reproduces the agent's ORIGINAL creature, which
/// after a re-roll is a face that agent no longer wears. A GitHub App
/// avatar minted by derivation and a phone rendering the stored row would
/// then disagree permanently, each looking correct.
fn export_png_from_genome(genome: &Genome, mood: Mood, size: u32) -> Result<Vec<u8>, String> {
    let phenotype = decode(genome);
    let params = RenderParams {
        mood,
        growth: Q32::ONE,
        canvas: 10_000,
    };
    let svg = render_svg(&phenotype, &params).map_err(|e| e.to_string())?;
    rasterize(&svg, size)
}

fn export_png(stable_id: &str, mood: Mood, size: u32) -> Result<Vec<u8>, String> {
    let genome = derive_genome(&SeatIdentity::Registered {
        project_id: stable_id.to_owned(),
    });
    let phenotype = decode(&genome);
    let params = RenderParams {
        mood,
        growth: Q32::ONE,
        canvas: 10_000,
    };
    let svg = render_svg(&phenotype, &params).map_err(|e| e.to_string())?;
    rasterize(&svg, size)
}

fn rasterize(svg: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default())
        .map_err(|e| format!("svg parse: {e}"))?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| "pixmap alloc failed".to_owned())?;
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| format!("png encode: {e}"))
}

/// Reads agent ids from a comma list or, with a leading `@`, from a file of
/// one id per line (`#` comments allowed).
fn read_ids(arg: &str) -> Vec<String> {
    let text;
    let source = match arg.strip_prefix('@') {
        Some(path) => {
            text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
            text.as_str()
        }
        None => arg,
    };
    let split: Box<dyn Iterator<Item = &str>> = if arg.starts_with('@') {
        Box::new(source.lines())
    } else {
        Box::new(source.split(','))
    };
    split
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// Mints the genome deposit manifest for a batch of agents.
///
/// The operator ruled that the genome is STORED on the agent row and served,
/// so derivation runs once per agent at creation or backfill rather than per
/// render. This is the backfill half: one auditable artifact for a batch,
/// deposited through prefrontal's gated set-avatar op.
///
/// Every record carries `avatar_type`, because a genome is positional and
/// uninterpretable without knowing which LAYOUT minted it. A row holding
/// genome bytes with no type is corrupt rather than defaulting.
fn mint_manifest(ids: &[String]) -> String {
    let mut out = String::from("[\n");
    for (i, id) in ids.iter().enumerate() {
        let genome = derive_genome(&SeatIdentity::Registered {
            project_id: id.clone(),
        });
        let comma = if i + 1 == ids.len() { "" } else { "," };
        out.push_str(&format!(
            "  {{\"agent_id\":\"{id}\",\"avatar_genome\":\"{}\",\"avatar_type\":\"{AVATAR_TYPE}\",\"avatar_version\":{AVATAR_VERSION}}}{comma}\n",
            genome.to_hex(),
        ));
    }
    out.push_str("]\n");
    out
}

/// Minimal manifest reader: extracts every `"stable_id": "…"` string value.
/// Deliberately not a JSON library — the schema is one required key, and a
/// dependency-free scan keeps the export crate's supply chain equal to the
/// pipeline's own.
fn manifest_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let key = b"\"stable_id\"";
    let mut i = 0;
    while let Some(pos) = text[i..].find(std::str::from_utf8(key).unwrap()) {
        let mut j = i + pos + key.len();
        // Skip whitespace and the colon, then read the string value.
        while j < bytes.len() && (bytes[j] as char).is_whitespace() {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b':' {
            j += 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                if let Some(end) = text[j + 1..].find('"') {
                    out.push(text[j + 1..j + 1 + end].to_owned());
                }
            }
        }
        i = j;
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
    };

    let size: u32 = get("--size").and_then(|v| v.parse().ok()).unwrap_or(400);
    let mood = match mood_from(&get("--mood").unwrap_or_else(|| "neutral".into())) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    // Renders a genome the caller already holds — the path anything
    // depicting a live agent must use, since after a re-roll the stored
    // genome and a derived one are different creatures.
    if let Some(hex) = get("--genome") {
        let genome = match Genome::from_hex(hex.trim()) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("--genome: {e}");
                std::process::exit(2);
            }
        };
        let out = get("--out").unwrap_or_else(|| "avatar.png".to_owned());
        match export_png_from_genome(&genome, mood, size) {
            Ok(png) => {
                std::fs::write(&out, png).expect("write png");
                println!("{out}");
            }
            Err(e) => {
                eprintln!("export: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(ids_arg) = get("--mint-genomes") {
        let ids = read_ids(&ids_arg);
        if ids.is_empty() {
            eprintln!("--mint-genomes: no agent ids given");
            std::process::exit(2);
        }
        let manifest = mint_manifest(&ids);
        match get("--out") {
            Some(path) => {
                std::fs::write(&path, &manifest).expect("write manifest");
                eprintln!("minted {} genomes -> {path}", ids.len());
            }
            None => print!("{manifest}"),
        }
        return;
    }

    if let Some(stable_id) = get("--stable-id") {
        let out = get("--out").unwrap_or_else(|| format!("{stable_id}.png"));
        match export_png(&stable_id, mood, size) {
            Ok(png) => {
                std::fs::write(&out, png).expect("write png");
                println!("{out}");
            }
            Err(e) => {
                eprintln!("export {stable_id}: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(manifest_path) = get("--manifest") {
        let out_dir = get("--out-dir").unwrap_or_else(|| ".".into());
        std::fs::create_dir_all(&out_dir).expect("mkdir out-dir");
        let text = std::fs::read_to_string(&manifest_path).expect("read manifest");
        let ids = manifest_ids(&text);
        if ids.is_empty() {
            eprintln!("manifest contains no stable_id entries");
            std::process::exit(2);
        }
        let mut failures = 0;
        for id in &ids {
            let out = format!("{out_dir}/{id}.png");
            match export_png(id, mood, size) {
                Ok(png) => {
                    std::fs::write(&out, png).expect("write png");
                    println!("{out}");
                }
                Err(e) => {
                    eprintln!("export {id}: {e}");
                    failures += 1;
                }
            }
        }
        if failures > 0 {
            std::process::exit(1);
        }
        return;
    }

    eprintln!(
        "usage: avatar-export --stable-id <id> [--out f.png] | --manifest agents.json [--out-dir d]\n       options: --size N (default 400), --mood neutral|working|happy|waiting|blocked|sad|sleepy"
    );
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_is_deterministic_and_png_shaped() {
        let a = export_png("pj-0000000000000001", Mood::Neutral, 400).unwrap();
        let b = export_png("pj-0000000000000001", Mood::Neutral, 400).unwrap();
        assert_eq!(a, b, "same stableId must produce identical PNG bytes");
        assert_eq!(&a[..8], b"\x89PNG\r\n\x1a\n", "PNG magic");
    }

    #[test]
    fn different_ids_differ() {
        let a = export_png("pj-0000000000000001", Mood::Neutral, 64).unwrap();
        let b = export_png("pj-000000000000dead", Mood::Neutral, 64).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn minted_manifest_holds_the_properties_the_fleet_deposits_on() {
        // These are the exact claims made to the registry when handing over
        // the backfill: reproducible, one genome per agent, and every genome
        // the declared layout size so the op's length gate passes. A change
        // that broke any of them would deposit wrong data into 30 rows.
        let ids: Vec<String> = ["agent_aaa", "agent_bbb", "agent_00000003"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();

        let first = mint_manifest(&ids);
        assert_eq!(first, mint_manifest(&ids), "mint must be reproducible");

        let genomes: Vec<&str> = first
            .split("\"avatar_genome\":\"")
            .skip(1)
            .map(|rest| rest.split('"').next().expect("closing quote"))
            .collect();
        assert_eq!(genomes.len(), ids.len());
        for hex in &genomes {
            assert_eq!(hex.len(), 2048, "1024 bytes as hex");
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert_ne!(genomes[0], genomes[1], "distinct ids, distinct genomes");
        // Short ids are real registry ids, not truncation: the derivation
        // hashes an opaque string, so length must not matter.
        assert_eq!(genomes[2].len(), 2048);

        assert!(first.contains("\"avatar_type\":\"creature.classic\""));
        assert!(first.contains("\"avatar_version\":2"));
    }

    #[test]
    fn stored_and_derived_paths_agree_until_a_reroll_separates_them() {
        // The two paths must coincide for an un-re-rolled agent, which is
        // what makes the stored path a safe drop-in today. They stop
        // coinciding the moment a genome is re-rolled: derivation still
        // reproduces the ORIGINAL creature while the row wears a new one.
        // Anything depicting a live agent must therefore render the stored
        // genome, or a GitHub avatar and a phone will disagree permanently,
        // each looking entirely correct.
        let id = "agent_0000000000000001";
        let derived = derive_genome(&SeatIdentity::Registered {
            project_id: id.to_owned(),
        });

        let from_id = export_png(id, Mood::Neutral, 64).expect("derived render");
        let from_genome =
            export_png_from_genome(&derived, Mood::Neutral, 64).expect("stored render");
        assert_eq!(from_id, from_genome);

        // A different genome must not render the same creature, or the
        // assertion above would pass for the wrong reason.
        let other = derive_genome(&SeatIdentity::Registered {
            project_id: "agent_somebody_else".to_owned(),
        });
        let from_other = export_png_from_genome(&other, Mood::Neutral, 64).expect("other render");
        assert_ne!(from_genome, from_other);
    }

    #[test]
    fn ids_parse_from_a_list_or_a_file() {
        assert_eq!(read_ids("a, b ,c"), vec!["a", "b", "c"]);
        let path = std::env::temp_dir().join("avatar-export-ids-test.txt");
        std::fs::write(&path, "# roster\nagent_x\n\n  agent_y  \n").unwrap();
        assert_eq!(
            read_ids(&format!("@{}", path.display())),
            vec!["agent_x", "agent_y"]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn manifest_scan_reads_ids_and_ignores_unknown_keys() {
        let ids =
            manifest_ids(r#"[{"agent":"AVA","stable_id":"pj-1"},{"stable_id": "pj-2","x":1}]"#);
        assert_eq!(ids, vec!["pj-1", "pj-2"]);
    }
}
