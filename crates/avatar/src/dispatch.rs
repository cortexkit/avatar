//! Stored-avatar type and generation-version routing.
//!
//! A registry row names the representation that produced its genome. Routing
//! preserves that name rather than selecting a newer renderer, because a
//! generator change must never silently change an existing avatar.

use avatar_genome::{decode, Genome};
use avatar_phenotype::Q32;
use avatar_svg::wild::{self, Generation};
use avatar_svg::{build_render_tree, Mood, RenderError, RenderParams, RenderTree};

/// The stable registry type for the classic creature representation.
pub const CLASSIC: &str = "creature.classic";

/// The stable registry type for the wild creature representation.
pub const WILD: &str = "creature.wild";

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
        WILD => match version {
            // Keep every shipped arm when adding a newer one. Stored rows
            // render at their named generation, so version 1 keeps drawing
            // its off-eye mood lids for the rows that still store it.
            1 => render_wild(genome_bytes, mood, Generation::V1),
            2 => render_wild(genome_bytes, mood, Generation::V2),
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

fn render_wild(
    genome_bytes: &[u8],
    mood: Mood,
    generation: Generation,
) -> Result<RenderTree, DispatchError> {
    let bytes = genome_bytes
        .try_into()
        .map_err(|_| DispatchError::GenomeLength {
            expected: wild::GENOME_LEN,
            actual: genome_bytes.len(),
        })?;
    Ok(wild::render_tree(bytes, mood, generation))
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
        let result = render_tree_for(
            &genome(),
            "creature.unknown",
            CLASSIC_VERSION,
            Mood::Neutral,
        );

        match result {
            Err(DispatchError::UnsupportedType { requested }) => {
                assert_eq!(requested, "creature.unknown");
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

    #[test]
    fn wild_dispatch_renders_every_mood_on_its_own_canvas() {
        let bytes = wild::probe_genome(0);
        for (version, generation) in [(1, Generation::V1), (2, Generation::V2)] {
            for mood in wild::mood::ALL {
                let tree = render_tree_for_bytes(&bytes, WILD, version, mood).unwrap();
                assert_eq!(tree.canvas, 240);
                assert_eq!(tree.extent(), 24_000);
                assert_eq!(tree.background, None);
                assert!(!tree.primitives.is_empty());
                assert_eq!(tree, wild::render_tree(&bytes, mood, generation));
            }
        }
    }

    #[test]
    fn dispatch_checks_type_then_version_then_type_specific_length() {
        for length in [0, 1_024, 16_383, 16_384, 16_385] {
            let bytes = vec![0; length];
            assert_eq!(
                render_tree_for_bytes(&bytes, "creature.unknown", 1, Mood::Neutral),
                Err(DispatchError::UnsupportedType {
                    requested: "creature.unknown".into()
                })
            );
            for version in [0, 3, u32::MAX] {
                assert_eq!(
                    render_tree_for_bytes(&bytes, WILD, version, Mood::Neutral),
                    Err(DispatchError::UnsupportedVersion {
                        avatar_type: WILD.into(),
                        requested: version
                    })
                );
            }
            if length != wild::GENOME_LEN {
                for version in [1, 2] {
                    assert_eq!(
                        render_tree_for_bytes(&bytes, WILD, version, Mood::Neutral),
                        Err(DispatchError::GenomeLength {
                            expected: wild::GENOME_LEN,
                            actual: length
                        })
                    );
                }
            }
        }
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
            if let avatar_svg::Shape::Path { segments, .. } = &primitive.shape {
                for segment in segments {
                    if let avatar_svg::Segment::LineTo { x, y }
                    | avatar_svg::Segment::MoveTo { x, y } = segment
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
