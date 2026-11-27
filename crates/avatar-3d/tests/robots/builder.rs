//! The builder returns a robot for every genome it is given.

use avatar_3d::robot::{build_with, BuildOptions, Robot};

use crate::cases;

/// The `random_sheet` seed of the random genomes that are only built, to
/// show the builder never fails; no digest pins them.
const RANDOM_SEED: u64 = 7;
/// How many genomes of that seed are built.
const RANDOM_COUNT: u64 = 1_000;

/// Checks what every robot must have: parts with geometry and finite
/// values, a bone for every rigid part, weights that sum to one.
pub fn sound(label: &str, robot: &Robot) {
    assert!(!robot.parts.is_empty(), "{label}: no parts");
    assert!(
        robot.stand_ins.is_empty(),
        "{label}: stand-ins {:?}",
        robot.stand_ins
    );
    for part in &robot.parts {
        let m = &part.mesh;
        assert!(
            !m.triangles.is_empty(),
            "{label}: part {} is empty",
            part.name
        );
        assert!(
            m.positions
                .iter()
                .chain(&m.normals)
                .flatten()
                .all(|v| v.is_finite()),
            "{label}: part {} has a non-finite value",
            part.name
        );
        match &part.skin {
            avatar_3d::robot::Skin::Rigid { bone } => assert!(*bone < robot.bones.len()),
            avatar_3d::robot::Skin::Skinned { joints, weights } => {
                assert_eq!(joints.len(), m.positions.len(), "{label}: {}", part.name);
                for (j, w) in joints.iter().zip(weights) {
                    assert!(j.iter().all(|j| (*j as usize) < robot.bones.len()));
                    let sum: f64 = w.iter().sum();
                    assert!(
                        (sum - 1.0).abs() < 1e-12,
                        "{label}: {} weights sum to {sum}",
                        part.name
                    );
                }
            }
            _ => unreachable!("every skin is rigid or skinned"),
        }
    }
    let names: Vec<&str> = robot.parts.iter().map(|p| p.name.as_str()).collect();
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        names.len(),
        "{label}: part names repeat: {names:?}"
    );
}

#[test]
fn builds_every_corpus_coverage_and_random_genome() {
    let mut built = 0;
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        sound(&label, &build_with(&genome, BuildOptions::untextured()));
        built += 1;
    }
    for i in 0..RANDOM_COUNT {
        let genome = cases::random_genome(RANDOM_SEED, i);
        sound(
            &format!("random {RANDOM_SEED}/{i}"),
            &build_with(&genome, BuildOptions::untextured()),
        );
        built += 1;
    }
    assert!(built >= 12 + RANDOM_COUNT as usize);
}

#[test]
fn coverage_search_finds_the_recorded_prefix() {
    let found = cases::coverage_search(cases::COVERAGE_SEED, cases::COVERAGE_LIMIT)
        .unwrap_or_else(|message| panic!("{message}"));
    assert_eq!(
        found,
        cases::committed_prefix(),
        "the coverage prefix differs from the pins; rerun the robot_pins mint"
    );
}

/// Prints the coverage prefix and each corpus robot's triangle count, for
/// a first look: `cargo test --test robots stats -- --ignored --nocapture`.
#[test]
#[ignore = "prints numbers for a person to read"]
fn stats() {
    println!(
        "coverage: {:?}",
        cases::coverage_search(cases::COVERAGE_SEED, cases::COVERAGE_LIMIT)
    );
    for (label, genome) in cases::cases(0) {
        let robot = build_with(&genome, BuildOptions::untextured());
        println!(
            "{label}: {} parts, {} triangles, {} textures, bones {}",
            robot.parts.len(),
            robot.triangle_count(),
            robot.textures.len(),
            robot.bones.len()
        );
    }
}
