//! Canonical JSON projection of the flat render contract.
//!
//! The wire representation keeps fixed-point values as their raw integers so
//! drawing identity never depends on decimal formatting or floating-point
//! parsing in a consumer.

use std::fmt::{self, Write as _};

use avatar_phenotype::{Rgb, Q32};

use crate::tree::{
    Effect, FillRule, GradientStop, Paint, Primitive, RenderTree, Segment, Shape, Stroke,
    StrokeCap, StrokeJoin, Transform, TransformOp,
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
}

impl Writer {
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
    writer.object(|object| {
        object.field("background", |writer| {
            write_optional(writer, background, write_rgb)
        });
        object.field("canvas", |writer| writer.integer(*canvas));
        object.field("primitives", |writer| {
            writer.array(primitives, write_primitive)
        });
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
    } = primitive;
    writer.object(|object| {
        object.field("clip", |writer| write_optional(writer, clip, write_shape));
        object.field("effects", |writer| writer.array(effects, write_effect));
        object.field("label", |writer| writer.string(label));
        object.field("opacity", |writer| {
            write_optional(writer, opacity, write_q32)
        });
        object.field("paint", |writer| write_paint(writer, paint));
        object.field("shape", |writer| write_shape(writer, shape));
        object.field("stroke", |writer| {
            write_optional(writer, stroke, write_stroke)
        });
        object.field("transform", |writer| {
            write_optional(writer, transform, write_transform)
        });
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
    let GradientStop { offset_q32, color } = stop;
    writer.object(|object| {
        object.field("color", |writer| write_rgb(writer, color));
        object.field("offset_q32", |writer| write_q32(writer, offset_q32));
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
        Effect::Masked { mask } => writer.object(|object| {
            object.field("kind", |writer| writer.string("masked"));
            object.field("mask", |writer| writer.array(mask, write_primitive));
        }),
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
        assert!(json.contains("\"kind\":\"masked\""));
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
            "{\"background\":null,\"canvas\":10000,\"primitives\":[{\"clip\":null,\"effects\":[],\"label\":\"ordered\",\"opacity\":null,\"paint\":{\"kind\":\"none\"},\"shape\":{\"h\":4,\"kind\":\"rect\",\"w\":3,\"x\":1,\"y\":2},\"stroke\":null,\"transform\":null}]}"
        );
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
