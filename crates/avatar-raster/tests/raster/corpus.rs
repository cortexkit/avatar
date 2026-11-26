//! Real creatures: the image of a generated tree equals resvg's own render of
//! that tree's committed golden SVG in `vectors/tree-svg/`.

use std::fs;

use avatar_genome::{decode, Genome};
use avatar_raster::{rasterize_tree, Alpha, ImageOptions};
use avatar_svg::wild::{self, Generation};
use avatar_svg::{build_render_tree, Effect, Mood, RenderParams, RenderTree, Q32};
use resvg::{tiny_skia, usvg};

use crate::{pixels, vectors};

const SIZE_PX: u32 = 256;

fn genome_bytes(path: &str) -> Vec<u8> {
    let hex = fs::read_to_string(vectors().join(path)).unwrap();
    let hex = hex.trim().as_bytes();
    assert_eq!(hex.len() % 2, 0, "{path}: odd hex length");
    hex.as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// Renders a committed SVG with resvg directly, scaled to `size_px` across,
/// on a transparent pixmap: the reference the raster crate must reproduce.
fn resvg_render(svg_path: &str, size_px: u32) -> Vec<u8> {
    let svg = fs::read(vectors().join(svg_path)).unwrap();
    let document = usvg::Tree::from_data(&svg, &usvg::Options::default()).unwrap();
    let mut pixmap = tiny_skia::Pixmap::new(size_px, size_px).unwrap();
    let scale = size_px as f32 / document.size().width();
    resvg::render(
        &document,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.take()
}

fn assert_matches_golden(tree: &RenderTree, svg_path: &str) {
    let image = rasterize_tree(tree, SIZE_PX, &ImageOptions::default()).unwrap();
    assert_eq!(image.alpha, Alpha::Premultiplied);
    let expected = resvg_render(svg_path, SIZE_PX);
    assert!(
        pixels(&expected).iter().any(|pixel| pixel[3] == 255),
        "{svg_path}: the reference render is empty"
    );
    assert!(image.pixels == expected, "{svg_path}: pixels differ");
}

#[test]
fn classic_image_equals_the_render_of_its_golden_svg() {
    let bytes: [u8; 1_024] = genome_bytes("dna/all_zero.hex").try_into().unwrap();
    let tree = build_render_tree(
        &decode(&Genome::from_bytes(bytes)),
        &RenderParams {
            mood: Mood::Neutral,
            growth: Q32::ONE,
            canvas: 10_000,
        },
    )
    .unwrap();
    assert_matches_golden(&tree, "tree-svg/all_zero__neutral.svg");
}

#[test]
fn masked_and_transformed_wild_image_equals_the_render_of_its_golden_svg() {
    let bytes: [u8; wild::GENOME_LEN] = genome_bytes("wild/wild-port-0077.hex").try_into().unwrap();
    let tree = wild::render_tree(&bytes, Mood::Neutral, Generation::V1);
    // This genome is chosen because it exercises both: a pattern mask over
    // part of the body, and transformed primitives.
    assert!(tree.primitives.iter().any(|primitive| primitive
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::Masked { .. }))));
    assert!(tree
        .primitives
        .iter()
        .any(|primitive| primitive.transform.is_some()));
    assert_matches_golden(&tree, "tree-svg/wild-port-0077__neutral.svg");
}
