//! Mint the wild golden corpus pairs through public dispatch, into a scratch
//! directory, without touching the committed goldens.
//!
//! scripts/wild-cross-platform.sh compiles and runs this natively and inside
//! Linux containers, then compares the outputs byte for byte.

use avatar_svg::{render_tree_for_bytes, serialize_tree_json, Mood};
use std::{collections::BTreeSet, fs, path::Path};

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
    let corpus = fs::read_to_string(root.join("vectors/wild/corpus.tsv")).unwrap();
    let mut names = BTreeSet::new();
    let mut rows = 0;
    for line in corpus
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        let (id, moods) = line.split_once('\t').expect("project_id TAB moods");
        assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        // The avatar-vectors tests prove each committed genome re-derives
        // from its project id, so the file is the frozen genome.
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
            let filename = format!("{id}__{name}.json");
            assert!(names.insert(filename.clone()), "duplicate pair");
            let tree = render_tree_for_bytes(&bytes, "creature.wild", 1, mood).unwrap();
            assert_eq!(tree.canvas, 240);
            assert!(tree.background.is_none());
            fs::write(out.join(filename), serialize_tree_json(&tree)).unwrap();
        }
        rows += 1;
    }
    assert_eq!((rows, names.len()), (12, 18), "frozen corpus is incomplete");
    println!("minted {} trees from {rows} frozen genomes", names.len());
}
