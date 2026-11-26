use super::*;
use avatar_genome::derive_genome_bytes;
use avatar_svg::{render_tree_for_bytes, wild::decode_genome, Primitive, RenderTree, TransformOp};
use serde::Serialize;

const GENOME_LEN: usize = 16_384;
// The version 1 identity/mood pairs have saved trees and population sheets
// made by the original playground renderer using standard-library maths.
// Reuse that list so every new golden has an original tree available for
// visual comparison. The version 2 row reuses one of those genomes.
const FROZEN_CORPUS: &str = include_str!("../../../../vectors/wild/corpus.tsv");

/// The wild reference corpus. Each vector records the `creature.wild`
/// generation its trees are rendered at, because the corpus pins trees from
/// more than one version and a single top-level version would misdescribe
/// some of them.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WildManifest {
    genome_bytes: usize,
    #[serde(rename = "type")]
    creature_type: String,
    producer_sha: String,
    vectors: Vec<WildVector>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WildVector {
    project_id: String,
    version: u32,
    moods: Vec<String>,
}

impl WildVector {
    /// Version 1 trees keep the names they shipped with. Later versions add
    /// a version infix, so one genome's trees from two generations can sit
    /// side by side in `vectors/tree`.
    fn tree_name(&self, mood: &str) -> String {
        match self.version {
            1 => format!("{}__{mood}.json", self.project_id),
            version => format!("{}__v{version}__{mood}.json", self.project_id),
        }
    }
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
        .flat_map(|vector| vector.moods.iter().map(|mood| vector.tree_name(mood)))
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
fn every_wild_tree_has_exactly_one_ground_shadow_role() {
    fn assert_shadow(tree: &avatar_svg::RenderTree, name: &str) {
        let mut pending: Vec<_> = tree.primitives.iter().collect();
        let mut roles = Vec::new();
        while let Some(primitive) = pending.pop() {
            if let Some(role) = primitive.role {
                roles.push((role, primitive.label.as_str()));
            }
            for effect in &primitive.effects {
                if let avatar_svg::Effect::Masked { mask } = effect {
                    pending.extend(mask);
                }
            }
        }
        assert_eq!(
            roles,
            [(avatar_svg::Role::GroundShadow, "wild-shadow")],
            "{name}: only the ground shadow may carry the ground_shadow role"
        );
    }

    let mut count = 0;
    for vector in read_manifest().vectors {
        let genome = read_genome(&vector.project_id);
        for &mood in ALL_MOODS {
            let tree =
                render_tree_for_bytes(&genome, "creature.wild", vector.version, mood).unwrap();
            assert_shadow(&tree, &vector.project_id);
            count += 1;
        }
    }
    for &(name, _, _) in WILD_TREE_FIXTURES {
        assert_shadow(&read_tree_fixture(name), name);
    }
    assert_eq!(count, 91);
    println!(
        "verified ground_shadow on {count} generated trees and {} saved trees",
        WILD_TREE_FIXTURES.len()
    );
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
            let tree = render_tree_for_bytes(&genome, "creature.wild", vector.version, mood)
                .expect("render committed wild genome");
            assert_eq!(tree.canvas, 240);
            assert!(tree.background.is_none());
            rendered_pairs += 1;
            // Render every genome at every mood; only the pairs chosen for the
            // saved visual-review baseline have golden files to compare.
            if !vector.moods.iter().any(|name| name == mood_name(mood)) {
                continue;
            }
            let name = vector.tree_name(mood_name(mood));
            assert_current(
                &corpus_root().join("tree").join(&name),
                serialize_tree_json(&tree).as_bytes(),
            );
            assert_current(&tree_svg_fixture_path(&name), &serialize_tree(&tree));
            golden_pairs += 1;
        }
    }
    assert_eq!(
        (rendered_pairs, golden_pairs),
        (91, 25),
        "the frozen corpus requires twelve genomes at version 1 and one at version 2, each in seven moods, and twenty-five golden pairs"
    );
    println!(
        "rendered {rendered_pairs} wild corpus/mood pairs; compared {golden_pairs} golden pairs"
    );
}

/// Version 2 changes only where mood lids and squints are drawn. Every other
/// primitive must match version 1 exactly, and each overlay must be version
/// 1's overlay with the eyes' face-axis transform put in front of its own.
#[test]
fn wild_v2_differs_from_v1_only_in_lid_and_squint_placement() {
    fn is_overlay(primitive: &Primitive) -> bool {
        primitive.label.starts_with("wild-mood-lid-")
            || primitive.label.starts_with("wild-mood-squint-")
    }
    fn operations(primitive: &Primitive) -> &[TransformOp] {
        primitive
            .transform
            .as_ref()
            .map_or(&[], |transform| &transform.ops)
    }
    fn split(tree: &RenderTree) -> (Vec<&Primitive>, Vec<&Primitive>) {
        tree.primitives
            .iter()
            .partition(|primitive| !is_overlay(primitive))
    }

    let genomes = read_manifest()
        .vectors
        .into_iter()
        .map(|vector| vector.project_id)
        .collect::<BTreeSet<_>>();
    let (mut pairs, mut differing) = (0, 0);
    for project_id in &genomes {
        let genome = read_genome(project_id);
        for &mood in ALL_MOODS {
            let label = format!("{project_id} {}", mood_name(mood));
            let render =
                |version| render_tree_for_bytes(&genome, "creature.wild", version, mood).unwrap();
            let (v1, v2) = (render(1), render(2));
            pairs += 1;
            if v1 != v2 {
                differing += 1;
            }
            if mood == Mood::Neutral {
                // A neutral face has no lids or squints, so nothing may differ.
                assert_eq!(v1, v2, "{label}");
                continue;
            }

            let (v1_rest, v1_overlays) = split(&v1);
            let (v2_rest, v2_overlays) = split(&v2);
            assert_eq!(
                v1_rest, v2_rest,
                "{label}: a primitive other than a lid or squint changed"
            );
            assert_eq!(v1_overlays.len(), v2_overlays.len(), "{label}");
            let moved = v1_overlays
                .iter()
                .zip(&v2_overlays)
                .filter(|(old, new)| old != new)
                .count();
            // The eyes are scaled as a pair, so either every overlay follows
            // them or, with unscaled eyes, none needs to move.
            assert!(
                moved == 0 || moved == v2_overlays.len(),
                "{label}: only {moved} of {} overlays moved",
                v2_overlays.len()
            );
            if moved == 0 {
                continue;
            }
            let eyes: Vec<_> = v2_rest
                .iter()
                .filter(|primitive| primitive.label.starts_with("wild-eye-"))
                .collect();
            assert!(
                !eyes.is_empty(),
                "{label}: overlays moved on a face with no eyes"
            );
            for (old, new) in v1_overlays.iter().zip(&v2_overlays) {
                let (old_ops, new_ops) = (operations(old), operations(new));
                assert_eq!(new_ops.len(), old_ops.len() + 3, "{label}: {}", new.label);
                let (prefix, rest) = new_ops.split_at(3);
                assert_eq!(rest, old_ops, "{label}: {}", new.label);
                for eye in &eyes {
                    assert!(
                        operations(eye).starts_with(prefix),
                        "{label}: {} is not transformed like {}",
                        new.label,
                        eye.label
                    );
                }
                let mut expected = (*old).clone();
                expected.transform = new.transform.clone();
                assert_eq!(&expected, *new, "{label}: {}", new.label);
            }
        }
    }
    assert_eq!(pairs, 84);
    assert!(
        differing > 0,
        "no corpus pair exercised the version 2 change"
    );
    println!(
        "version 2 differs from version 1 in {differing} of {pairs} corpus genome and mood pairs"
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
        for mood in &vector.moods {
            let name = vector.tree_name(mood);
            let mood = parse_mood(mood);
            let tree = render_tree_for_bytes(&genome, "creature.wild", vector.version, mood)
                .expect("render wild genome while minting");
            let json = serialize_tree_json(&tree);
            write_corpus_file(&root.join("tree").join(&name), json.as_bytes());
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
            let fields: Vec<&str> = line.split('\t').collect();
            let [project_id, moods, version] = fields[..] else {
                panic!("frozen corpus row is not project_id, moods and version: {line:?}")
            };
            WildVector {
                project_id: project_id.to_owned(),
                version: version
                    .parse()
                    .unwrap_or_else(|_| panic!("frozen corpus row has no version: {line:?}")),
                moods: moods.split(',').map(str::to_owned).collect(),
            }
        })
        .collect()
}

fn manifest_bytes(producer_sha: &str) -> Vec<u8> {
    serde_json::to_vec(&WildManifest {
        genome_bytes: GENOME_LEN,
        creature_type: "creature.wild".to_owned(),
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
