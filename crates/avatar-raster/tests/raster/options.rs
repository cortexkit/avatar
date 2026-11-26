//! Image options, size limits and SVG errors.

use avatar_raster::{
    rasterize_svg, rasterize_tree, Alpha, GroundShadow, Image, ImageOptions, RasterError,
    MAX_SIZE_PX,
};
use avatar_svg::{serialize_tree, Paint, RenderTree, Rgb, Role, Shape, Q32};

use crate::features::fixture;
use crate::{assert_near, opaque, pixel, pixels, primitive, straight, TRANSPARENT};

const SHADOW: Rgb = Rgb {
    r: 40,
    g: 20,
    b: 60,
};
const BODY: Rgb = Rgb {
    r: 250,
    g: 150,
    b: 30,
};

/// A body over two identical quarter-opacity shadows. Only the left one is
/// tagged as the ground shadow; the right one has the same shape, paint,
/// opacity and label.
fn shadowed_tree() -> RenderTree {
    let shadow = |cx, role| {
        let mut drawn = primitive(
            Shape::Ellipse {
                cx,
                cy: 800,
                rx: 200,
                ry: 60,
            },
            Paint::Solid { color: SHADOW },
        );
        drawn.opacity = Some(Q32(Q32::ONE.0 / 4));
        drawn.label = "shadow".to_owned();
        drawn.role = role;
        drawn
    };
    RenderTree {
        canvas: 10,
        background: None,
        pixel_grid: None,
        primitives: vec![
            shadow(300, Some(Role::GroundShadow)),
            shadow(700, None),
            primitive(
                Shape::Circle {
                    cx: 500,
                    cy: 400,
                    r: 250,
                },
                Paint::Solid { color: BODY },
            ),
        ],
    }
}

fn draw(tree: &RenderTree, ground_shadow: GroundShadow, alpha: Alpha) -> Image {
    let image = rasterize_tree(
        tree,
        100,
        &ImageOptions {
            ground_shadow,
            alpha,
        },
    )
    .unwrap();
    assert_eq!(image.size_px, 100);
    assert_eq!(image.alpha, alpha);
    assert_eq!(image.pixels.len(), 100 * 100 * 4);
    image
}

#[test]
fn default_options_keep_the_shadow_and_premultiply() {
    assert_eq!(
        ImageOptions::default(),
        ImageOptions {
            ground_shadow: GroundShadow::Keep,
            alpha: Alpha::Premultiplied,
        }
    );
}

#[test]
fn dropping_the_shadow_removes_only_the_tagged_primitive_under_every_option() {
    let tree = shadowed_tree();
    let mut untagged_only = tree.clone();
    untagged_only.primitives.remove(0);
    // 10 px per unit: the tagged shadow is centred on pixel (30, 80), its
    // untagged twin on (70, 80) and the body on (50, 40).
    let quarter = (0.25_f64 * 255.0).round() as u8;
    for ground_shadow in [GroundShadow::Keep, GroundShadow::Drop] {
        let expected_tree = match ground_shadow {
            GroundShadow::Keep => &tree,
            GroundShadow::Drop => &untagged_only,
        };
        for alpha in [Alpha::Premultiplied, Alpha::Straight] {
            let context = format!("{ground_shadow:?}, {alpha:?}");
            let image = draw(&tree, ground_shadow, alpha);
            // Drawing the tree without the tagged primitive, with the shadow
            // kept, gives the same image: nothing else was removed.
            assert_eq!(
                image,
                draw(expected_tree, GroundShadow::Keep, alpha),
                "{context}"
            );

            let twin = pixel(&image, 70, 80);
            assert!(
                twin[3].abs_diff(quarter) <= 2,
                "{context}: the untagged twin stays, alpha {}",
                twin[3]
            );
            assert_eq!(pixel(&image, 50, 40), opaque(BODY), "{context}: body");
            let tagged = pixel(&image, 30, 80);
            match ground_shadow {
                GroundShadow::Keep => assert_eq!(tagged, twin, "{context}"),
                GroundShadow::Drop => assert_eq!(tagged, TRANSPARENT, "{context}"),
            }
            if alpha == Alpha::Straight {
                assert_near(
                    twin,
                    [SHADOW.r, SHADOW.g, SHADOW.b, quarter],
                    2,
                    &format!("{context}: straight shadow colour"),
                );
            }
        }
    }
    assert_ne!(
        draw(&tree, GroundShadow::Keep, Alpha::Premultiplied),
        draw(&tree, GroundShadow::Drop, Alpha::Premultiplied)
    );
}

#[test]
fn straight_alpha_follows_the_integer_formula_on_every_pixel() {
    let tree = shadowed_tree();
    for ground_shadow in [GroundShadow::Keep, GroundShadow::Drop] {
        let premultiplied = draw(&tree, ground_shadow, Alpha::Premultiplied);
        let converted = draw(&tree, ground_shadow, Alpha::Straight);

        // A partial-alpha pixel, where the conversion changes the colour.
        let twin = pixel(&premultiplied, 70, 80);
        assert!(twin[3] > 0 && twin[3] < 255, "{twin:?}");
        assert_ne!(straight(twin), twin);
        assert_eq!(pixel(&converted, 70, 80), straight(twin));
        // A pixel with a = 0.
        assert_eq!(pixel(&premultiplied, 0, 0)[3], 0);
        assert_eq!(pixel(&converted, 0, 0), TRANSPARENT);

        for (index, (&actual, &source)) in pixels(&converted.pixels)
            .iter()
            .zip(pixels(&premultiplied.pixels))
            .enumerate()
        {
            assert_eq!(actual, straight(source), "{ground_shadow:?}, pixel {index}");
        }
    }
}

#[test]
fn sizes_outside_one_to_2048_are_refused() {
    let tree = fixture("compound-path");
    let svg = serialize_tree(&tree);
    assert_eq!(MAX_SIZE_PX, 2_048);
    for size_px in [0, 2_049, u32::MAX] {
        let refused = Err(RasterError::ImageSize { requested: size_px });
        for alpha in [Alpha::Premultiplied, Alpha::Straight] {
            assert_eq!(rasterize_svg(&svg, size_px, alpha), refused);
            // The size is checked before the SVG is parsed.
            assert_eq!(rasterize_svg(b"not svg", size_px, alpha), refused);
            let options = ImageOptions {
                alpha,
                ..ImageOptions::default()
            };
            assert_eq!(rasterize_tree(&tree, size_px, &options), refused);
        }
    }
}

#[test]
fn sizes_one_and_2048_give_exactly_size_squared_pixels() {
    // The grid fixture takes the smoothly scaled route at 1 px and the
    // pixel-grid route at 2,048 px.
    let trees = [fixture("compound-path"), fixture("pixel-grid")];
    for size_px in [1, 2_048] {
        let length = (size_px * size_px * 4) as usize;
        for tree in &trees {
            for alpha in [Alpha::Premultiplied, Alpha::Straight] {
                let options = ImageOptions {
                    alpha,
                    ..ImageOptions::default()
                };
                let image = rasterize_tree(tree, size_px, &options).unwrap();
                assert_eq!((image.size_px, image.pixels.len()), (size_px, length));
                let image = rasterize_svg(&serialize_tree(tree), size_px, alpha).unwrap();
                assert_eq!((image.size_px, image.pixels.len()), (size_px, length));
            }
        }
    }
}

#[test]
fn svg_that_does_not_parse_is_refused() {
    for svg in [&b"not svg"[..], b"<svg", b""] {
        assert!(matches!(
            rasterize_svg(svg, 16, Alpha::Premultiplied),
            Err(RasterError::InvalidSvg { .. })
        ));
    }
}
