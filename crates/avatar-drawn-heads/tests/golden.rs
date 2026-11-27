//! The committed golden corpus of `creature.drawn-heads` in
//! `vectors/drawn-heads/`.
//!
//! `corpus.tsv` lists the genomes and moods: twelve wild genomes in neutral
//! and all seven moods of the first, read from `vectors/wild/<id>.hex`. For
//! each tree the directory holds its canonical JSON (`tree/`) and its generic
//! SVG (`tree-svg/`), and `manifest.json` pins each file's size and BLAKE3
//! hash. This test checks that the directory holds exactly those files and
//! that each equals what the generator draws today.
//!
//! To mint after an intended change, run with `UPDATE_GOLDEN=1`; the files
//! are rewritten and then checked.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use avatar_drawn_heads::{mood_name, render_tree, CREATURE_TYPE, GENOME_LEN, MOODS};
use avatar_svg::{serialize_tree, serialize_tree_json, Mood};

fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

fn root() -> PathBuf {
    vectors().join("drawn-heads")
}

/// One row of `corpus.tsv`.
struct Row {
    project_id: String,
    moods: Vec<Mood>,
    version: u32,
}

fn corpus() -> Vec<Row> {
    let text = fs::read_to_string(root().join("corpus.tsv")).expect("read corpus.tsv");
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let [project_id, moods, version] = fields[..] else {
                panic!("corpus row is not project_id, moods and version: {line:?}")
            };
            Row {
                project_id: project_id.to_owned(),
                moods: moods
                    .split(',')
                    .map(|name| {
                        *MOODS
                            .iter()
                            .find(|m| mood_name(**m) == name)
                            .unwrap_or_else(|| panic!("unknown mood {name:?}"))
                    })
                    .collect(),
                version: version.parse().expect("corpus row version"),
            }
        })
        .collect()
}

fn genome(project_id: &str) -> [u8; GENOME_LEN] {
    let hex = fs::read_to_string(vectors().join("wild").join(format!("{project_id}.hex")))
        .expect("read wild genome");
    assert_eq!(hex.len(), 2 * GENOME_LEN, "{project_id}.hex length");
    let mut out = [0u8; GENOME_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex genome");
    }
    out
}

/// Every golden file the generator produces today, by path under the root.
fn expected_files() -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut entries = Vec::new();
    for row in corpus() {
        // Version 1 is the only version of this type.
        assert_eq!(
            row.version, 1,
            "{} names a version other than 1",
            row.project_id
        );
        let bytes = genome(&row.project_id);
        for mood in row.moods {
            let name = format!("{}__{}", row.project_id, mood_name(mood));
            let tree = render_tree(&bytes, mood, None);
            let json = serialize_tree_json(&tree).into_bytes();
            let svg = serialize_tree(&tree);
            entries.push(format!(
                "    {{\"name\": \"{name}\", \"project_id\": \"{}\", \"mood\": \"{}\", \"tree_bytes\": {}, \"tree_blake3\": \"{}\", \"svg_bytes\": {}, \"svg_blake3\": \"{}\"}}",
                row.project_id,
                mood_name(mood),
                json.len(),
                blake3::hash(&json).to_hex(),
                svg.len(),
                blake3::hash(&svg).to_hex(),
            ));
            files.push((format!("tree/{name}.json"), json));
            files.push((format!("tree-svg/{name}.svg"), svg));
        }
    }
    let manifest = format!(
        "{{\n  \"type\": \"{CREATURE_TYPE}\",\n  \"version\": 1,\n  \"genome_bytes\": {GENOME_LEN},\n  \"genomes\": \"vectors/wild/<project_id>.hex\",\n  \"trees\": [\n{}\n  ]\n}}\n",
        entries.join(",\n")
    );
    files.push(("manifest.json".to_owned(), manifest.into_bytes()));
    files
}

fn names_in(dir: &Path) -> BTreeSet<String> {
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

#[test]
fn golden_corpus_matches_the_committed_vectors() {
    let files = expected_files();
    let root = root();
    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        for dir in ["tree", "tree-svg"] {
            let dir = root.join(dir);
            if dir.exists() {
                fs::remove_dir_all(&dir).expect("clear a golden directory");
            }
            fs::create_dir_all(&dir).expect("create a golden directory");
        }
        for (name, bytes) in &files {
            fs::write(root.join(name), bytes).expect("write a golden file");
        }
    }
    // The directory holds exactly the corpus list, the manifest and the trees.
    let top: BTreeSet<String> = ["corpus.tsv", "manifest.json", "tree", "tree-svg"]
        .map(str::to_owned)
        .into();
    assert_eq!(names_in(&root), top);
    for dir in ["tree", "tree-svg"] {
        let wanted: BTreeSet<String> = files
            .iter()
            .filter_map(|(name, _)| name.strip_prefix(&format!("{dir}/")).map(str::to_owned))
            .collect();
        assert_eq!(wanted.len(), 18, "the corpus draws 18 trees");
        assert_eq!(
            names_in(&root.join(dir)),
            wanted,
            "{dir}/ holds other files"
        );
    }
    for (name, bytes) in &files {
        let committed = fs::read(root.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
        assert!(
            committed == *bytes,
            "{name} differs from what the generator draws; mint with UPDATE_GOLDEN=1 only for an intended change"
        );
    }
}
