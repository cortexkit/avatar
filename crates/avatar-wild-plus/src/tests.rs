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
        "9035d6b226711432c7d58eb9c0c69fba9571469a40f72ba67b5ee1314e2966e4",
    ),
    (
        "wild-port-0005__working",
        489425,
        "a4a8b3679e17fb0d79c0fedbf73eb219b70788df106fd718497d3ffb45a04519",
    ),
    (
        "wild-port-0005__happy",
        501372,
        "390e5d7a79d495add76012832d79af2e0446a2b066256fa3bdb812ebc7c93a73",
    ),
    (
        "wild-port-0005__waiting",
        487382,
        "ac52c542c4f44f74b25fcfe373c1dbf64ab2d8e7f47fcf82b92f24db390f3cc2",
    ),
    (
        "wild-port-0005__blocked",
        499683,
        "e7fd21130f8466c6a51f8449d99680255d11028dc7e65cfdd37ec3b46de73e43",
    ),
    (
        "wild-port-0005__sad",
        498877,
        "30e785771d6000e4ad5a5a8c8174a90ca59803a82e5f1db1c17498766dbc4f1b",
    ),
    (
        "wild-port-0005__sleepy",
        489891,
        "d3dd8a068eb23bff4e42be00ccb63f268f47dcb72050fc89a93d33558646dca2",
    ),
    (
        "wild-port-0031__neutral",
        321786,
        "52c15b8f73260516fe9bf6cf45120898ba121b0b757856919fd3520c61e32d5f",
    ),
    (
        "wild-port-0009__neutral",
        195312,
        "640ee106eb23e7b908ab3959e9849efc4c9f519deac571d2f159174f5fabed21",
    ),
    (
        "wild-port-0022__neutral",
        507323,
        "7789f6972b775e84b8b326c9019b8f671ba961988fb1e32e4a27009ef48ba481",
    ),
    (
        "wild-port-0077__neutral",
        597519,
        "7163c5b7202916bd127c3dfd056b23bf2d87a31ff047658e70db860159c25874",
    ),
    (
        "wild-port-0043__neutral",
        155156,
        "cbf9986b7d8f6fb707cdb2eed01fd033ecc6cf5fffc4abd502aed2a5f3e929da",
    ),
    (
        "wild-port-0016__neutral",
        112814,
        "4bd2cebde2118914acc948ce531b044a2692a5d0bc6988f5c2a90dd5fda880ed",
    ),
    (
        "wild-port-0021__neutral",
        451988,
        "1888cb2f74a9ef3eb91907f85bce630a7def6bd26506ee12c6cde28eabd66949",
    ),
    (
        "wild-port-0000__neutral",
        158397,
        "f40cb51b11180a7f36b10d9ea1b5223ce7c9f68654cece9741c6420317d54a61",
    ),
    (
        "wild-port-0001__neutral",
        236571,
        "b577253e0c21c59431cdaf5681b3defddb8cf87f454fcab2c5a8d3ccf43c0294",
    ),
    (
        "wild-port-0002__neutral",
        202530,
        "da82802fe32158113d3f37ab6827de53bc068cad51a3e4ac4fa8a088470882e0",
    ),
    (
        "wild-port-0007__neutral",
        714941,
        "78a4b51ae94066e9b886b74c91ae63bb17229932bd5e67bb0ec2bc739100722f",
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
