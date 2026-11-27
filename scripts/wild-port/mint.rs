//! Mint every type's golden corpus trees through the `avatar` crate's public
//! dispatch, into a scratch directory, without touching the committed goldens.
//!
//! The output holds one directory per type, named by the type's stem: `wild`
//! for `creature.wild`, and `wild-plus` and so on for the six newer types.
//! The corpora pin the same file names (every type pins
//! `wild-port-0005__neutral.json`), so each type's trees stay in its own
//! directory.
//!
//! scripts/wild-cross-platform.sh compiles and runs this natively and inside
//! Linux containers, then compares the outputs byte for byte.

use avatar::{
    render_tree_for_bytes, serialize_tree_json, Mood, DRAWN_HEADS, KNITTED_TOYS, PIXEL_CRITTERS,
    SMALL_ROBOTS, SPROUTS, WILD, WILD_PLUS,
};
use std::{collections::BTreeSet, fs, path::Path};

/// Each type with the number of corpus rows and trees its corpus list,
/// `vectors/<stem>/corpus.tsv`, must hold. Wild pins 12 genomes at version 1
/// and one at version 2; each newer type pins 12 genomes at version 1, one
/// of them in all seven moods.
const TYPES: [(&str, usize, usize); 7] = [
    (WILD, 13, 25),
    (WILD_PLUS, 12, 18),
    (SPROUTS, 12, 18),
    (PIXEL_CRITTERS, 12, 18),
    (KNITTED_TOYS, 12, 18),
    (SMALL_ROBOTS, 12, 18),
    (DRAWN_HEADS, 12, 18),
];

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: wild-mint REPOSITORY OUTPUT_DIRECTORY"
    );
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).expect("output must be a new directory");
    let mut total = 0;
    for (avatar_type, expected_rows, expected_trees) in TYPES {
        let stem = avatar_type
            .strip_prefix("creature.")
            .expect("type names start with creature.");
        let directory = out.join(stem);
        fs::create_dir(&directory).unwrap();
        let (rows, trees) = mint_type(root, avatar_type, stem, &directory);
        assert_eq!(
            (rows, trees),
            (expected_rows, expected_trees),
            "the {stem} corpus is incomplete"
        );
        println!("{stem}: minted {trees} trees from {rows} corpus rows");
        total += trees;
    }
    println!("minted {total} trees for {} types", TYPES.len());
}

/// Mints one type's corpus into `directory`. Returns the number of corpus
/// rows and trees.
fn mint_type(root: &Path, avatar_type: &str, stem: &str, directory: &Path) -> (usize, usize) {
    let corpus = fs::read_to_string(root.join(format!("vectors/{stem}/corpus.tsv"))).unwrap();
    let mut names = BTreeSet::new();
    let mut rows = 0;
    for line in corpus
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let [id, moods, version] = fields[..] else {
            panic!("project_id TAB moods TAB version: {line:?}")
        };
        assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        let version: u32 = version.parse().expect("version is a number");
        // Every type reads the committed wild genomes. The avatar-vectors
        // tests prove each one re-derives from its project id, so the file
        // is the frozen genome.
        let hex = fs::read_to_string(root.join(format!("vectors/wild/{id}.hex"))).unwrap();
        let hex = hex.trim();
        assert_eq!(hex.len(), 32_768);
        let bytes: Vec<_> = (0..16_384)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        for name in moods.split(',') {
            let mood = match name {
                "neutral" => Mood::Neutral,
                "working" => Mood::Working,
                "happy" => Mood::Happy,
                "waiting" => Mood::Waiting,
                "blocked" => Mood::Blocked,
                "sad" => Mood::Sad,
                "sleepy" => Mood::Sleepy,
                _ => panic!("unknown mood: {name}"),
            };
            // Version 1 trees keep their shipped names; later versions add
            // a version infix, matching the committed goldens.
            let filename = match version {
                1 => format!("{id}__{name}.json"),
                _ => format!("{id}__v{version}__{name}.json"),
            };
            assert!(names.insert(filename.clone()), "duplicate pair");
            // Full detail, as every committed corpus tree is drawn.
            let tree = render_tree_for_bytes(&bytes, avatar_type, version, mood).unwrap();
            if avatar_type == WILD {
                assert_eq!(tree.canvas, 240);
                assert!(tree.background.is_none());
            }
            fs::write(directory.join(filename), serialize_tree_json(&tree)).unwrap();
        }
        rows += 1;
    }
    (rows, names.len())
}
