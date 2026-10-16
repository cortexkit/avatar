use super::*;
use avatar_genome::derive_genome_bytes;
use avatar_svg::{render_tree_for_bytes, wild::decode_genome};
use serde::Serialize;

const GENOME_LEN: usize = 16_384;
// These identity/mood pairs have saved trees and population sheets made by the
// original playground renderer using standard-library maths. Reuse that list
// so every new golden has an original tree available for visual comparison.
const FROZEN_CORPUS: &str = include_str!("../../../../vectors/wild/corpus.tsv");

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WildManifest {
    genome_bytes: usize,
    #[serde(rename = "type")]
    creature_type: String,
    version: u16,
    producer_sha: String,
    vectors: Vec<WildVector>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WildVector {
    project_id: String,
    moods: Vec<String>,
}

#[test]
fn wild_manifest_matches_frozen_corpus() {
    if update_golden_enabled() {
        return;
    }
    let manifest = read_manifest();
    assert_valid_git_sha(&manifest.producer_sha);
    assert_current(&manifest_path(), &manifest_bytes(&manifest.producer_sha));

    let expected_names = manifest
        .vectors
        .iter()
        .flat_map(|vector| {
            vector
                .moods
                .iter()
                .map(|mood| format!("{}__{mood}.json", vector.project_id))
        })
        .collect::<BTreeSet<_>>();
    let pinned_names = WILD_TREE_FIXTURES
        .iter()
        .map(|(name, _, _)| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        pinned_names, expected_names,
        "wild tree pins omit a frozen pair"
    );
    assert_directory();
}

#[test]
fn wild_genomes_match_registered_derivation() {
    if update_golden_enabled() {
        return;
    }
    for vector in read_manifest().vectors {
        assert_current(
            &genome_path(&vector.project_id),
            hex(&derive(&vector.project_id)).as_bytes(),
        );
    }
}

#[test]
fn wild_trees_match_shipped_renderer() {
    if update_golden_enabled() {
        return;
    }
    let mut rendered_pairs = 0;
    let mut golden_pairs = 0;
    for vector in read_manifest().vectors {
        let genome = read_genome(&vector.project_id);
        for &mood in ALL_MOODS {
            let tree = render_tree_for_bytes(&genome, "creature.wild", 1, mood)
                .expect("render committed wild genome");
            assert_eq!(tree.canvas, 240);
            assert!(tree.background.is_none());
            rendered_pairs += 1;
            // Render every genome at every mood; only the pairs chosen for the
            // saved visual-review baseline have golden files to compare.
            if !vector.moods.iter().any(|name| name == mood_name(mood)) {
                continue;
            }
            assert_current(
                &tree_path(&vector.project_id, mood),
                serialize_tree_json(&tree).as_bytes(),
            );
            assert_current(
                &tree_svg_fixture_path(&format!("{}__{}.json", vector.project_id, mood_name(mood))),
                &serialize_tree(&tree),
            );
            golden_pairs += 1;
        }
    }
    assert_eq!(
        (rendered_pairs, golden_pairs),
        (84, 18),
        "the frozen corpus requires twelve genomes in seven moods and eighteen golden pairs"
    );
    println!(
        "rendered {rendered_pairs} wild corpus/mood pairs; compared {golden_pairs} golden pairs"
    );
}

#[test]
fn wild_corpus_covers_selectors_and_seven_moods() {
    if update_golden_enabled() {
        return;
    }
    let mut covered = BTreeSet::new();
    for vector in read_manifest().vectors {
        // Observe the shipped decoder, not a second implementation of its byte
        // offsets: a decoder change must be able to invalidate corpus coverage.
        let creature = decode_genome(&read_genome(&vector.project_id));
        covered.extend([
            format!("spokes={}", creature.wb.spokes.len()),
            format!("pairs={}", creature.exts.len()),
            format!("topology={}", creature.topology),
            format!("recipe={}", creature.recipe),
            format!("eyes={:?}", creature.eyes),
            format!("mouth={:?}", creature.mouth),
            format!("voronoi={}", creature.voronoi),
            format!("belly={}", creature.belly_variant),
            format!("brow={}", creature.face_style.brow),
        ]);
        covered.extend(creature.ext_fills.iter().map(|fill| format!("fill={fill}")));
        covered.extend(vector.moods.iter().map(|mood| format!("mood={mood}")));
    }

    let mut required = BTreeSet::new();
    for (selector, values) in [
        ("spokes", 5..=16),
        ("pairs", 0..=4),
        ("topology", 0..=2),
        ("belly", 0..=3),
        ("brow", 0..=3),
        ("fill", 0..=2),
    ] {
        required.extend(values.map(|value| format!("{selector}={value}")));
    }
    required.extend([0, 2, 3, 4].map(|recipe| format!("recipe={recipe}")));
    required.extend(
        ["DotShine", "Lidded", "HappyArc", "Uneven", "HalfLid"].map(|eyes| format!("eyes={eyes}")),
    );
    required.extend(
        ["Arc", "CatW", "OpenHappy", "Snaggle", "Flat", "Fangs"]
            .map(|mouth| format!("mouth={mouth}")),
    );
    required.extend([false, true].map(|voronoi| format!("voronoi={voronoi}")));
    required.extend(
        ALL_MOODS
            .iter()
            .map(|mood| format!("mood={}", mood_name(*mood))),
    );
    let missing = required.difference(&covered).collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "missing wild corpus coverage: {missing:?}"
    );
    println!("wild corpus covers 48 selectors and all seven moods");
}

pub(super) fn assert_directory() {
    let mut names = frozen_vectors()
        .iter()
        .map(|vector| format!("{}.hex", vector.project_id))
        .collect::<BTreeSet<_>>();
    names.insert("manifest.json".to_owned());
    // The frozen genome list this corpus is minted from lives beside it.
    names.insert("corpus.tsv".to_owned());
    assert_file_names("wild", &names);
}

pub(super) fn mint_corpus() {
    let root = corpus_root();
    for directory in ["wild", "tree", "tree-svg"] {
        fs::create_dir_all(root.join(directory)).expect("create wild corpus directory");
    }
    println!("const WILD_TREE_FIXTURES: &[(&str, usize, &str)] = &[");
    for vector in frozen_vectors() {
        let genome = derive(&vector.project_id);
        write_corpus_file(&genome_path(&vector.project_id), hex(&genome).as_bytes());
        for mood in vector.moods {
            let mood = parse_mood(&mood);
            let tree = render_tree_for_bytes(&genome, "creature.wild", 1, mood)
                .expect("render wild genome while minting");
            let json = serialize_tree_json(&tree);
            write_corpus_file(&tree_path(&vector.project_id, mood), json.as_bytes());
            let name = format!("{}__{}.json", vector.project_id, mood_name(mood));
            write_corpus_file(&tree_svg_fixture_path(&name), &serialize_tree(&tree));
            println!(
                "    ({name:?}, {}, {:?}),",
                json.len(),
                blake3::hash(json.as_bytes()).to_hex().as_str()
            );
        }
    }
    println!("];");
    write_corpus_file(&manifest_path(), &manifest_bytes(&producer_sha(&root)));

    let mut total_size = 0;
    let mut hasher = blake3::Hasher::new();
    for name in entry_names(&root.join("tree-svg")) {
        let bytes = fs::read(root.join("tree-svg").join(&name)).expect("read tree SVG for pin");
        total_size += bytes.len();
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    println!("const TREE_SVG_BOUNDARY_SIZE: usize = {total_size};");
    println!(
        "const TREE_SVG_BOUNDARY_HASH: &str = {:?};",
        hasher.finalize().to_hex().as_str()
    );
}

fn frozen_vectors() -> Vec<WildVector> {
    FROZEN_CORPUS
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let (project_id, moods) = line.split_once('\t').expect("frozen corpus row has moods");
            WildVector {
                project_id: project_id.to_owned(),
                moods: moods.split(',').map(str::to_owned).collect(),
            }
        })
        .collect()
}

fn manifest_bytes(producer_sha: &str) -> Vec<u8> {
    serde_json::to_vec(&WildManifest {
        genome_bytes: GENOME_LEN,
        creature_type: "creature.wild".to_owned(),
        version: 1,
        producer_sha: producer_sha.to_owned(),
        vectors: frozen_vectors(),
    })
    .expect("serialize wild manifest")
}

fn manifest_path() -> PathBuf {
    corpus_root().join("wild/manifest.json")
}

fn read_manifest() -> WildManifest {
    serde_json::from_slice(&fs::read(manifest_path()).expect("read wild manifest"))
        .expect("parse wild manifest")
}

fn genome_path(project_id: &str) -> PathBuf {
    corpus_root().join("wild").join(format!("{project_id}.hex"))
}

fn derive(project_id: &str) -> Vec<u8> {
    derive_genome_bytes(
        &SeatIdentity::Registered {
            project_id: project_id.to_owned(),
        },
        GENOME_LEN,
    )
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(hex, "{byte:02x}").expect("write hex");
    }
    hex
}

fn read_genome(project_id: &str) -> [u8; GENOME_LEN] {
    let hex = fs::read_to_string(genome_path(project_id)).expect("read wild genome hex");
    assert_eq!(hex.len(), GENOME_LEN * 2, "{project_id} genome hex length");
    assert!(hex
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    let mut genome = [0; GENOME_LEN];
    let (pairs, _) = hex.as_bytes().as_chunks::<2>();
    for (byte, pair) in genome.iter_mut().zip(pairs) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16)
            .expect("decode wild genome hex");
    }
    genome
}

fn parse_mood(name: &str) -> Mood {
    ALL_MOODS
        .iter()
        .copied()
        .find(|mood| mood_name(*mood) == name)
        .unwrap_or_else(|| panic!("unknown wild mood: {name}"))
}
