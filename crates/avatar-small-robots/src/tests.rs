//! The committed golden corpus in `vectors/small-robots/`, and checks that
//! hold for every robot.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use avatar_svg::{serialize_tree, serialize_tree_json, Role, Shape};

use crate::{decode, mood_name, render_tree, Mood, GENOME_LEN, MOODS};

const TYPE: &str = "creature.small-robots";

fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

fn root() -> PathBuf {
    vectors().join("small-robots")
}

/// One committed tree: the genome it is drawn from and its mood.
struct Case {
    project_id: String,
    mood: Mood,
}

impl Case {
    fn name(&self) -> String {
        format!("{}__{}", self.project_id, mood_name(self.mood))
    }
}

/// The corpus rows: each genome at version 1 in the moods its row names.
fn corpus() -> Vec<Case> {
    let text = fs::read_to_string(root().join("corpus.tsv")).expect("read the corpus list");
    let mut cases = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let [project_id, moods, version] = fields[..] else {
            panic!("corpus row is not project_id, moods and version: {line:?}")
        };
        assert_eq!(
            version, "1",
            "every small-robots corpus row is at version 1"
        );
        for mood in moods.split(',') {
            let mood = MOODS
                .into_iter()
                .find(|m| mood_name(*m) == mood)
                .unwrap_or_else(|| panic!("unknown mood {mood:?}"));
            cases.push(Case {
                project_id: project_id.to_owned(),
                mood,
            });
        }
    }
    cases
}

pub(crate) fn corpus_ids() -> impl Iterator<Item = String> {
    corpus()
        .into_iter()
        .map(|case| case.project_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
}

pub(crate) fn genome(project_id: &str) -> Box<[u8; GENOME_LEN]> {
    let path = vectors().join("wild").join(format!("{project_id}.hex"));
    let hex = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let hex = hex.trim();
    assert_eq!(hex.len(), GENOME_LEN * 2, "{project_id} is a whole genome");
    let mut out = Box::new([0_u8; GENOME_LEN]);
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("genome hex");
    }
    out
}

/// Every committed file the corpus mints, by path under `vectors/small-robots/`.
fn minted() -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut trees = Vec::new();
    for case in corpus() {
        let tree = render_tree(&genome(&case.project_id), case.mood, None);
        let json = serialize_tree_json(&tree).into_bytes();
        let svg = serialize_tree(&tree);
        trees.push(serde_json::json!({
            "name": case.name(),
            "project_id": case.project_id,
            "mood": mood_name(case.mood),
            "tree_bytes": json.len(),
            "tree_blake3": blake3::hash(&json).to_hex().as_str(),
            "svg_bytes": svg.len(),
            "svg_blake3": blake3::hash(&svg).to_hex().as_str(),
        }));
        files.push((format!("tree/{}.json", case.name()), json));
        files.push((format!("tree-svg/{}.svg", case.name()), svg));
    }
    let manifest = serde_json::json!({
        "type": TYPE,
        "version": 1,
        "genome_bytes": GENOME_LEN,
        "trees": trees,
    });
    let mut manifest = serde_json::to_string_pretty(&manifest).expect("serialize the manifest");
    manifest.push('\n');
    files.push(("manifest.json".to_owned(), manifest.into_bytes()));
    files
}

fn entry_names(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 name")
        })
        .collect()
}

/// Writes the golden corpus. Run it with `UPDATE_GOLDEN=1` after a change
/// that is meant to change what the committed trees draw.
#[test]
#[ignore]
fn mint() {
    assert_eq!(
        std::env::var("UPDATE_GOLDEN").as_deref(),
        Ok("1"),
        "minting rewrites vectors/small-robots/; set UPDATE_GOLDEN=1 to do it"
    );
    for dir in ["tree", "tree-svg"] {
        let dir = root().join(dir);
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clear a minted directory");
        }
        fs::create_dir_all(&dir).expect("create a minted directory");
    }
    for (name, bytes) in minted() {
        fs::write(root().join(&name), bytes).unwrap_or_else(|e| panic!("write {name}: {e}"));
    }
    println!("minted {} trees into vectors/small-robots/", corpus().len());
}

/// The committed corpus is exactly what this generator draws: the same
/// files, byte for byte, and nothing else in the directory.
#[test]
fn golden_corpus_matches() {
    let files = minted();
    let mut top = BTreeSet::from(["corpus.tsv".to_owned()]);
    let mut trees = BTreeSet::new();
    let mut svgs = BTreeSet::new();
    for (name, bytes) in &files {
        let committed =
            fs::read(root().join(name)).unwrap_or_else(|e| panic!("read committed {name}: {e}"));
        assert!(
            committed == *bytes,
            "{name} differs from what the generator draws"
        );
        match name.split_once('/') {
            Some(("tree", file)) => {
                top.insert("tree".to_owned());
                trees.insert(file.to_owned());
            }
            Some(("tree-svg", file)) => {
                top.insert("tree-svg".to_owned());
                svgs.insert(file.to_owned());
            }
            _ => {
                top.insert(name.clone());
            }
        }
    }
    assert_eq!(entry_names(&root()), top);
    assert_eq!(entry_names(&root().join("tree")), trees);
    assert_eq!(entry_names(&root().join("tree-svg")), svgs);
    assert_eq!(trees.len(), 18, "the corpus is 18 trees");
}

/// The reference draws a ground shadow under every robot in every mood:
/// one ellipse under it, before everything else. That primitive, and only
/// it, carries the ground-shadow role.
#[test]
fn only_the_shadow_is_a_ground_shadow() {
    for case in corpus() {
        let tree = render_tree(&genome(&case.project_id), case.mood, None);
        let tagged: Vec<usize> = tree
            .primitives
            .iter()
            .enumerate()
            .filter(|(_, p)| p.role == Some(Role::GroundShadow))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(tagged, [0], "{}", case.name());
        let shadow = &tree.primitives[0];
        assert!(
            matches!(shadow.shape, Shape::Ellipse { .. }),
            "{}",
            case.name()
        );
        assert_eq!(shadow.label, "rb-shadow", "{}", case.name());
        assert!(
            tree.primitives[1..].iter().all(|p| p.label != "rb-shadow"),
            "{}",
            case.name()
        );
    }
}

/// The hull is painted the body colour the genome's creature.wild has.
#[test]
fn body_is_the_wild_body_colour() {
    use avatar_svg::wild::{decode_genome, palette, render::css_rgb};
    for case in corpus().iter().filter(|c| c.mood == Mood::Neutral) {
        let bytes = genome(&case.project_id);
        let wild = decode_genome(&bytes);
        let expected = css_rgb(&palette::resolve(wild.recipe, wild.hue).body);
        let robot = decode(&bytes);
        assert_eq!(robot.body, expected, "{}", case.project_id);
        assert_eq!(
            usize::from(robot.palette_recipe),
            wild.recipe,
            "{}",
            case.project_id
        );
    }
}

/// Smaller drawings leave out detail, and every size draws without panic.
#[test]
fn detail_follows_the_size() {
    let bytes = genome("wild-port-0005");
    let full = render_tree(&bytes, Mood::Happy, None);
    assert_eq!(full, render_tree(&bytes, Mood::Happy, Some(256)));
    let icon = render_tree(&bytes, Mood::Happy, Some(48));
    assert!(icon.primitives.len() < full.primitives.len());
    // The reference draws a size of 0 as its own 240-unit frame.
    assert_eq!(
        render_tree(&bytes, Mood::Happy, Some(0)),
        render_tree(&bytes, Mood::Happy, Some(240))
    );
    for px in [1, 16, 56, 96, 1_080, 2_048, u32::MAX] {
        for mood in MOODS {
            assert!(!render_tree(&bytes, mood, Some(px)).primitives.is_empty());
        }
    }
}

/// Genomes from a fixed pseudo-random sequence draw in every mood, and
/// their trees serialize; the serializers assert every tree invariant.
#[test]
fn random_genomes_draw() {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for _ in 0..48 {
        let mut bytes = Box::new([0_u8; GENOME_LEN]);
        for byte in bytes.iter_mut() {
            // xorshift64: any fixed sequence will do.
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = (state >> 32) as u8;
        }
        let robot = decode(&bytes);
        for mood in MOODS {
            for px in [None, Some(48), Some(96)] {
                let tree = robot.render_tree(mood, px);
                assert!(!serialize_tree_json(&tree).is_empty());
                assert!(!serialize_tree(&tree).is_empty());
            }
        }
    }
}
