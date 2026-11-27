//! Genomes for the tests: the small-robots corpus, and genomes derived as
//! the `random_sheet` example derives them.

use std::fs;
use std::path::{Path, PathBuf};

use avatar_small_robots::GENOME_LEN;

pub type Genome = Box<[u8; GENOME_LEN]>;

fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

/// The corpus genomes in `corpus.tsv` order, each read from
/// `vectors/wild/<project_id>.hex`.
pub fn corpus() -> Vec<Genome> {
    let list = vectors().join("small-robots/corpus.tsv");
    let text = fs::read_to_string(&list).unwrap_or_else(|e| panic!("read {}: {e}", list.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|line| {
            let id = line
                .split('\t')
                .next()
                .expect("a corpus row starts with its id");
            let path = vectors().join(format!("wild/{id}.hex"));
            let hex = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let hex = hex.trim();
            let mut genome = Box::new([0_u8; GENOME_LEN]);
            for (i, byte) in genome.iter_mut().enumerate() {
                *byte =
                    u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("genome files are hex");
            }
            genome
        })
        .collect()
}

/// Genome `index` of the sheet drawn with `seed`, as the `random_sheet`
/// example derives it: the BLAKE3 extendable output of the text `avatar
/// random_sheet genome`, keyed by the seed and the index as little-endian
/// u64 values followed by 16 zero bytes.
pub fn random(seed: u64, index: u64) -> Genome {
    let mut key = [0_u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&index.to_le_bytes());
    let mut hasher = blake3::Hasher::new_keyed(&key);
    hasher.update(b"avatar random_sheet genome");
    let mut genome = Box::new([0_u8; GENOME_LEN]);
    hasher.finalize_xof().fill(&mut genome[..]);
    genome
}
