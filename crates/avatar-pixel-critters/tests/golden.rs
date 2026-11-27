//! The committed golden corpus in `vectors/pixel-critters/`: the twelve
//! wild corpus genomes at version 1 in neutral, plus all seven moods of
//! `wild-port-0005`, as tree JSON and tree SVG, with a manifest that pins
//! each file's size and BLAKE3 hash.
//!
//! `golden_corpus_matches_generator` checks the directory's exact contents.
//! To regenerate after an intended change (which for a shipped type is a
//! new version, never an edit of version 1), run
//! `UPDATE_GOLDEN=1 cargo test -p cortexkit-avatar-pixel-critters --test golden -- --ignored mint`
//! in a checkout, not on a build server, and review the trees.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use avatar_pixel_critters::{render_tree, GENOME_LEN, TYPE, VERSION};
use avatar_svg::{serialize_tree, serialize_tree_json, Mood, RenderTree, Shape};

/// The corpus rows: project id, moods and version.
const CORPUS: &str = include_str!("../../../vectors/pixel-critters/corpus.tsv");

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
                version: version
                    .parse()
                    .unwrap_or_else(|_| panic!("corpus row has no version: {line:?}")),
            }
        })
        .collect()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden_root() -> PathBuf {
    repo_root().join("vectors/pixel-critters")
}

fn parse_mood(name: &str) -> Mood {
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

/// Reads a committed wild genome: 32,768 hex digits.
fn read_genome(project_id: &str) -> [u8; GENOME_LEN] {
    let path = repo_root().join(format!("vectors/wild/{project_id}.hex"));
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let text = text.trim();
    assert_eq!(text.len(), 2 * GENOME_LEN, "{project_id}: genome length");
    let mut genome = [0u8; GENOME_LEN];
    for (i, byte) in genome.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .unwrap_or_else(|e| panic!("{project_id}: byte {i}: {e}"));
    }
    genome
}

/// One golden tree: its file stem and its two serializations.
struct Golden {
    name: String,
    tree: RenderTree,
    json: Vec<u8>,
    svg: Vec<u8>,
}

fn goldens() -> Vec<Golden> {
    let mut out = Vec::new();
    for row in rows() {
        assert_eq!(row.version, VERSION, "{}: corpus version", row.project_id);
        let genome = read_genome(&row.project_id);
        for mood in &row.moods {
            let tree = render_tree(&genome, parse_mood(mood));
            out.push(Golden {
                name: format!("{}__{mood}", row.project_id),
                json: serialize_tree_json(&tree).into_bytes(),
                svg: serialize_tree(&tree),
                tree,
            });
        }
    }
    out
}

/// The manifest, written by hand so its bytes never depend on a JSON
/// library's key order. Every string in it is an identifier or hex.
fn manifest_bytes(producer_sha: &str, goldens: &[Golden]) -> Vec<u8> {
    let mut out = format!(
        "{{\n  \"type\": \"{TYPE}\",\n  \"version\": {VERSION},\n  \"genome_bytes\": {GENOME_LEN},\n  \"genomes\": \"vectors/wild/<project_id>.hex\",\n  \"producer_sha\": \"{producer_sha}\",\n  \"trees\": ["
    );
    for (i, g) in goldens.iter().enumerate() {
        out.push_str(if i == 0 { "\n" } else { ",\n" });
        out.push_str(&format!(
            "    {{\"name\": \"{}\", \"json_bytes\": {}, \"json_blake3\": \"{}\", \"svg_bytes\": {}, \"svg_blake3\": \"{}\"}}",
            g.name,
            g.json.len(),
            blake3::hash(&g.json).to_hex(),
            g.svg.len(),
            blake3::hash(&g.svg).to_hex(),
        ));
    }
    out.push_str("\n  ]\n}\n");
    out.into_bytes()
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

fn assert_bytes(path: &Path, expected: &[u8]) {
    let actual = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    if actual != expected {
        let offset = expected
            .iter()
            .zip(&actual)
            .position(|(a, b)| a != b)
            .unwrap_or(expected.len().min(actual.len()));
        panic!(
            "golden drift in {}: first differing byte {offset}. Changing what a shipped version draws is a new version (docs/design/012-version-and-type-procedure.md); regenerate only for a new type with UPDATE_GOLDEN=1 and the ignored mint test.",
            path.display()
        );
    }
}

/// The producer SHA recorded in the committed manifest.
fn recorded_sha(manifest: &str) -> String {
    let start = manifest
        .find("\"producer_sha\": \"")
        .expect("manifest records a producer_sha")
        + "\"producer_sha\": \"".len();
    let sha = &manifest[start..start + 40];
    assert!(
        sha.bytes().all(|b| b.is_ascii_hexdigit()),
        "producer_sha is a 40-digit git SHA"
    );
    sha.to_owned()
}

#[test]
fn golden_corpus_matches_generator() {
    if update_golden() {
        return;
    }
    let root = golden_root();
    let goldens = goldens();
    assert_eq!(
        goldens.len(),
        18,
        "twelve genomes in neutral plus six more moods"
    );
    let manifest = fs::read_to_string(root.join("manifest.json")).expect("read manifest.json");
    let sha = recorded_sha(&manifest);
    assert_bytes(&root.join("manifest.json"), &manifest_bytes(&sha, &goldens));
    for g in &goldens {
        assert_bytes(&root.join("tree").join(format!("{}.json", g.name)), &g.json);
        assert_bytes(
            &root.join("tree-svg").join(format!("{}.svg", g.name)),
            &g.svg,
        );
    }
    // Exactly these files, and nothing else.
    let expect = |suffix: &str| -> BTreeSet<String> {
        goldens
            .iter()
            .map(|g| format!("{}.{suffix}", g.name))
            .collect()
    };
    assert_eq!(
        entry_names(&root),
        ["corpus.tsv", "manifest.json", "tree", "tree-svg"]
            .map(str::to_owned)
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(entry_names(&root.join("tree")), expect("json"));
    assert_eq!(entry_names(&root.join("tree-svg")), expect("svg"));
}

#[test]
fn golden_trees_are_whole_cells_on_the_grid_and_carry_no_role() {
    for g in goldens() {
        let tree = &g.tree;
        assert_eq!(tree.canvas, 24, "{}", g.name);
        assert_eq!(tree.pixel_grid, Some(24), "{}", g.name);
        assert_eq!(
            tree.extent(),
            2_400,
            "{}: a cell is 100 coordinates",
            g.name
        );
        assert!(tree.background.is_none(), "{}", g.name);
        assert!(!tree.primitives.is_empty(), "{}", g.name);
        for p in &tree.primitives {
            // The reference draws no ground shadow, so no primitive may
            // claim to be one.
            assert_eq!(p.role, None, "{}: {}", g.name, p.label);
            let Shape::Rect { x, y, w, h } = p.shape else {
                panic!("{}: {} is not a rectangle", g.name, p.label);
            };
            assert!(
                [x, y, w, h].iter().all(|v| v % 100 == 0),
                "{}: {} is not whole cells",
                g.name,
                p.label
            );
            assert!(x >= 0 && y >= 0 && w > 0 && h > 0 && x + w <= 2_400 && y + h <= 2_400);
        }
    }
}

#[test]
#[ignore = "writes vectors/pixel-critters; run with UPDATE_GOLDEN=1 in a checkout"]
fn mint() {
    assert!(
        update_golden(),
        "set UPDATE_GOLDEN=1 to rewrite vectors/pixel-critters"
    );
    let root = golden_root();
    let goldens = goldens();
    for dir in ["tree", "tree-svg"] {
        let path = root.join(dir);
        if path.exists() {
            fs::remove_dir_all(&path).expect("clear the old golden files");
        }
        fs::create_dir_all(&path).expect("create golden directory");
    }
    for g in &goldens {
        fs::write(root.join("tree").join(format!("{}.json", g.name)), &g.json)
            .expect("write tree JSON");
        fs::write(
            root.join("tree-svg").join(format!("{}.svg", g.name)),
            &g.svg,
        )
        .expect("write tree SVG");
    }
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root())
        .output()
        .expect("run git rev-parse HEAD");
    assert!(output.status.success(), "git rev-parse HEAD failed");
    let sha = String::from_utf8(output.stdout).expect("UTF-8 SHA");
    fs::write(
        root.join("manifest.json"),
        manifest_bytes(sha.trim(), &goldens),
    )
    .expect("write manifest");
    println!("minted {} pixel-critters trees", goldens.len());
}
