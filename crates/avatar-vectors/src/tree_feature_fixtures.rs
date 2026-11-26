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
    let square = |lo, hi| {
        vec![
            Segment::MoveTo { x: lo, y: lo },
            Segment::LineTo { x: hi, y: lo },
            Segment::LineTo { x: hi, y: hi },
            Segment::LineTo { x: lo, y: hi },
        ]
    };
    Shape::CompoundPath {
        subpaths: vec![square(100, 900), square(400, 600)],
        fill_rule: FillRule::EvenOdd,
    }
}

fn cases() -> [(&'static str, RenderTree); 4] {
    let tree = |primitive| RenderTree {
        canvas: 10,
        background: None,
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
    [
        ("radial-gradient-at", positioned),
        ("stop-opacity", opacity),
        ("compound-path", compound),
        ("compound-path-clip", tree(clipped)),
    ]
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
    let gradient = |cx, cy, r, fx, fy| {
        let mut doc = document();
        doc["primitives"][0]["paint"] = json!({"kind":"radial_gradient_at", "cx":cx, "cy":cy, "r":r, "fx":fx, "fy":fy, "stops":[]});
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

#[test]
fn refuses_empty_or_malformed_compound_paths() {
    let mv = json!({"kind":"move_to","x":0,"y":0});
    let line = json!({"kind":"line_to","x":1,"y":1});
    for (subpaths, expected) in [
        (json!([]), "compound path must have a subpath"),
        (
            json!([[]]),
            "compound subpath must start with its only move",
        ),
        (
            json!([[mv]]),
            "compound subpath must start with its only move",
        ),
        (
            json!([[line, line]]),
            "compound subpath must start with its only move",
        ),
        (
            json!([[mv, line, mv]]),
            "compound subpath must start with its only move",
        ),
    ] {
        let mut doc = document();
        doc["primitives"][0]["shape"] =
            json!({"kind":"compound_path", "fill_rule":"even_odd", "subpaths":subpaths});
        refuses(&doc, expected);
    }
}
