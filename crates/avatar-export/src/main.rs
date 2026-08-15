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
//!     when a genome-affecting change ships (generator_version bump or an
//!     identity-migration era event); nothing is cached here.
//!
//! Usage:
//!   avatar-export --stable-id pj-0000000000000001 --out ava.png
//!   avatar-export --manifest agents.json --out-dir avatars/
//!     manifest: JSON array of {"stable_id": "pj-…"} (unknown keys ignored,
//!     so ALF can pass richer records); output files are <stable_id>.png.
//!   Options: --size N (default 400), --mood M (default neutral).

use avatar_genome::{decode, derive_genome, SeatIdentity};
use avatar_svg::{render_svg, Mood, Q32, RenderParams};

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

    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())
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
    fn manifest_scan_reads_ids_and_ignores_unknown_keys() {
        let ids = manifest_ids(
            r#"[{"agent":"AVA","stable_id":"pj-1"},{"stable_id": "pj-2","x":1}]"#,
        );
        assert_eq!(ids, vec!["pj-1", "pj-2"]);
    }
}
