//! Canonical JSON projection of the flat render contract.
//!
//! The wire representation keeps fixed-point values as their raw integers so
//! drawing identity never depends on decimal formatting or floating-point
//! parsing in a consumer.
//!
//! Masks are written once, in the tree's top-level `masks` table, and a
//! masked effect names its entry by index (`mask_ref`). Wild creatures clone
//! one body mask into many primitives, so writing each copy in full made most
//! of the document repeated mask contents. Entries are numbered in the
//! post-order of a depth-first walk: a mask nested inside another mask's
//! primitives gets the lower index, so every reference points to an earlier
//! entry. Two masks share an entry when their canonical JSON is identical.

use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use avatar_phenotype::{Rgb, Q32};

use crate::tree::{
    assert_compound_path, assert_radial_gradient_at, Effect, FillRule, GradientStop, Paint,
    Primitive, RenderTree, Role, Segment, Shape, Stroke, StrokeCap, StrokeJoin, Transform,
    TransformOp,
};

/// Serializes a render tree as whitespace-free JSON with byte-sorted object keys.
pub fn serialize_tree_json(tree: &RenderTree) -> String {
    let mut writer = Writer::default();
    write_tree(&mut writer, tree);
    writer.output
}

#[derive(Default)]
struct Writer {
    output: String,
    /// Canonical JSON of each distinct mask, in table order.
    masks: Vec<String>,
    /// The table index of each mask, keyed by its canonical JSON.
    mask_indices: HashMap<String, usize>,
}

impl Writer {
    /// Runs `write` against an empty output and returns what it wrote, leaving
    /// the output written so far untouched. The mask table is not swapped out,
    /// so masks met inside still receive their table indices.
    fn detached(&mut self, write: impl FnOnce(&mut Self)) -> String {
        let outer = std::mem::take(&mut self.output);
        write(self);
        std::mem::replace(&mut self.output, outer)
    }

    /// Returns the table index of `mask`, adding it to the table when no
    /// identical mask is there yet.
    fn mask_ref(&mut self, mask: &[Primitive]) -> usize {
        // Writing the entry first gives every mask nested inside it an index
        // before this one, which keeps all references pointing backwards.
        let entry = self.detached(|writer| writer.array(mask, write_primitive));
        if let Some(&index) = self.mask_indices.get(&entry) {
            return index;
        }
        let index = self.masks.len();
        self.mask_indices.insert(entry.clone(), index);
        self.masks.push(entry);
        index
    }

    fn integer<T: fmt::Display>(&mut self, value: T) {
        write!(self.output, "{value}").expect("writing to String cannot fail");
    }

    fn boolean(&mut self, value: bool) {
        self.output.push_str(if value { "true" } else { "false" });
    }

    fn string(&mut self, value: &str) {
        self.output.push('"');
        for character in value.chars() {
            match character {
                '"' => self.output.push_str("\\\""),
                '\\' => self.output.push_str("\\\\"),
                '\u{08}' => self.output.push_str("\\b"),
                '\u{0c}' => self.output.push_str("\\f"),
                '\n' => self.output.push_str("\\n"),
                '\r' => self.output.push_str("\\r"),
                '\t' => self.output.push_str("\\t"),
                character if character <= '\u{1f}' => {
                    write!(self.output, "\\u{:04x}", character as u32)
                        .expect("writing to String cannot fail");
                }
                character => self.output.push(character),
            }
        }
        self.output.push('"');
    }

    fn null(&mut self) {
        self.output.push_str("null");
    }

    fn object(&mut self, write_fields: impl FnOnce(&mut ObjectWriter<'_>)) {
        self.output.push('{');
        let mut object = ObjectWriter {
            writer: self,
            is_first: true,
        };
        write_fields(&mut object);
        object.writer.output.push('}');
    }

    fn array<T>(&mut self, values: &[T], write_value: impl Fn(&mut Self, &T)) {
        self.output.push('[');
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                self.output.push(',');
            }
            write_value(self, value);
        }
        self.output.push(']');
    }
}

struct ObjectWriter<'a> {
    writer: &'a mut Writer,
    is_first: bool,
}

impl ObjectWriter<'_> {
    fn field(&mut self, key: &str, write_value: impl FnOnce(&mut Writer)) {
        if !self.is_first {
            self.writer.output.push(',');
        }
        self.is_first = false;
        self.writer.string(key);
        self.writer.output.push(':');
        write_value(self.writer);
    }
}

fn write_tree(writer: &mut Writer, tree: &RenderTree) {
    let RenderTree {
        canvas,
        background,
        primitives,
    } = tree;
    // The mask table comes before the primitives in key order but is complete
    // only once they have been walked, so the primitives are written aside.
    let primitives = writer.detached(|writer| writer.array(primitives, write_primitive));
    let masks = std::mem::take(&mut writer.masks);
    writer.object(|object| {
        object.field("background", |writer| {
            write_optional(writer, background, write_rgb)
        });
        object.field("canvas", |writer| writer.integer(*canvas));
        object.field("masks", |writer| {
            writer.array(&masks, |writer, entry| writer.output.push_str(entry))
        });
        object.field("primitives", |writer| writer.output.push_str(&primitives));
    });
}

fn write_primitive(writer: &mut Writer, primitive: &Primitive) {
    let Primitive {
        shape,
        transform,
        paint,
        stroke,
        opacity,
        clip,
        effects,
        label,
        role,
    } = primitive;
    writer.object(|object| {
        object.field("clip", |writer| write_optional(writer, clip, write_shape));
        object.field("effects", |writer| writer.array(effects, write_effect));
        object.field("label", |writer| writer.string(label));
        object.field("opacity", |writer| {
            write_optional(writer, opacity, write_q32)
        });
        object.field("paint", |writer| write_paint(writer, paint));
        object.field("role", |writer| write_optional(writer, role, write_role));
        object.field("shape", |writer| write_shape(writer, shape));
        object.field("stroke", |writer| {
            write_optional(writer, stroke, write_stroke)
        });
        object.field("transform", |writer| {
            write_optional(writer, transform, write_transform)
        });
    });
}

fn write_role(writer: &mut Writer, role: &Role) {
    writer.string(match role {
        Role::GroundShadow => "ground_shadow",
    });
}

fn write_shape(writer: &mut Writer, shape: &Shape) {
    match shape {
        Shape::Path {
            segments,
            closed,
            fill_rule,
        } => writer.object(|object| {
            object.field("closed", |writer| writer.boolean(*closed));
            object.field("fill_rule", |writer| write_fill_rule(writer, *fill_rule));
            object.field("kind", |writer| writer.string("path"));
            object.field("segments", |writer| writer.array(segments, write_segment));
        }),
        Shape::CompoundPath {
            subpaths,
            fill_rule,
        } => {
            assert_compound_path(subpaths);
            writer.object(|object| {
                object.field("fill_rule", |writer| write_fill_rule(writer, *fill_rule));
                object.field("kind", |writer| writer.string("compound_path"));
                object.field("subpaths", |writer| {
                    writer.array(subpaths, |writer, segments| {
                        writer.array(segments, write_segment)
                    });
                });
            });
        }
        Shape::Circle { cx, cy, r } => writer.object(|object| {
            object.field("cx", |writer| writer.integer(*cx));
            object.field("cy", |writer| writer.integer(*cy));
            object.field("kind", |writer| writer.string("circle"));
            object.field("r", |writer| writer.integer(*r));
        }),
        Shape::Ellipse { cx, cy, rx, ry } => writer.object(|object| {
            object.field("cx", |writer| writer.integer(*cx));
            object.field("cy", |writer| writer.integer(*cy));
            object.field("kind", |writer| writer.string("ellipse"));
            object.field("rx", |writer| writer.integer(*rx));
            object.field("ry", |writer| writer.integer(*ry));
        }),
        Shape::Rect { x, y, w, h } => writer.object(|object| {
            object.field("h", |writer| writer.integer(*h));
            object.field("kind", |writer| writer.string("rect"));
            object.field("w", |writer| writer.integer(*w));
            object.field("x", |writer| writer.integer(*x));
            object.field("y", |writer| writer.integer(*y));
        }),
    }
}

fn write_segment(writer: &mut Writer, segment: &Segment) {
    match *segment {
        Segment::MoveTo { x, y } => writer.object(|object| {
            object.field("kind", |writer| writer.string("move_to"));
            object.field("x", |writer| writer.integer(x));
            object.field("y", |writer| writer.integer(y));
        }),
        Segment::CubicTo {
            cp1x,
            cp1y,
            cp2x,
            cp2y,
            x,
            y,
        } => writer.object(|object| {
            object.field("cp1x", |writer| writer.integer(cp1x));
            object.field("cp1y", |writer| writer.integer(cp1y));
            object.field("cp2x", |writer| writer.integer(cp2x));
            object.field("cp2y", |writer| writer.integer(cp2y));
            object.field("kind", |writer| writer.string("cubic_to"));
            object.field("x", |writer| writer.integer(x));
            object.field("y", |writer| writer.integer(y));
        }),
        Segment::LineTo { x, y } => writer.object(|object| {
            object.field("kind", |writer| writer.string("line_to"));
            object.field("x", |writer| writer.integer(x));
            object.field("y", |writer| writer.integer(y));
        }),
    }
}

fn write_fill_rule(writer: &mut Writer, fill_rule: FillRule) {
    writer.string(match fill_rule {
        FillRule::NonZero => "non_zero",
        FillRule::EvenOdd => "even_odd",
    });
}

fn write_gradient_stop(writer: &mut Writer, stop: &GradientStop) {
    let opacity = stop.checked_opacity();
    writer.object(|object| {
        object.field("color", |writer| write_rgb(writer, &stop.color));
        object.field("offset_q32", |writer| write_q32(writer, &stop.offset_q32));
        if let Some(opacity) = opacity {
            object.field("opacity_q32", |writer| write_q32(writer, &opacity));
        }
    });
}

fn write_paint(writer: &mut Writer, paint: &Paint) {
    match paint {
        Paint::Solid { color } => writer.object(|object| {
            object.field("color", |writer| write_rgb(writer, color));
            object.field("kind", |writer| writer.string("solid"));
        }),
        Paint::RadialGradient { stops } => writer.object(|object| {
            object.field("kind", |writer| writer.string("radial_gradient"));
            object.field("stops", |writer| writer.array(stops, write_gradient_stop));
        }),
        Paint::RadialGradientAt {
            cx,
            cy,
            r,
            focal,
            stops,
        } => {
            assert_radial_gradient_at(*cx, *cy, *r, *focal);
            writer.object(|object| {
                object.field("cx", |writer| writer.integer(*cx));
                object.field("cy", |writer| writer.integer(*cy));
                if let Some((fx, fy)) = focal {
                    object.field("fx", |writer| writer.integer(*fx));
                    object.field("fy", |writer| writer.integer(*fy));
                }
                object.field("kind", |writer| writer.string("radial_gradient_at"));
                object.field("r", |writer| writer.integer(*r));
                object.field("stops", |writer| writer.array(stops, write_gradient_stop));
            });
        }
        Paint::None => writer.object(|object| {
            object.field("kind", |writer| writer.string("none"));
        }),
    }
}

fn write_stroke(writer: &mut Writer, stroke: &Stroke) {
    let Stroke {
        color,
        width,
        cap,
        join,
    } = stroke;
    writer.object(|object| {
        object.field("cap", |writer| write_stroke_cap(writer, *cap));
        object.field("color", |writer| write_rgb(writer, color));
        object.field("join", |writer| write_stroke_join(writer, *join));
        object.field("width", |writer| writer.integer(*width));
    });
}

fn write_stroke_cap(writer: &mut Writer, cap: StrokeCap) {
    writer.string(match cap {
        StrokeCap::Butt => "butt",
        StrokeCap::Round => "round",
        StrokeCap::Square => "square",
    });
}

fn write_stroke_join(writer: &mut Writer, join: StrokeJoin) {
    writer.string(match join {
        StrokeJoin::Miter => "miter",
        StrokeJoin::Round => "round",
        StrokeJoin::Bevel => "bevel",
    });
}

fn write_effect(writer: &mut Writer, effect: &Effect) {
    match effect {
        Effect::Lit { light_y } => writer.object(|object| {
            object.field("kind", |writer| writer.string("lit"));
            object.field("light_y", |writer| writer.integer(*light_y));
        }),
        Effect::Masked { mask } => {
            let mask_ref = writer.mask_ref(mask);
            writer.object(|object| {
                object.field("kind", |writer| writer.string("masked"));
                object.field("mask_ref", |writer| writer.integer(mask_ref));
            });
        }
        Effect::Turbulence {
            seed,
            freq_h,
            freq_v,
            octaves,
            scale,
        } => writer.object(|object| {
            object.field("freq_h", |writer| write_q32(writer, freq_h));
            object.field("freq_v", |writer| write_q32(writer, freq_v));
            object.field("kind", |writer| writer.string("turbulence"));
            object.field("octaves", |writer| writer.integer(*octaves));
            object.field("scale", |writer| write_q32(writer, scale));
            object.field("seed", |writer| writer.integer(*seed));
        }),
    }
}

fn write_transform(writer: &mut Writer, transform: &Transform) {
    writer.object(|object| {
        object.field("ops", |writer| {
            writer.array(&transform.ops, write_transform_op)
        });
    });
}

fn write_transform_op(writer: &mut Writer, operation: &TransformOp) {
    match *operation {
        TransformOp::Translate { tx, ty } => writer.object(|object| {
            object.field("kind", |writer| writer.string("translate"));
            object.field("tx", |writer| writer.integer(tx));
            object.field("ty", |writer| writer.integer(ty));
        }),
        TransformOp::Rotate { angle, cx, cy } => writer.object(|object| {
            object.field("angle", |writer| writer.integer(angle.0));
            object.field("cx", |writer| writer.integer(cx));
            object.field("cy", |writer| writer.integer(cy));
            object.field("kind", |writer| writer.string("rotate"));
        }),
        TransformOp::Scale { sx, sy } => writer.object(|object| {
            object.field("kind", |writer| writer.string("scale"));
            object.field("sx", |writer| write_q32(writer, &sx));
            object.field("sy", |writer| write_q32(writer, &sy));
        }),
    }
}

fn write_rgb(writer: &mut Writer, color: &Rgb) {
    let Rgb { r, g, b } = color;
    writer.object(|object| {
        object.field("b", |writer| writer.integer(*b));
        object.field("g", |writer| writer.integer(*g));
        object.field("r", |writer| writer.integer(*r));
    });
}

fn write_q32(writer: &mut Writer, value: &Q32) {
    writer.integer(value.0);
}

fn write_optional<T>(writer: &mut Writer, value: &Option<T>, write_value: fn(&mut Writer, &T)) {
    match value {
        Some(value) => write_value(writer, value),
        None => writer.null(),
    }
}

#[cfg(test)]
mod tests {
    use avatar_phenotype::{Bam16, Rgb, Q32};

    use super::*;

    fn color(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    fn primitive(shape: Shape, paint: Paint, label: &str) -> Primitive {
        Primitive {
            shape,
            transform: None,
            paint,
            stroke: None,
            opacity: None,
            clip: None,
            effects: Vec::new(),
            label: label.to_owned(),
            role: None,
        }
    }

    #[test]
    fn every_shape_paint_effect_and_transform_variant_survives_json_round_trip() {
        let mask = primitive(
            Shape::Rect {
                x: -1,
                y: 2,
                w: 3,
                h: 4,
            },
            Paint::None,
            "mask",
        );
        let mut path = primitive(
            Shape::Path {
                segments: vec![
                    Segment::MoveTo { x: 1, y: 2 },
                    Segment::CubicTo {
                        cp1x: 3,
                        cp1y: 4,
                        cp2x: 5,
                        cp2y: 6,
                        x: 7,
                        y: 8,
                    },
                    Segment::LineTo { x: 9, y: 10 },
                ],
                closed: true,
                fill_rule: FillRule::EvenOdd,
            },
            Paint::RadialGradient {
                stops: vec![GradientStop {
                    offset_q32: Q32(-4_294_967_296),
                    color: color(1, 2, 3),
                    opacity_q32: None,
                }],
            },
            "path \"quoted\"\n雪",
        );
        path.transform = Some(Transform {
            ops: vec![
                TransformOp::Translate { tx: -11, ty: 12 },
                TransformOp::Rotate {
                    angle: Bam16(65_535),
                    cx: 13,
                    cy: 14,
                },
                TransformOp::Scale {
                    sx: Q32(4_294_967_296),
                    sy: Q32(-2_147_483_648),
                },
            ],
        });
        path.stroke = Some(Stroke {
            color: color(4, 5, 6),
            width: 15,
            cap: StrokeCap::Square,
            join: StrokeJoin::Bevel,
        });
        path.opacity = Some(Q32(i64::MAX));
        path.clip = Some(Shape::Circle {
            cx: 16,
            cy: 17,
            r: 18,
        });
        path.effects = vec![
            Effect::Lit { light_y: 19 },
            Effect::Masked { mask: vec![mask] },
            Effect::Turbulence {
                seed: u8::MAX,
                freq_h: Q32(20),
                freq_v: Q32(21),
                octaves: 22,
                scale: Q32(23),
            },
        ];
        let tree = RenderTree {
            canvas: u32::MAX,
            background: Some(color(7, 8, 9)),
            primitives: vec![
                path,
                primitive(
                    Shape::Circle {
                        cx: 24,
                        cy: 25,
                        r: 26,
                    },
                    Paint::Solid {
                        color: color(10, 11, 12),
                    },
                    "circle",
                ),
                primitive(
                    Shape::Ellipse {
                        cx: 27,
                        cy: 28,
                        rx: 29,
                        ry: 30,
                    },
                    Paint::None,
                    "ellipse",
                ),
                primitive(
                    Shape::Rect {
                        x: 31,
                        y: 32,
                        w: 33,
                        h: 34,
                    },
                    Paint::None,
                    "rect",
                ),
            ],
        };

        let json = serialize_tree_json(&tree);
        let value: serde_json::Value = serde_json::from_str(&json).expect("encoder emits JSON");
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert!(json.contains("\"kind\":\"radial_gradient\""));
        assert!(json.contains("\"kind\":\"masked\",\"mask_ref\":0"));
        assert!(json.contains("\"fill_rule\":\"even_odd\""));
        assert!(json.contains("\"kind\":\"rotate\""));
        assert!(json.contains("\"opacity\":9223372036854775807"));
        assert!(json.contains("path \\\"quoted\\\"\\n雪"));
    }

    #[test]
    fn object_keys_are_emitted_in_byte_lexicographic_order() {
        let json = serialize_tree_json(&RenderTree {
            canvas: 10_000,
            background: None,
            primitives: vec![primitive(
                Shape::Rect {
                    x: 1,
                    y: 2,
                    w: 3,
                    h: 4,
                },
                Paint::None,
                "ordered",
            )],
        });
        assert_eq!(
            json,
            "{\"background\":null,\"canvas\":10000,\"masks\":[],\"primitives\":[{\"clip\":null,\"effects\":[],\"label\":\"ordered\",\"opacity\":null,\"paint\":{\"kind\":\"none\"},\"role\":null,\"shape\":{\"h\":4,\"kind\":\"rect\",\"w\":3,\"x\":1,\"y\":2},\"stroke\":null,\"transform\":null}]}"
        );
    }

    fn circle(label: &str) -> Primitive {
        primitive(Shape::Circle { cx: 1, cy: 2, r: 3 }, Paint::None, label)
    }

    fn masked_by(label: &str, mask: Vec<Primitive>) -> Primitive {
        let mut primitive = circle(label);
        primitive.effects = vec![Effect::Masked { mask }];
        primitive
    }

    fn tree_value(primitives: Vec<Primitive>) -> serde_json::Value {
        let json = serialize_tree_json(&RenderTree {
            canvas: 240,
            background: None,
            primitives,
        });
        serde_json::from_str(&json).expect("encoder emits JSON")
    }

    /// The table index each top-level primitive's first effect refers to.
    fn top_level_refs(tree: &serde_json::Value) -> Vec<u64> {
        tree["primitives"]
            .as_array()
            .unwrap()
            .iter()
            .map(|primitive| primitive["effects"][0]["mask_ref"].as_u64().unwrap())
            .collect()
    }

    #[test]
    fn masked_effect_is_written_as_a_reference_into_the_mask_table() {
        let json = serialize_tree_json(&RenderTree {
            canvas: 1,
            background: None,
            primitives: vec![masked_by("drawn", vec![circle("mask")])],
        });
        let circle_json = |label: &str, effects: &str| {
            format!("{{\"clip\":null,\"effects\":[{effects}],\"label\":\"{label}\",\"opacity\":null,\"paint\":{{\"kind\":\"none\"}},\"role\":null,\"shape\":{{\"cx\":1,\"cy\":2,\"kind\":\"circle\",\"r\":3}},\"stroke\":null,\"transform\":null}}")
        };
        assert_eq!(
            json,
            format!(
                "{{\"background\":null,\"canvas\":1,\"masks\":[[{}]],\"primitives\":[{}]}}",
                circle_json("mask", ""),
                circle_json("drawn", "{\"kind\":\"masked\",\"mask_ref\":0}"),
            )
        );
    }

    #[test]
    fn identical_masks_share_one_table_entry() {
        let mask = vec![circle("mask")];
        let json = serialize_tree_json(&RenderTree {
            canvas: 240,
            background: None,
            primitives: vec![
                masked_by("first", mask.clone()),
                masked_by("second", mask.clone()),
                masked_by("third", mask),
            ],
        });
        let tree: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(tree["masks"].as_array().unwrap().len(), 1);
        assert_eq!(top_level_refs(&tree), [0, 0, 0]);
        assert_eq!(json.matches("\"label\":\"mask\"").count(), 1);
    }

    #[test]
    fn masks_are_numbered_in_depth_first_post_order() {
        let inner = vec![circle("inner")];
        let outer = vec![masked_by("outer", inner.clone())];
        let tree = tree_value(vec![
            // Walking the outer mask reaches the inner one first: inner 0, outer 1.
            masked_by("first", outer.clone()),
            masked_by("second", vec![circle("other")]),
            masked_by("third", inner),
            masked_by("fourth", outer),
        ]);
        let labels = tree["masks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry[0]["label"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["inner", "outer", "other"]);
        assert_eq!(tree["masks"][1][0]["effects"][0]["mask_ref"], 0);
        assert_eq!(top_level_refs(&tree), [1, 2, 0, 1]);
    }

    #[test]
    fn masks_differing_only_inside_a_nested_mask_are_separate_entries() {
        let tree = tree_value(vec![
            masked_by("first", vec![masked_by("outer", vec![circle("inner a")])]),
            masked_by("second", vec![masked_by("outer", vec![circle("inner b")])]),
        ]);
        let masks = tree["masks"].as_array().unwrap();
        assert_eq!(masks.len(), 4);
        assert_eq!(masks[0][0]["label"], "inner a");
        assert_eq!(masks[1][0]["effects"][0]["mask_ref"], 0);
        assert_eq!(masks[2][0]["label"], "inner b");
        assert_eq!(masks[3][0]["effects"][0]["mask_ref"], 2);
        assert_eq!(top_level_refs(&tree), [1, 3]);
    }

    #[test]
    fn every_mask_reference_points_to_an_earlier_entry() {
        let leaf = vec![circle("leaf")];
        let middle = vec![masked_by("middle", leaf.clone()), masked_by("beside", leaf)];
        let tree = tree_value(vec![
            masked_by("top", vec![masked_by("upper", middle.clone())]),
            masked_by("again", middle),
        ]);
        for (index, entry) in tree["masks"].as_array().unwrap().iter().enumerate() {
            for primitive in entry.as_array().unwrap() {
                for effect in primitive["effects"].as_array().unwrap() {
                    let mask_ref = effect["mask_ref"].as_u64().unwrap();
                    assert!(
                        (mask_ref as usize) < index,
                        "entry {index} refers to {mask_ref}"
                    );
                }
            }
        }
        assert_eq!(tree["masks"].as_array().unwrap().len(), 3);
        assert_eq!(top_level_refs(&tree), [2, 1]);
    }

    #[test]
    fn simple_enum_tags_cover_every_value() {
        let mut writer = Writer::default();
        for fill_rule in [FillRule::NonZero, FillRule::EvenOdd] {
            write_fill_rule(&mut writer, fill_rule);
        }
        for cap in [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square] {
            write_stroke_cap(&mut writer, cap);
        }
        for join in [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel] {
            write_stroke_join(&mut writer, join);
        }
        assert_eq!(
            writer.output,
            "\"non_zero\"\"even_odd\"\"butt\"\"round\"\"square\"\"miter\"\"round\"\"bevel\""
        );
    }
}
