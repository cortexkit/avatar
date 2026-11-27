//! The pinned robot cases, shared by the `robot_pins` example and the
//! robot tests: the corpus genomes, the coverage genomes and the digests
//! of their rest meshes.
//!
//! The corpus genomes are the 12 of `vectors/small-robots/corpus.tsv`, each
//! read from `vectors/wild/<project_id>.hex`. The coverage genomes are the
//! fewest leading genomes of [`COVERAGE_SEED`], derived as the
//! `random_sheet` example derives its genomes, such that the corpus and
//! they together show every catalogue value the 2D layout marks reachable.
//! The search looks at no more than [`COVERAGE_LIMIT`] genomes.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use avatar_3d::robot::{build_with, mesh_bytes, BuildOptions};
use avatar_small_robots::layout::{BackLayout, Layout, CATALOGUES};
use avatar_small_robots::{decode, GENOME_LEN};
use sha2::{Digest, Sha256};

/// The `random_sheet` seed the coverage genomes come from.
pub const COVERAGE_SEED: u64 = 3301;
/// The most genomes of the seed the coverage search looks at.
pub const COVERAGE_LIMIT: u64 = 4000;
/// The pins file, relative to the repository's `vectors/`.
pub const PINS: &str = "robots-3d/digests.tsv";

pub type Genome = Box<[u8; GENOME_LEN]>;

/// The repository's `vectors/` directory.
pub fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

/// The corpus genomes in `corpus.tsv` order, with their project ids.
pub fn corpus() -> Vec<(String, Genome)> {
    let list = vectors().join("small-robots/corpus.tsv");
    let text = fs::read_to_string(&list).unwrap_or_else(|e| panic!("read {}: {e}", list.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|line| {
            let id = line
                .split('\t')
                .next()
                .expect("a corpus row starts with its id");
            (
                id.to_owned(),
                read_genome(&vectors().join(format!("wild/{id}.hex"))),
            )
        })
        .collect()
}

fn read_genome(path: &Path) -> Genome {
    let hex = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let hex = hex.trim();
    assert_eq!(
        hex.len(),
        2 * GENOME_LEN,
        "{} holds one genome",
        path.display()
    );
    let mut genome = Box::new([0_u8; GENOME_LEN]);
    for (i, byte) in genome.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("genome files are hex");
    }
    genome
}

/// Genome `index` of the sheet drawn with `seed`, exactly as the
/// `random_sheet` example derives it: the BLAKE3 extendable output of the
/// text `avatar random_sheet genome`, keyed by the seed and the index as
/// little-endian u64 values followed by 16 zero bytes.
pub fn random_genome(seed: u64, index: u64) -> Genome {
    let mut key = [0_u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&index.to_le_bytes());
    let mut hasher = blake3::Hasher::new_keyed(&key);
    hasher.update(b"avatar random_sheet genome");
    let mut genome = Box::new([0_u8; GENOME_LEN]);
    hasher.finalize_xof().fill(&mut genome[..]);
    genome
}

/// The catalogue values a robot shows, as (field, value) pairs.
pub fn values(layout: &Layout) -> Vec<(&'static str, &'static str)> {
    let mut out = vec![
        ("plan", layout.plan),
        ("head", layout.head.kind),
        ("face", layout.face.kind.name()),
        ("mouth", layout.face.mouth_kind.name()),
        ("top", layout.top.kind),
        ("side", layout.sides.kind),
        ("gear", layout.gear.kind),
        ("chest", layout.chest.kind),
        (
            "decal",
            layout.decor.decal.as_ref().map_or("none", |d| d.kind),
        ),
        ("decor", layout.decor.scheme),
        (
            "wear",
            layout.decor.wear.as_ref().map_or("none", |d| d.kind),
        ),
        ("metal", layout.palette.metal_kind),
        (
            "back",
            match layout.back {
                Some(BackLayout::Cable { .. }) => "cable",
                Some(BackLayout::Pipe { .. }) => "pipe",
                _ => "none",
            },
        ),
    ];
    out.extend(layout.chassis.as_ref().map(|h| ("chassis", h.kind)));
    out.extend(layout.pelvis.as_ref().map(|h| ("pelvis", h.kind)));
    out.extend(layout.joints.neck.map(|n| ("neck", n.kind)));
    out.extend(layout.face.screen.map(|(_, eye)| ("screen_eye", eye)));
    out.extend(
        layout
            .top
            .masts
            .iter()
            .filter_map(|m| m.tip.map(|t| ("tip", t.kind))),
    );
    for arm in layout.arms.iter().flatten() {
        out.push(("arms", arm.kind));
        out.push(("hand", arm.hand));
    }
    out.extend(
        layout
            .gear
            .supports
            .iter()
            .filter_map(|s| s.foot.map(|f| ("foot", f.kind))),
    );
    out
}

/// Every (field, value) the layout marks reachable.
pub fn reachable() -> BTreeSet<(&'static str, &'static str)> {
    CATALOGUES
        .iter()
        .flat_map(|c| {
            c.values
                .iter()
                .filter(|v| c.is_reachable(v))
                .map(|v| (c.field, *v))
        })
        .collect()
}

/// The fewest leading genomes of `seed`, looking at no more than `limit`,
/// that with the corpus show every reachable value; an error names the
/// values still missing.
pub fn coverage_search(seed: u64, limit: u64) -> Result<u64, String> {
    let mut missing = reachable();
    for (_, genome) in corpus() {
        for v in values(&Layout::new(&decode(&genome))) {
            missing.remove(&v);
        }
    }
    let mut count = 0;
    while !missing.is_empty() {
        if count == limit {
            return Err(format!(
                "after {limit} genomes of seed {seed}, these reachable values never appear: {missing:?}"
            ));
        }
        let genome = random_genome(seed, count);
        for v in values(&Layout::new(&decode(&genome))) {
            missing.remove(&v);
        }
        count += 1;
    }
    Ok(count)
}

/// The pinned cases in order: the corpus genomes, then `count` coverage
/// genomes, each with its label.
pub fn cases(count: u64) -> Vec<(String, Genome)> {
    let mut out: Vec<(String, Genome)> = corpus()
        .into_iter()
        .map(|(id, g)| (format!("mesh/corpus/{id}"), g))
        .collect();
    out.extend((0..count).map(|i| {
        (
            format!("mesh/coverage/{COVERAGE_SEED}/{i}"),
            random_genome(COVERAGE_SEED, i),
        )
    }));
    out
}

/// The pins file's lines for the given coverage count: the seed and the
/// prefix, then one labelled SHA-256 per case.
pub fn pin_lines(count: u64) -> Vec<String> {
    let mut lines = vec![
        format!("coverage_seed\t{COVERAGE_SEED}"),
        format!("coverage_prefix\t{count}"),
    ];
    for (label, genome) in cases(count) {
        let robot = build_with(&genome, BuildOptions::untextured());
        let bytes = mesh_bytes(&robot).unwrap_or_else(|e| panic!("{label}: {e}"));
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        lines.push(format!("{label}\t{digest}"));
    }
    lines
}

/// The header comment of the pins file.
pub const HEADER: &str = "# Rest-mesh digests of the 3D small robots. Written only by\n\
# `cargo run -p cortexkit-avatar-3d --example robot_pins -- --write`.";

/// The committed pins file's lines, without comments.
pub fn committed() -> Vec<String> {
    let path = vectors().join(PINS);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The coverage prefix the committed pins record.
pub fn committed_prefix() -> u64 {
    committed()
        .iter()
        .find_map(|l| l.strip_prefix("coverage_prefix\t"))
        .expect("the pins record the coverage prefix")
        .parse()
        .expect("the coverage prefix is a whole number")
}
