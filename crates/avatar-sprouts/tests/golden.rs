//! The committed golden corpus in `vectors/sprouts/`.
//!
//! The corpus is the eighteen trees `vectors/sprouts/corpus.tsv` names: each
//! genome is read from the committed `vectors/wild/<id>.hex` and drawn at
//! version 1 in the listed moods. `manifest.json`, `tree/<id>__<mood>.json`
//! and `tree-svg/<id>__<mood>.svg` are written by the ignored `mint` test:
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p cortexkit-avatar-sprouts --test golden mint -- --ignored --nocapture
//! ```
//!
//! It prints the `TREE_PINS` table below. With `UPDATE_GOLDEN` unset, the
//! other tests check that the generator still draws exactly the committed
//! files, that the directory holds nothing else, and that only the ground
//! shadow carries the ground-shadow role.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use avatar_sprouts::{render_tree, GENOME_LEN, TYPE};
use avatar_svg::{serialize_tree, serialize_tree_json, Effect, Mood, Primitive, Role};

const CORPUS: &str = include_str!("../../../vectors/sprouts/corpus.tsv");

/// Each golden tree's file name, its length in bytes and its BLAKE3 hash,
/// as the mint printed them.
const TREE_PINS: &[(&str, usize, &str)] = &[];

struct Row {
    project_id: String,
    moods: Vec<String>,
    version: u32,
}

fn rows() -> Vec<Row> {
    CORPUS
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let [project_id, moods, version] = fields[..] else {
                panic!("corpus row is not project_id, moods and version: {line:?}")
            };
            Row {
                project_id: project_id.to_owned(),
                moods: moods.split(',').map(str::to_owned).collect(),
                version: version.parse().expect("corpus version is a number"),
            }
        })
        .collect()
}

fn vectors_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

fn stem_root() -> PathBuf {
    vectors_root().join("sprouts")
}

fn genome(project_id: &str) -> Box<[u8; GENOME_LEN]> {
    let path = vectors_root()
        .join("wild")
        .join(format!("{project_id}.hex"));
    let hex = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert_eq!(hex.len(), GENOME_LEN * 2, "{project_id} genome hex length");
    let mut bytes = Box::new([0_u8; GENOME_LEN]);
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .unwrap_or_else(|_| panic!("{project_id} genome is not lowercase hex"));
    }
    bytes
}

fn mood(name: &str) -> Mood {
    match name {
        "neutral" => Mood::Neutral,
        "working" => Mood::Working,
        "happy" => Mood::Happy,
        "waiting" => Mood::Waiting,
        "blocked" => Mood::Blocked,
        "sad" => Mood::Sad,
        "sleepy" => Mood::Sleepy,
        other => panic!("unknown mood {other:?}"),
    }
}

/// Every golden tree: its name without extension and the tree.
fn trees() -> Vec<(String, avatar_svg::RenderTree)> {
    let mut out = Vec::new();
    for row in rows() {
        assert_eq!(row.version, 1, "creature.sprouts has only version 1");
        let bytes = genome(&row.project_id);
        for name in &row.moods {
            out.push((
                format!("{}__{name}", row.project_id),
                render_tree(&bytes, mood(name)),
            ));
        }
    }
    out
}

fn manifest_bytes(producer_sha: &str) -> Vec<u8> {
    let vectors: Vec<String> = rows()
        .iter()
        .map(|row| {
            let moods: Vec<String> = row.moods.iter().map(|m| format!("\"{m}\"")).collect();
            format!(
                "{{\"project_id\":\"{}\",\"version\":{},\"moods\":[{}]}}",
                row.project_id,
                row.version,
                moods.join(",")
            )
        })
        .collect();
    format!(
        "{{\"genome_bytes\":{GENOME_LEN},\"type\":\"{TYPE}\",\"producer_sha\":\"{producer_sha}\",\"vectors\":[{}]}}",
        vectors.join(",")
    )
    .into_bytes()
}

fn assert_git_sha(sha: &str) {
    assert!(
        sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
        "producer_sha must be a 40-character git SHA, not {sha:?}"
    );
}

fn update_golden() -> bool {
    std::env::var_os("UPDATE_GOLDEN").is_some()
}

fn entry_names(path: &Path) -> BTreeSet<String> {
    fs::read_dir(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .map(|entry| {
            entry
                .expect("read directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 file name")
        })
        .collect()
}

fn assert_same(path: &Path, expected: &[u8]) {
    let actual = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    if actual == expected {
        return;
    }
    let offset = expected
        .iter()
        .zip(&actual)
        .position(|(a, b)| a != b)
        .unwrap_or(expected.len().min(actual.len()));
    panic!(
        "{} differs from what the generator draws, first at byte {offset}. Changing what a \
         shipped version draws is a new version of the type (docs/design/012-version-and-type-procedure.md); \
         an intended change to an unshipped version is minted again with UPDATE_GOLDEN=1",
        path.display()
    );
}

#[test]
fn corpus_matches_committed_files() {
    if update_golden() {
        return;
    }
    let root = stem_root();
    let manifest = fs::read_to_string(root.join("manifest.json")).expect("read manifest.json");
    let sha = manifest
        .split("\"producer_sha\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("manifest names its producer");
    assert_git_sha(sha);
    assert_same(&root.join("manifest.json"), &manifest_bytes(sha));

    let trees = trees();
    assert_eq!(trees.len(), 18, "the corpus is eighteen trees");
    let names: BTreeSet<String> = trees.iter().map(|(name, _)| name.clone()).collect();
    let pinned: BTreeSet<String> = TREE_PINS
        .iter()
        .map(|(name, _, _)| name.trim_end_matches(".json").to_owned())
        .collect();
    assert_eq!(pinned, names, "the pins name exactly the corpus trees");
    for (name, tree) in &trees {
        let json = serialize_tree_json(tree);
        let &(_, size, hash) = TREE_PINS
            .iter()
            .find(|(pin, _, _)| *pin == format!("{name}.json"))
            .expect("every tree is pinned");
        assert_eq!(json.len(), size, "{name}: pinned size");
        assert_eq!(
            blake3::hash(json.as_bytes()).to_hex().as_str(),
            hash,
            "{name}: pinned hash"
        );
        assert_same(
            &root.join("tree").join(format!("{name}.json")),
            json.as_bytes(),
        );
        assert_same(
            &root.join("tree-svg").join(format!("{name}.svg")),
            &serialize_tree(tree),
        );
    }

    // The directory holds exactly the corpus and nothing else.
    let expected_root: BTreeSet<String> = ["corpus.tsv", "manifest.json", "tree", "tree-svg"]
        .map(str::to_owned)
        .into();
    assert_eq!(entry_names(&root), expected_root, "vectors/sprouts entries");
    let json_names: BTreeSet<String> = names.iter().map(|n| format!("{n}.json")).collect();
    assert_eq!(
        entry_names(&root.join("tree")),
        json_names,
        "vectors/sprouts/tree entries"
    );
    let svg_names: BTreeSet<String> = names.iter().map(|n| format!("{n}.svg")).collect();
    assert_eq!(
        entry_names(&root.join("tree-svg")),
        svg_names,
        "vectors/sprouts/tree-svg entries"
    );
}

#[test]
fn only_the_ground_shadow_carries_its_role() {
    fn roles(primitives: &[Primitive], out: &mut Vec<(Role, String)>) {
        for primitive in primitives {
            if let Some(role) = primitive.role {
                out.push((role, primitive.label.clone()));
            }
            for effect in &primitive.effects {
                if let Effect::Masked { mask } = effect {
                    roles(mask, out);
                }
            }
        }
    }
    // The reference draws one ground shadow in every tree, and this is it.
    for (name, tree) in trees() {
        let mut found = Vec::new();
        roles(&tree.primitives, &mut found);
        assert_eq!(
            found,
            [(Role::GroundShadow, "sp-shadow".to_owned())],
            "{name}: exactly the shadow carries the ground-shadow role"
        );
        let shadows = tree
            .primitives
            .iter()
            .filter(|p| p.label == "sp-shadow")
            .count();
        assert_eq!(shadows, 1, "{name}: one shadow primitive");
        assert_eq!(
            tree.primitives[0].role,
            Some(Role::GroundShadow),
            "{name}: the shadow is drawn first, under everything"
        );
    }
}

/// Writes the golden corpus and prints the pins. Run with `UPDATE_GOLDEN=1`;
/// `PRODUCER_SHA` names the commit the trees were drawn from, or the current
/// `HEAD` is used.
#[test]
#[ignore = "rewrites vectors/sprouts; run with UPDATE_GOLDEN=1 to mint"]
fn mint() {
    assert!(
        update_golden(),
        "minting rewrites vectors/sprouts: set UPDATE_GOLDEN=1"
    );
    let root = stem_root();
    let sha = std::env::var("PRODUCER_SHA").unwrap_or_else(|_| {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&root)
            .output()
            .expect("run git rev-parse HEAD");
        String::from_utf8(output.stdout)
            .expect("git prints UTF-8")
            .trim()
            .to_owned()
    });
    assert_git_sha(&sha);
    for directory in ["tree", "tree-svg"] {
        let path = root.join(directory);
        if path.exists() {
            fs::remove_dir_all(&path).expect("clear an old corpus directory");
        }
        fs::create_dir_all(&path).expect("create a corpus directory");
    }
    fs::write(root.join("manifest.json"), manifest_bytes(&sha)).expect("write manifest.json");
    println!("const TREE_PINS: &[(&str, usize, &str)] = &[");
    for (name, tree) in trees() {
        let json = serialize_tree_json(&tree);
        fs::write(root.join("tree").join(format!("{name}.json")), &json).expect("write tree");
        fs::write(
            root.join("tree-svg").join(format!("{name}.svg")),
            serialize_tree(&tree),
        )
        .expect("write tree SVG");
        println!(
            "    ({:?}, {}, {:?}),",
            format!("{name}.json"),
            json.len(),
            blake3::hash(json.as_bytes()).to_hex().as_str()
        );
    }
    println!("];");
}
