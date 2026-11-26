use std::fmt;

use avatar_svg::{
    Bam16, Effect, FillRule, GradientStop, GradientUnits, Paint, Primitive, RenderTree, Rgb, Role,
    Segment, Shape, Stroke, StrokeCap, StrokeJoin, Subpath, Transform, TransformOp, Q32,
};
use serde::Deserialize;

/// Decodes a committed tree file strictly: every field must be present and
/// known, and every mask reference must resolve under the mask table's rules.
pub(crate) fn decode_tree_fixture(bytes: &[u8]) -> Result<RenderTree, FixtureError> {
    let tree = serde_json::from_slice::<FixtureTree>(bytes).map_err(FixtureError::Json)?;
    MaskTable::resolve_tree(tree)
}

/// Why a tree file was refused.
#[derive(Debug)]
pub(crate) enum FixtureError {
    /// The JSON does not match the schema.
    Json(serde_json::Error),
    /// A `mask_ref` names no entry of the mask table.
    MaskRefOutOfRange { mask_ref: usize, entries: usize },
    /// A mask table entry refers to itself or to a later entry. Allowing only
    /// earlier entries is what keeps references from forming a cycle.
    MaskRefNotEarlier { entry: usize, mask_ref: usize },
    /// A mask table entry that no masked effect refers to.
    UnreferencedMask { entry: usize },
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "{error}"),
            Self::MaskRefOutOfRange { mask_ref, entries } => write!(
                formatter,
                "mask_ref {mask_ref} is outside the mask table of {entries} entries"
            ),
            Self::MaskRefNotEarlier { entry, mask_ref } => write!(
                formatter,
                "mask entry {entry} refers to entry {mask_ref}; entries may refer only to earlier entries"
            ),
            Self::UnreferencedMask { entry } => {
                write!(formatter, "mask entry {entry} is never referenced")
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureTree {
    canvas: u32,
    background: Option<FixtureRgb>,
    masks: Vec<Vec<FixturePrimitive>>,
    #[serde(default, deserialize_with = "pixel_grid")]
    pixel_grid: Option<u32>,
    primitives: Vec<FixturePrimitive>,
}

fn pixel_grid<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let cells = u32::deserialize(d)?;
    if cells == 0 {
        return Err(serde::de::Error::custom(
            "pixel grid must have at least one cell",
        ));
    }
    Ok(Some(cells))
}

/// Resolves `mask_ref` indices back into the inline masks the in-memory tree
/// carries.
struct MaskTable {
    /// The number of entries in the table being resolved.
    entries: usize,
    /// The entries resolved so far. While entry `i` resolves this holds
    /// exactly the entries before it, which are the ones it may refer to.
    resolved: Vec<Vec<Primitive>>,
    referenced: Vec<bool>,
}

impl MaskTable {
    fn resolve_tree(tree: FixtureTree) -> Result<RenderTree, FixtureError> {
        let mut table = Self {
            entries: tree.masks.len(),
            resolved: Vec::with_capacity(tree.masks.len()),
            referenced: vec![false; tree.masks.len()],
        };
        for entry in tree.masks {
            let entry = table.primitives(entry)?;
            table.resolved.push(entry);
        }
        let primitives = table.primitives(tree.primitives)?;
        if let Some(entry) = table.referenced.iter().position(|referenced| !referenced) {
            return Err(FixtureError::UnreferencedMask { entry });
        }
        Ok(RenderTree {
            canvas: tree.canvas,
            background: tree.background.map(Into::into),
            pixel_grid: tree.pixel_grid,
            primitives,
        })
    }

    fn primitives(
        &mut self,
        primitives: Vec<FixturePrimitive>,
    ) -> Result<Vec<Primitive>, FixtureError> {
        primitives
            .into_iter()
            .map(|primitive| self.primitive(primitive))
            .collect()
    }

    fn primitive(&mut self, primitive: FixturePrimitive) -> Result<Primitive, FixtureError> {
        Ok(Primitive {
            shape: primitive.shape.0,
            transform: primitive.transform.map(Into::into),
            paint: primitive.paint.0,
            stroke: primitive.stroke.map(Into::into),
            opacity: primitive.opacity.map(Q32),
            clip: primitive.clip.map(|shape| shape.0),
            effects: primitive
                .effects
                .into_iter()
                .map(|effect| self.effect(effect))
                .collect::<Result<_, _>>()?,
            label: primitive.label,
            role: primitive.role.map(Into::into),
        })
    }

    fn effect(&mut self, effect: FixtureEffect) -> Result<Effect, FixtureError> {
        Ok(match effect {
            FixtureEffect::Lit { light_y } => Effect::Lit { light_y },
            FixtureEffect::Masked { mask_ref } => Effect::Masked {
                mask: self.mask(mask_ref)?,
            },
            FixtureEffect::Turbulence {
                seed,
                freq_h,
                freq_v,
                octaves,
                scale,
            } => Effect::Turbulence {
                seed,
                freq_h: Q32(freq_h),
                freq_v: Q32(freq_v),
                octaves,
                scale: Q32(scale),
            },
        })
    }

    fn mask(&mut self, mask_ref: usize) -> Result<Vec<Primitive>, FixtureError> {
        if mask_ref >= self.entries {
            return Err(FixtureError::MaskRefOutOfRange {
                mask_ref,
                entries: self.entries,
            });
        }
        if mask_ref >= self.resolved.len() {
            return Err(FixtureError::MaskRefNotEarlier {
                entry: self.resolved.len(),
                mask_ref,
            });
        }
        self.referenced[mask_ref] = true;
        Ok(self.resolved[mask_ref].clone())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixturePrimitive {
    shape: DecodedShape,
    transform: Option<FixtureTransform>,
    paint: DecodedPaint,
    stroke: Option<FixtureStroke>,
    opacity: Option<i64>,
    clip: Option<DecodedShape>,
    effects: Vec<FixtureEffect>,
    label: String,
    role: Option<FixtureRole>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureRole {
    GroundShadow,
}

impl From<FixtureRole> for Role {
    fn from(role: FixtureRole) -> Self {
        match role {
            FixtureRole::GroundShadow => Self::GroundShadow,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureFillRule {
    NonZero,
    EvenOdd,
}

impl From<FixtureFillRule> for FillRule {
    fn from(fill_rule: FixtureFillRule) -> Self {
        match fill_rule {
            FixtureFillRule::NonZero => Self::NonZero,
            FixtureFillRule::EvenOdd => Self::EvenOdd,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureSegment {
    #[serde(rename = "move_to")]
    Move { x: i64, y: i64 },
    #[serde(rename = "cubic_to")]
    Cubic {
        cp1x: i64,
        cp1y: i64,
        cp2x: i64,
        cp2y: i64,
        x: i64,
        y: i64,
    },
    #[serde(rename = "line_to")]
    Line { x: i64, y: i64 },
}

impl From<FixtureSegment> for Segment {
    fn from(segment: FixtureSegment) -> Self {
        match segment {
            FixtureSegment::Move { x, y } => Self::MoveTo { x, y },
            FixtureSegment::Cubic {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            } => Self::CubicTo {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            },
            FixtureSegment::Line { x, y } => Self::LineTo { x, y },
        }
    }
}

#[derive(Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
struct FixtureSubpath {
    closed: bool,
    segments: Vec<FixtureSegment>,
}

impl<'de> Deserialize<'de> for FixtureSubpath {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Object;

        impl<'de> serde::de::Visitor<'de> for Object {
            type Value = FixtureSubpath;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a subpath object with closed and segments keys")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                FixtureSubpath::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }

        // A tuple must not satisfy the requirement for an explicit `closed` key.
        d.deserialize_map(Object)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureShape {
    Path {
        segments: Vec<FixtureSegment>,
        closed: bool,
        fill_rule: FixtureFillRule,
    },
    CompoundPath {
        subpaths: Vec<FixtureSubpath>,
        fill_rule: FixtureFillRule,
    },
    Circle {
        cx: i64,
        cy: i64,
        r: i64,
    },
    Ellipse {
        cx: i64,
        cy: i64,
        rx: i64,
        ry: i64,
    },
    Rect {
        x: i64,
        y: i64,
        w: i64,
        h: i64,
    },
}

#[derive(Deserialize)]
#[serde(try_from = "FixtureShape")]
struct DecodedShape(Shape);

impl TryFrom<FixtureShape> for DecodedShape {
    type Error = &'static str;

    fn try_from(shape: FixtureShape) -> Result<Self, Self::Error> {
        Ok(Self(match shape {
            FixtureShape::Path {
                segments,
                closed,
                fill_rule,
            } => Shape::Path {
                segments: segments.into_iter().map(Into::into).collect(),
                closed,
                fill_rule: fill_rule.into(),
            },
            FixtureShape::CompoundPath {
                subpaths,
                fill_rule,
            } => {
                if subpaths.is_empty() {
                    return Err("compound path must have a subpath");
                }
                for subpath in &subpaths {
                    if !matches!(subpath.segments.as_slice(), [FixtureSegment::Move { .. }, rest @ ..]
                        if !rest.is_empty() && rest.iter().all(|s| matches!(s, FixtureSegment::Line { .. } | FixtureSegment::Cubic { .. })))
                    {
                        return Err("compound subpath must start with its only move and have a line or cubic");
                    }
                }
                Shape::CompoundPath {
                    subpaths: subpaths
                        .into_iter()
                        .map(|subpath| Subpath {
                            closed: subpath.closed,
                            segments: subpath.segments.into_iter().map(Into::into).collect(),
                        })
                        .collect(),
                    fill_rule: fill_rule.into(),
                }
            }
            FixtureShape::Circle { cx, cy, r } => Shape::Circle { cx, cy, r },
            FixtureShape::Ellipse { cx, cy, rx, ry } => Shape::Ellipse { cx, cy, rx, ry },
            FixtureShape::Rect { x, y, w, h } => Shape::Rect { x, y, w, h },
        }))
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRgb {
    r: u8,
    g: u8,
    b: u8,
}

impl From<FixtureRgb> for Rgb {
    fn from(color: FixtureRgb) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureGradientStop {
    offset_q32: i64,
    color: FixtureRgb,
    #[serde(default, deserialize_with = "stop_opacity")]
    opacity_q32: Option<i64>,
}

impl From<FixtureGradientStop> for GradientStop {
    fn from(stop: FixtureGradientStop) -> Self {
        Self {
            offset_q32: Q32(stop.offset_q32),
            color: stop.color.into(),
            opacity_q32: stop.opacity_q32.map(Q32),
        }
    }
}

// `default` handles absent keys. Present keys must hold integers, not null.
fn present_integer<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    i64::deserialize(d).map(Some)
}

fn stop_opacity<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let opacity = i64::deserialize(d)?;
    if (0..Q32::ONE.0).contains(&opacity) {
        Ok(Some(opacity))
    } else {
        Err(serde::de::Error::custom(
            "gradient stop opacity must be in [0, 1)",
        ))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureGradientUnits {
    UserSpace,
    ObjectBoundingBox,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixturePaint {
    Solid {
        color: FixtureRgb,
    },
    RadialGradient {
        stops: Vec<FixtureGradientStop>,
    },
    RadialGradientAt {
        units: FixtureGradientUnits,
        cx: i64,
        cy: i64,
        r: i64,
        #[serde(default, deserialize_with = "present_integer")]
        fx: Option<i64>,
        #[serde(default, deserialize_with = "present_integer")]
        fy: Option<i64>,
        stops: Vec<FixtureGradientStop>,
    },
    None,
}

#[derive(Deserialize)]
#[serde(try_from = "FixturePaint")]
struct DecodedPaint(Paint);

impl TryFrom<FixturePaint> for DecodedPaint {
    type Error = &'static str;

    fn try_from(paint: FixturePaint) -> Result<Self, Self::Error> {
        Ok(Self(match paint {
            FixturePaint::Solid { color } => Paint::Solid {
                color: color.into(),
            },
            FixturePaint::RadialGradient { stops } => Paint::RadialGradient {
                stops: stops.into_iter().map(Into::into).collect(),
            },
            FixturePaint::RadialGradientAt {
                units,
                cx,
                cy,
                r,
                fx,
                fy,
                stops,
            } => {
                if r <= 0 {
                    return Err("radial gradient radius must be positive");
                }
                let focal = match (fx, fy) {
                    (None, None) => None,
                    (Some(fx), Some(fy)) => {
                        let dx = (i128::from(fx) - i128::from(cx)).abs();
                        let dy = (i128::from(fy) - i128::from(cy)).abs();
                        let r = i128::from(r);
                        // Reject large differences before squaring them.
                        if dx >= r || dy >= r || dx * dx + dy * dy >= r * r {
                            return Err("radial gradient focus must be strictly inside the circle");
                        }
                        Some((fx, fy))
                    }
                    _ => return Err("radial gradient fx and fy must be supplied together"),
                };
                Paint::RadialGradientAt {
                    units: match units {
                        FixtureGradientUnits::UserSpace => GradientUnits::UserSpace,
                        FixtureGradientUnits::ObjectBoundingBox => GradientUnits::ObjectBoundingBox,
                    },
                    cx,
                    cy,
                    r,
                    focal,
                    stops: stops.into_iter().map(Into::into).collect(),
                }
            }
            FixturePaint::None => Paint::None,
        }))
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureStrokeCap {
    Butt,
    Round,
    Square,
}

impl From<FixtureStrokeCap> for StrokeCap {
    fn from(cap: FixtureStrokeCap) -> Self {
        match cap {
            FixtureStrokeCap::Butt => Self::Butt,
            FixtureStrokeCap::Round => Self::Round,
            FixtureStrokeCap::Square => Self::Square,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureStrokeJoin {
    Miter,
    Round,
    Bevel,
}

impl From<FixtureStrokeJoin> for StrokeJoin {
    fn from(join: FixtureStrokeJoin) -> Self {
        match join {
            FixtureStrokeJoin::Miter => Self::Miter,
            FixtureStrokeJoin::Round => Self::Round,
            FixtureStrokeJoin::Bevel => Self::Bevel,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureStroke {
    color: FixtureRgb,
    width: i64,
    cap: FixtureStrokeCap,
    join: FixtureStrokeJoin,
}

impl From<FixtureStroke> for Stroke {
    fn from(stroke: FixtureStroke) -> Self {
        Self {
            color: stroke.color.into(),
            width: stroke.width,
            cap: stroke.cap.into(),
            join: stroke.join.into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureEffect {
    Lit {
        light_y: i64,
    },
    Masked {
        mask_ref: usize,
    },
    Turbulence {
        seed: u8,
        freq_h: i64,
        freq_v: i64,
        octaves: u8,
        scale: i64,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureTransformOp {
    Translate { tx: i64, ty: i64 },
    Rotate { angle: u16, cx: i64, cy: i64 },
    Scale { sx: i64, sy: i64 },
}

impl From<FixtureTransformOp> for TransformOp {
    fn from(operation: FixtureTransformOp) -> Self {
        match operation {
            FixtureTransformOp::Translate { tx, ty } => Self::Translate { tx, ty },
            FixtureTransformOp::Rotate { angle, cx, cy } => Self::Rotate {
                angle: Bam16(angle),
                cx,
                cy,
            },
            FixtureTransformOp::Scale { sx, sy } => Self::Scale {
                sx: Q32(sx),
                sy: Q32(sy),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureTransform {
    ops: Vec<FixtureTransformOp>,
}

impl From<FixtureTransform> for Transform {
    fn from(transform: FixtureTransform) -> Self {
        Self {
            ops: transform.ops.into_iter().map(Into::into).collect(),
        }
    }
}

/// The tree format that preceded the mask table, where a masked effect carried
/// its mask primitives in full. Kept only so the conversion check below can
/// read files minted in that format.
mod inline_masks {
    use super::*;

    pub(super) fn decode(bytes: &[u8]) -> Result<RenderTree, serde_json::Error> {
        serde_json::from_slice::<InlineTree>(bytes).map(Into::into)
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InlineTree {
        canvas: u32,
        background: Option<FixtureRgb>,
        primitives: Vec<InlinePrimitive>,
    }

    impl From<InlineTree> for RenderTree {
        fn from(tree: InlineTree) -> Self {
            Self {
                canvas: tree.canvas,
                background: tree.background.map(Into::into),
                pixel_grid: None,
                primitives: tree.primitives.into_iter().map(Into::into).collect(),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InlinePrimitive {
        shape: DecodedShape,
        transform: Option<FixtureTransform>,
        paint: DecodedPaint,
        stroke: Option<FixtureStroke>,
        opacity: Option<i64>,
        clip: Option<DecodedShape>,
        effects: Vec<InlineEffect>,
        label: String,
        role: Option<FixtureRole>,
    }

    impl From<InlinePrimitive> for Primitive {
        fn from(primitive: InlinePrimitive) -> Self {
            Self {
                shape: primitive.shape.0,
                transform: primitive.transform.map(Into::into),
                paint: primitive.paint.0,
                stroke: primitive.stroke.map(Into::into),
                opacity: primitive.opacity.map(Q32),
                clip: primitive.clip.map(|shape| shape.0),
                effects: primitive.effects.into_iter().map(Into::into).collect(),
                label: primitive.label,
                role: primitive.role.map(Into::into),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum InlineEffect {
        Lit {
            light_y: i64,
        },
        Masked {
            mask: Vec<InlinePrimitive>,
        },
        Turbulence {
            seed: u8,
            freq_h: i64,
            freq_v: i64,
            octaves: u8,
            scale: i64,
        },
    }

    impl From<InlineEffect> for Effect {
        fn from(effect: InlineEffect) -> Self {
            match effect {
                InlineEffect::Lit { light_y } => Self::Lit { light_y },
                InlineEffect::Masked { mask } => Self::Masked {
                    mask: mask.into_iter().map(Into::into).collect(),
                },
                InlineEffect::Turbulence {
                    seed,
                    freq_h,
                    freq_v,
                    octaves,
                    scale,
                } => Self::Turbulence {
                    seed,
                    freq_h: Q32(freq_h),
                    freq_v: Q32(freq_v),
                    octaves,
                    scale: Q32(scale),
                },
            }
        }
    }
}

#[path = "tree_feature_fixtures.rs"]
mod tree_features;

mod tests {
    use std::{fs, path::Path};

    use avatar_svg::{serialize_tree, serialize_tree_json};

    use super::*;

    fn primitive(label: &str, effects: &[String]) -> String {
        format!(
            r#"{{"clip":null,"effects":[{}],"label":"{label}","opacity":null,"paint":{{"kind":"none"}},"role":null,"shape":{{"cx":0,"cy":0,"kind":"circle","r":1}},"stroke":null,"transform":null}}"#,
            effects.join(",")
        )
    }

    fn masked(mask_ref: &str) -> String {
        format!(r#"{{"kind":"masked","mask_ref":{mask_ref}}}"#)
    }

    fn document(masks: &[&[String]], primitives: &[String]) -> String {
        let masks = masks
            .iter()
            .map(|entry| format!("[{}]", entry.join(",")))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"background":null,"canvas":240,"masks":[{masks}],"primitives":[{}]}}"#,
            primitives.join(",")
        )
    }

    fn decode(document: &str) -> Result<RenderTree, FixtureError> {
        decode_tree_fixture(document.as_bytes())
    }

    /// The message of a schema error; panics on success or on any other error.
    fn schema_error(document: &str) -> String {
        match decode(document) {
            Err(FixtureError::Json(error)) => error.to_string(),
            other => panic!("expected a schema error, got {other:?}"),
        }
    }

    fn circle(label: &str, effects: Vec<Effect>) -> Primitive {
        Primitive {
            shape: Shape::Circle { cx: 0, cy: 0, r: 1 },
            transform: None,
            paint: Paint::None,
            stroke: None,
            opacity: None,
            clip: None,
            effects,
            label: label.to_owned(),
            role: None,
        }
    }

    #[test]
    fn shared_mask_references_resolve_to_inline_masks() {
        let document = document(
            &[
                &[primitive("inner", &[])],
                &[primitive("outer", &[masked("0")])],
            ],
            &[
                primitive("a", &[masked("1")]),
                primitive("b", &[masked("1")]),
                primitive("c", &[masked("0")]),
            ],
        );
        let inner = vec![circle("inner", vec![])];
        let outer = vec![circle(
            "outer",
            vec![Effect::Masked {
                mask: inner.clone(),
            }],
        )];
        let tree = decode(&document).unwrap();
        assert_eq!(
            tree,
            RenderTree {
                canvas: 240,
                background: None,
                pixel_grid: None,
                primitives: vec![
                    circle(
                        "a",
                        vec![Effect::Masked {
                            mask: outer.clone()
                        }]
                    ),
                    circle("b", vec![Effect::Masked { mask: outer }]),
                    circle("c", vec![Effect::Masked { mask: inner }]),
                ],
            }
        );
        // The document is in canonical form, so the encoder reproduces it.
        assert_eq!(serialize_tree_json(&tree), document);
    }

    #[test]
    fn rejects_a_mask_ref_outside_the_table() {
        let past_the_end = document(
            &[&[primitive("m", &[])]],
            &[
                primitive("a", &[masked("0")]),
                primitive("b", &[masked("1")]),
            ],
        );
        assert!(
            matches!(
                decode(&past_the_end),
                Err(FixtureError::MaskRefOutOfRange {
                    mask_ref: 1,
                    entries: 1
                })
            ),
            "{:?}",
            decode(&past_the_end)
        );
        let empty_table = document(&[], &[primitive("a", &[masked("0")])]);
        assert!(matches!(
            decode(&empty_table),
            Err(FixtureError::MaskRefOutOfRange {
                mask_ref: 0,
                entries: 0
            })
        ));
        // A negative index is refused too, by the unsigned schema type.
        assert!(decode(&document(
            &[&[primitive("m", &[])]],
            &[
                primitive("a", &[masked("0")]),
                primitive("b", &[masked("-1")]),
            ],
        ))
        .is_err());
    }

    #[test]
    fn rejects_a_mask_entry_that_refers_to_itself_or_a_later_entry() {
        let itself = document(
            &[&[primitive("m", &[masked("0")])]],
            &[primitive("a", &[masked("0")])],
        );
        assert!(
            matches!(
                decode(&itself),
                Err(FixtureError::MaskRefNotEarlier {
                    entry: 0,
                    mask_ref: 0
                })
            ),
            "{:?}",
            decode(&itself)
        );
        let later = document(
            &[&[primitive("m", &[masked("1")])], &[primitive("n", &[])]],
            &[
                primitive("a", &[masked("0")]),
                primitive("b", &[masked("1")]),
            ],
        );
        assert!(matches!(
            decode(&later),
            Err(FixtureError::MaskRefNotEarlier {
                entry: 0,
                mask_ref: 1
            })
        ));
    }

    #[test]
    fn rejects_an_inline_mask_member() {
        let beside_reference = document(
            &[&[primitive("m", &[])]],
            &[primitive(
                "a",
                &[r#"{"kind":"masked","mask":[],"mask_ref":0}"#.to_owned()],
            )],
        );
        let error = schema_error(&beside_reference);
        assert!(error.contains("unknown field `mask`"), "{error}");
        let instead_of_reference = document(
            &[],
            &[primitive(
                "a",
                &[format!(
                    r#"{{"kind":"masked","mask":[{}]}}"#,
                    primitive("m", &[])
                )],
            )],
        );
        schema_error(&instead_of_reference);
    }

    #[test]
    fn rejects_an_unreferenced_mask_entry() {
        let unused = document(&[&[primitive("m", &[])]], &[primitive("a", &[])]);
        assert!(
            matches!(
                decode(&unused),
                Err(FixtureError::UnreferencedMask { entry: 0 })
            ),
            "{:?}",
            decode(&unused)
        );
        // When an unused entry is the only one referring to another, the
        // unused one is reported.
        let unused_chain = document(
            &[&[primitive("m", &[])], &[primitive("n", &[masked("0")])]],
            &[primitive("a", &[])],
        );
        assert!(matches!(
            decode(&unused_chain),
            Err(FixtureError::UnreferencedMask { entry: 1 })
        ));
    }

    #[test]
    fn rejects_a_missing_mask_table() {
        let error = schema_error(r#"{"background":null,"canvas":240,"primitives":[]}"#);
        assert!(error.contains("missing field `masks`"), "{error}");
    }

    #[test]
    fn rejects_a_non_integer_mask_ref() {
        for mask_ref in ["0.0", "0.5", "1e0", "\"0\"", "null"] {
            schema_error(&document(
                &[&[primitive("m", &[])]],
                &[primitive("a", &[masked(mask_ref)])],
            ));
        }
    }

    /// Checks the switch from inline masks to the mask table against the whole
    /// committed corpus: each tree, read in the inline-mask format it was
    /// minted in, must encode in the table format and decode back to the same
    /// tree. Prints the byte totals of both formats. It reads only inline-mask
    /// files, so it stops applying once the corpus is re-minted.
    #[test]
    #[ignore = "reads tree files in the inline-mask format that preceded the mask table"]
    fn inline_mask_corpus_round_trips_through_the_mask_table() {
        let vectors = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors");
        let mut names = fs::read_dir(vectors.join("tree"))
            .expect("read vectors/tree")
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.ends_with(".json"))
            .collect::<Vec<_>>();
        names.sort();
        let (mut inline_bytes, mut table_bytes) = (0, 0);
        for name in &names {
            let bytes = fs::read(vectors.join("tree").join(name)).unwrap();
            let original =
                inline_masks::decode(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
            // The committed generic SVG pins the drawing independently of
            // either JSON format, so a match shows the old reader lost nothing.
            let svg_name = name.replace(".json", ".svg");
            assert_eq!(
                serialize_tree(&original),
                fs::read(vectors.join("tree-svg").join(&svg_name)).unwrap(),
                "{name}: inline-mask reader disagrees with {svg_name}"
            );
            let table = serialize_tree_json(&original);
            let decoded = decode_tree_fixture(table.as_bytes())
                .unwrap_or_else(|error| panic!("{name}: mask table form: {error}"));
            assert_eq!(
                decoded, original,
                "{name}: tree changed in the mask table form"
            );
            assert_eq!(
                serialize_tree_json(&decoded),
                table,
                "{name}: mask table form does not re-encode to itself"
            );
            println!("{name}: {} -> {} bytes", bytes.len(), table.len());
            inline_bytes += bytes.len();
            table_bytes += table.len();
        }
        assert_eq!(names.len(), 58);
        println!(
            "{} trees: {inline_bytes} bytes inline, {table_bytes} bytes with the mask table",
            names.len()
        );
    }
}
