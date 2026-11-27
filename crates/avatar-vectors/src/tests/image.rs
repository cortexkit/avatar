//! Golden finished images in `vectors/image`.
//!
//! Every golden is minted by `avatar_raster::rasterize_tree` from a committed
//! golden tree, read back through the tree fixture reader. `render_image` is
//! never used to mint, so the goldens stay independent of the call they test:
//! `render_image` must regenerate each tree from its stored genome and draw it
//! to the same bytes.
//!
//! Each PNG is 8-bit RGBA holding `Image::pixels` verbatim. A premultiplied
//! image therefore stays premultiplied in its file, unlike a PNG written by
//! `tiny_skia::Pixmap::encode_png`, which converts to straight alpha.
//!
//! On the platform that minted them the goldens are reproduced exactly. On
//! other platforms an image may differ by a few levels in a few pixels;
//! `ImageDifference` states the rule and why it exists.

use std::time::{Duration, Instant};

use avatar::{
    render_image, Alpha, DispatchError, GroundShadow, Image, ImageOptions, Role, CLASSIC,
    DRAWN_HEADS, KNITTED_TOYS, PIXEL_CRITTERS, SMALL_ROBOTS, SPROUTS, WILD, WILD_PLUS,
};
use avatar_raster::rasterize_tree;
use serde::Serialize;

use super::wild::{frozen_vectors, genome_path, parse_corpus, read_genome, WildVector};
use super::*;

const IMAGE_DIRECTORY: &str = "image";
/// The smallest size apps use, a middle size and the largest size apps use.
const WILD_VERSION_1_SIZES: [u32; 3] = [16, 256, 1_080];
/// The size of every image that varies the mood, the options or the type.
const CASE_SIZE_PX: u32 = 256;
/// The one wild genome whose version 2 trees are committed.
const WILD_VERSION_2_GENOME: &str = "wild-port-0022";
/// Version 2 is the only classic version.
const CLASSIC_VERSION: u32 = 2;
/// The six types that read the stored genome wild reads, in manifest order.
/// Each commits its golden trees in `vectors/<stem>/tree`.
const NEWER_TYPES: [&str; 6] = [
    WILD_PLUS,
    SPROUTS,
    PIXEL_CRITTERS,
    KNITTED_TOYS,
    SMALL_ROBOTS,
    DRAWN_HEADS,
];
/// The newer types whose reference draws a ground shadow under the creature.
/// Pixel critters and drawn heads draw none.
const GROUND_SHADOW_TYPES: [&str; 4] = [WILD_PLUS, SPROUTS, KNITTED_TOYS, SMALL_ROBOTS];
/// Version 1 is the only version of each newer type.
const NEWER_TYPE_VERSION: u32 = 1;
/// The corpus genome each newer type draws under every combination of
/// options. It is the corpus row whose trees are committed in every mood.
const NEWER_TYPE_OPTIONS_GENOME: &str = "wild-port-0005";
const MINT_COMMAND: &str = "UPDATE_IMAGE_GOLDEN=1 cargo test -p avatar-vectors tests::image::mint_images -- --exact --nocapture --ignored";

/// One image in the golden matrix.
struct Cell {
    avatar_type: &'static str,
    /// The genome's name: a wild project id or a classic vector name.
    genome: String,
    version: u32,
    mood: Mood,
    size_px: u32,
    options: ImageOptions,
    /// The committed genome, relative to `vectors/`.
    genome_hex: String,
    /// The committed golden tree this image is minted from, relative to
    /// `vectors/`.
    tree: String,
}

/// The manifest's description of one image. The first seven fields are the
/// key; no two entries share one.
#[derive(Serialize)]
struct ManifestEntry {
    #[serde(rename = "type")]
    avatar_type: &'static str,
    genome: String,
    version: u32,
    mood: &'static str,
    size_px: u32,
    ground_shadow: &'static str,
    alpha: &'static str,
    genome_hex: String,
    tree: String,
    file: String,
}

#[derive(Serialize)]
struct ImageManifest {
    entries: Vec<ManifestEntry>,
}

impl Cell {
    fn wild(row: &WildVector, mood: Mood, size_px: u32, options: ImageOptions) -> Self {
        Self {
            avatar_type: WILD,
            genome: row.project_id.clone(),
            version: row.version,
            mood,
            size_px,
            options,
            genome_hex: relative_to_corpus(&genome_path(&row.project_id)),
            tree: format!("tree/{}", row.tree_name(mood_name(mood))),
        }
    }

    /// A newer type's neutral image of one genome from its corpus, minted
    /// from the type's own golden tree. The genome is the wild genome the
    /// corpus row names.
    fn newer_type(avatar_type: &'static str, row: &WildVector, options: ImageOptions) -> Self {
        let mood = Mood::Neutral;
        Self {
            avatar_type,
            genome: row.project_id.clone(),
            version: row.version,
            mood,
            size_px: CASE_SIZE_PX,
            options,
            genome_hex: relative_to_corpus(&genome_path(&row.project_id)),
            tree: format!(
                "{}/tree/{}__{}.json",
                stem(avatar_type),
                row.project_id,
                mood_name(mood)
            ),
        }
    }

    fn entry(&self) -> ManifestEntry {
        ManifestEntry {
            avatar_type: self.avatar_type,
            genome: self.genome.clone(),
            version: self.version,
            mood: mood_name(self.mood),
            size_px: self.size_px,
            ground_shadow: ground_shadow_name(self.options.ground_shadow),
            alpha: alpha_name(self.options.alpha),
            genome_hex: self.genome_hex.clone(),
            tree: self.tree.clone(),
            file: self.file_name(),
        }
    }

    /// The type's stem, then every key field, so each key has its own file.
    fn file_name(&self) -> String {
        format!(
            "{}__{}__v{}__{}__{}px__{}__{}.png",
            stem(self.avatar_type),
            self.genome,
            self.version,
            mood_name(self.mood),
            self.size_px,
            ground_shadow_name(self.options.ground_shadow),
            alpha_name(self.options.alpha),
        )
    }

    fn label(&self) -> String {
        format!("vectors/{IMAGE_DIRECTORY}/{}", self.file_name())
    }

    fn genome_bytes(&self) -> Vec<u8> {
        match self.avatar_type {
            // The newer types read the same 16,384-byte genome as wild.
            avatar_type if avatar_type == WILD || NEWER_TYPES.contains(&avatar_type) => {
                read_genome(&self.genome).to_vec()
            }
            CLASSIC => {
                let hex = fs::read_to_string(corpus_root().join(&self.genome_hex))
                    .unwrap_or_else(|error| panic!("read {}: {error}", self.genome_hex));
                Genome::from_hex(hex.trim())
                    .unwrap_or_else(|error| panic!("decode {}: {error:?}", self.genome_hex))
                    .as_bytes()
                    .to_vec()
            }
            other => panic!("no genome reader for {other}"),
        }
    }

    fn render(&self, genome: &[u8]) -> Result<Image, DispatchError> {
        render_image(
            genome,
            self.avatar_type,
            self.version,
            self.mood,
            self.size_px,
            &self.options,
        )
    }

    /// The golden's expected pixels: the committed tree, rasterized.
    fn rasterize_committed_tree(&self) -> Image {
        rasterize_tree(&self.committed_tree(), self.size_px, &self.options)
            .unwrap_or_else(|error| panic!("rasterize {}: {error}", self.tree))
    }

    /// The committed golden tree, read through the tree fixture reader.
    fn committed_tree(&self) -> avatar_svg::RenderTree {
        let bytes = fs::read(corpus_root().join(&self.tree))
            .unwrap_or_else(|error| panic!("read vectors/{}: {error}", self.tree));
        crate::tree_fixture::decode_tree_fixture(&bytes)
            .unwrap_or_else(|error| panic!("parse vectors/{}: {error}", self.tree))
    }
}

/// A type's name without `creature.`, which names its golden directory.
fn stem(avatar_type: &str) -> &str {
    avatar_type
        .strip_prefix("creature.")
        .expect("avatar types start with creature.")
}

/// The rows of a newer type's `vectors/<stem>/corpus.tsv`.
fn newer_type_corpus(avatar_type: &str) -> Vec<WildVector> {
    let path = corpus_root().join(stem(avatar_type)).join("corpus.tsv");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    parse_corpus(&text)
}

/// Every combination of the two options, with the defaults first.
fn every_option_combination() -> Vec<ImageOptions> {
    let mut combinations = Vec::new();
    for ground_shadow in [GroundShadow::Keep, GroundShadow::Drop] {
        for alpha in [Alpha::Premultiplied, Alpha::Straight] {
            combinations.push(ImageOptions {
                ground_shadow,
                alpha,
            });
        }
    }
    combinations
}

/// The golden matrix, in manifest order.
fn matrix() -> Vec<Cell> {
    let corpus = frozen_vectors();
    let default = ImageOptions::default();
    let mut cells = Vec::new();

    // The twelve wild genomes at version 1, neutral, at every size.
    let version_1 = corpus
        .iter()
        .filter(|row| row.version == 1)
        .collect::<Vec<_>>();
    assert_eq!(version_1.len(), 12, "the wild corpus has twelve genomes");
    for row in version_1 {
        assert!(row.moods.iter().any(|mood| mood == "neutral"));
        for size_px in WILD_VERSION_1_SIZES {
            cells.push(Cell::wild(row, Mood::Neutral, size_px, default));
        }
    }

    // One wild genome at version 2: every mood, then neutral under every
    // combination of options.
    let version_2 = corpus
        .iter()
        .find(|row| row.version == 2 && row.project_id == WILD_VERSION_2_GENOME)
        .expect("the wild corpus commits version 2 trees for one genome");
    assert_eq!(version_2.moods.len(), ALL_MOODS.len());
    for &mood in ALL_MOODS {
        cells.push(Cell::wild(version_2, mood, CASE_SIZE_PX, default));
    }
    for options in every_option_combination() {
        cells.push(Cell::wild(version_2, Mood::Neutral, CASE_SIZE_PX, options));
    }

    // The first classic vector, neutral.
    let classic = super::read_manifest()
        .vectors
        .into_iter()
        .next()
        .expect("the classic corpus has vectors");
    assert!(classic.moods.iter().any(|mood| mood == "neutral"));
    cells.push(Cell {
        avatar_type: CLASSIC,
        genome_hex: relative_to_corpus(&dna_path(&classic.name)),
        tree: relative_to_corpus(&tree_path(&classic.name, Mood::Neutral)),
        genome: classic.name,
        version: CLASSIC_VERSION,
        mood: Mood::Neutral,
        size_px: CASE_SIZE_PX,
        options: default,
    });

    // Each newer type: its twelve corpus genomes at version 1, neutral, then
    // one of them in neutral under every combination of options.
    for avatar_type in NEWER_TYPES {
        let rows = newer_type_corpus(avatar_type);
        assert_eq!(
            rows.len(),
            12,
            "the {avatar_type} corpus has twelve genomes"
        );
        for row in &rows {
            assert_eq!(
                row.version, NEWER_TYPE_VERSION,
                "{avatar_type} {}: corpus version",
                row.project_id
            );
            assert!(row.moods.iter().any(|mood| mood == "neutral"));
            cells.push(Cell::newer_type(avatar_type, row, default));
        }
        let options_row = rows
            .iter()
            .find(|row| row.project_id == NEWER_TYPE_OPTIONS_GENOME)
            .unwrap_or_else(|| panic!("the {avatar_type} corpus has {NEWER_TYPE_OPTIONS_GENOME}"));
        for options in every_option_combination() {
            cells.push(Cell::newer_type(avatar_type, options_row, options));
        }
    }

    // A key listed twice, such as version 2 neutral with the default
    // options, keeps its first place only.
    let mut seen = BTreeSet::new();
    cells.retain(|cell| seen.insert(cell.file_name()));
    cells
}

fn ground_shadow_name(ground_shadow: GroundShadow) -> &'static str {
    match ground_shadow {
        GroundShadow::Keep => "keep",
        GroundShadow::Drop => "drop",
    }
}

fn alpha_name(alpha: Alpha) -> &'static str {
    match alpha {
        Alpha::Premultiplied => "premultiplied",
        Alpha::Straight => "straight",
    }
}

fn relative_to_corpus(path: &Path) -> String {
    path.strip_prefix(corpus_root())
        .unwrap_or_else(|_| panic!("{} is outside vectors/", path.display()))
        .to_str()
        .expect("corpus paths are UTF-8")
        .replace('\\', "/")
}

fn image_root() -> PathBuf {
    corpus_root().join(IMAGE_DIRECTORY)
}

fn image_manifest_path() -> PathBuf {
    image_root().join("manifest.json")
}

fn image_manifest_bytes() -> Vec<u8> {
    let manifest = ImageManifest {
        entries: matrix().iter().map(Cell::entry).collect(),
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest).expect("serialize image manifest");
    bytes.push(b'\n');
    bytes
}

/// Writes the pixels verbatim as 8-bit RGBA.
fn encode_png(image: &Image) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.size_px, image.size_px);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    encoder.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
    let mut writer = encoder.write_header().expect("write PNG header");
    writer
        .write_image_data(&image.pixels)
        .expect("write PNG pixels");
    writer.finish().expect("finish PNG");
    bytes
}

/// Reads a golden PNG back into the image it was written from.
fn read_golden(cell: &Cell) -> Image {
    let label = cell.label();
    let bytes = fs::read(image_root().join(cell.file_name()))
        .unwrap_or_else(|error| panic!("read {label}: {error}"));
    let mut reader = png::Decoder::new(bytes.as_slice())
        .read_info()
        .unwrap_or_else(|error| panic!("read {label} header: {error}"));
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut pixels)
        .unwrap_or_else(|error| panic!("decode {label}: {error}"));
    assert_eq!(
        (frame.color_type, frame.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight),
        "{label} is not 8-bit RGBA"
    );
    assert_eq!(
        (frame.width, frame.height),
        (cell.size_px, cell.size_px),
        "{label} has the wrong size"
    );
    pixels.truncate(frame.buffer_size());
    Image {
        size_px: frame.width,
        alpha: cell.options.alpha,
        pixels,
    }
}

/// The largest channel difference the cross-platform image rule allows.
const TOLERANCE_CHANNEL: u8 = 8;

/// How a finished image differs from its golden, pixel by pixel.
///
/// Render trees are byte-identical on every platform. Finished images are
/// identical on one platform, but may differ slightly between platforms,
/// because the rasterizer uses the platform's maths library. When tiny-skia
/// 0.11.4 clips a cubic curve at the edge of the area being drawn
/// (`edge_clipper.rs`, through `path_geometry::chop_mono_cubic_at_y`), it
/// solves for the crossing with `path64::cubic64::roots_real`. That function
/// calls `f64::acos` and `f64::cos`, which come from the system's maths
/// library and can differ in the last bit between, for example, macOS and
/// Linux. A different last bit can move a clipped edge by one sample of the
/// anti-aliasing grid, which changes a pixel's coverage by 1/16.
///
/// So an image matches its golden if its pixels are identical, or if no
/// channel differs by more than [`TOLERANCE_CHANNEL`] and at most
/// `max(1, pixels / 1000)` pixels differ (0.1%). The Swift golden-image test
/// and `scripts/wild-port/compare_images.py` apply the same rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ImageDifference {
    /// The largest difference in any channel of any pixel.
    max_channel: u8,
    /// How many pixels differ in at least one channel.
    differing_pixels: usize,
    /// How many pixels the image has.
    pixels: usize,
}

impl ImageDifference {
    /// Compares two RGBA8 pixel buffers of the same length.
    fn between(expected: &[u8], actual: &[u8]) -> Self {
        assert_eq!(
            expected.len(),
            actual.len(),
            "compared images differ in length"
        );
        let (expected, _) = expected.as_chunks::<4>();
        let (actual, _) = actual.as_chunks::<4>();
        let mut difference = Self {
            max_channel: 0,
            differing_pixels: 0,
            pixels: expected.len(),
        };
        for (expected, actual) in expected.iter().zip(actual) {
            if expected != actual {
                difference.differing_pixels += 1;
                for (expected, actual) in expected.iter().zip(actual) {
                    difference.max_channel = difference.max_channel.max(expected.abs_diff(*actual));
                }
            }
        }
        difference
    }

    fn is_identical(&self) -> bool {
        self.differing_pixels == 0
    }

    /// How many pixels may differ: 0.1% of the image, but at least one.
    fn allowed_differing_pixels(&self) -> usize {
        (self.pixels / 1_000).max(1)
    }

    fn is_within_tolerance(&self) -> bool {
        self.max_channel <= TOLERANCE_CHANNEL
            && self.differing_pixels <= self.allowed_differing_pixels()
    }
}

/// The platform an image was drawn on, as the image reports name it.
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Checks an image against its golden under the cross-platform rule (see
/// [`ImageDifference`]). Size, alpha form and length must be equal. Every
/// image that is not identical is printed with its platform, case, largest
/// channel difference and differing pixel count, so drift stays visible even
/// when it passes. A failure names the first differing pixel rather than
/// printing two multi-megabyte dumps. Returns whether the image was identical.
fn assert_matches_golden(label: &str, expected: &Image, actual: &Image, remedy: &str) -> bool {
    assert_eq!(
        (actual.size_px, actual.alpha, actual.pixels.len()),
        (expected.size_px, expected.alpha, expected.pixels.len()),
        "{label}: size, alpha form or length differs"
    );
    let difference = ImageDifference::between(&expected.pixels, &actual.pixels);
    if difference.is_identical() {
        return true;
    }
    let measured = format!(
        "{} {label}: max channel difference {}/255, {} of {} pixels differ",
        platform(),
        difference.max_channel,
        difference.differing_pixels,
        difference.pixels
    );
    if difference.is_within_tolerance() {
        println!("{measured}; within the image tolerance");
        return false;
    }
    let index = expected
        .pixels
        .iter()
        .zip(&actual.pixels)
        .position(|(expected, actual)| expected != actual)
        .expect("a differing image has a differing byte");
    let pixel = index / 4;
    panic!(
        "{measured}, beyond the image tolerance of {TOLERANCE_CHANNEL}/255 on at most {} pixels. Pixel ({}, {}) is {:?}, expected {:?}. {remedy}",
        difference.allowed_differing_pixels(),
        pixel as u32 % expected.size_px,
        pixel as u32 / expected.size_px,
        &actual.pixels[pixel * 4..pixel * 4 + 4],
        &expected.pixels[pixel * 4..pixel * 4 + 4],
    );
}

#[test]
#[ignore = "set UPDATE_IMAGE_GOLDEN to mint the golden images"]
fn mint_images() {
    assert!(
        std::env::var_os("UPDATE_IMAGE_GOLDEN").is_some(),
        "set UPDATE_IMAGE_GOLDEN=1 to rewrite vectors/{IMAGE_DIRECTORY}"
    );
    let root = image_root();
    // Start empty, so an image whose cell left the matrix does not linger.
    if root.exists() {
        fs::remove_dir_all(&root).expect("clear the image goldens");
    }
    fs::create_dir_all(&root).expect("create the image golden directory");
    for cell in matrix() {
        let image = cell.rasterize_committed_tree();
        write_corpus_file(&root.join(cell.file_name()), &encode_png(&image));
    }
    write_corpus_file(&image_manifest_path(), &image_manifest_bytes());
}

#[test]
fn image_manifest_lists_the_matrix_and_the_directory_holds_exactly_it() {
    if update_golden_enabled() {
        return;
    }
    assert_current(&image_manifest_path(), &image_manifest_bytes());

    let cells = matrix();
    // Wild: 12 genomes at 3 sizes, 7 moods, 3 more option combinations; then
    // 1 classic; then for each newer type 12 genomes and 3 more option
    // combinations.
    assert_eq!(cells.len(), 36 + 7 + 3 + 1 + NEWER_TYPES.len() * (12 + 3));
    let keys = cells
        .iter()
        .map(|cell| {
            let entry = cell.entry();
            (
                entry.avatar_type,
                entry.genome,
                entry.version,
                entry.mood,
                entry.size_px,
                entry.ground_shadow,
                entry.alpha,
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(keys.len(), cells.len(), "two entries share a key");

    let mut names = cells.iter().map(Cell::file_name).collect::<BTreeSet<_>>();
    assert_eq!(names.len(), cells.len(), "two entries share a file");
    names.insert("manifest.json".to_owned());
    assert_file_names(IMAGE_DIRECTORY, &names);

    // Every image has a committed tree and genome.
    for cell in &cells {
        for source in [&cell.tree, &cell.genome_hex] {
            assert!(
                corpus_root().join(source).is_file(),
                "{}: vectors/{source} is not committed",
                cell.label()
            );
        }
    }
}

#[test]
fn golden_images_are_the_committed_trees_rasterized() {
    if update_golden_enabled() {
        return;
    }
    let remedy = format!(
        "If the rasterizer or the committed tree changed on purpose, re-mint with {MINT_COMMAND}."
    );
    let cells = matrix();
    let mut identical = 0;
    for cell in &cells {
        identical += usize::from(assert_matches_golden(
            &cell.label(),
            &read_golden(cell),
            &cell.rasterize_committed_tree(),
            &remedy,
        ));
    }
    println!(
        "{}: {identical} of {} committed trees rasterize to their golden images exactly",
        platform(),
        cells.len()
    );
}

/// `render_image` must draw, from the stored genome, exactly the image this
/// platform rasterizes from the committed tree, and that image must match the
/// golden under the cross-platform rule.
#[test]
fn render_image_matches_every_golden_image() {
    if update_golden_enabled() {
        return;
    }
    let cells = matrix();
    let mut identical = 0;
    for cell in &cells {
        let image = cell
            .render(&cell.genome_bytes())
            .unwrap_or_else(|error| panic!("{}: render_image failed: {error}", cell.label()));
        // Both images are drawn here from the same tree, so the platform's
        // maths library cannot tell them apart: they must be identical.
        assert!(
            image == cell.rasterize_committed_tree(),
            "{}: render_image no longer draws the committed tree from its stored genome",
            cell.label()
        );
        identical += usize::from(assert_matches_golden(
            &cell.label(),
            &read_golden(cell),
            &image,
            "render_image no longer draws the committed tree from its stored genome.",
        ));
    }
    println!(
        "{}: render_image matched {} golden images, {identical} of them exactly",
        platform(),
        cells.len()
    );
}

/// A golden-like image whose channels leave room to move up by a few levels.
fn tolerance_test_pixels(size_px: usize) -> Vec<u8> {
    (0..size_px * size_px * 4)
        .map(|index| u8::try_from(index * 37 % 200 + 20).expect("20..220 fits a byte"))
        .collect()
}

/// `pixels` copied with the green channel of its first `count` pixels raised
/// by `by` levels.
fn raise_green(pixels: &[u8], count: usize, by: u8) -> Vec<u8> {
    let mut changed = pixels.to_vec();
    for pixel in 0..count {
        changed[pixel * 4 + 1] += by;
    }
    changed
}

/// Identical images match. Up to 8/255 on up to max(1, 0.1%) of the pixels
/// matches. One more level, or one more pixel, does not.
#[test]
fn image_tolerance_bounds_the_channel_difference_and_the_pixel_count() {
    // 65,536 pixels allow 65 to differ; 256 pixels allow 1.
    for (size_px, allowed) in [(256, 65), (16, 1)] {
        let golden = tolerance_test_pixels(size_px);
        let pixels = size_px * size_px;

        let identical = ImageDifference::between(&golden, &golden);
        assert!(identical.is_identical());
        assert!(identical.is_within_tolerance());

        let within =
            ImageDifference::between(&golden, &raise_green(&golden, allowed, TOLERANCE_CHANNEL));
        assert_eq!(
            within,
            ImageDifference {
                max_channel: TOLERANCE_CHANNEL,
                differing_pixels: allowed,
                pixels,
            }
        );
        assert!(!within.is_identical());
        assert!(within.is_within_tolerance(), "{size_px} px: {within:?}");

        let over_difference =
            ImageDifference::between(&golden, &raise_green(&golden, 1, TOLERANCE_CHANNEL + 1));
        assert_eq!(over_difference.differing_pixels, 1);
        assert!(
            !over_difference.is_within_tolerance(),
            "{size_px} px: {over_difference:?}"
        );

        let over_count = ImageDifference::between(&golden, &raise_green(&golden, allowed + 1, 1));
        assert_eq!(over_count.max_channel, 1);
        assert!(
            !over_count.is_within_tolerance(),
            "{size_px} px: {over_count:?}"
        );
    }
}

/// The golden-image tests accept an image within the tolerance and report it
/// as not identical.
#[test]
fn an_image_within_the_tolerance_matches_its_golden() {
    let golden = tolerance_test_pixels(16);
    let image = |pixels| Image {
        size_px: 16,
        alpha: Alpha::Premultiplied,
        pixels,
    };
    assert!(assert_matches_golden(
        "identical",
        &image(golden.clone()),
        &image(golden.clone()),
        "",
    ));
    assert!(!assert_matches_golden(
        "within",
        &image(golden.clone()),
        &image(raise_green(&golden, 1, TOLERANCE_CHANNEL)),
        "",
    ));
}

/// The golden-image tests fail an image beyond the tolerance.
#[test]
#[should_panic(expected = "beyond the image tolerance")]
fn an_image_beyond_the_tolerance_fails_its_golden() {
    let golden = tolerance_test_pixels(16);
    let image = |pixels| Image {
        size_px: 16,
        alpha: Alpha::Premultiplied,
        pixels,
    };
    assert_matches_golden(
        "beyond",
        &image(golden.clone()),
        &image(raise_green(&golden, 2, 1)),
        "",
    );
}

/// A newer type tags its reference's ground shadow, if the reference draws
/// one, and nothing else. So dropping the shadow must change the images of
/// exactly the types whose reference draws one, in both alpha forms. For the
/// other types nothing is dropped and the images stay equal.
#[test]
fn dropping_the_ground_shadow_changes_the_images_of_exactly_the_shadowed_types() {
    if update_golden_enabled() {
        return;
    }
    let cells = matrix();
    let golden = |avatar_type: &str, options: ImageOptions| {
        let cell = cells
            .iter()
            .find(|cell| {
                cell.avatar_type == avatar_type
                    && cell.genome == NEWER_TYPE_OPTIONS_GENOME
                    && cell.mood == Mood::Neutral
                    && cell.options == options
            })
            .unwrap_or_else(|| {
                panic!("no {avatar_type} {NEWER_TYPE_OPTIONS_GENOME} image with {options:?}")
            });
        (cell, read_golden(cell))
    };
    for avatar_type in NEWER_TYPES {
        let draws_shadow = GROUND_SHADOW_TYPES.contains(&avatar_type);
        for alpha in [Alpha::Premultiplied, Alpha::Straight] {
            let keep = ImageOptions {
                ground_shadow: GroundShadow::Keep,
                alpha,
            };
            let drop = ImageOptions {
                ground_shadow: GroundShadow::Drop,
                ..keep
            };
            let (kept_cell, kept) = golden(avatar_type, keep);
            let (_, dropped) = golden(avatar_type, drop);
            let tagged = kept_cell
                .committed_tree()
                .primitives
                .iter()
                .any(|primitive| primitive.role == Some(Role::GroundShadow));
            assert_eq!(
                tagged, draws_shadow,
                "{}: a ground shadow is tagged exactly when the reference draws one",
                kept_cell.tree
            );
            assert_eq!(
                kept.pixels != dropped.pixels,
                draws_shadow,
                "{avatar_type} {alpha:?}: dropping the ground shadow changes the image exactly when the reference draws one"
            );
        }
    }
}

#[test]
fn eight_threads_render_the_golden_matrix_like_a_sequential_run() {
    const THREADS: usize = 8;
    let cells = matrix();
    let genomes = cells.iter().map(Cell::genome_bytes).collect::<Vec<_>>();
    let sequential = cells
        .iter()
        .zip(&genomes)
        .map(|(cell, genome)| cell.render(genome).expect("render_image succeeds"))
        .collect::<Vec<_>>();

    let mismatches = std::thread::scope(|scope| {
        let workers = (0..THREADS)
            .map(|thread| {
                let (cells, genomes, sequential) = (&cells, &genomes, &sequential);
                scope.spawn(move || {
                    // Each thread starts at a different cell, so different
                    // cells are being drawn at the same moment.
                    let start = thread * cells.len() / THREADS;
                    let mut mismatches = Vec::new();
                    for step in 0..cells.len() {
                        let index = (start + step) % cells.len();
                        let image = cells[index].render(&genomes[index]);
                        if image.as_ref() != Ok(&sequential[index]) {
                            mismatches.push(format!("thread {thread}: {}", cells[index].label()));
                        }
                    }
                    mismatches
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a render thread panicked"))
            .collect::<Vec<_>>()
    });
    assert!(
        mismatches.is_empty(),
        "concurrent renders differ from the sequential run: {mismatches:?}"
    );
}

/// Times the whole `render_image` call, from stored bytes to RGBA, at 288 px
/// with the default options, on the wild corpus genome that draws the most
/// primitives at neutral. Run it in release:
/// `cargo test --release -p avatar-vectors tests::image::render_image_timing_at_288_px -- --exact --ignored --nocapture`.
#[test]
#[ignore = "timing probe; run in release"]
fn render_image_timing_at_288_px() {
    const SIZE_PX: u32 = 288;
    const VERSION: u32 = 2;
    const WARM_UP: usize = 20;
    const CALLS: usize = 200;
    const MEDIAN_LIMIT: Duration = Duration::from_millis(20);

    fn count(primitives: &[avatar_svg::Primitive]) -> usize {
        primitives
            .iter()
            .map(|primitive| {
                1 + primitive
                    .effects
                    .iter()
                    .map(|effect| match effect {
                        avatar_svg::Effect::Masked { mask } => count(mask),
                        _ => 0,
                    })
                    .sum::<usize>()
            })
            .sum()
    }

    let project_ids = frozen_vectors()
        .into_iter()
        .map(|row| row.project_id)
        .collect::<BTreeSet<_>>();
    let (primitives, project_id, genome) = project_ids
        .into_iter()
        .map(|project_id| {
            let genome = read_genome(&project_id);
            let tree = avatar::render_tree_for_bytes(&genome, WILD, VERSION, Mood::Neutral)
                .expect("a corpus genome renders");
            (count(&tree.primitives), project_id, genome)
        })
        .max_by(|left, right| left.0.cmp(&right.0).then(right.1.cmp(&left.1)))
        .expect("the wild corpus has genomes");

    let options = ImageOptions::default();
    let call = || {
        std::hint::black_box(
            render_image(
                std::hint::black_box(&genome),
                WILD,
                VERSION,
                Mood::Neutral,
                SIZE_PX,
                &options,
            )
            .expect("render_image succeeds"),
        )
    };
    for _ in 0..WARM_UP {
        call();
    }
    let mut times = (0..CALLS)
        .map(|_| {
            let start = Instant::now();
            call();
            start.elapsed()
        })
        .collect::<Vec<_>>();
    times.sort_unstable();
    let median = (times[CALLS / 2 - 1] + times[CALLS / 2]) / 2;
    // Nearest rank: the smallest time at least 95% of calls do not exceed.
    let p95 = times[(CALLS * 95).div_ceil(100) - 1];
    println!(
        "render_image timing: {project_id} (wild version {VERSION}, neutral, {primitives} primitives) at {SIZE_PX} px, premultiplied, shadow kept; {CALLS} calls after {WARM_UP} warm-up: median {median:?}, p95 {p95:?}, min {:?}, max {:?}",
        times[0],
        times[CALLS - 1]
    );
    assert!(
        median <= MEDIAN_LIMIT,
        "median {median:?} exceeds {MEDIAN_LIMIT:?}"
    );
}
