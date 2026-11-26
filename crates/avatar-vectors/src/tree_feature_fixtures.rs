use std::{fs, path::PathBuf};

use avatar_svg::{serialize_tree, serialize_tree_json};
use serde_json::{json, Value};

use super::*;

const BLUE: Rgb = Rgb {
    r: 0,
    g: 96,
    b: 224,
};
const RED: Rgb = Rgb {
    r: 240,
    g: 32,
    b: 16,
};
const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};
const ELLIPSE: Shape = Shape::Ellipse {
    cx: 500,
    cy: 500,
    rx: 400,
    ry: 300,
};
const RECT: Shape = Shape::Rect {
    x: 100,
    y: 100,
    w: 800,
    h: 800,
};

fn primitive(shape: Shape, paint: Paint) -> Primitive {
    Primitive {
        shape,
        paint,
        transform: None,
        stroke: None,
        opacity: None,
        clip: None,
        effects: vec![],
        label: String::new(),
        role: None,
    }
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

fn cases() -> [(&'static str, RenderTree); 7] {
    let tree = |primitive| RenderTree {
        canvas: 10,
        background: None,
        pixel_grid: None,
        primitives: vec![primitive],
    };
    let stop = GradientStop::with_opacity;
    let positioned = tree(primitive(
        ELLIPSE,
        Paint::radial_gradient_at(
            500,
            500,
            300,
            Some((350, 450)),
            vec![stop(Q32(0), RED, 1.0), stop(Q32::ONE, BLUE, 1.0)],
        ),
    ));
    let opacity = tree(primitive(
        ELLIPSE,
        Paint::RadialGradient {
            stops: vec![stop(Q32(0), BLUE, 0.25), stop(Q32::ONE, BLUE, 1.0)],
        },
    ));
    let compound = tree(primitive(contours(), Paint::Solid { color: BLUE }));
    let mut clipped = primitive(RECT, Paint::Solid { color: BLUE });
    clipped.effects.push(Effect::Masked {
        mask: vec![primitive(contours(), Paint::Solid { color: WHITE })],
    });
    let mixed = mixed_contours();
    let box_gradient = tree(primitive(
        ELLIPSE,
        Paint::radial_gradient_in_box(
            Q32(1_803_886_264),                             // .42
            Q32(1_546_188_227),                             // .36
            Q32(3_092_376_453),                             // .72
            Some((Q32(1_546_188_227), Q32(1_202_590_843))), // (.36, .28)
            vec![
                stop(Q32(0), WHITE, 0.2),
                stop(Q32(1_803_886_264), WHITE, 0.0),
                stop(Q32(3_178_275_799), Rgb { r: 0, g: 0, b: 0 }, 0.05),
                stop(Q32::ONE, Rgb { r: 0, g: 0, b: 0 }, 0.3),
            ],
        ),
    ));
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
    let grid = RenderTree {
        canvas: 10,
        background: None,
        pixel_grid: Some(5),
        primitives: vec![
            cell(200, 200, BLUE),
            cell(400, 200, RED),
            cell(200, 400, BLUE),
        ],
    };
    [
        ("radial-gradient-at", positioned),
        ("stop-opacity", opacity),
        ("compound-path", compound),
        ("compound-path-clip", tree(clipped)),
        ("compound-path-mixed", tree(mixed)),
        ("radial-gradient-in-box", box_gradient),
        ("pixel-grid", grid),
    ]
}

fn mixed_contours() -> Primitive {
    let contour = |start: (i64, i64), rest: &[(i64, i64)], closed| {
        let segments = std::iter::once(Segment::MoveTo {
            x: start.0,
            y: start.1,
        })
        .chain(rest.iter().map(|&(x, y)| Segment::LineTo { x, y }))
        .collect();
        Subpath { closed, segments }
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
    mixed
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors/tree-features")
}

#[test]
#[ignore = "set UPDATE_TREE_FEATURE_GOLDEN to mint the tree feature fixtures"]
fn mint_tree_features() {
    assert!(std::env::var_os("UPDATE_TREE_FEATURE_GOLDEN").is_some());
    let root = root();
    fs::create_dir_all(&root).unwrap();
    for (name, tree) in cases() {
        fs::write(
            root.join(format!("{name}.json")),
            serialize_tree_json(&tree),
        )
        .unwrap();
        fs::write(root.join(format!("{name}.svg")), serialize_tree(&tree)).unwrap();
    }
}

#[test]
fn feature_trees_round_trip_and_match_svg() {
    const CONTOURS: &str = "M1 1 L9 1 L9 9 L1 9 Z M4 4 L6 4 L6 6 L4 6 Z";
    let expected = [
        vec![
            r##"<radialGradient id="gradient-primitive-0" gradientUnits="userSpaceOnUse" cx="5" cy="5" r="3" fx="3.5" fy="4.5" spreadMethod="pad"><stop offset="0" stop-color="#f02010"/><stop offset="1" stop-color="#0060e0"/></radialGradient>"##.to_owned(),
            r##"<ellipse cx="5" cy="5" rx="4" ry="3" fill="url(#gradient-primitive-0)"/>"##.to_owned(),
        ],
        vec![r##"<radialGradient id="gradient-primitive-0"><stop offset="0" stop-color="#0060e0" stop-opacity="0.25"/><stop offset="1" stop-color="#0060e0"/></radialGradient>"##.to_owned()],
        vec![format!(r##"<path d="{CONTOURS}" fill-rule="evenodd" fill="#0060e0"/>"##)],
        vec![
            format!(r#"<clipPath id="mask-group-0-0" clipPathUnits="userSpaceOnUse"><path d="{CONTOURS}" clip-rule="evenodd"/></clipPath>"#),
            r##"<g clip-path="url(#mask-group-0-0)"><rect x="1" y="1" width="8" height="8" fill="#0060e0"/>"##.to_owned(),
        ],
        vec![r##"<path d="M1 3 L8 3 L8 4 M3 3 L7 3 L7 7 L3 7 Z M3 1 L3 8 L4 8" fill-rule="nonzero" fill="none" stroke="#0060e0" stroke-width="0.6" stroke-linecap="round" stroke-linejoin="round" opacity="0.5"/>"##.to_owned()],
        vec![
            r#"<radialGradient id="gradient-primitive-0" cx="0.41999999992549419403076171875" cy="0.36000000010244548320770263671875" r="0.71999999997206032276153564453125" fx="0.36000000010244548320770263671875" fy="0.28000000002793967723846435546875" spreadMethod="pad">"#.to_owned(),
            r##"<stop offset="0" stop-color="#ffffff" stop-opacity="0.2"/><stop offset="0.42" stop-color="#ffffff" stop-opacity="0"/><stop offset="0.74" stop-color="#000000" stop-opacity="0.05"/><stop offset="1" stop-color="#000000" stop-opacity="0.3"/>"##.to_owned(),
            r##"<ellipse cx="5" cy="5" rx="4" ry="3" fill="url(#gradient-primitive-0)"/>"##.to_owned(),
        ],
        vec![
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" shape-rendering="crispEdges">"#.to_owned(),
            r##"<rect x="2" y="2" width="2" height="2" fill="#0060e0"/>"##.to_owned(),
        ],
    ];
    for ((name, tree), markup) in cases().into_iter().zip(expected) {
        let json = fs::read(root().join(format!("{name}.json"))).unwrap();
        let decoded = decode_tree_fixture(&json).unwrap();
        assert_eq!(tree, decoded, "{name}");
        assert_eq!(serialize_tree_json(&decoded).as_bytes(), json, "{name}");
        let svg = fs::read_to_string(root().join(format!("{name}.svg"))).unwrap();
        assert_eq!(serialize_tree(&decoded), svg.as_bytes(), "{name}");
        for fragment in markup {
            assert!(svg.contains(&fragment), "{name} missing {fragment}: {svg}");
        }
        assert!(!svg.contains("<mask"), "{name} unexpectedly emitted a mask");
    }
}

fn document() -> Value {
    let [(_, positioned), ..] = cases();
    serde_json::from_str(&serialize_tree_json(&positioned)).unwrap()
}

fn refuses(document: &Value, expected: &str) {
    let bytes = serde_json::to_vec(document).unwrap();
    let Err(FixtureError::Json(error)) = decode_tree_fixture(&bytes) else {
        panic!("expected a schema error for {document}");
    };
    assert!(
        error.to_string().contains(expected),
        "expected {expected}, got {error} for {document}"
    );
}

#[test]
fn refuses_explicit_null_or_partial_focus() {
    for coordinate in ["fx", "fy"] {
        let mut doc = document();
        doc["primitives"][0]["paint"][coordinate] = Value::Null;
        refuses(&doc, "invalid type: null");
        doc["primitives"][0]["paint"]
            .as_object_mut()
            .unwrap()
            .remove(coordinate);
        refuses(&doc, "fx and fy must be supplied together");
    }
}

#[test]
fn refuses_null_and_out_of_range_stop_opacity() {
    for (opacity, expected) in [
        (Value::Null, "invalid type: null"),
        (json!(-1), "gradient stop opacity must be in [0, 1)"),
        (json!(Q32::ONE.0), "gradient stop opacity must be in [0, 1)"),
    ] {
        let mut doc = document();
        doc["primitives"][0]["paint"]["stops"][0]["opacity_q32"] = opacity;
        refuses(&doc, expected);
    }
}

#[test]
fn refuses_invalid_radius_or_focus_without_overflow() {
    for units in ["user_space", "object_bounding_box"] {
        let gradient = |cx, cy, r, fx, fy| {
            let mut doc = document();
            doc["primitives"][0]["paint"] = json!({"kind":"radial_gradient_at", "units":units, "cx":cx, "cy":cy, "r":r, "fx":fx, "fy":fy, "stops":[]});
            doc
        };
        for r in [0, -1, i64::MIN] {
            refuses(&gradient(0, 0, r, 0, 0), "radius must be positive");
        }
        for doc in [
            gradient(0, 0, 5, 3, 4),
            gradient(0, 0, 5, 4, 4),
            gradient(i64::MIN, 0, i64::MAX, i64::MAX, 0),
            gradient(0, i64::MAX, i64::MAX, 0, i64::MIN),
            gradient(0, 0, i64::MAX, i64::MAX - 1, i64::MAX - 1),
        ] {
            refuses(&doc, "focus must be strictly inside the circle");
        }
    }
}

#[test]
fn refuses_empty_or_malformed_compound_paths() {
    let mv = json!({"kind":"move_to","x":0,"y":0});
    let line = json!({"kind":"line_to","x":1,"y":1});
    for (subpaths, expected) in [
        (json!([]), "compound path must have a subpath"),
        (
            json!([{"closed":true,"segments":[]}]),
            "compound subpath must start with its only move",
        ),
        (
            json!([{"closed":false,"segments":[mv]}]),
            "compound subpath must start with its only move",
        ),
        (
            json!([{"closed":true,"segments":[line,line]}]),
            "compound subpath must start with its only move",
        ),
        (
            json!([{"closed":false,"segments":[mv,line,mv]}]),
            "compound subpath must start with its only move",
        ),
        (json!([{"segments":[mv,line]}]), "missing field `closed`"),
        (
            json!([{"closed":null,"segments":[mv,line]}]),
            "invalid type: null",
        ),
        (json!([[false, [mv, line]]]), "invalid type: sequence"),
    ] {
        let mut doc = document();
        doc["primitives"][0]["shape"] =
            json!({"kind":"compound_path", "fill_rule":"even_odd", "subpaths":subpaths});
        refuses(&doc, expected);
    }
}

#[test]
fn positioned_gradient_requires_known_units() {
    let mut doc = document();
    doc["primitives"][0]["paint"]["units"] = json!("screen_space");
    refuses(&doc, "unknown variant `screen_space`");
    doc["primitives"][0]["paint"]["units"] = Value::Null;
    refuses(&doc, "invalid type: null");
    doc["primitives"][0]["paint"]
        .as_object_mut()
        .unwrap()
        .remove("units");
    refuses(&doc, "missing field `units`");
}

#[test]
fn refuses_invalid_pixel_grid() {
    for (field, expected) in [
        (json!(0), "pixel grid must have at least one cell"),
        (Value::Null, "invalid type: null"),
        (json!(-1), "expected u32"),
        (json!(4_294_967_296_u64), "expected u32"),
        (json!(1.5), "expected u32"),
        (json!("5"), "expected u32"),
    ] {
        let mut doc = document();
        doc["pixel_grid"] = field;
        refuses(&doc, expected);
    }
}
