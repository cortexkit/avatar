//! Stored-avatar type and generation-version routing.
//!
//! A registry row names the representation that produced its genome. Routing
//! preserves that name rather than selecting a newer renderer, because a
//! generator change must never silently change an existing avatar.

use avatar_genome::{decode, Genome};
use avatar_phenotype::Q32;

use crate::{build_render_tree, Mood, RenderError, RenderParams, RenderTree};

/// The stable registry type for the classic creature representation.
pub const CLASSIC: &str = "creature.classic";

const CLASSIC_VERSION: u32 = 2;
const CLASSIC_GENOME_BYTES: usize = 1_024;
const CLASSIC_CANVAS: u32 = 10_000;

/// Why a stored avatar could not be rendered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchError {
    /// The registry named a representation this library does not implement.
    UnsupportedType { requested: String },
    /// The representation is known, but this generation was not shipped here.
    UnsupportedVersion { avatar_type: String, requested: u32 },
    /// The stored bytes do not fit the selected representation's genome layout.
    GenomeLength { expected: usize, actual: usize },
    /// The selected renderer rejected one of its own render-time inputs.
    Render(RenderError),
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedType { requested } => {
                write!(formatter, "unsupported avatar type {requested:?}")
            }
            Self::UnsupportedVersion {
                avatar_type,
                requested,
            } => write!(
                formatter,
                "unsupported avatar version {requested} for type {avatar_type:?}"
            ),
            Self::GenomeLength { expected, actual } => {
                write!(
                    formatter,
                    "genome must contain {expected} bytes, received {actual}"
                )
            }
            Self::Render(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DispatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Render(error) => Some(error),
            Self::UnsupportedType { .. }
            | Self::UnsupportedVersion { .. }
            | Self::GenomeLength { .. } => None,
        }
    }
}

/// Builds the geometry tree for the type and generation version stored by a
/// registry row.
///
/// This takes a [`Genome`] so normal callers retain the genome crate's exact
/// 1 KiB invariant. The byte-level route still verifies the selected layout
/// before decoding, which keeps the fail-closed boundary explicit if a future
/// wire adapter supplies unvalidated bytes.
pub fn render_tree_for(
    genome: &Genome,
    avatar_type: &str,
    version: u32,
    mood: Mood,
) -> Result<RenderTree, DispatchError> {
    render_tree_for_bytes(genome.as_bytes(), avatar_type, version, mood)
}

/// Byte-level dispatch for callers holding UNVALIDATED genome bytes, such as
/// the C boundary an embedded app calls.
///
/// Checks run in a fixed order: type, then version, then the genome length
/// THAT TYPE requires. The order is part of the contract. A client must be
/// able to tell "this build cannot depict this type", which it answers by
/// drawing nothing, from "this row is malformed", which it reports. Checking
/// a length first would turn every row of a newer type, whose genome is a
/// different size, into a false report of corruption. And the length lives
/// with the type here, so no caller needs its own copy of it.
pub fn render_tree_for_bytes(
    genome_bytes: &[u8],
    avatar_type: &str,
    version: u32,
    mood: Mood,
) -> Result<RenderTree, DispatchError> {
    match avatar_type {
        CLASSIC => match version {
            // Keep this arm when adding a newer version. Stored rows render at
            // their named generation; a new arm is additive rather than a
            // replacement for the already-shipped classic v2 behavior.
            CLASSIC_VERSION => render_classic_v2(genome_bytes, mood),
            requested => Err(DispatchError::UnsupportedVersion {
                avatar_type: avatar_type.to_owned(),
                requested,
            }),
        },
        requested => Err(DispatchError::UnsupportedType {
            requested: requested.to_owned(),
        }),
    }
}

fn render_classic_v2(genome_bytes: &[u8], mood: Mood) -> Result<RenderTree, DispatchError> {
    let actual = genome_bytes.len();
    if actual != CLASSIC_GENOME_BYTES {
        return Err(DispatchError::GenomeLength {
            expected: CLASSIC_GENOME_BYTES,
            actual,
        });
    }

    let bytes: &[u8; CLASSIC_GENOME_BYTES] = genome_bytes
        .try_into()
        .expect("length is checked against the classic v2 genome layout");
    let genome = Genome::from_bytes(*bytes);
    let params = RenderParams {
        mood,
        growth: Q32::ONE,
        canvas: CLASSIC_CANVAS,
    };
    build_render_tree(&decode(&genome), &params).map_err(DispatchError::Render)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome() -> Genome {
        Genome::from_bytes([0; CLASSIC_GENOME_BYTES])
    }

    #[test]
    fn supported_pair_routes_to_the_existing_tree_builder() {
        let genome = genome();
        let mood = Mood::Happy;
        let expected = build_render_tree(
            &decode(&genome),
            &RenderParams {
                mood,
                growth: Q32::ONE,
                canvas: CLASSIC_CANVAS,
            },
        )
        .unwrap();

        assert_eq!(
            render_tree_for(&genome, CLASSIC, CLASSIC_VERSION, mood),
            Ok(expected)
        );
    }

    #[test]
    fn unknown_type_is_refused_without_rendering() {
        let result = render_tree_for(&genome(), "creature.wild", CLASSIC_VERSION, Mood::Neutral);

        match result {
            Err(DispatchError::UnsupportedType { requested }) => {
                assert_eq!(requested, "creature.wild");
            }
            other => panic!("expected unsupported type, got {other:?}"),
        }
    }

    #[test]
    fn unshipped_classic_version_is_refused_without_rendering() {
        let result = render_tree_for(&genome(), CLASSIC, 1, Mood::Neutral);

        match result {
            Err(DispatchError::UnsupportedVersion {
                avatar_type,
                requested,
            }) => {
                assert_eq!(avatar_type, CLASSIC);
                assert_eq!(requested, 1);
            }
            other => panic!("expected unsupported version, got {other:?}"),
        }
    }

    #[test]
    fn wrong_length_is_refused_before_the_classic_renderer_runs() {
        let result = render_tree_for_bytes(
            &[0; CLASSIC_GENOME_BYTES - 1],
            CLASSIC,
            CLASSIC_VERSION,
            Mood::Neutral,
        );

        assert_eq!(
            result,
            Err(DispatchError::GenomeLength {
                expected: CLASSIC_GENOME_BYTES,
                actual: CLASSIC_GENOME_BYTES - 1,
            })
        );
    }
}

#[cfg(test)]
mod extent_tests {
    use super::*;

    #[test]
    fn extent_matches_the_coordinates_primitives_actually_use() {
        // Pins the relationship a consumer relies on. If a future change made
        // canvas and coordinates agree, or diverge by a different factor,
        // extent() would start lying and every geometry consumer would draw
        // at the wrong scale while looking plausible.
        let genome = avatar_genome::derive_genome(&avatar_genome::SeatIdentity::Registered {
            project_id: "agent_extent_probe".to_owned(),
        });
        let tree = render_tree_for(&genome, CLASSIC, CLASSIC_VERSION, Mood::Neutral)
            .expect("classic v2 renders");

        let mut max_seen = 0_i64;
        for primitive in &tree.primitives {
            if let crate::tree::Shape::Path { segments, .. } = &primitive.shape {
                for segment in segments {
                    if let crate::tree::Segment::LineTo { x, y }
                    | crate::tree::Segment::MoveTo { x, y } = segment
                    {
                        max_seen = max_seen.max(*x).max(*y);
                    }
                }
            }
        }

        assert!(
            max_seen > tree.extent() / 4,
            "geometry should span the extent"
        );
        assert!(
            max_seen <= tree.extent(),
            "geometry must not exceed the declared extent: {max_seen} > {}",
            tree.extent()
        );
        assert_eq!(tree.extent(), 1_000_000, "classic canvas is 10_000 units");
    }
}
