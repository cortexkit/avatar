//! Pixel-grid trees: whole device pixels per cell, centred, with hard edges.

use avatar_raster::{rasterize_svg, rasterize_tree, Alpha, Image, ImageOptions};
use avatar_svg::{serialize_tree, Paint, RenderTree, Rgb, Shape};

use crate::features::fixture;
use crate::{opaque, pixels, primitive, BLUE, RED, TRANSPARENT};

/// The painted cells of the pixel-grid fixture's 5 × 5 grid, as
/// (column, row, colour).
const FIXTURE_CELLS: [(u32, u32, Rgb); 3] = [(1, 1, BLUE), (2, 1, RED), (1, 2, BLUE)];

fn options(alpha: Alpha) -> ImageOptions {
    ImageOptions {
        alpha,
        ..ImageOptions::default()
    }
}

/// The image a grid of `cells` × `cells` should give at `size_px`, written
/// out pixel by pixel: each cell `size_px / cells` pixels wide, the drawing
/// centred, the odd spare pixel on the right and bottom, and every pixel
/// outside a painted cell transparent.
fn expected_grid(size_px: u32, cells: u32, painted: &[(u32, u32, Rgb)]) -> Vec<u8> {
    let cell_px = size_px / cells;
    let margin = (size_px - cell_px * cells) / 2;
    let mut pixels = vec![0; (size_px * size_px * 4) as usize];
    for &(column, row, color) in painted {
        let left = margin + column * cell_px;
        let top = margin + row * cell_px;
        for y in top..top + cell_px {
            for x in left..left + cell_px {
                let start = ((y * size_px + x) * 4) as usize;
                pixels[start..start + 4].copy_from_slice(&opaque(color));
            }
        }
    }
    pixels
}

fn assert_pixels(image: &Image, expected: &[u8], context: &str) {
    assert_eq!(image.pixels.len(), expected.len(), "{context}: length");
    let size = image.size_px as usize;
    let (actual, expected) = (pixels(&image.pixels), pixels(expected));
    if let Some(index) = (0..actual.len()).find(|&index| actual[index] != expected[index]) {
        panic!(
            "{context}: pixel ({}, {}) is {:?}, expected {:?}",
            index % size,
            index / size,
            actual[index],
            expected[index],
        );
    }
}

#[test]
fn grid_cells_are_whole_pixels_centred_with_transparent_margins() {
    let tree = fixture("pixel-grid");
    // 5, 50 and 250 divide evenly into the 5 cells. 53 leaves 3 spare pixels
    // (margins 1 and 2), 256 and 1,081 leave 1 (margins 0 and 1).
    for size_px in [5, 50, 250, 53, 256, 1_081] {
        let expected = expected_grid(size_px, 5, &FIXTURE_CELLS);
        for alpha in [Alpha::Premultiplied, Alpha::Straight] {
            let image = rasterize_tree(&tree, size_px, &options(alpha)).unwrap();
            assert_pixels(&image, &expected, &format!("{size_px} px, {alpha:?}"));
        }
    }
}

#[test]
fn grid_draws_curves_and_clips_without_anti_aliasing() {
    let mut disc = primitive(
        Shape::Circle {
            cx: 500,
            cy: 500,
            r: 330,
        },
        Paint::Solid { color: BLUE },
    );
    disc.clip = Some(Shape::Ellipse {
        cx: 450,
        cy: 550,
        rx: 310,
        ry: 270,
    });
    let grid = RenderTree {
        canvas: 10,
        background: None,
        pixel_grid: Some(10),
        primitives: vec![disc],
    };
    let smooth = RenderTree {
        pixel_grid: None,
        ..grid.clone()
    };
    for size_px in [100, 97] {
        let crisp = rasterize_tree(&grid, size_px, &options(Alpha::Premultiplied)).unwrap();
        assert!(
            pixels(&crisp.pixels)
                .iter()
                .all(|&p| p == TRANSPARENT || p == opaque(BLUE)),
            "{size_px} px: a pixel-grid image has only hard edges"
        );
        assert!(pixels(&crisp.pixels).contains(&opaque(BLUE)));
        // The same tree off the grid does anti-alias, so the check above
        // could fail.
        let soft = rasterize_tree(&smooth, size_px, &options(Alpha::Premultiplied)).unwrap();
        assert!(pixels(&soft.pixels).iter().any(|p| p[3] > 0 && p[3] < 255));
    }
}

#[test]
fn below_one_pixel_per_cell_the_grid_tree_draws_smoothly_scaled() {
    let tree = fixture("pixel-grid");
    let smooth = RenderTree {
        pixel_grid: None,
        ..tree.clone()
    };
    for size_px in [1, 4] {
        for alpha in [Alpha::Premultiplied, Alpha::Straight] {
            let image = rasterize_tree(&tree, size_px, &options(alpha)).unwrap();
            let ordinary = rasterize_svg(&serialize_tree(&smooth), size_px, alpha).unwrap();
            assert_eq!(image, ordinary, "{size_px} px, {alpha:?}");
        }
    }
    // At 4 px a cell is 0.8 px wide and straddles pixel edges, so the
    // anti-aliasing shows as partial alpha.
    let image = rasterize_tree(&tree, 4, &options(Alpha::Premultiplied)).unwrap();
    assert!(pixels(&image.pixels).iter().any(|p| p[3] > 0 && p[3] < 255));
}
