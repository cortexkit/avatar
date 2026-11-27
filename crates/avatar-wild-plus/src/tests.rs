use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use avatar_svg::{serialize_tree, serialize_tree_json, Effect, Mood, Primitive, RenderTree, Role};
use serde_json::{json, Value};

use super::*;

/// The frozen list of genomes and moods the golden trees are drawn from.
const CORPUS: &str = include_str!("../../../vectors/wild-plus/corpus.tsv");

/// Each golden tree's name, JSON length and BLAKE3 hash, as `tests::mint`
/// printed them. A change to what the type draws shows up here as well as in
/// the files, so it cannot pass review as a refreshed fixture alone.
const TREE_PINS: &[(&str, usize, &str)] = &[
    (
        "wild-port-0005__neutral",
        483470,
        "2ba1c0154b72896cd0de05ab3c364fba3d75a205305b9ec1cf77615783c654b3",
    ),
    (
        "wild-port-0005__working",
        489425,
        "9cb4d640ce6c083024299755da62ba5d3f9340dd6a6641a5930ddf1196f7bed9",
    ),
    (
        "wild-port-0005__happy",
        501372,
        "aeea82b7db5e10146e14d29020709b18edaab78ed8407b0a7ed71e2da32c32ee",
    ),
    (
        "wild-port-0005__waiting",
        487382,
        "1425d0d5f0d2330faa6d69a3b50be60c79cf8cc46f68aa00a15139bb8c1c3f10",
    ),
    (
        "wild-port-0005__blocked",
        499683,
        "1d164599a000485d30e69b8c06b81b69a4d7b2cdcc057445b81f5623620bdf21",
    ),
    (
        "wild-port-0005__sad",
        498877,
        "fa4de6f1aebda9d4d58f0213d326ea0ae673819b57b11225bf968075e1a9b402",
    ),
    (
        "wild-port-0005__sleepy",
        489891,
        "38113a166a7bb0da1ee95cebfc8b38de27561d6ee062d83652681e0a97abd839",
    ),
    (
        "wild-port-0031__neutral",
        312217,
        "6c1dcd84af03326d515eb715e46bb4a29e31967c34f3b1385e50dbba4591438a",
    ),
    (
        "wild-port-0009__neutral",
        202331,
        "525e79d72abc32d92a56e391a09e150a7eaea39f3cb90acb29bc14120a01269c",
    ),
    (
        "wild-port-0022__neutral",
        507323,
        "7789f6972b775e84b8b326c9019b8f671ba961988fb1e32e4a27009ef48ba481",
    ),
    (
        "wild-port-0077__neutral",
        585466,
        "db655a8e983b16e703e43fe0227244fda860e9c11283df27cbd6c291390b1e3e",
    ),
    (
        "wild-port-0043__neutral",
        155156,
        "65d9dc6e12a067705c51f38a8ea29617c12f61fc9dabfe87c87f1e651419a776",
    ),
    (
        "wild-port-0016__neutral",
        112826,
        "86a72f4abdcb211d842994e8f2461832330fbbc50809d9a9f3cadbb2a00806d7",
    ),
    (
        "wild-port-0021__neutral",
        451988,
        "65d631e1ccde38bf2737531b4faf6a27e3f3edfb2e87e2a1954054880715e4dc",
    ),
    (
        "wild-port-0000__neutral",
        146337,
        "cea1bffdbb9db291e5b75275a8876706b49cbdda9a13035b36d6b67c41a2241e",
    ),
    (
        "wild-port-0001__neutral",
        236573,
        "57f7da2e7e2b57c70fb67d3d0976d1e0495de6c659742dab9655bae783a067ac",
    ),
    (
        "wild-port-0002__neutral",
        202530,
        "da82802fe32158113d3f37ab6827de53bc068cad51a3e4ac4fa8a088470882e0",
    ),
    (
        "wild-port-0007__neutral",
        698142,
        "95267378ee82c45209c4a188b6b08b585c919b44f8f9e38d6890160e153e2f41",
    ),
];

const MOODS: [(Mood, &str); 7] = [
    (Mood::Neutral, "neutral"),
    (Mood::Working, "working"),
    (Mood::Happy, "happy"),
    (Mood::Waiting, "waiting"),
    (Mood::Blocked, "blocked"),
    (Mood::Sad, "sad"),
    (Mood::Sleepy, "sleepy"),
];

fn vectors_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

fn type_root() -> PathBuf {
    vectors_root().join("wild-plus")
}

fn update_golden_enabled() -> bool {
    std::env::var_os("UPDATE_GOLDEN").is_some()
}

/// One corpus row: a wild genome and the moods its trees are kept in.
struct Row {
    project_id: String,
    moods: Vec<String>,
}

fn corpus() -> Vec<Row> {
    CORPUS
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let [project_id, moods, version] = fields[..] else {
                panic!("corpus row is not project_id, moods and version: {line:?}")
            };
            assert_eq!(version, "1", "creature.wild-plus has only version 1");
            Row {
                project_id: project_id.to_owned(),
                moods: moods.split(',').map(str::to_owned).collect(),
            }
        })
        .collect()
}

fn mood_of(name: &str) -> Mood {
    MOODS
        .iter()
        .find(|(_, n)| *n == name)
        .unwrap_or_else(|| panic!("unknown mood {name}"))
        .0
}

/// Genomes are read from the wild corpus, not copied.
fn read_genome(project_id: &str) -> [u8; avatar_svg::wild::GENOME_LEN] {
    let path = vectors_root()
        .join("wild")
        .join(format!("{project_id}.hex"));
    let hex = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert_eq!(
        hex.len(),
        2 * avatar_svg::wild::GENOME_LEN,
        "{project_id} genome length"
    );
    let mut genome = [0; avatar_svg::wild::GENOME_LEN];
    for (i, byte) in genome.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("genome hex");
    }
    genome
}

/// Every golden tree: its file stem and the tree drawn now.
fn golden_trees() -> Vec<(String, RenderTree)> {
    let mut out = Vec::new();
    for row in corpus() {
        let genome = read_genome(&row.project_id);
        for mood in &row.moods {
            out.push((
                format!("{}__{mood}", row.project_id),
                render_tree(&genome, mood_of(mood), None),
            ));
        }
    }
    out
}

fn manifest_bytes(trees: &[(String, RenderTree)]) -> Vec<u8> {
    let entries: Vec<Value> = trees
        .iter()
        .map(|(name, tree)| {
            let (project_id, mood) = name.split_once("__").expect("tree name");
            json!({
                "name": name,
                "project_id": project_id,
                "mood": mood,
                "json_blake3": blake3::hash(serialize_tree_json(tree).as_bytes()).to_hex().as_str(),
                "svg_blake3": blake3::hash(&serialize_tree(tree)).to_hex().as_str(),
            })
        })
        .collect();
    let mut bytes = serde_json::to_vec_pretty(&json!({
        "type": "creature.wild-plus",
        "version": 1,
        "genome_bytes": avatar_svg::wild::GENOME_LEN,
        "genomes_from": "vectors/wild/<project_id>.hex",
        "trees": entries,
    }))
    .expect("manifest JSON");
    bytes.push(b'\n');
    bytes
}

fn entry_names(path: &Path) -> BTreeSet<String> {
    fs::read_dir(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 name")
        })
        .collect()
}

fn assert_current(path: &Path, expected: &[u8]) {
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
        "golden drift in {}: first differing byte {offset}. A version 1 creature must keep \
         drawing what it drew when it shipped; a change to its drawing is a new version \
         (docs/design/012-version-and-type-procedure.md). Regenerate only for an intended \
         change, with UPDATE_GOLDEN=1 cargo test -p cortexkit-avatar-wild-plus \
         tests::mint -- --exact --nocapture --ignored",
        path.display()
    );
}

/// Writes `vectors/wild-plus/`: the manifest and every golden tree as JSON
/// and as SVG, and prints the pins for `TREE_PINS`.
#[test]
#[ignore = "writes the golden corpus; run with UPDATE_GOLDEN=1"]
fn mint() {
    assert!(update_golden_enabled(), "set UPDATE_GOLDEN=1 to mint");
    let root = type_root();
    for dir in ["tree", "tree-svg"] {
        let path = root.join(dir);
        if path.exists() {
            fs::remove_dir_all(&path).expect("clear old trees");
        }
        fs::create_dir_all(&path).expect("create tree directory");
    }
    let trees = golden_trees();
    println!("const TREE_PINS: &[(&str, usize, &str)] = &[");
    for (name, tree) in &trees {
        let json = serialize_tree_json(tree);
        fs::write(
            root.join("tree").join(format!("{name}.json")),
            json.as_bytes(),
        )
        .expect("write tree");
        fs::write(
            root.join("tree-svg").join(format!("{name}.svg")),
            serialize_tree(tree),
        )
        .expect("write SVG");
        println!(
            "    ({name:?}, {}, {:?}),",
            json.len(),
            blake3::hash(json.as_bytes()).to_hex().as_str()
        );
    }
    println!("];");
    fs::write(root.join("manifest.json"), manifest_bytes(&trees)).expect("write manifest");
}

#[test]
fn golden_directory_holds_exactly_the_corpus_trees() {
    if update_golden_enabled() {
        return;
    }
    let root = type_root();
    let trees = golden_trees();
    assert_eq!(
        trees.len(),
        18,
        "twelve genomes in neutral plus six more moods of one"
    );
    let expected_root: BTreeSet<String> = ["corpus.tsv", "manifest.json", "tree", "tree-svg"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        entry_names(&root),
        expected_root,
        "vectors/wild-plus holds other entries"
    );
    for (dir, ext) in [("tree", "json"), ("tree-svg", "svg")] {
        let expected: BTreeSet<String> = trees.iter().map(|(n, _)| format!("{n}.{ext}")).collect();
        assert_eq!(
            entry_names(&root.join(dir)),
            expected,
            "vectors/wild-plus/{dir} entries"
        );
    }
    assert_current(&root.join("manifest.json"), &manifest_bytes(&trees));
    let pinned: Vec<&str> = TREE_PINS.iter().map(|(n, _, _)| *n).collect();
    let names: Vec<&str> = trees.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(pinned, names, "pins name every golden tree in corpus order");
    for ((name, tree), (_, len, hash)) in trees.iter().zip(TREE_PINS) {
        let json = serialize_tree_json(tree);
        assert_current(
            &root.join("tree").join(format!("{name}.json")),
            json.as_bytes(),
        );
        assert_current(
            &root.join("tree-svg").join(format!("{name}.svg")),
            &serialize_tree(tree),
        );
        assert_eq!(
            (json.len(), blake3::hash(json.as_bytes()).to_hex().as_str()),
            (*len, *hash),
            "{name} pin"
        );
    }
}

/// The primitives that carry a role, masks included, with their labels.
fn roles(tree: &RenderTree) -> Vec<(Role, String)> {
    let mut pending: Vec<&Primitive> = tree.primitives.iter().collect();
    let mut found = Vec::new();
    while let Some(p) = pending.pop() {
        if let Some(role) = p.role {
            found.push((role, p.label.clone()));
        }
        for effect in &p.effects {
            if let Effect::Masked { mask } = effect {
                pending.extend(mask);
            }
        }
    }
    found
}

#[test]
fn only_the_ground_shadow_carries_the_ground_shadow_role() {
    let shadow = vec![(Role::GroundShadow, "wp-shadow".to_owned())];
    let mut count = 0;
    for row in corpus() {
        let genome = read_genome(&row.project_id);
        for (mood, name) in MOODS {
            let tree = render_tree(&genome, mood, None);
            assert_eq!(roles(&tree), shadow, "{}__{name}", row.project_id);
            // The shadow is the soft ellipse under the body, drawn first.
            let first = &tree.primitives[0];
            assert!(matches!(first.shape, avatar_svg::Shape::Ellipse { .. }));
            assert_eq!(first.role, Some(Role::GroundShadow));
            count += 1;
        }
    }
    assert_eq!(count, 84);
}

#[test]
fn trees_are_drawn_on_the_wild_canvas_and_ignore_the_size() {
    let genome = read_genome("wild-port-0005");
    let tree = render_tree(&genome, Mood::Happy, None);
    assert_eq!(
        (tree.canvas, tree.background, tree.pixel_grid),
        (240, None, None)
    );
    for size in [48, 96, 256] {
        assert_eq!(render_tree(&genome, Mood::Happy, Some(size)), tree);
    }
}

#[test]
fn decode_reports_the_creature_the_tree_draws() {
    let c = decode(&read_genome("wild-port-0005"));
    assert!(PLANS.contains(&c.plan));
    assert!(COATS.contains(&c.coat));
    assert!(SURFACES.contains(&c.surface) && SURFACES.contains(&c.surface_second));
    assert!(EYE_PLANS.contains(&c.eye_plan) && EYE_STYLES.contains(&c.eye_style));
    assert!(MOUTH_PLANS.contains(&c.mouth));
    assert!([0, 2, 3, 4].contains(&c.recipe));
    assert!(c.scheme < SCHEMES.len() && c.nose <= 5);
    let tree = render_tree(&read_genome("wild-port-0005"), Mood::Neutral, None);
    let body = tree
        .primitives
        .iter()
        .find(|p| p.label == "wp-body")
        .expect("a body primitive");
    assert_eq!(body.paint, avatar_svg::Paint::Solid { color: c.body });
}
