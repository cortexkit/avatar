//! Does the golden corpus cover what the fleet actually looks like?
//!
//! The corpus is a hand-picked 18-vector set: boundaries (`point_min`,
//! `point_max`, `sliver`), every pattern class, and six real seat
//! derivations. Conformance across three languages is proved on THAT set.
//! The fleet's stored genomes are a different sample — 30 uniform draws
//! from blake3 — and a decode branch they reach but the corpus never does
//! is a branch no port parity test has ever compared.
//!
//! So this measures rather than assumes: decode both populations, and
//! report every discriminating selector value present in the fleet and
//! absent from the corpus. An empty report means corpus conformance
//! already speaks for every face in the fleet.
//!
//! Usage:
//!   cargo run -p avatar-genome --example fleet_coverage -- <manifest.json>
//!
//! The manifest is the deposit file minted by `avatar-export
//! --mint-genomes` (records carrying `agent_id` and `genome`).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use avatar_genome::{decode, Genome, SeatIdentity};
use avatar_phenotype::Phenotype;

/// One discriminating axis: a name and the value a phenotype takes on it.
/// Only selectors that STEER the renderer belong here — a continuous
/// coefficient differing between populations is expected and says nothing
/// about branch coverage, while a gate or family index picks a code path.
fn selectors(p: &Phenotype) -> Vec<(&'static str, i64)> {
    vec![
        ("body.point_count", p.body.point_count as i64),
        ("pattern.class", p.pattern.r#class as i64),
        ("mouth.family", p.face.mouth.family as i64),
        (
            "mouth.width_scale_gate",
            p.face.mouth.width_scale_gate as i64,
        ),
        (
            "mouth.aspect_widen_gate",
            p.face.mouth.aspect_widen_gate as i64,
        ),
        ("lashes.count", p.face.lashes.count as i64),
        ("lashes.removed", p.face.lashes.removed as i64),
        // Deliberately NOT here: lashes.direction, eye coefficients, and
        // every other continuous field. They are raw bytes normalized into
        // a curve, so an uncovered value means only "a different number" —
        // no corpus of any size covers 256 of them, and listing them makes
        // the report cry wolf loudly enough to bury the real gaps. What
        // belongs here steers a code path: a dispatch index, a gate, or a
        // loop bound.
    ]
}

fn covered(genomes: &[Genome]) -> Vec<(&'static str, BTreeSet<i64>)> {
    let mut axes: Vec<(&'static str, BTreeSet<i64>)> = Vec::new();
    for g in genomes {
        let p = decode(g);
        for (name, value) in selectors(&p) {
            match axes.iter_mut().find(|(n, _)| *n == name) {
                Some((_, set)) => {
                    set.insert(value);
                }
                None => {
                    let mut set = BTreeSet::new();
                    set.insert(value);
                    axes.push((name, set));
                }
            }
        }
    }
    axes
}

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: fleet_coverage <deposit-manifest.json>");
            std::process::exit(2);
        }
    };

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(2);
    });
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(2);
    });
    let records = doc
        .get("agents")
        .and_then(|v| v.as_array())
        .or_else(|| doc.as_array())
        .unwrap_or_else(|| {
            eprintln!("{path}: expected an array of agent records");
            std::process::exit(2);
        });

    let mut fleet = Vec::new();
    for rec in records {
        // The deposit manifest names its fields after the registry columns
        // it feeds, so the genome arrives as `avatar_genome`.
        let hex = rec
            .get("avatar_genome")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                eprintln!("record without an `avatar_genome` field");
                std::process::exit(2);
            });
        fleet.push(Genome::from_hex(hex).unwrap_or_else(|e| {
            eprintln!("bad genome in manifest: {e}");
            std::process::exit(2);
        }));
    }

    // The corpus vectors, read from the same files CI byte-compares.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors/dna");
    let mut corpus = Vec::new();
    let mut corpus_names = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| {
            eprintln!("{dir}: {e}");
            std::process::exit(2);
        })
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "hex"))
        .collect();
    entries.sort();
    for p in entries {
        let hex = std::fs::read_to_string(&p).expect("read vector");
        corpus.push(Genome::from_hex(hex.trim()).expect("parse vector"));
        corpus_names.push(p.file_stem().unwrap().to_string_lossy().to_string());
    }

    println!(
        "fleet: {} genomes  ·  corpus: {} vectors\n",
        fleet.len(),
        corpus.len()
    );

    let fleet_axes = covered(&fleet);
    let corpus_axes = covered(&corpus);

    let mut gaps = 0usize;
    let mut report = String::new();
    for (name, fleet_values) in &fleet_axes {
        let empty = BTreeSet::new();
        let corpus_values = corpus_axes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s)
            .unwrap_or(&empty);
        let missing: Vec<i64> = fleet_values.difference(corpus_values).copied().collect();
        let width = 24;
        if missing.is_empty() {
            let _ = writeln!(
                report,
                "  \x1b[32mok\x1b[0m   {name:<width$} fleet {:>2} value(s), all covered",
                fleet_values.len()
            );
        } else {
            gaps += missing.len();
            let corpus_list: Vec<i64> = corpus_values.iter().copied().collect();
            let _ = writeln!(
                report,
                "  \x1b[31mGAP\x1b[0m  {name:<width$} uncovered: {missing:?}\n       {:<width$} corpus has: {corpus_list:?}",
                ""
            );
        }
    }
    print!("{report}");

    if gaps == 0 {
        println!("\n\x1b[32mno coverage gaps\x1b[0m: every selector value the fleet reaches is exercised by a corpus vector");
        return;
    }

    println!("\n\x1b[31m{gaps} uncovered selector value(s)\x1b[0m: these decode branches are proved in Rust only, never compared against the TS or Swift ports");

    if !std::env::args().any(|a| a == "--mint-cover") {
        println!("re-run with --mint-cover to write covering vectors into vectors/dna/");
        std::process::exit(1);
    }

    // Every missing (axis, value) pair, as a flat worklist.
    let mut wanted: Vec<(&'static str, i64)> = Vec::new();
    for (name, fleet_values) in &fleet_axes {
        let empty = BTreeSet::new();
        let corpus_values = corpus_axes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s)
            .unwrap_or(&empty);
        for v in fleet_values.difference(corpus_values) {
            wanted.push((name, *v));
        }
    }

    // Search DERIVED genomes rather than embedding fleet DNA: a vector
    // named by a seed string is reproducible by anyone from this repo
    // alone, while a copied agent genome would make the corpus depend on
    // a registry row that can be re-rolled out from under it.
    let mut candidates: Vec<(String, Vec<(&'static str, i64)>)> = Vec::new();
    for n in 0..20_000u32 {
        let seed = format!("corpus-cover-{n}");
        let g = avatar_genome::derive_genome(&SeatIdentity::Registered {
            project_id: seed.clone(),
        });
        let p = decode(&g);
        let hits: Vec<(&'static str, i64)> = selectors(&p)
            .into_iter()
            .filter(|pair| wanted.contains(pair))
            .collect();
        if !hits.is_empty() {
            candidates.push((seed, hits));
        }
    }

    // Greedy set cover: fewest vectors that close every gap. Each genome
    // carries a point count AND a mouth family AND a lash count, so one
    // well-chosen vector routinely closes three gaps at once.
    let mut remaining: BTreeSet<(&'static str, i64)> = wanted.iter().copied().collect();
    let mut chosen: Vec<(String, Vec<(&'static str, i64)>)> = Vec::new();
    while !remaining.is_empty() {
        // Ties resolve to the LOWEST-numbered seed: max_by_key would keep
        // the last maximum, so widening the search range later would
        // silently re-pick different vectors for an unchanged gap set.
        // Same tiebreak rule the contour chainer uses, for the same reason.
        let mut best_idx = None;
        let mut best_score = 0usize;
        for (i, (_, hits)) in candidates.iter().enumerate() {
            let score = hits.iter().filter(|h| remaining.contains(h)).count();
            if score > best_score {
                best_score = score;
                best_idx = Some(i);
            }
        }
        let Some(best_idx) = best_idx else { break };
        // remove(), not swap_remove(): swapping reorders the tail and makes
        // the lowest-index tiebreak above depend on selection history.
        let (seed, hits) = candidates.remove(best_idx);
        let closes: Vec<(&'static str, i64)> =
            hits.into_iter().filter(|h| remaining.contains(h)).collect();
        if closes.is_empty() {
            break;
        }
        for h in &closes {
            remaining.remove(h);
        }
        chosen.push((seed, closes));
    }

    println!("\ncovering set: {} vector(s)", chosen.len());
    let dna_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors/dna");
    for (seed, closes) in &chosen {
        // Name the vector after the first gap it closes, so a reader
        // opening vectors/dna/ can see why each file exists.
        let (axis, value) = closes[0];
        let short = axis.split('.').next_back().unwrap_or(axis);
        let name = format!("cover_{short}_{value}");
        let g = avatar_genome::derive_genome(&SeatIdentity::Registered {
            project_id: seed.clone(),
        });
        let path = format!("{dna_dir}/{name}.hex");
        std::fs::write(&path, format!("{}\n", g.to_hex())).expect("write vector");
        println!("  {name:<22} from \"{seed}\"  closes {closes:?}");
    }
    if !remaining.is_empty() {
        println!("\n\x1b[31mstill uncovered\x1b[0m: {remaining:?}");
        std::process::exit(1);
    }
    println!("\nnow register them in vectors/manifest.json and mint:");
    println!("  UPDATE_GOLDEN=1 cargo test -p avatar-vectors");
}
