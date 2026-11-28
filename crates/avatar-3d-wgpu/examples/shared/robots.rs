//! What the examples share: how a genome is derived from a seed and an
//! index, which robots they show, the colour behind them, and where the
//! plaza's robots stand.

use avatar_3d::motion::Actor;
use avatar_3d::robot::Robot;
use avatar_3d_wgpu::plaza::{self, Footprint, Plaza};
use avatar_small_robots::{Mood, Rgb, GENOME_LEN};

/// The stated robot, shown alone on the stage: a seed and a genome index.
pub const STATED: (u64, u64) = (501, 14);
/// The seed of the plaza's robots.
pub const PLAZA_SEED: u64 = 503;
/// The plaza's robots: genomes 0 to 32 of its seed.
pub const PLAZA_ROBOTS: u64 = 33;
/// The plaza's frame, which [`place_plaza`] frames the robots for.
pub const PLAZA_SIZE: (u32, u32) = (1600, 1000);
/// Behind every frame: a light grey, so white parts still show.
pub const BACKGROUND: Rgb = Rgb {
    r: 0xf2,
    g: 0xf3,
    b: 0xf5,
};

/// Genome `index` of the sheet drawn with `seed`, as the `random_sheet`
/// example derives it: the BLAKE3 extendable output of the text `avatar
/// random_sheet genome`, keyed by the seed and the index as little-endian
/// u64 values followed by 16 zero bytes.
pub fn genome(seed: u64, index: u64) -> Box<[u8; GENOME_LEN]> {
    let mut key = [0_u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&index.to_le_bytes());
    let mut hasher = blake3::Hasher::new_keyed(&key);
    hasher.update(b"avatar random_sheet genome");
    let mut genome = Box::new([0_u8; GENOME_LEN]);
    hasher.finalize_xof().fill(&mut genome[..]);
    genome
}

/// The plaza's robots placed by `plaza::layout` in a [`PLAZA_SIZE`] frame,
/// from each robot's footprint in its neutral rest pose without idle, with
/// those footprints. A placement that breaks the plaza's rules is reported
/// and used all the same.
pub fn place_plaza<'r>(robots: impl IntoIterator<Item = &'r Robot>) -> (Vec<Footprint>, Plaza) {
    let prints: Vec<Footprint> = robots
        .into_iter()
        .map(|robot| {
            let pose = Actor::new(robot, Mood::Neutral)
                .without_idle()
                .evaluate(0.0);
            plaza::footprint(robot, &pose)
        })
        .collect();
    let (width, height) = PLAZA_SIZE;
    let placed = plaza::layout(&prints, width, height);
    let clashes = plaza::clashes(&placed, &prints);
    if !clashes.is_empty() {
        eprintln!("the plaza breaks its rules: {clashes:?}");
    }
    (prints, placed)
}
