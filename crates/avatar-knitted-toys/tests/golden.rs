//! The committed golden corpus in `vectors/knitted-toys/`.
//!
//! The corpus is the eighteen trees `corpus.tsv` lists: twelve genomes in
//! neutral and one genome in all seven moods, read from the committed
//! `vectors/wild/<id>.hex`. Each tree is committed as canonical tree JSON
//! (`tree/<id>__<mood>.json`) and as generic SVG
//! (`tree-svg/<id>__<mood>.svg`), and `manifest.json` lists them with their
//! sizes and BLAKE3 hashes.
//!
//! `golden_corpus_matches` redraws every tree and checks the directory's
//! exact contents. To regenerate the files after an intended change, run
//! `UPDATE_GOLDEN=1 cargo test -p cortexkit-avatar-knitted-toys --test golden mint -- --ignored`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use avatar_knitted_toys::render_tree;
use avatar_svg::{serialize_tree, serialize_tree_json, Mood, RenderTree, Role, Shape};
use serde_json::{json, Value};

const TYPE: &str = "creature.knitted-toys";

fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

fn corpus_dir() -> PathBuf {
    vectors().join("knitted-toys")
}

fn update_golden() -> bool {
    std::env::var_os("UPDATE_GOLDEN").is_some_and(|v| !v.is_empty() && v != "0")
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
        other => panic!("corpus.tsv names an unknown mood {other:?}"),
    }
}

/// One tree of the corpus.
struct Case {
    project_id: String,
    mood: String,
    version: u32,
}

impl Case {
    fn name(&self) -> String {
        format!("{}__{}", self.project_id, self.mood)
    }
}

/// The corpus rows: `project_id TAB moods TAB version`, `#` lines skipped.
fn cases() -> Vec<Case> {
    let text = fs::read_to_string(corpus_dir().join("corpus.tsv")).expect("read corpus.tsv");
    let mut out = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 3, "corpus.tsv row {line:?}");
        let version: u32 = fields[2].parse().expect("corpus.tsv version");
        for m in fields[1].split(',') {
            out.push(Case {
                project_id: fields[0].to_owned(),
                mood: m.to_owned(),
                version,
            });
        }
    }
    out
}

fn genome(project_id: &str) -> Box<[u8; 16_384]> {
    let path = vectors().join("wild").join(format!("{project_id}.hex"));
    let hex = fs::read_to_string(&path).expect("read the corpus genome");
    let hex = hex.trim();
    assert_eq!(hex.len(), 32_768, "{} is not 16,384 bytes", path.display());
    let mut out = Box::new([0_u8; 16_384]);
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("genome hex");
    }
    out
}

/// Every corpus tree with its canonical JSON and SVG.
fn render_corpus() -> Vec<(Case, RenderTree, String, Vec<u8>)> {
    cases()
        .into_iter()
        .map(|case| {
            assert_eq!(case.version, 1, "knitted toys have only version 1");
            let tree = render_tree(&genome(&case.project_id), mood(&case.mood), None);
            let json = serialize_tree_json(&tree);
            let svg = serialize_tree(&tree);
            (case, tree, json, svg)
        })
        .collect()
}

fn manifest(corpus: &[(Case, RenderTree, String, Vec<u8>)]) -> String {
    let trees: Vec<Value> = corpus
        .iter()
        .map(|(case, _, json, svg)| {
            json!({
                "project_id": case.project_id,
                "mood": case.mood,
                "version": case.version,
                "tree": format!("tree/{}.json", case.name()),
                "tree_bytes": json.len(),
                "tree_blake3": blake3::hash(json.as_bytes()).to_hex().as_str(),
                "svg": format!("tree-svg/{}.svg", case.name()),
                "svg_bytes": svg.len(),
                "svg_blake3": blake3::hash(svg).to_hex().as_str(),
            })
        })
        .collect();
    let doc = json!({
        "type": TYPE,
        "genome_bytes": 16_384,
        "trees": trees,
    });
    serde_json::to_string_pretty(&doc).expect("manifest serializes") + "\n"
}

fn entry_names(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 file name")
        })
        .collect()
}

#[test]
#[ignore = "writes the golden corpus; run with UPDATE_GOLDEN=1"]
fn mint() {
    assert!(update_golden(), "set UPDATE_GOLDEN=1 to mint");
    let root = corpus_dir();
    for sub in ["tree", "tree-svg"] {
        let dir = root.join(sub);
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clear the old trees");
        }
        fs::create_dir_all(&dir).expect("create the tree directory");
    }
    let corpus = render_corpus();
    for (case, _, json, svg) in &corpus {
        fs::write(
            root.join("tree").join(format!("{}.json", case.name())),
            json,
        )
        .expect("write tree JSON");
        fs::write(
            root.join("tree-svg").join(format!("{}.svg", case.name())),
            svg,
        )
        .expect("write tree SVG");
    }
    fs::write(root.join("manifest.json"), manifest(&corpus)).expect("write manifest");
    println!("minted {} knitted-toys trees", corpus.len());
}

#[test]
fn golden_corpus_matches() {
    if update_golden() {
        return;
    }
    let root = corpus_dir();
    let corpus = render_corpus();
    assert_eq!(corpus.len(), 18, "the corpus is 18 trees");
    assert_eq!(
        entry_names(&root),
        BTreeSet::from([
            "corpus.tsv".to_owned(),
            "manifest.json".to_owned(),
            "tree".to_owned(),
            "tree-svg".to_owned(),
        ]),
        "vectors/knitted-toys holds exactly the corpus, its manifest and its trees"
    );
    let names: BTreeSet<String> = corpus.iter().map(|(case, ..)| case.name()).collect();
    assert_eq!(
        entry_names(&root.join("tree")),
        names.iter().map(|n| format!("{n}.json")).collect(),
        "vectors/knitted-toys/tree holds exactly the corpus trees"
    );
    assert_eq!(
        entry_names(&root.join("tree-svg")),
        names.iter().map(|n| format!("{n}.svg")).collect(),
        "vectors/knitted-toys/tree-svg holds exactly the corpus trees"
    );
    for (case, _, json, svg) in &corpus {
        let name = case.name();
        let committed = fs::read_to_string(root.join("tree").join(format!("{name}.json")))
            .expect("read committed tree JSON");
        assert!(
            committed == *json,
            "tree/{name}.json differs from the drawing"
        );
        let committed = fs::read(root.join("tree-svg").join(format!("{name}.svg")))
            .expect("read committed tree SVG");
        assert!(
            committed == *svg,
            "tree-svg/{name}.svg differs from the drawing"
        );
    }
    let committed =
        fs::read_to_string(root.join("manifest.json")).expect("read committed manifest");
    assert_eq!(committed, manifest(&corpus), "manifest.json is stale");
}

/// The reference draws one ground shadow under every toy, in every mood: a
/// soft ellipse on the ground line, drawn first. Exactly that primitive
/// carries the ground-shadow role; the drop shadows and the contact shadow
/// under a head are shading on the toy itself and carry none.
#[test]
fn only_the_ground_shadow_has_the_ground_shadow_role() {
    let mut other_shadows = 0;
    for (case, tree, ..) in render_corpus() {
        let tagged: Vec<usize> = tree
            .primitives
            .iter()
            .enumerate()
            .filter(|(_, p)| p.role == Some(Role::GroundShadow))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            tagged,
            [0],
            "{}: the ground shadow is primitive 0",
            case.name()
        );
        let shadow = &tree.primitives[0];
        assert!(
            matches!(shadow.shape, Shape::Ellipse { .. }) && shadow.label == "ground-shadow",
            "{}: the tagged primitive is the ground-shadow ellipse",
            case.name()
        );
        other_shadows += tree.primitives[1..]
            .iter()
            .filter(|p| p.label.contains("shadow"))
            .count();
    }
    // The corpus has shadows that look like the ground shadow but are not
    // it, so the check above has something to tell apart.
    assert!(
        other_shadows > 0,
        "the corpus draws drop or contact shadows"
    );
}

/// Smaller sizes leave detail out: stitches from 100 px, rows from 56 px.
#[test]
fn detail_follows_the_size_drawn_for() {
    let bytes = genome("wild-port-0005");
    let count = |px: Option<u32>, label: &str| {
        render_tree(&bytes, Mood::Neutral, px)
            .primitives
            .iter()
            .filter(|p| p.label == label)
            .count()
    };
    assert!(count(None, "stitches") > 0);
    assert_eq!(count(None, "stitches rows"), 0);
    assert_eq!(count(Some(96), "stitches"), 0);
    assert!(count(Some(96), "stitches rows") > 0);
    assert_eq!(count(Some(48), "stitches"), 0);
    assert_eq!(count(Some(48), "stitches rows"), 0);
    assert_eq!(
        render_tree(&bytes, Mood::Neutral, None),
        render_tree(&bytes, Mood::Neutral, Some(256))
    );
}
