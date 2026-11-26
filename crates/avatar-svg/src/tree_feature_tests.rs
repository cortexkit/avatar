use super::*;
use crate::{serialize_tree, serialize_tree_json};

const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
const MOVE: Segment = Segment::MoveTo { x: 0, y: 0 };
const LINE: Segment = Segment::LineTo { x: 0, y: 1 };

fn tree(shape: Shape, paint: Paint) -> RenderTree {
    RenderTree {
        canvas: 10,
        background: None,
        pixel_grid: None,
        primitives: vec![Primitive {
            shape,
            transform: None,
            paint,
            stroke: None,
            opacity: None,
            clip: None,
            effects: Vec::new(),
            label: String::new(),
            role: None,
        }],
    }
}

const CIRCLE: Shape = Shape::Circle { cx: 0, cy: 0, r: 4 };

#[test]
fn optional_focus_and_stop_opacity_are_omitted_or_explicit() {
    let gradient = |focal, opacity_q32| {
        tree(
            CIRCLE,
            Paint::radial_gradient_at(
                0,
                0,
                400,
                focal,
                vec![GradientStop {
                    offset_q32: Q32(0),
                    color: BLACK,
                    opacity_q32,
                }],
            ),
        )
    };
    let absent = gradient(None, None);
    let json = serialize_tree_json(&absent);
    assert!(json.contains(r#""paint":{"cx":0,"cy":0,"kind":"radial_gradient_at","r":400,"stops":[{"color":{"b":0,"g":0,"r":0},"offset_q32":0}],"units":"user_space"}"#), "{json}");
    let svg = String::from_utf8(serialize_tree(&absent)).unwrap();
    assert!(
        svg.contains(
            r##"cx="0" cy="0" r="4" spreadMethod="pad"><stop offset="0" stop-color="#000000"/>"##
        ),
        "{svg}"
    );

    let present = gradient(Some((0, -25)), Some(Q32(0)));
    let json = serialize_tree_json(&present);
    assert!(
        json.contains(r#""cx":0,"cy":0,"fx":0,"fy":-25,"kind":"radial_gradient_at""#),
        "{json}"
    );
    assert!(json.contains(r#""offset_q32":0,"opacity_q32":0"#), "{json}");
    let svg = String::from_utf8(serialize_tree(&present)).unwrap();
    assert!(
        svg.contains(r#"r="4" fx="0" fy="-0.25" spreadMethod="pad""#),
        "{svg}"
    );
    assert!(svg.contains(r#"stop-opacity="0""#), "{svg}");
}

macro_rules! invalid_values {
    ($module:ident, $serialize:ident) => {
        mod $module {
            use super::*;

            fn radial(r: i64, focal: Option<(i64, i64)>) {
                $serialize(&tree(
                    CIRCLE,
                    Paint::RadialGradientAt {
                        units: GradientUnits::UserSpace,
                        cx: 0,
                        cy: 0,
                        r,
                        focal,
                        stops: vec![],
                    },
                ));
            }

            fn compound(subpaths: Vec<Vec<Segment>>) {
                $serialize(&tree(
                    Shape::CompoundPath {
                        subpaths: subpaths
                            .into_iter()
                            .map(|segments| Subpath {
                                segments,
                                closed: true,
                            })
                            .collect(),
                        fill_rule: FillRule::EvenOdd,
                    },
                    Paint::None,
                ));
            }

            #[test]
            #[should_panic(expected = "gradient stop opacity must be in [0, 1)")]
            fn refuses_explicit_opaque_stop() {
                $serialize(&tree(
                    CIRCLE,
                    Paint::RadialGradient {
                        stops: vec![GradientStop {
                            offset_q32: Q32(0),
                            color: BLACK,
                            opacity_q32: Some(Q32::ONE),
                        }],
                    },
                ));
            }

            #[test]
            #[should_panic(expected = "radial gradient radius must be positive")]
            fn refuses_zero_radius() {
                radial(0, None);
            }

            #[test]
            #[should_panic(expected = "radial gradient radius must be positive")]
            fn refuses_negative_radius() {
                radial(-1, None);
            }

            #[test]
            #[should_panic(expected = "radial gradient focus must be strictly inside the circle")]
            fn refuses_on_circle_focus() {
                radial(5, Some((3, 4)));
            }

            #[test]
            #[should_panic(expected = "compound path must have a subpath")]
            fn refuses_empty_compound_path() {
                compound(vec![]);
            }

            #[test]
            #[should_panic(expected = "compound subpath must start with its only move")]
            fn refuses_missing_move() {
                compound(vec![vec![LINE, LINE]]);
            }

            #[test]
            #[should_panic(expected = "compound subpath must start with its only move")]
            fn refuses_misplaced_move() {
                compound(vec![vec![MOVE, LINE, MOVE]]);
            }

            #[test]
            #[should_panic(expected = "compound subpath must start with its only move")]
            fn refuses_move_only() {
                compound(vec![vec![MOVE]]);
            }

            #[test]
            #[should_panic(expected = "pixel grid must have at least one cell")]
            fn refuses_zero_pixel_grid() {
                let mut tree = tree(CIRCLE, Paint::None);
                tree.pixel_grid = Some(0);
                $serialize(&tree);
            }
        }
    };
}
invalid_values!(json, serialize_tree_json);
invalid_values!(svg, serialize_tree);

#[test]
fn opacity_clamps_and_canonicalizes_after_quantization() {
    let opacity = |value| GradientStop::with_opacity(Q32(0), BLACK, value).opacity_q32;
    assert_eq!(opacity(1.0 - 1.0 / (1_u64 << 34) as f64), None);
    assert_eq!(opacity(2.0), None);
    assert_eq!(opacity(-1.0), Some(Q32(0)));
    assert_eq!(opacity(0.5), Some(Q32(Q32::ONE.0 / 2)));
}

fn focal(cx: i64, cy: i64, r: i64, fx: i64, fy: i64) -> (i64, i64) {
    let Paint::RadialGradientAt {
        focal: Some(point), ..
    } = Paint::radial_gradient_at(cx, cy, r, Some((fx, fy)), vec![])
    else {
        panic!("constructor dropped the focus");
    };
    point
}

#[test]
fn constructor_moves_outside_and_on_circle_foci() {
    assert_eq!(focal(0, 0, 4, 4, 4), (2, 3));
    assert_eq!(focal(0, 0, 4, 4, 0), (3, 0));
}

#[test]
#[should_panic(expected = "radial gradient radius must be positive")]
fn constructor_rejects_zero_radius() {
    focal(0, 0, 0, 0, 0);
}

#[test]
#[should_panic(expected = "radial gradient constructor coordinates must be within ±2³¹")]
fn constructor_rejects_out_of_range_focus() {
    focal(0, 0, 4, (1 << 31) + 1, 0);
}

#[test]
fn constructor_matches_brute_force_over_small_disks() {
    // The oracle scans every interior point in signed lexicographic order.
    // Use a translated centre so the comparison also checks local offsets.
    let (cx, cy) = (15, -20);
    for r in 1_i32..=64 {
        let disk: Vec<_> = (-r..=r)
            .flat_map(|x| (-r..=r).map(move |y| (x, y)))
            .filter(|&(x, y)| x * x + y * y < r * r)
            .collect();
        for fx in -3 * r..=3 * r {
            for fy in -3 * r..=3 * r {
                let (mut distance, mut best) = (i32::MAX, (0, 0));
                for &(x, y) in &disk {
                    let (dx, dy) = (fx - x, fy - y);
                    let d = dx * dx + dy * dy;
                    if d < distance {
                        (distance, best) = (d, (x, y));
                    }
                }
                let actual = focal(cx, cy, i64::from(r), cx + i64::from(fx), cy + i64::from(fy));
                assert_eq!(
                    actual,
                    (cx + i64::from(best.0), cy + i64::from(best.1)),
                    "r={r} target delta=({fx},{fy})"
                );
            }
        }
    }
}

#[test]
fn constructor_handles_coordinate_and_radius_limits() {
    let limit = 1_i64 << 31;
    let start = std::time::Instant::now();
    assert_eq!(focal(-limit, limit, limit, limit, limit), (-1, limit));
    // The greatest possible sum of the reflected coordinates is 3,037,000,499.
    // Its two balanced points minimize squared distance; smaller signed x wins.
    assert_eq!(
        focal(limit, limit, limit, -limit, -limit),
        (628_983_398, 628_983_399)
    );
    eprintln!(
        "focus search at coordinate and radius limits: {:?}",
        start.elapsed()
    );
}

fn box_focal(cx: i64, cy: i64, r: i64, fx: i64, fy: i64) -> (i64, i64) {
    let Paint::RadialGradientAt {
        units: GradientUnits::ObjectBoundingBox,
        focal: Some(point),
        ..
    } = Paint::radial_gradient_in_box(Q32(cx), Q32(cy), Q32(r), Some((Q32(fx), Q32(fy))), vec![])
    else {
        panic!("expected a positioned bounding-box focus");
    };
    point
}

#[test]
fn box_constructor_clamps_in_fraction_space() {
    assert_eq!(box_focal(0, 0, 4, 4, 4), (2, 3));
    assert_eq!(box_focal(0, 0, 4, 4, 0), (3, 0));
}

#[test]
#[should_panic(expected = "radial gradient radius must be positive")]
fn box_constructor_rejects_zero_radius() {
    box_focal(0, 0, 0, 0, 0);
}

#[test]
#[should_panic(expected = "bounding-box radial gradient constructor fractions must be within ±1")]
fn box_constructor_rejects_out_of_range_fraction() {
    box_focal(0, 0, 4, Q32::ONE.0 + 1, 0);
}

#[test]
fn box_constructor_handles_fraction_and_radius_limits() {
    let limit = Q32::ONE.0;
    let start = std::time::Instant::now();
    assert_eq!(box_focal(-limit, limit, limit, limit, limit), (-1, limit));
    assert_eq!(
        box_focal(limit, limit, limit, -limit, -limit),
        (1_257_966_796, 1_257_966_797)
    );
    eprintln!(
        "bounding-box focus search at fraction and radius limits: {:?}",
        start.elapsed()
    );
}

#[test]
fn svg_bounding_box_radius_keeps_one_q32_tick() {
    let gradient = Paint::radial_gradient_in_box(Q32(0), Q32(0), Q32(1), None, vec![]);
    let svg = String::from_utf8(serialize_tree(&tree(CIRCLE, gradient))).unwrap();
    assert!(
        svg.contains(r#"r="0.00000000023283064365386962890625""#),
        "{svg}"
    );
    assert!(!svg.contains("gradientUnits"), "{svg}");
}

#[test]
fn unspecified_pixel_grid_adds_no_json_key_or_svg_attribute() {
    let tree = tree(CIRCLE, Paint::None);
    assert!(!serialize_tree_json(&tree).contains("pixel_grid"));
    assert!(!String::from_utf8(serialize_tree(&tree))
        .unwrap()
        .contains("shape-rendering"));
}
