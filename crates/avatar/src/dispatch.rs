//! Stored-avatar type and generation-version routing, and the finished image
//! drawn from a stored avatar.
//!
//! A registry row names the representation that produced its genome. Routing
//! preserves that name rather than selecting a newer renderer, because a
//! generator change must never silently change an existing avatar.

use avatar_genome::{decode, Genome};
use avatar_phenotype::Q32;
use avatar_raster::{rasterize_tree, Image, ImageOptions, RasterError, MAX_SIZE_PX};
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
    /// The requested image edge length was outside `1..=MAX_SIZE_PX` pixels.
    /// Only [`render_image`] returns this, and only for a row it accepted.
    ImageSize { requested: u32 },
    /// The rasterizer could not draw the tree. Only [`render_image`] returns
    /// this.
    Raster(RasterError),
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
            Self::ImageSize { requested } => write!(
                formatter,
                "image size must be 1 to {MAX_SIZE_PX} pixels, received {requested}"
            ),
            Self::Raster(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DispatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Render(error) => Some(error),
            Self::Raster(error) => Some(error),
            Self::UnsupportedType { .. }
            | Self::UnsupportedVersion { .. }
            | Self::GenomeLength { .. }
            | Self::ImageSize { .. } => None,
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
/// the C boundary an embedded app calls. The tree is drawn at full detail.
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
    route(genome_bytes, avatar_type, version)?.render(mood, None)
}

/// Byte-level dispatch, like [`render_tree_for_bytes`], for a tree that will
/// be drawn `detail_px` pixels wide.
///
/// A type may draw less detail at small sizes, by design, so that a creature
/// the size of a dock icon stays legible. [`render_tree_for_bytes`] always
/// draws full detail. Classic and wild draw the same tree at every size, so
/// for them the result equals [`render_tree_for_bytes`]. The checks, and
/// their order, are the same as there.
pub fn render_tree_for_bytes_at(
    genome_bytes: &[u8],
    avatar_type: &str,
    version: u32,
    mood: Mood,
    detail_px: u32,
) -> Result<RenderTree, DispatchError> {
    route(genome_bytes, avatar_type, version)?.render(mood, Some(detail_px))
}

/// Draws a stored avatar as a finished square image `size_px` pixels wide.
///
/// Checks run in a fixed order. Type, version and genome length come first,
/// exactly as [`render_tree_for_bytes`] checks them, so a row this build
/// cannot depict is reported as such whatever size was asked for. Then
/// `size_px` must lie in `1..=MAX_SIZE_PX`, or the call fails with
/// [`DispatchError::ImageSize`].
///
/// The tree is drawn at the detail level for `size_px`, as
/// [`render_tree_for_bytes_at`] draws it, and rasterized by
/// [`avatar_raster::rasterize_tree`] with `options`. A rasterizer failure is
/// returned as [`DispatchError::Raster`].
///
/// The image is a pure function of the arguments. Nothing is cached and no
/// state outlives the call, so a host may cache images keyed by every
/// argument, and calls are safe to make from several threads at once.
///
/// ```
/// use avatar::{render_image, Alpha, GroundShadow, ImageOptions, Mood, WILD};
///
/// let genome = vec![7_u8; 16_384];
/// let options = ImageOptions {
///     ground_shadow: GroundShadow::Drop,
///     alpha: Alpha::Straight,
/// };
/// let image = render_image(&genome, WILD, 2, Mood::Neutral, 48, &options)
///     .expect("wild version 2 renders");
///
/// assert_eq!(image.size_px, 48);
/// assert_eq!(image.alpha, Alpha::Straight);
/// assert_eq!(image.pixels.len(), 48 * 48 * 4);
/// ```
pub fn render_image(
    genome_bytes: &[u8],
    avatar_type: &str,
    version: u32,
    mood: Mood,
    size_px: u32,
    options: &ImageOptions,
) -> Result<Image, DispatchError> {
    let route = route(genome_bytes, avatar_type, version)?;
    if !(1..=MAX_SIZE_PX).contains(&size_px) {
        return Err(DispatchError::ImageSize { requested: size_px });
    }
    let tree = route.render(mood, Some(size_px))?;
    rasterize_tree(&tree, size_px, options).map_err(DispatchError::Raster)
}

/// A stored row whose type, version and genome length have been accepted,
/// holding the genome in the layout its generator reads.
enum Route<'a> {
    ClassicV2(&'a [u8; CLASSIC_GENOME_BYTES]),
    Wild(&'a [u8; wild::GENOME_LEN], Generation),
}

/// Accepts or refuses a stored row, checking type, then version, then the
/// genome length that type requires, as [`render_tree_for_bytes`] documents.
/// Every public call goes through here, so they all refuse a row the same way.
fn route<'a>(
    genome_bytes: &'a [u8],
    avatar_type: &str,
    version: u32,
) -> Result<Route<'a>, DispatchError> {
    match avatar_type {
        CLASSIC => match version {
            // Keep this arm when adding a newer version. Stored rows render at
            // their named generation; a new arm is additive rather than a
            // replacement for the already-shipped classic v2 behavior.
            CLASSIC_VERSION => Ok(Route::ClassicV2(genome_layout(genome_bytes)?)),
            requested => Err(DispatchError::UnsupportedVersion {
                avatar_type: avatar_type.to_owned(),
                requested,
            }),
        },
        WILD => match version {
            // Keep every shipped arm when adding a newer one. Stored rows
            // render at their named generation, so version 1 keeps drawing
            // its off-eye mood lids for the rows that still store it.
            1 => Ok(Route::Wild(genome_layout(genome_bytes)?, Generation::V1)),
            2 => Ok(Route::Wild(genome_layout(genome_bytes)?, Generation::V2)),
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

/// Views the stored bytes as a genome of exactly `LEN` bytes, the length the
/// selected type requires.
fn genome_layout<const LEN: usize>(genome_bytes: &[u8]) -> Result<&[u8; LEN], DispatchError> {
    genome_bytes
        .try_into()
        .map_err(|_| DispatchError::GenomeLength {
            expected: LEN,
            actual: genome_bytes.len(),
        })
}

impl Route<'_> {
    /// Draws the accepted row in `mood`. `detail_px` is the width in pixels
    /// the tree will be drawn at, or `None` for full detail. Classic and wild
    /// draw the same tree at every size, so neither reads it; a type that
    /// draws less detail at small sizes does.
    fn render(self, mood: Mood, _detail_px: Option<u32>) -> Result<RenderTree, DispatchError> {
        match self {
            Self::ClassicV2(bytes) => render_classic_v2(bytes, mood),
            Self::Wild(bytes, generation) => Ok(wild::render_tree(bytes, mood, generation)),
        }
    }
}

fn render_classic_v2(
    bytes: &[u8; CLASSIC_GENOME_BYTES],
    mood: Mood,
) -> Result<RenderTree, DispatchError> {
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

    /// Rows that cover every route: the classic generation and both wild
    /// generations, each with a genome of the length it requires.
    fn accepted_rows() -> [(Vec<u8>, &'static str, u32); 3] {
        let wild_bytes = wild::probe_genome(0).to_vec();
        [
            (vec![0; CLASSIC_GENOME_BYTES], CLASSIC, CLASSIC_VERSION),
            (wild_bytes.clone(), WILD, 1),
            (wild_bytes, WILD, 2),
        ]
    }

    fn all_options() -> [ImageOptions; 4] {
        use avatar_raster::{Alpha, GroundShadow};
        [
            (GroundShadow::Keep, Alpha::Premultiplied),
            (GroundShadow::Keep, Alpha::Straight),
            (GroundShadow::Drop, Alpha::Premultiplied),
            (GroundShadow::Drop, Alpha::Straight),
        ]
        .map(|(ground_shadow, alpha)| ImageOptions {
            ground_shadow,
            alpha,
        })
    }

    #[test]
    fn classic_and_wild_draw_the_full_detail_tree_at_every_detail_size() {
        for (bytes, avatar_type, version) in accepted_rows() {
            for mood in [Mood::Neutral, Mood::Sleepy] {
                let full = render_tree_for_bytes(&bytes, avatar_type, version, mood)
                    .expect("an accepted row renders");
                for detail_px in [0, 1, 16, 56, 100, 288, 1_080, 2_048, u32::MAX] {
                    assert_eq!(
                        render_tree_for_bytes_at(&bytes, avatar_type, version, mood, detail_px),
                        Ok(full.clone()),
                        "{avatar_type} version {version} at {detail_px} px"
                    );
                }
            }
        }
    }

    #[test]
    fn tree_at_a_size_and_image_refuse_rows_exactly_as_the_tree_call_does() {
        let refused_rows = [0, 1, 1_024, 16_383, 16_384, 16_385]
            .into_iter()
            .flat_map(|length| {
                [
                    ("creature.unknown", 1),
                    (CLASSIC, 1),
                    (CLASSIC, CLASSIC_VERSION),
                    (WILD, 0),
                    (WILD, 1),
                    (WILD, 2),
                    (WILD, 3),
                    (WILD, u32::MAX),
                ]
                .map(|(avatar_type, version)| (vec![0; length], avatar_type, version))
            })
            .filter_map(|(bytes, avatar_type, version)| {
                render_tree_for_bytes(&bytes, avatar_type, version, Mood::Neutral)
                    .err()
                    .map(|error| (bytes, avatar_type, version, error))
            })
            .collect::<Vec<_>>();
        // Each kind of refusal must be among the rows checked.
        let refused =
            |kind: fn(&DispatchError) -> bool| refused_rows.iter().any(|(.., error)| kind(error));
        assert!(refused(|error| matches!(
            error,
            DispatchError::UnsupportedType { .. }
        )));
        assert!(refused(|error| matches!(
            error,
            DispatchError::UnsupportedVersion { .. }
        )));
        assert!(refused(|error| matches!(
            error,
            DispatchError::GenomeLength { .. }
        )));

        for (bytes, avatar_type, version, error) in refused_rows {
            for size_px in [0, 1, 256, 2_048, 2_049, u32::MAX] {
                let label = format!("{avatar_type} version {version}, {} bytes", bytes.len());
                assert_eq!(
                    render_tree_for_bytes_at(&bytes, avatar_type, version, Mood::Neutral, size_px)
                        .as_ref(),
                    Err(&error),
                    "{label}"
                );
                // An unusable size never hides why the row itself was refused.
                assert_eq!(
                    render_image(
                        &bytes,
                        avatar_type,
                        version,
                        Mood::Neutral,
                        size_px,
                        &ImageOptions::default()
                    ),
                    Err(error.clone()),
                    "{label} at {size_px} px"
                );
            }
        }
    }

    #[test]
    fn image_size_is_checked_after_the_row_is_accepted() {
        for (bytes, avatar_type, version) in accepted_rows() {
            for size_px in [0, 2_049, u32::MAX] {
                assert_eq!(
                    render_image(
                        &bytes,
                        avatar_type,
                        version,
                        Mood::Neutral,
                        size_px,
                        &ImageOptions::default()
                    ),
                    Err(DispatchError::ImageSize { requested: size_px }),
                    "{avatar_type} version {version}"
                );
            }
        }

        let bytes = wild::probe_genome(0);
        for size_px in [1, 2_048] {
            let image = render_image(
                &bytes,
                WILD,
                2,
                Mood::Neutral,
                size_px,
                &ImageOptions::default(),
            )
            .expect("the smallest and largest sizes render");
            assert_eq!(image.size_px, size_px);
            assert_eq!(image.pixels.len(), (size_px * size_px * 4) as usize);
        }
    }

    #[test]
    fn image_is_the_rasterized_tree_dispatch_draws_under_each_option() {
        for (bytes, avatar_type, version) in accepted_rows() {
            let tree = render_tree_for_bytes(&bytes, avatar_type, version, Mood::Happy)
                .expect("an accepted row renders");
            let mut images = Vec::new();
            for options in all_options() {
                let expected = rasterize_tree(&tree, 64, &options).expect("the tree rasterizes");
                let image = render_image(&bytes, avatar_type, version, Mood::Happy, 64, &options);
                assert_eq!(
                    image.as_ref(),
                    Ok(&expected),
                    "{avatar_type} version {version} with {options:?}"
                );
                images.push(expected.pixels);
            }
            // The wild creature has a ground shadow and soft edges, so every
            // option changes its pixels; a call that ignored the options
            // could not match all four.
            if avatar_type == WILD {
                for (index, image) in images.iter().enumerate() {
                    assert!(
                        !images[index + 1..].contains(image),
                        "two option sets gave the same wild image"
                    );
                }
            }
        }
    }

    #[test]
    fn image_errors_describe_themselves() {
        use std::error::Error;

        let size = DispatchError::ImageSize { requested: 2_049 };
        assert_eq!(
            size.to_string(),
            "image size must be 1 to 2048 pixels, received 2049"
        );
        assert!(size.source().is_none());

        // No generated tree fails to parse, so a rasterizer failure cannot be
        // provoked through dispatch; this pins how one is reported.
        let cause = RasterError::InvalidSvg {
            message: "unexpected end of stream".to_owned(),
        };
        let raster = DispatchError::Raster(cause.clone());
        assert_eq!(raster.to_string(), cause.to_string());
        assert_eq!(
            raster.source().map(ToString::to_string),
            Some(cause.to_string())
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
