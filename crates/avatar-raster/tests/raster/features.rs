//! The committed tree-feature fixtures, drawn and checked pixel by pixel
//! against what SVG defines for each feature.

use std::fs;

use avatar_raster::{rasterize_tree, Alpha, Image, ImageOptions};
use avatar_svg::{
    serialize_tree, serialize_tree_json, Effect, FillRule, GradientStop, Paint, RenderTree, Rgb,
    Segment, Shape, Stroke, StrokeCap, StrokeJoin, Subpath, Q32,
};

use crate::{assert_near, opaque, pixel, primitive, vectors, BLUE, RED, TRANSPARENT, WHITE};

const ELLIPSE: Shape = Shape::Ellipse {
    cx: 500,
    cy: 500,
    rx: 400,
    ry: 300,
};

fn tree(primitives: Vec<avatar_svg::Primitive>) -> RenderTree {
    RenderTree {
        canvas: 10,
        background: None,
        pixel_grid: None,
        primitives,
    }
}

fn stop(offset: Q32, color: Rgb, opacity: f64) -> GradientStop {
    GradientStop::with_opacity(
        offset,
        color,
        Q32((opacity * Q32::ONE.0 as f64).round() as i64),
    )
}

fn contours() -> Shape {
    let square = |lo, hi| Subpath {
        closed: true,
        segments: vec![
            Segment::MoveTo { x: lo, y: lo },
            Segment::LineTo { x: hi, y: lo },
            Segment::LineTo { x: hi, y: hi },
            Segment::LineTo { x: lo, y: hi },
        ],
    };
    Shape::CompoundPath {
        subpaths: vec![square(100, 900), square(400, 600)],
        fill_rule: FillRule::EvenOdd,
    }
}

/// Rebuilds the named fixture of `vectors/tree-features/` and checks that it
/// is the committed one: its canonical JSON and its SVG both match the files
/// byte for byte, so the pixels below are the committed fixture's pixels.
pub fn fixture(name: &str) -> RenderTree {
    let built = match name {
        "radial-gradient-at" => tree(vec![primitive(
            ELLIPSE,
            Paint::radial_gradient_at(
                500,
                500,
                300,
                Some((350, 450)),
                vec![stop(Q32(0), RED, 1.0), stop(Q32::ONE, BLUE, 1.0)],
            ),
        )]),
        "stop-opacity" => tree(vec![primitive(
            ELLIPSE,
            Paint::RadialGradient {
                stops: vec![stop(Q32(0), BLUE, 0.25), stop(Q32::ONE, BLUE, 1.0)],
            },
        )]),
        "compound-path" => tree(vec![primitive(contours(), Paint::Solid { color: BLUE })]),
        "compound-path-clip" => {
            let mut clipped = primitive(
                Shape::Rect {
                    x: 100,
                    y: 100,
                    w: 800,
                    h: 800,
                },
                Paint::Solid { color: BLUE },
            );
            clipped.effects.push(Effect::Masked {
                mask: vec![primitive(contours(), Paint::Solid { color: WHITE })],
            });
            tree(vec![clipped])
        }
        "compound-path-mixed" => {
            let contour = |start: (i64, i64), rest: &[(i64, i64)], closed| Subpath {
                closed,
                segments: std::iter::once(Segment::MoveTo {
                    x: start.0,
                    y: start.1,
                })
                .chain(rest.iter().map(|&(x, y)| Segment::LineTo { x, y }))
                .collect(),
            };
            let mut mixed = primitive(
                Shape::CompoundPath {
                    subpaths: vec![
                        contour((100, 300), &[(800, 300), (800, 400)], false),
                        contour((300, 300), &[(700, 300), (700, 700), (300, 700)], true),
                        contour((300, 100), &[(300, 800), (400, 800)], false),
                    ],
                    fill_rule: FillRule::NonZero,
                },
                Paint::None,
            );
            mixed.stroke = Some(Stroke {
                color: BLUE,
                width: 60,
                cap: StrokeCap::Round,
                join: StrokeJoin::Round,
            });
            mixed.opacity = Some(Q32(Q32::ONE.0 / 2));
            tree(vec![mixed])
        }
        "radial-gradient-in-box" => tree(vec![primitive(
            ELLIPSE,
            Paint::radial_gradient_in_box(
                Q32(1_803_886_264),
                Q32(1_546_188_227),
                Q32(3_092_376_453),
                Some((Q32(1_546_188_227), Q32(1_202_590_843))),
                vec![
                    stop(Q32(0), WHITE, 0.2),
                    stop(Q32(1_803_886_264), WHITE, 0.0),
                    stop(Q32(3_178_275_799), Rgb { r: 0, g: 0, b: 0 }, 0.05),
                    stop(Q32::ONE, Rgb { r: 0, g: 0, b: 0 }, 0.3),
                ],
            ),
        )]),
        "pixel-grid" => {
            let cell = |x, y, color| {
                primitive(
                    Shape::Rect {
                        x,
                        y,
                        w: 200,
                        h: 200,
                    },
                    Paint::Solid { color },
                )
            };
            RenderTree {
                canvas: 10,
                background: None,
                pixel_grid: Some(5),
                primitives: vec![
                    cell(200, 200, BLUE),
                    cell(400, 200, RED),
                    cell(200, 400, BLUE),
                ],
            }
        }
        other => panic!("no tree-feature fixture named {other}"),
    };
    let root = vectors().join("tree-features");
    let json = fs::read_to_string(root.join(format!("{name}.json"))).unwrap();
    assert_eq!(serialize_tree_json(&built), json, "{name} JSON");
    let svg = fs::read(root.join(format!("{name}.svg"))).unwrap();
    assert_eq!(serialize_tree(&built), svg, "{name} SVG");
    built
}

fn draw(tree: &RenderTree, size_px: u32, alpha: Alpha) -> Image {
    let options = ImageOptions {
        alpha,
        ..ImageOptions::default()
    };
    let image = rasterize_tree(tree, size_px, &options).unwrap();
    assert_eq!(image.size_px, size_px);
    assert_eq!(image.alpha, alpha);
    assert_eq!(image.pixels.len(), (size_px * size_px * 4) as usize);
    image
}

/// The gradient position SVG assigns to point `p` for a radial gradient whose
/// end circle has centre `c` and radius `r` and whose focal point is `f`. The
/// gradient is a family of circles growing from a point at the focus (0) to
/// the end circle (1); the answer is the member that passes through `p`.
fn gradient_t(p: (f64, f64), c: (f64, f64), r: f64, f: (f64, f64)) -> f64 {
    let d = (p.0 - f.0, p.1 - f.1);
    let e = (c.0 - f.0, c.1 - f.1);
    let d_dot_e = d.0 * e.0 + d.1 * e.1;
    let d_dot_d = d.0 * d.0 + d.1 * d.1;
    // Negative, because the focus lies strictly inside the end circle.
    let a = e.0 * e.0 + e.1 * e.1 - r * r;
    (d_dot_e - (d_dot_e * d_dot_e - a * d_dot_d).sqrt()) / a
}

/// Pixel centres, in canvas units, on a coarse lattice well inside the
/// fixtures' ellipse (centre 5,5, radii 4 and 3), where its anti-aliased edge
/// cannot reach.
fn interior_samples(size_px: u32) -> Vec<(u32, u32, (f64, f64))> {
    let scale = f64::from(size_px) / 10.0;
    let mut samples = Vec::new();
    for y in (0..size_px).step_by(5) {
        for x in (0..size_px).step_by(5) {
            let p = ((f64::from(x) + 0.5) / scale, (f64::from(y) + 0.5) / scale);
            let (u, v) = ((p.0 - 5.0) / 4.0, (p.1 - 5.0) / 3.0);
            if u * u + v * v < 0.81 {
                samples.push((x, y, p));
            }
        }
    }
    assert!(samples.len() > 500, "too few interior samples");
    samples
}

#[test]
fn focal_gradient_starts_at_the_focus_with_the_inner_stop() {
    let tree = fixture("radial-gradient-at");
    // At 25 px per unit the focus (3.5, 4.5) is the centre of pixel (87, 112).
    let image = draw(&tree, 250, Alpha::Straight);
    assert_near(pixel(&image, 87, 112), opaque(RED), 2, "focal pixel");

    // Every interior pixel follows the user-space focal gradient: red at the
    // focus, blue on and beyond the circle of radius 3 about (5, 5).
    for (x, y, p) in interior_samples(250) {
        let t = gradient_t(p, (5.0, 5.0), 3.0, (3.5, 4.5)).clamp(0.0, 1.0);
        let mix = |from: u8, to: u8| {
            (f64::from(from) + (f64::from(to) - f64::from(from)) * t).round() as u8
        };
        let expected = [
            mix(RED.r, BLUE.r),
            mix(RED.g, BLUE.g),
            mix(RED.b, BLUE.b),
            255,
        ];
        assert_near(
            pixel(&image, x, y),
            expected,
            2,
            &format!("pixel ({x}, {y}), t {t:.3}"),
        );
    }
}

#[test]
fn stop_opacity_sets_the_alpha_at_its_offset() {
    let tree = fixture("stop-opacity");
    // At 25.1 px per unit the ellipse centre (5, 5), where the gradient is at
    // its first stop, is the centre of pixel (125, 125).
    let image = draw(&tree, 251, Alpha::Straight);
    let quarter = (0.25_f64 * 255.0).round() as u8;
    assert_near(
        pixel(&image, 125, 125),
        [BLUE.r, BLUE.g, BLUE.b, quarter],
        2,
        "centre",
    );

    // The default bounding-box gradient stretches to the ellipse, so opacity
    // rises from 0.25 at the centre to 1 at the ellipse's edge.
    let premultiplied = draw(&tree, 251, Alpha::Premultiplied);
    for (x, y, p) in interior_samples(251) {
        let (u, v) = ((p.0 - 5.0) / 4.0, (p.1 - 5.0) / 3.0);
        let t = (u * u + v * v).sqrt();
        let alpha = (255.0 * (0.25 + 0.75 * t)).round() as u8;
        let actual = pixel(&premultiplied, x, y);
        assert!(
            actual[3].abs_diff(alpha) <= 2,
            "pixel ({x}, {y}): alpha {} not {alpha}",
            actual[3]
        );
    }
}

#[test]
fn bounding_box_focal_gradient_stretches_with_the_shape() {
    let tree = fixture("radial-gradient-in-box");
    let image = draw(&tree, 250, Alpha::Premultiplied);
    let fraction = |raw: i64| raw as f64 / Q32::ONE.0 as f64;
    let centre = (fraction(1_803_886_264), fraction(1_546_188_227));
    let radius = fraction(3_092_376_453);
    let focus = (fraction(1_546_188_227), fraction(1_202_590_843));
    // (offset, opacity) as the SVG writes the stops.
    let stops = [(0.0, 0.2), (0.42, 0.0), (0.74, 0.05), (1.0, 0.3)];
    let opacity = |t: f64| {
        let t = t.clamp(0.0, 1.0);
        let pair = stops.windows(2).find(|pair| t <= pair[1].0).unwrap();
        let ((lo, from), (hi, to)) = (pair[0], pair[1]);
        from + (to - from) * (t - lo) / (hi - lo)
    };

    // The ellipse's bounding box runs from (1, 2) to (9, 8). Bounding-box
    // units map it to the unit square, so a circle there is an ellipse 8 wide
    // and 6 high on the canvas.
    for (x, y, p) in interior_samples(250) {
        let unit = ((p.0 - 1.0) / 8.0, (p.1 - 2.0) / 6.0);
        let t = gradient_t(unit, centre, radius, focus);
        let alpha = (255.0 * opacity(t)).round() as u8;
        let actual = pixel(&image, x, y);
        assert!(
            actual[3].abs_diff(alpha) <= 2,
            "pixel ({x}, {y}), t {t:.3}: alpha {} not {alpha}",
            actual[3]
        );
    }
}

#[test]
fn even_odd_hole_is_transparent_when_painted_and_when_clipped() {
    for name in ["compound-path", "compound-path-clip"] {
        let tree = fixture(name);
        // 10 px per unit: the hole spans pixels 40 to 59 and the ring 10 to 89.
        for alpha in [Alpha::Premultiplied, Alpha::Straight] {
            let image = draw(&tree, 100, alpha);
            assert_eq!(pixel(&image, 50, 50), TRANSPARENT, "{name} hole");
            assert_eq!(pixel(&image, 25, 25), opaque(BLUE), "{name} ring");
            assert_eq!(pixel(&image, 5, 5), TRANSPARENT, "{name} outside");
        }
    }
}

#[test]
fn nonzero_hole_needs_the_opposite_winding() {
    let outer = Subpath {
        closed: true,
        segments: vec![
            Segment::MoveTo { x: 100, y: 100 },
            Segment::LineTo { x: 900, y: 100 },
            Segment::LineTo { x: 900, y: 900 },
            Segment::LineTo { x: 100, y: 900 },
        ],
    };
    let inner = |corners: [(i64, i64); 4]| Subpath {
        closed: true,
        segments: vec![
            Segment::MoveTo {
                x: corners[0].0,
                y: corners[0].1,
            },
            Segment::LineTo {
                x: corners[1].0,
                y: corners[1].1,
            },
            Segment::LineTo {
                x: corners[2].0,
                y: corners[2].1,
            },
            Segment::LineTo {
                x: corners[3].0,
                y: corners[3].1,
            },
        ],
    };
    let same = inner([(400, 400), (600, 400), (600, 600), (400, 600)]);
    let opposite = inner([(400, 400), (400, 600), (600, 600), (600, 400)]);
    for (inner, hole) in [(same, opaque(BLUE)), (opposite, TRANSPARENT)] {
        let shape = Shape::CompoundPath {
            subpaths: vec![outer.clone(), inner],
            fill_rule: FillRule::NonZero,
        };
        let image = draw(
            &tree(vec![primitive(shape, Paint::Solid { color: BLUE })]),
            100,
            Alpha::Premultiplied,
        );
        assert_eq!(pixel(&image, 50, 50), hole);
        assert_eq!(pixel(&image, 25, 25), opaque(BLUE));
    }
}

#[test]
fn open_contours_are_stroked_without_a_closing_segment() {
    let tree = fixture("compound-path-mixed");
    let image = draw(&tree, 100, Alpha::Straight);
    // On the segments a Z would add: (8, 4) back to (1, 3), and (4, 8) back
    // to (3, 1). Each probe is 0.55 units from every drawn stroke, which is
    // 0.3 units wide on either side.
    assert_eq!(pixel(&image, 45, 35), TRANSPARENT, "first contour closing");
    assert_eq!(pixel(&image, 35, 45), TRANSPARENT, "third contour closing");
    // A drawn stroke, at the element's half opacity.
    assert_near(
        pixel(&image, 60, 30),
        [BLUE.r, BLUE.g, BLUE.b, 128],
        2,
        "stroke",
    );
}

#[test]
fn an_open_contour_fills_as_closed_but_strokes_as_open() {
    let triangle = |closed| {
        let mut drawn = primitive(
            Shape::CompoundPath {
                subpaths: vec![Subpath {
                    closed,
                    segments: vec![
                        Segment::MoveTo { x: 100, y: 100 },
                        Segment::LineTo { x: 900, y: 100 },
                        Segment::LineTo { x: 100, y: 900 },
                    ],
                }],
                fill_rule: FillRule::NonZero,
            },
            Paint::Solid { color: BLUE },
        );
        drawn.stroke = Some(Stroke {
            color: RED,
            width: 100,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
        });
        tree(vec![drawn])
    };
    // The contour runs (1, 1), (9, 1), (1, 9), so closing it would add the
    // left edge from (1, 9) back to (1, 1). Pixel (12, 50) lies just inside
    // that edge and pixel (7, 50) just outside it, both within the stroke's
    // half width of 0.5 units.
    let open = draw(&triangle(false), 100, Alpha::Premultiplied);
    assert_eq!(pixel(&open, 25, 25), opaque(BLUE));
    assert_eq!(pixel(&open, 12, 50), opaque(BLUE), "filled up to the gap");
    assert_eq!(pixel(&open, 7, 50), TRANSPARENT, "no closing stroke");

    let closed = draw(&triangle(true), 100, Alpha::Premultiplied);
    assert_eq!(pixel(&closed, 25, 25), opaque(BLUE));
    assert_eq!(pixel(&closed, 12, 50), opaque(RED));
    assert_eq!(pixel(&closed, 7, 50), opaque(RED));
}
