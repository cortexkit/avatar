//! The cast a host asks for: one row per creature, and how a batch of rows
//! is checked before it reaches the scene.

use std::collections::HashSet;
use std::sync::Arc;

use avatar_small_robots::GENOME_LEN;

/// The one creature type the stage draws in 3D: the small robots, named as
/// the 2D dispatch names them.
pub const SMALL_ROBOTS: &str = "creature.small-robots";

/// The one version of [`SMALL_ROBOTS`] the stage draws. A later version of
/// the 2D type is refused as an unknown version until 3D supports it.
pub const SMALL_ROBOTS_VERSION: u32 = 1;

/// One creature as a host stores it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatureSpec {
    /// The caller's name for the creature, such as an agent's id. Commands
    /// and events name creatures by it.
    pub key: String,
    /// The stored genome: [`GENOME_LEN`] bytes for [`SMALL_ROBOTS`].
    pub genome: Vec<u8>,
    /// The stored creature type.
    pub avatar_type: String,
    /// The stored generation version of that type.
    pub avatar_version: u32,
}

impl CreatureSpec {
    /// A small robot at the version the stage draws.
    pub fn small_robot(key: impl Into<String>, genome: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            genome: genome.into(),
            avatar_type: SMALL_ROBOTS.to_owned(),
            avatar_version: SMALL_ROBOTS_VERSION,
        }
    }
}

/// Why a row of [`crate::Stage::set_creatures`] was skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkipReason {
    /// The type is not [`SMALL_ROBOTS`]. Checked first, so a row of an
    /// unknown type is reported as this whatever its version and genome.
    UnknownType,
    /// The type is known but the version is not [`SMALL_ROBOTS_VERSION`].
    UnknownVersion,
    /// The genome is not [`GENOME_LEN`] bytes long.
    GenomeLength,
    /// An earlier valid row of the same batch has the same key.
    DuplicateKey,
}

/// An accepted row. Only one type and version are accepted, so the genome
/// alone tells two accepted specs of one key apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Accepted {
    pub key: String,
    pub genome: Arc<[u8; GENOME_LEN]>,
}

/// A checked batch: the new cast in batch order, and every skipped row with
/// its reason, in batch order.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cast {
    pub rows: Vec<Accepted>,
    pub skipped: Vec<(String, SkipReason)>,
}

/// Why one row cannot be drawn, checking its type, then its version, then
/// its genome's length, as the 2D dispatch does.
pub(crate) fn refusal(spec: &CreatureSpec) -> Option<SkipReason> {
    if spec.avatar_type != SMALL_ROBOTS {
        Some(SkipReason::UnknownType)
    } else if spec.avatar_version != SMALL_ROBOTS_VERSION {
        Some(SkipReason::UnknownVersion)
    } else if spec.genome.len() != GENOME_LEN {
        Some(SkipReason::GenomeLength)
    } else {
        None
    }
}

/// Checks a batch in order. The new cast is the first valid row of each
/// key; a later valid row of a key is a duplicate, and an invalid row is
/// skipped for its own reason whether or not its key also has a valid row.
pub(crate) fn check(specs: &[CreatureSpec]) -> Cast {
    let mut cast = Cast::default();
    let mut seen = HashSet::new();
    for spec in specs {
        if let Some(reason) = refusal(spec) {
            cast.skipped.push((spec.key.clone(), reason));
        } else if !seen.insert(spec.key.as_str()) {
            cast.skipped
                .push((spec.key.clone(), SkipReason::DuplicateKey));
        } else {
            let mut genome = Arc::new([0_u8; GENOME_LEN]);
            Arc::get_mut(&mut genome)
                .expect("a new Arc has one owner")
                .copy_from_slice(&spec.genome);
            cast.rows.push(Accepted {
                key: spec.key.clone(),
                genome,
            });
        }
    }
    cast
}

#[cfg(test)]
mod tests {
    use super::*;

    fn robot(key: &str, fill: u8) -> CreatureSpec {
        CreatureSpec::small_robot(key, vec![fill; GENOME_LEN])
    }

    fn keys(cast: &Cast) -> Vec<(&str, u8)> {
        cast.rows
            .iter()
            .map(|r| (r.key.as_str(), r.genome[0]))
            .collect()
    }

    #[test]
    fn the_accepted_type_and_version_are_the_dispatch_ones() {
        assert_eq!(SMALL_ROBOTS, avatar::SMALL_ROBOTS);
        assert_eq!(GENOME_LEN, 16_384);
    }

    #[test]
    fn rows_are_checked_for_type_then_version_then_length() {
        let mut row = robot("a", 1);
        row.avatar_type = "creature.sprouts".into();
        row.avatar_version = 9;
        row.genome.pop();
        assert_eq!(refusal(&row), Some(SkipReason::UnknownType));
        row.avatar_type = SMALL_ROBOTS.into();
        assert_eq!(refusal(&row), Some(SkipReason::UnknownVersion));
        row.avatar_version = 1;
        assert_eq!(refusal(&row), Some(SkipReason::GenomeLength));
        row.genome.push(1);
        assert_eq!(refusal(&row), None);
    }

    #[test]
    fn an_unknown_type_with_a_short_genome_is_an_unknown_type() {
        let row = CreatureSpec {
            key: "a".into(),
            genome: vec![0; 100],
            avatar_type: "creature.unknown".into(),
            avatar_version: 1,
        };
        let cast = check(&[row]);
        assert!(cast.rows.is_empty());
        assert_eq!(cast.skipped, [("a".to_owned(), SkipReason::UnknownType)]);
    }

    #[test]
    fn each_key_keeps_its_first_valid_row_and_reports_the_other_once() {
        let mut bad = robot("a", 2);
        bad.avatar_version = 2;

        // A valid row, then a duplicate.
        let cast = check(&[robot("a", 1), robot("a", 2)]);
        assert_eq!(keys(&cast), [("a", 1)]);
        assert_eq!(cast.skipped, [("a".to_owned(), SkipReason::DuplicateKey)]);

        // An invalid row, then a valid one: the valid one is no duplicate.
        let cast = check(&[bad.clone(), robot("a", 1)]);
        assert_eq!(keys(&cast), [("a", 1)]);
        assert_eq!(cast.skipped, [("a".to_owned(), SkipReason::UnknownVersion)]);

        // A valid row, then an invalid one, reported for its own reason.
        let cast = check(&[robot("a", 1), bad]);
        assert_eq!(keys(&cast), [("a", 1)]);
        assert_eq!(cast.skipped, [("a".to_owned(), SkipReason::UnknownVersion)]);
    }

    #[test]
    fn the_cast_keeps_batch_order() {
        let cast = check(&[robot("c", 3), robot("a", 1), robot("b", 2)]);
        assert_eq!(keys(&cast), [("c", 3), ("a", 1), ("b", 2)]);
        assert!(cast.skipped.is_empty());
    }
}
