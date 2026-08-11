//! Typed phenotype data and its canonical wire representation.
//!
//! Values crossing the phenotype boundary are integers in fixed units. Keeping
//! the units in distinct newtypes prevents renderers and decoders from
//! accidentally treating a raw coefficient as a canvas coordinate.

use std::fmt;

use serde_json::{Map, Value};

/// A canvas length or coordinate in milliunits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Milli(pub i64);

impl From<i64> for Milli {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

/// A raw signed Q32.32 fixed-point coefficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Q32(pub i64);

impl From<i64> for Q32 {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

/// An angle expressed as a binary angle measurement with 65,536 units per turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bam16(pub u16);

impl From<u16> for Bam16 {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

/// An 8-bit RGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phenotype {
    pub genome_version: u16,
    pub body: Body,
    pub palette: Palette,
    pub face: Face,
    pub pattern: Pattern,
    pub animation: Animation,
    pub mood_response: MoodResponse,
    pub asymmetry: Asymmetry,
    pub layers: Layers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    pub point_count: u8,
    pub size: Milli,
    pub points: [BodyPoint; 16],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyPoint {
    pub x: Milli,
    pub y: Milli,
    pub tension: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub back: Rgb,
    pub primary: Rgb,
    pub primary_lighter: Rgb,
    pub primary_darkish: Rgb,
    pub primary_darker: Rgb,
    pub primary_darkest: Rgb,
    pub harmonic1: Rgb,
    pub harmonic1_darkish: Rgb,
    pub harmonic1_darkest: Rgb,
    pub harmonic2: Rgb,
    pub harmonic3: Rgb,
    pub harmonic4: Rgb,
    pub secondary_darker: Rgb,
    pub dark: Rgb,
    pub white: Rgb,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub eyes: Eyes,
    pub brows: Brows,
    pub lashes: Lashes,
    pub mouth: Mouth,
    pub placement: FacePlacement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eyes {
    pub position_mode: u8,
    pub size_h: Milli,
    pub ipd: Milli,
    pub height_upper: Q32,
    pub height_lower: Q32,
    pub height_bias_gate: bool,
    pub height_bias: Q32,
    pub vertical_pull_gate: bool,
    pub vertical_pull: Q32,
    pub contour_inset1: Q32,
    pub contour_inset2: Q32,
    pub eyeball_radius_scale: Q32,
    pub pupil_scale: Q32,
    pub iris_shade_opacity: Q32,
    pub iris_shade_reduce: Q32,
    pub iris_saturation_reduce: Q32,
    pub lid_close_upper_neutral: Q32,
    pub lid_close_lower_neutral: Q32,
    pub lid_extension_gate: u8,
    pub lid_extension_side: u8,
    pub noise_freq_h: Q32,
    pub noise_freq_v: Q32,
    pub noise_seed: u8,
    pub rotation_seed: u8,
    pub lid_color_gene: u8,
    pub iris_color_gene: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brows {
    pub end_height1: Q32,
    pub mid_x: Q32,
    pub end_height2: Q32,
    pub tensions: [Q32; 3],
    pub width_coupling: Q32,
    pub clearance: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lashes {
    pub count: u8,
    pub removed: u8,
    pub length: Milli,
    pub stroke: Milli,
    pub direction: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mouth {
    pub family: u8,
    pub width_scale_gate: bool,
    pub width_scale: Q32,
    pub aspect_widen_gate: bool,
    pub params: [Q32; 8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacePlacement {
    pub width_coeff: Q32,
    pub height_coeff: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub r#class: u8,
    pub circler: Circler,
    pub voronoi: Voronoi,
    pub cut: Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circler {
    pub point_reduce: u8,
    pub size: Q32,
    pub y_offset_div: Q32,
    pub tension: Q32,
    pub pull_min: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voronoi {
    pub index: u8,
    pub rotation: Bam16,
    pub scale: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cut {
    pub spread: Q32,
    pub y_adjust_div: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    pub breath_period: u16,
    pub breath_amplitude: Q32,
    pub blink_interval: u16,
    pub blink_jitter_seed: u8,
    pub mood_transition_tempo: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoodResponse {
    pub lid_reactivity: Q32,
    pub brow_reactivity: Q32,
    pub eye_rotation_reactivity: Q32,
    pub mouth_reactivity: Q32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asymmetry {
    pub deltas: [Q32; 8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layers {
    pub count: u8,
    pub params: [Q32; 16],
}

/// Serializes a phenotype as avatar canonical JSON v1.
///
/// This writer is deliberately hand-rolled instead of using `serde_json` for
/// output: canonical key ordering and byte shape are part of the wire
/// contract, not an incidental property of a dependency's serializer. Gate
/// booleans are encoded as integer `0` or `1` because canonical JSON v1
/// permits no boolean token.
pub fn to_canonical_json(phenotype: &Phenotype) -> Vec<u8> {
    let mut writer = Writer::default();
    write_phenotype(&mut writer, phenotype);
    writer.into_bytes()
}

/// Parses a semantically valid phenotype JSON object.
///
/// Parsing accepts non-canonical whitespace and key order so consumers can
/// receive ordinary JSON, but rejects unknown and missing fields. Strict shape
/// validation prevents a silently ignored field from changing an avatar's
/// identity when it is canonicalized again.
pub fn from_canonical_json(bytes: &[u8]) -> Result<Phenotype, ParseError> {
    let value = serde_json::from_slice(bytes)
        .map_err(|error| ParseError::new(format!("invalid JSON: {error}")))?;
    parse_phenotype(&value)
}

/// An error returned when input cannot represent the complete phenotype schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn integer<T: fmt::Display>(&mut self, value: T) {
        self.bytes.extend_from_slice(value.to_string().as_bytes());
    }

    fn gate(&mut self, value: bool) {
        self.integer(u8::from(value));
    }

    fn object(&mut self, write_fields: impl FnOnce(&mut ObjectWriter<'_>)) {
        self.bytes.push(b'{');
        let mut object = ObjectWriter {
            writer: self,
            is_first: true,
        };
        write_fields(&mut object);
        object.writer.bytes.push(b'}');
    }

    fn array<T>(&mut self, values: &[T], write_value: impl Fn(&mut Self, &T)) {
        self.bytes.push(b'[');
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                self.bytes.push(b',');
            }
            write_value(self, value);
        }
        self.bytes.push(b']');
    }

    fn key(&mut self, key: &str) {
        self.bytes.push(b'"');
        self.bytes.extend_from_slice(key.as_bytes());
        self.bytes.extend_from_slice(b"\":");
    }
}

struct ObjectWriter<'a> {
    writer: &'a mut Writer,
    is_first: bool,
}

impl ObjectWriter<'_> {
    fn field(&mut self, key: &str, write_value: impl FnOnce(&mut Writer)) {
        if !self.is_first {
            self.writer.bytes.push(b',');
        }
        self.is_first = false;
        self.writer.key(key);
        write_value(self.writer);
    }
}

fn write_phenotype(writer: &mut Writer, phenotype: &Phenotype) {
    let Phenotype {
        genome_version,
        body,
        palette,
        face,
        pattern,
        animation,
        mood_response,
        asymmetry,
        layers,
    } = phenotype;
    writer.object(|object| {
        object.field("animation", |writer| write_animation(writer, animation));
        object.field("asymmetry", |writer| write_asymmetry(writer, asymmetry));
        object.field("body", |writer| write_body(writer, body));
        object.field("face", |writer| write_face(writer, face));
        object.field("genome_version", |writer| writer.integer(*genome_version));
        object.field("layers", |writer| write_layers(writer, layers));
        object.field("mood_response", |writer| {
            write_mood_response(writer, mood_response)
        });
        object.field("palette", |writer| write_palette(writer, palette));
        object.field("pattern", |writer| write_pattern(writer, pattern));
    });
}

fn write_body(writer: &mut Writer, body: &Body) {
    let Body {
        point_count,
        size,
        points,
    } = body;
    writer.object(|object| {
        object.field("point_count", |writer| writer.integer(*point_count));
        object.field("points", |writer| {
            writer.array(points, write_body_point);
        });
        object.field("size", |writer| writer.integer(size.0));
    });
}

fn write_body_point(writer: &mut Writer, point: &BodyPoint) {
    let BodyPoint { x, y, tension } = point;
    writer.object(|object| {
        object.field("tension", |writer| writer.integer(tension.0));
        object.field("x", |writer| writer.integer(x.0));
        object.field("y", |writer| writer.integer(y.0));
    });
}

fn write_palette(writer: &mut Writer, palette: &Palette) {
    let Palette {
        back,
        primary,
        primary_lighter,
        primary_darkish,
        primary_darker,
        primary_darkest,
        harmonic1,
        harmonic1_darkish,
        harmonic1_darkest,
        harmonic2,
        harmonic3,
        harmonic4,
        secondary_darker,
        dark,
        white,
    } = palette;
    writer.object(|object| {
        object.field("back", |writer| write_rgb(writer, back));
        object.field("dark", |writer| write_rgb(writer, dark));
        object.field("harmonic1", |writer| write_rgb(writer, harmonic1));
        object.field("harmonic1_darkish", |writer| {
            write_rgb(writer, harmonic1_darkish)
        });
        object.field("harmonic1_darkest", |writer| {
            write_rgb(writer, harmonic1_darkest)
        });
        object.field("harmonic2", |writer| write_rgb(writer, harmonic2));
        object.field("harmonic3", |writer| write_rgb(writer, harmonic3));
        object.field("harmonic4", |writer| write_rgb(writer, harmonic4));
        object.field("primary", |writer| write_rgb(writer, primary));
        object.field("primary_darker", |writer| write_rgb(writer, primary_darker));
        object.field("primary_darkest", |writer| {
            write_rgb(writer, primary_darkest)
        });
        object.field("primary_darkish", |writer| {
            write_rgb(writer, primary_darkish)
        });
        object.field("primary_lighter", |writer| {
            write_rgb(writer, primary_lighter)
        });
        object.field("secondary_darker", |writer| {
            write_rgb(writer, secondary_darker)
        });
        object.field("white", |writer| write_rgb(writer, white));
    });
}

fn write_rgb(writer: &mut Writer, rgb: &Rgb) {
    let Rgb { r, g, b } = rgb;
    writer.object(|object| {
        object.field("b", |writer| writer.integer(*b));
        object.field("g", |writer| writer.integer(*g));
        object.field("r", |writer| writer.integer(*r));
    });
}

fn write_face(writer: &mut Writer, face: &Face) {
    let Face {
        eyes,
        brows,
        lashes,
        mouth,
        placement,
    } = face;
    writer.object(|object| {
        object.field("brows", |writer| write_brows(writer, brows));
        object.field("eyes", |writer| write_eyes(writer, eyes));
        object.field("lashes", |writer| write_lashes(writer, lashes));
        object.field("mouth", |writer| write_mouth(writer, mouth));
        object.field("placement", |writer| {
            write_face_placement(writer, placement)
        });
    });
}

fn write_eyes(writer: &mut Writer, eyes: &Eyes) {
    let Eyes {
        position_mode,
        size_h,
        ipd,
        height_upper,
        height_lower,
        height_bias_gate,
        height_bias,
        vertical_pull_gate,
        vertical_pull,
        contour_inset1,
        contour_inset2,
        eyeball_radius_scale,
        pupil_scale,
        iris_shade_opacity,
        iris_shade_reduce,
        iris_saturation_reduce,
        lid_close_upper_neutral,
        lid_close_lower_neutral,
        lid_extension_gate,
        lid_extension_side,
        noise_freq_h,
        noise_freq_v,
        noise_seed,
        rotation_seed,
        lid_color_gene,
        iris_color_gene,
    } = eyes;
    writer.object(|object| {
        object.field("contour_inset1", |writer| writer.integer(contour_inset1.0));
        object.field("contour_inset2", |writer| writer.integer(contour_inset2.0));
        object.field("eyeball_radius_scale", |writer| {
            writer.integer(eyeball_radius_scale.0)
        });
        object.field("height_bias", |writer| writer.integer(height_bias.0));
        object.field("height_bias_gate", |writer| writer.gate(*height_bias_gate));
        object.field("height_lower", |writer| writer.integer(height_lower.0));
        object.field("height_upper", |writer| writer.integer(height_upper.0));
        object.field("ipd", |writer| writer.integer(ipd.0));
        object.field("iris_color_gene", |writer| writer.integer(*iris_color_gene));
        object.field("iris_saturation_reduce", |writer| {
            writer.integer(iris_saturation_reduce.0)
        });
        object.field("iris_shade_opacity", |writer| {
            writer.integer(iris_shade_opacity.0)
        });
        object.field("iris_shade_reduce", |writer| {
            writer.integer(iris_shade_reduce.0)
        });
        object.field("lid_close_lower_neutral", |writer| {
            writer.integer(lid_close_lower_neutral.0)
        });
        object.field("lid_close_upper_neutral", |writer| {
            writer.integer(lid_close_upper_neutral.0)
        });
        object.field("lid_color_gene", |writer| writer.integer(*lid_color_gene));
        object.field("lid_extension_gate", |writer| {
            writer.integer(*lid_extension_gate)
        });
        object.field("lid_extension_side", |writer| {
            writer.integer(*lid_extension_side)
        });
        object.field("noise_freq_h", |writer| writer.integer(noise_freq_h.0));
        object.field("noise_freq_v", |writer| writer.integer(noise_freq_v.0));
        object.field("noise_seed", |writer| writer.integer(*noise_seed));
        object.field("position_mode", |writer| writer.integer(*position_mode));
        object.field("pupil_scale", |writer| writer.integer(pupil_scale.0));
        object.field("rotation_seed", |writer| writer.integer(*rotation_seed));
        object.field("size_h", |writer| writer.integer(size_h.0));
        object.field("vertical_pull", |writer| writer.integer(vertical_pull.0));
        object.field("vertical_pull_gate", |writer| {
            writer.gate(*vertical_pull_gate)
        });
    });
}

fn write_brows(writer: &mut Writer, brows: &Brows) {
    let Brows {
        end_height1,
        mid_x,
        end_height2,
        tensions,
        width_coupling,
        clearance,
    } = brows;
    writer.object(|object| {
        object.field("clearance", |writer| writer.integer(clearance.0));
        object.field("end_height1", |writer| writer.integer(end_height1.0));
        object.field("end_height2", |writer| writer.integer(end_height2.0));
        object.field("mid_x", |writer| writer.integer(mid_x.0));
        object.field("tensions", |writer| writer.array(tensions, write_q32));
        object.field("width_coupling", |writer| writer.integer(width_coupling.0));
    });
}

fn write_lashes(writer: &mut Writer, lashes: &Lashes) {
    let Lashes {
        count,
        removed,
        length,
        stroke,
        direction,
    } = lashes;
    writer.object(|object| {
        object.field("count", |writer| writer.integer(*count));
        object.field("direction", |writer| writer.integer(*direction));
        object.field("length", |writer| writer.integer(length.0));
        object.field("removed", |writer| writer.integer(*removed));
        object.field("stroke", |writer| writer.integer(stroke.0));
    });
}

fn write_mouth(writer: &mut Writer, mouth: &Mouth) {
    let Mouth {
        family,
        width_scale_gate,
        width_scale,
        aspect_widen_gate,
        params,
    } = mouth;
    writer.object(|object| {
        object.field("aspect_widen_gate", |writer| {
            writer.gate(*aspect_widen_gate)
        });
        object.field("family", |writer| writer.integer(*family));
        object.field("params", |writer| writer.array(params, write_q32));
        object.field("width_scale", |writer| writer.integer(width_scale.0));
        object.field("width_scale_gate", |writer| writer.gate(*width_scale_gate));
    });
}

fn write_face_placement(writer: &mut Writer, placement: &FacePlacement) {
    let FacePlacement {
        width_coeff,
        height_coeff,
    } = placement;
    writer.object(|object| {
        object.field("height_coeff", |writer| writer.integer(height_coeff.0));
        object.field("width_coeff", |writer| writer.integer(width_coeff.0));
    });
}

fn write_pattern(writer: &mut Writer, pattern: &Pattern) {
    let Pattern {
        r#class,
        circler,
        voronoi,
        cut,
    } = pattern;
    writer.object(|object| {
        object.field("circler", |writer| write_circler(writer, circler));
        object.field("class", |writer| writer.integer(*r#class));
        object.field("cut", |writer| write_cut(writer, cut));
        object.field("voronoi", |writer| write_voronoi(writer, voronoi));
    });
}

fn write_circler(writer: &mut Writer, circler: &Circler) {
    let Circler {
        point_reduce,
        size,
        y_offset_div,
        tension,
        pull_min,
    } = circler;
    writer.object(|object| {
        object.field("point_reduce", |writer| writer.integer(*point_reduce));
        object.field("pull_min", |writer| writer.integer(pull_min.0));
        object.field("size", |writer| writer.integer(size.0));
        object.field("tension", |writer| writer.integer(tension.0));
        object.field("y_offset_div", |writer| writer.integer(y_offset_div.0));
    });
}

fn write_voronoi(writer: &mut Writer, voronoi: &Voronoi) {
    let Voronoi {
        index,
        rotation,
        scale,
    } = voronoi;
    writer.object(|object| {
        object.field("index", |writer| writer.integer(*index));
        object.field("rotation", |writer| writer.integer(rotation.0));
        object.field("scale", |writer| writer.integer(scale.0));
    });
}

fn write_cut(writer: &mut Writer, cut: &Cut) {
    let Cut {
        spread,
        y_adjust_div,
    } = cut;
    writer.object(|object| {
        object.field("spread", |writer| writer.integer(spread.0));
        object.field("y_adjust_div", |writer| writer.integer(y_adjust_div.0));
    });
}

fn write_animation(writer: &mut Writer, animation: &Animation) {
    let Animation {
        breath_period,
        breath_amplitude,
        blink_interval,
        blink_jitter_seed,
        mood_transition_tempo,
    } = animation;
    writer.object(|object| {
        object.field("blink_interval", |writer| writer.integer(*blink_interval));
        object.field("blink_jitter_seed", |writer| {
            writer.integer(*blink_jitter_seed)
        });
        object.field("breath_amplitude", |writer| {
            writer.integer(breath_amplitude.0)
        });
        object.field("breath_period", |writer| writer.integer(*breath_period));
        object.field("mood_transition_tempo", |writer| {
            writer.integer(mood_transition_tempo.0)
        });
    });
}

fn write_mood_response(writer: &mut Writer, mood_response: &MoodResponse) {
    let MoodResponse {
        lid_reactivity,
        brow_reactivity,
        eye_rotation_reactivity,
        mouth_reactivity,
    } = mood_response;
    writer.object(|object| {
        object.field("brow_reactivity", |writer| {
            writer.integer(brow_reactivity.0)
        });
        object.field("eye_rotation_reactivity", |writer| {
            writer.integer(eye_rotation_reactivity.0)
        });
        object.field("lid_reactivity", |writer| writer.integer(lid_reactivity.0));
        object.field("mouth_reactivity", |writer| {
            writer.integer(mouth_reactivity.0)
        });
    });
}

fn write_asymmetry(writer: &mut Writer, asymmetry: &Asymmetry) {
    let Asymmetry { deltas } = asymmetry;
    writer.object(|object| {
        object.field("deltas", |writer| writer.array(deltas, write_q32));
    });
}

fn write_layers(writer: &mut Writer, layers: &Layers) {
    let Layers { count, params } = layers;
    writer.object(|object| {
        object.field("count", |writer| writer.integer(*count));
        object.field("params", |writer| writer.array(params, write_q32));
    });
}

fn write_q32(writer: &mut Writer, value: &Q32) {
    writer.integer(value.0);
}

fn parse_phenotype(value: &Value) -> Result<Phenotype, ParseError> {
    let object = object(value, "phenotype")?;
    check_keys(
        object,
        &[
            "genome_version",
            "body",
            "palette",
            "face",
            "pattern",
            "animation",
            "mood_response",
            "asymmetry",
            "layers",
        ],
        "phenotype",
    )?;
    Ok(Phenotype {
        genome_version: u16_value(
            required(object, "genome_version", "phenotype")?,
            "genome_version",
        )?,
        body: parse_body(required(object, "body", "phenotype")?)?,
        palette: parse_palette(required(object, "palette", "phenotype")?)?,
        face: parse_face(required(object, "face", "phenotype")?)?,
        pattern: parse_pattern(required(object, "pattern", "phenotype")?)?,
        animation: parse_animation(required(object, "animation", "phenotype")?)?,
        mood_response: parse_mood_response(required(object, "mood_response", "phenotype")?)?,
        asymmetry: parse_asymmetry(required(object, "asymmetry", "phenotype")?)?,
        layers: parse_layers(required(object, "layers", "phenotype")?)?,
    })
}

fn parse_body(value: &Value) -> Result<Body, ParseError> {
    let object = object(value, "body")?;
    check_keys(object, &["point_count", "size", "points"], "body")?;
    Ok(Body {
        point_count: u8_value(required(object, "point_count", "body")?, "body.point_count")?,
        size: milli_value(required(object, "size", "body")?, "body.size")?,
        points: array_value(
            required(object, "points", "body")?,
            "body.points",
            |item, _| parse_body_point(item),
        )?,
    })
}

fn parse_body_point(value: &Value) -> Result<BodyPoint, ParseError> {
    let object = object(value, "body point")?;
    check_keys(object, &["x", "y", "tension"], "body point")?;
    Ok(BodyPoint {
        x: milli_value(required(object, "x", "body point")?, "body.points[].x")?,
        y: milli_value(required(object, "y", "body point")?, "body.points[].y")?,
        tension: q32_value(
            required(object, "tension", "body point")?,
            "body.points[].tension",
        )?,
    })
}

fn parse_palette(value: &Value) -> Result<Palette, ParseError> {
    let object = object(value, "palette")?;
    check_keys(
        object,
        &[
            "back",
            "primary",
            "primary_lighter",
            "primary_darkish",
            "primary_darker",
            "primary_darkest",
            "harmonic1",
            "harmonic1_darkish",
            "harmonic1_darkest",
            "harmonic2",
            "harmonic3",
            "harmonic4",
            "secondary_darker",
            "dark",
            "white",
        ],
        "palette",
    )?;
    Ok(Palette {
        back: parse_rgb(required(object, "back", "palette")?)?,
        primary: parse_rgb(required(object, "primary", "palette")?)?,
        primary_lighter: parse_rgb(required(object, "primary_lighter", "palette")?)?,
        primary_darkish: parse_rgb(required(object, "primary_darkish", "palette")?)?,
        primary_darker: parse_rgb(required(object, "primary_darker", "palette")?)?,
        primary_darkest: parse_rgb(required(object, "primary_darkest", "palette")?)?,
        harmonic1: parse_rgb(required(object, "harmonic1", "palette")?)?,
        harmonic1_darkish: parse_rgb(required(object, "harmonic1_darkish", "palette")?)?,
        harmonic1_darkest: parse_rgb(required(object, "harmonic1_darkest", "palette")?)?,
        harmonic2: parse_rgb(required(object, "harmonic2", "palette")?)?,
        harmonic3: parse_rgb(required(object, "harmonic3", "palette")?)?,
        harmonic4: parse_rgb(required(object, "harmonic4", "palette")?)?,
        secondary_darker: parse_rgb(required(object, "secondary_darker", "palette")?)?,
        dark: parse_rgb(required(object, "dark", "palette")?)?,
        white: parse_rgb(required(object, "white", "palette")?)?,
    })
}

fn parse_rgb(value: &Value) -> Result<Rgb, ParseError> {
    let object = object(value, "RGB color")?;
    check_keys(object, &["r", "g", "b"], "RGB color")?;
    Ok(Rgb {
        r: u8_value(required(object, "r", "RGB color")?, "color.r")?,
        g: u8_value(required(object, "g", "RGB color")?, "color.g")?,
        b: u8_value(required(object, "b", "RGB color")?, "color.b")?,
    })
}

fn parse_face(value: &Value) -> Result<Face, ParseError> {
    let object = object(value, "face")?;
    check_keys(
        object,
        &["eyes", "brows", "lashes", "mouth", "placement"],
        "face",
    )?;
    Ok(Face {
        eyes: parse_eyes(required(object, "eyes", "face")?)?,
        brows: parse_brows(required(object, "brows", "face")?)?,
        lashes: parse_lashes(required(object, "lashes", "face")?)?,
        mouth: parse_mouth(required(object, "mouth", "face")?)?,
        placement: parse_face_placement(required(object, "placement", "face")?)?,
    })
}

fn parse_eyes(value: &Value) -> Result<Eyes, ParseError> {
    let object = object(value, "eyes")?;
    check_keys(
        object,
        &[
            "position_mode",
            "size_h",
            "ipd",
            "height_upper",
            "height_lower",
            "height_bias_gate",
            "height_bias",
            "vertical_pull_gate",
            "vertical_pull",
            "contour_inset1",
            "contour_inset2",
            "eyeball_radius_scale",
            "pupil_scale",
            "iris_shade_opacity",
            "iris_shade_reduce",
            "iris_saturation_reduce",
            "lid_close_upper_neutral",
            "lid_close_lower_neutral",
            "lid_extension_gate",
            "lid_extension_side",
            "noise_freq_h",
            "noise_freq_v",
            "noise_seed",
            "rotation_seed",
            "lid_color_gene",
            "iris_color_gene",
        ],
        "eyes",
    )?;
    Ok(Eyes {
        position_mode: u8_value(
            required(object, "position_mode", "eyes")?,
            "eyes.position_mode",
        )?,
        size_h: milli_value(required(object, "size_h", "eyes")?, "eyes.size_h")?,
        ipd: milli_value(required(object, "ipd", "eyes")?, "eyes.ipd")?,
        height_upper: q32_value(
            required(object, "height_upper", "eyes")?,
            "eyes.height_upper",
        )?,
        height_lower: q32_value(
            required(object, "height_lower", "eyes")?,
            "eyes.height_lower",
        )?,
        height_bias_gate: gate_value(
            required(object, "height_bias_gate", "eyes")?,
            "eyes.height_bias_gate",
        )?,
        height_bias: q32_value(required(object, "height_bias", "eyes")?, "eyes.height_bias")?,
        vertical_pull_gate: gate_value(
            required(object, "vertical_pull_gate", "eyes")?,
            "eyes.vertical_pull_gate",
        )?,
        vertical_pull: q32_value(
            required(object, "vertical_pull", "eyes")?,
            "eyes.vertical_pull",
        )?,
        contour_inset1: q32_value(
            required(object, "contour_inset1", "eyes")?,
            "eyes.contour_inset1",
        )?,
        contour_inset2: q32_value(
            required(object, "contour_inset2", "eyes")?,
            "eyes.contour_inset2",
        )?,
        eyeball_radius_scale: q32_value(
            required(object, "eyeball_radius_scale", "eyes")?,
            "eyes.eyeball_radius_scale",
        )?,
        pupil_scale: q32_value(required(object, "pupil_scale", "eyes")?, "eyes.pupil_scale")?,
        iris_shade_opacity: q32_value(
            required(object, "iris_shade_opacity", "eyes")?,
            "eyes.iris_shade_opacity",
        )?,
        iris_shade_reduce: q32_value(
            required(object, "iris_shade_reduce", "eyes")?,
            "eyes.iris_shade_reduce",
        )?,
        iris_saturation_reduce: q32_value(
            required(object, "iris_saturation_reduce", "eyes")?,
            "eyes.iris_saturation_reduce",
        )?,
        lid_close_upper_neutral: q32_value(
            required(object, "lid_close_upper_neutral", "eyes")?,
            "eyes.lid_close_upper_neutral",
        )?,
        lid_close_lower_neutral: q32_value(
            required(object, "lid_close_lower_neutral", "eyes")?,
            "eyes.lid_close_lower_neutral",
        )?,
        lid_extension_gate: u8_value(
            required(object, "lid_extension_gate", "eyes")?,
            "eyes.lid_extension_gate",
        )?,
        lid_extension_side: u8_value(
            required(object, "lid_extension_side", "eyes")?,
            "eyes.lid_extension_side",
        )?,
        noise_freq_h: q32_value(
            required(object, "noise_freq_h", "eyes")?,
            "eyes.noise_freq_h",
        )?,
        noise_freq_v: q32_value(
            required(object, "noise_freq_v", "eyes")?,
            "eyes.noise_freq_v",
        )?,
        noise_seed: u8_value(required(object, "noise_seed", "eyes")?, "eyes.noise_seed")?,
        rotation_seed: u8_value(
            required(object, "rotation_seed", "eyes")?,
            "eyes.rotation_seed",
        )?,
        lid_color_gene: u8_value(
            required(object, "lid_color_gene", "eyes")?,
            "eyes.lid_color_gene",
        )?,
        iris_color_gene: u8_value(
            required(object, "iris_color_gene", "eyes")?,
            "eyes.iris_color_gene",
        )?,
    })
}

fn parse_brows(value: &Value) -> Result<Brows, ParseError> {
    let object = object(value, "brows")?;
    check_keys(
        object,
        &[
            "end_height1",
            "mid_x",
            "end_height2",
            "tensions",
            "width_coupling",
            "clearance",
        ],
        "brows",
    )?;
    Ok(Brows {
        end_height1: q32_value(
            required(object, "end_height1", "brows")?,
            "brows.end_height1",
        )?,
        mid_x: q32_value(required(object, "mid_x", "brows")?, "brows.mid_x")?,
        end_height2: q32_value(
            required(object, "end_height2", "brows")?,
            "brows.end_height2",
        )?,
        tensions: array_value(
            required(object, "tensions", "brows")?,
            "brows.tensions",
            q32_value,
        )?,
        width_coupling: q32_value(
            required(object, "width_coupling", "brows")?,
            "brows.width_coupling",
        )?,
        clearance: q32_value(required(object, "clearance", "brows")?, "brows.clearance")?,
    })
}

fn parse_lashes(value: &Value) -> Result<Lashes, ParseError> {
    let object = object(value, "lashes")?;
    check_keys(
        object,
        &["count", "removed", "length", "stroke", "direction"],
        "lashes",
    )?;
    Ok(Lashes {
        count: u8_value(required(object, "count", "lashes")?, "lashes.count")?,
        removed: u8_value(required(object, "removed", "lashes")?, "lashes.removed")?,
        length: milli_value(required(object, "length", "lashes")?, "lashes.length")?,
        stroke: milli_value(required(object, "stroke", "lashes")?, "lashes.stroke")?,
        direction: u8_value(required(object, "direction", "lashes")?, "lashes.direction")?,
    })
}

fn parse_mouth(value: &Value) -> Result<Mouth, ParseError> {
    let object = object(value, "mouth")?;
    check_keys(
        object,
        &[
            "family",
            "width_scale_gate",
            "width_scale",
            "aspect_widen_gate",
            "params",
        ],
        "mouth",
    )?;
    Ok(Mouth {
        family: u8_value(required(object, "family", "mouth")?, "mouth.family")?,
        width_scale_gate: gate_value(
            required(object, "width_scale_gate", "mouth")?,
            "mouth.width_scale_gate",
        )?,
        width_scale: q32_value(
            required(object, "width_scale", "mouth")?,
            "mouth.width_scale",
        )?,
        aspect_widen_gate: gate_value(
            required(object, "aspect_widen_gate", "mouth")?,
            "mouth.aspect_widen_gate",
        )?,
        params: array_value(
            required(object, "params", "mouth")?,
            "mouth.params",
            q32_value,
        )?,
    })
}

fn parse_face_placement(value: &Value) -> Result<FacePlacement, ParseError> {
    let object = object(value, "face placement")?;
    check_keys(object, &["width_coeff", "height_coeff"], "face placement")?;
    Ok(FacePlacement {
        width_coeff: q32_value(
            required(object, "width_coeff", "face placement")?,
            "placement.width_coeff",
        )?,
        height_coeff: q32_value(
            required(object, "height_coeff", "face placement")?,
            "placement.height_coeff",
        )?,
    })
}

fn parse_pattern(value: &Value) -> Result<Pattern, ParseError> {
    let object = object(value, "pattern")?;
    check_keys(object, &["class", "circler", "voronoi", "cut"], "pattern")?;
    Ok(Pattern {
        r#class: u8_value(required(object, "class", "pattern")?, "pattern.class")?,
        circler: parse_circler(required(object, "circler", "pattern")?)?,
        voronoi: parse_voronoi(required(object, "voronoi", "pattern")?)?,
        cut: parse_cut(required(object, "cut", "pattern")?)?,
    })
}

fn parse_circler(value: &Value) -> Result<Circler, ParseError> {
    let object = object(value, "circler pattern")?;
    check_keys(
        object,
        &[
            "point_reduce",
            "size",
            "y_offset_div",
            "tension",
            "pull_min",
        ],
        "circler pattern",
    )?;
    Ok(Circler {
        point_reduce: u8_value(
            required(object, "point_reduce", "circler pattern")?,
            "circler.point_reduce",
        )?,
        size: q32_value(required(object, "size", "circler pattern")?, "circler.size")?,
        y_offset_div: q32_value(
            required(object, "y_offset_div", "circler pattern")?,
            "circler.y_offset_div",
        )?,
        tension: q32_value(
            required(object, "tension", "circler pattern")?,
            "circler.tension",
        )?,
        pull_min: q32_value(
            required(object, "pull_min", "circler pattern")?,
            "circler.pull_min",
        )?,
    })
}

fn parse_voronoi(value: &Value) -> Result<Voronoi, ParseError> {
    let object = object(value, "voronoi pattern")?;
    check_keys(object, &["index", "rotation", "scale"], "voronoi pattern")?;
    Ok(Voronoi {
        index: u8_value(
            required(object, "index", "voronoi pattern")?,
            "voronoi.index",
        )?,
        rotation: bam16_value(
            required(object, "rotation", "voronoi pattern")?,
            "voronoi.rotation",
        )?,
        scale: q32_value(
            required(object, "scale", "voronoi pattern")?,
            "voronoi.scale",
        )?,
    })
}

fn parse_cut(value: &Value) -> Result<Cut, ParseError> {
    let object = object(value, "cut pattern")?;
    check_keys(object, &["spread", "y_adjust_div"], "cut pattern")?;
    Ok(Cut {
        spread: q32_value(required(object, "spread", "cut pattern")?, "cut.spread")?,
        y_adjust_div: q32_value(
            required(object, "y_adjust_div", "cut pattern")?,
            "cut.y_adjust_div",
        )?,
    })
}

fn parse_animation(value: &Value) -> Result<Animation, ParseError> {
    let object = object(value, "animation")?;
    check_keys(
        object,
        &[
            "breath_period",
            "breath_amplitude",
            "blink_interval",
            "blink_jitter_seed",
            "mood_transition_tempo",
        ],
        "animation",
    )?;
    Ok(Animation {
        breath_period: u16_value(
            required(object, "breath_period", "animation")?,
            "animation.breath_period",
        )?,
        breath_amplitude: q32_value(
            required(object, "breath_amplitude", "animation")?,
            "animation.breath_amplitude",
        )?,
        blink_interval: u16_value(
            required(object, "blink_interval", "animation")?,
            "animation.blink_interval",
        )?,
        blink_jitter_seed: u8_value(
            required(object, "blink_jitter_seed", "animation")?,
            "animation.blink_jitter_seed",
        )?,
        mood_transition_tempo: q32_value(
            required(object, "mood_transition_tempo", "animation")?,
            "animation.mood_transition_tempo",
        )?,
    })
}

fn parse_mood_response(value: &Value) -> Result<MoodResponse, ParseError> {
    let object = object(value, "mood response")?;
    check_keys(
        object,
        &[
            "lid_reactivity",
            "brow_reactivity",
            "eye_rotation_reactivity",
            "mouth_reactivity",
        ],
        "mood response",
    )?;
    Ok(MoodResponse {
        lid_reactivity: q32_value(
            required(object, "lid_reactivity", "mood response")?,
            "mood_response.lid_reactivity",
        )?,
        brow_reactivity: q32_value(
            required(object, "brow_reactivity", "mood response")?,
            "mood_response.brow_reactivity",
        )?,
        eye_rotation_reactivity: q32_value(
            required(object, "eye_rotation_reactivity", "mood response")?,
            "mood_response.eye_rotation_reactivity",
        )?,
        mouth_reactivity: q32_value(
            required(object, "mouth_reactivity", "mood response")?,
            "mood_response.mouth_reactivity",
        )?,
    })
}

fn parse_asymmetry(value: &Value) -> Result<Asymmetry, ParseError> {
    let object = object(value, "asymmetry")?;
    check_keys(object, &["deltas"], "asymmetry")?;
    Ok(Asymmetry {
        deltas: array_value(
            required(object, "deltas", "asymmetry")?,
            "asymmetry.deltas",
            q32_value,
        )?,
    })
}

fn parse_layers(value: &Value) -> Result<Layers, ParseError> {
    let object = object(value, "layers")?;
    check_keys(object, &["count", "params"], "layers")?;
    Ok(Layers {
        count: u8_value(required(object, "count", "layers")?, "layers.count")?,
        params: array_value(
            required(object, "params", "layers")?,
            "layers.params",
            q32_value,
        )?,
    })
}

fn object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, ParseError> {
    value
        .as_object()
        .ok_or_else(|| ParseError::new(format!("{context} must be an object")))
}

fn check_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> Result<(), ParseError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ParseError::new(format!(
                "unknown field `{key}` in {context}"
            )));
        }
    }
    Ok(())
}

fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a Value, ParseError> {
    object
        .get(key)
        .ok_or_else(|| ParseError::new(format!("missing field `{key}` in {context}")))
}

fn array_value<T, const N: usize>(
    value: &Value,
    context: &str,
    parse_item: impl Fn(&Value, &str) -> Result<T, ParseError>,
) -> Result<[T; N], ParseError> {
    let array = value
        .as_array()
        .ok_or_else(|| ParseError::new(format!("{context} must be an array")))?;
    if array.len() != N {
        return Err(ParseError::new(format!(
            "{context} must contain exactly {N} values, found {}",
            array.len()
        )));
    }

    array
        .iter()
        .enumerate()
        .map(|(index, item)| parse_item(item, &format!("{context}[{index}]")))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| ParseError::new(format!("{context} must contain exactly {N} values")))
}

fn i64_value(value: &Value, context: &str) -> Result<i64, ParseError> {
    value
        .as_i64()
        .ok_or_else(|| ParseError::new(format!("{context} must be an i64 integer")))
}

fn u8_value(value: &Value, context: &str) -> Result<u8, ParseError> {
    let value = value
        .as_u64()
        .ok_or_else(|| ParseError::new(format!("{context} must be a u8 integer")))?;
    u8::try_from(value).map_err(|_| ParseError::new(format!("{context} is outside the u8 range")))
}

fn u16_value(value: &Value, context: &str) -> Result<u16, ParseError> {
    let value = value
        .as_u64()
        .ok_or_else(|| ParseError::new(format!("{context} must be a u16 integer")))?;
    u16::try_from(value).map_err(|_| ParseError::new(format!("{context} is outside the u16 range")))
}

fn milli_value(value: &Value, context: &str) -> Result<Milli, ParseError> {
    i64_value(value, context).map(Milli)
}

fn q32_value(value: &Value, context: &str) -> Result<Q32, ParseError> {
    i64_value(value, context).map(Q32)
}

fn bam16_value(value: &Value, context: &str) -> Result<Bam16, ParseError> {
    u16_value(value, context).map(Bam16)
}

fn gate_value(value: &Value, context: &str) -> Result<bool, ParseError> {
    match u8_value(value, context)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ParseError::new(format!(
            "{context} must be encoded as canonical gate integer 0 or 1"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_fully_populated_phenotype() {
        let phenotype = sample_phenotype();
        let canonical = to_canonical_json(&phenotype);
        assert_eq!(from_canonical_json(&canonical).unwrap(), phenotype);
    }

    #[test]
    fn round_trips_neutral_phenotype() {
        let phenotype = neutral_phenotype();
        let canonical = to_canonical_json(&phenotype);
        assert_eq!(from_canonical_json(&canonical).unwrap(), phenotype);
    }

    #[test]
    fn canonicalization_is_stable_after_reordered_whitespace_input() {
        let phenotype = sample_phenotype();
        let canonical = to_canonical_json(&phenotype);
        assert_eq!(to_canonical_json(&phenotype), canonical);

        let members = top_level_members(&canonical);
        let reordered = format!(
            " {{\n  {}\n }} ",
            members
                .iter()
                .rev()
                .map(|member| std::str::from_utf8(member).unwrap())
                .collect::<Vec<_>>()
                .join(",\n  ")
        );
        assert_eq!(
            to_canonical_json(&from_canonical_json(reordered.as_bytes()).unwrap()),
            canonical
        );
    }

    #[test]
    fn top_level_keys_are_emitted_in_byte_lexicographic_order() {
        let json = to_canonical_json(&sample_phenotype());
        let keys = top_level_members(&json)
            .iter()
            .map(|member| member_key(member).to_owned())
            .collect::<Vec<_>>();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn parser_rejects_unknown_missing_out_of_range_float_and_boolean_tokens() {
        let canonical = String::from_utf8(to_canonical_json(&sample_phenotype())).unwrap();
        assert!(from_canonical_json(
            canonical
                .replacen(
                    "\"genome_version\":12",
                    "\"unknown\":1,\"genome_version\":12",
                    1
                )
                .as_bytes()
        )
        .is_err());
        let missing_layers = format!(
            "{{{}}}",
            top_level_members(canonical.as_bytes())
                .into_iter()
                .filter(|member| member_key(member) != "layers")
                .map(|member| std::str::from_utf8(member).unwrap())
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(from_canonical_json(missing_layers.as_bytes()).is_err());
        assert!(from_canonical_json(
            canonical
                .replacen("\"position_mode\":4", "\"position_mode\":256", 1)
                .as_bytes()
        )
        .is_err());
        assert!(from_canonical_json(
            canonical
                .replacen("\"size_h\":1200", "\"size_h\":1.0", 1)
                .as_bytes()
        )
        .is_err());
        assert!(from_canonical_json(
            canonical
                .replacen("\"height_bias_gate\":1", "\"height_bias_gate\":true", 1)
                .as_bytes()
        )
        .is_err());
    }

    #[test]
    fn serialized_sample_covers_every_schema_field_name() {
        let json = String::from_utf8(to_canonical_json(&sample_phenotype())).unwrap();
        for field in [
            "genome_version",
            "body",
            "palette",
            "face",
            "pattern",
            "animation",
            "mood_response",
            "asymmetry",
            "layers",
            "point_count",
            "size",
            "points",
            "x",
            "y",
            "tension",
            "back",
            "primary",
            "primary_lighter",
            "primary_darkish",
            "primary_darker",
            "primary_darkest",
            "harmonic1",
            "harmonic1_darkish",
            "harmonic1_darkest",
            "harmonic2",
            "harmonic3",
            "harmonic4",
            "secondary_darker",
            "dark",
            "white",
            "r",
            "g",
            "b",
            "eyes",
            "brows",
            "lashes",
            "mouth",
            "placement",
            "position_mode",
            "size_h",
            "ipd",
            "height_upper",
            "height_lower",
            "height_bias_gate",
            "height_bias",
            "vertical_pull_gate",
            "vertical_pull",
            "contour_inset1",
            "contour_inset2",
            "eyeball_radius_scale",
            "pupil_scale",
            "iris_shade_opacity",
            "iris_shade_reduce",
            "iris_saturation_reduce",
            "lid_close_upper_neutral",
            "lid_close_lower_neutral",
            "lid_extension_gate",
            "lid_extension_side",
            "noise_freq_h",
            "noise_freq_v",
            "noise_seed",
            "rotation_seed",
            "lid_color_gene",
            "iris_color_gene",
            "end_height1",
            "mid_x",
            "end_height2",
            "tensions",
            "width_coupling",
            "clearance",
            "count",
            "removed",
            "length",
            "stroke",
            "direction",
            "family",
            "width_scale_gate",
            "width_scale",
            "aspect_widen_gate",
            "params",
            "width_coeff",
            "height_coeff",
            "class",
            "circler",
            "voronoi",
            "cut",
            "point_reduce",
            "y_offset_div",
            "pull_min",
            "index",
            "rotation",
            "scale",
            "spread",
            "y_adjust_div",
            "breath_period",
            "breath_amplitude",
            "blink_interval",
            "blink_jitter_seed",
            "mood_transition_tempo",
            "lid_reactivity",
            "brow_reactivity",
            "eye_rotation_reactivity",
            "mouth_reactivity",
            "deltas",
        ] {
            assert!(
                json.contains(&format!("\"{field}\":")),
                "missing field {field}"
            );
        }
    }

    fn sample_phenotype() -> Phenotype {
        Phenotype {
            genome_version: 12,
            body: Body {
                point_count: 16,
                size: Milli(900_000),
                points: std::array::from_fn(|index| BodyPoint {
                    x: Milli(index as i64 * 100 - 750),
                    y: Milli(900 - index as i64 * 85),
                    tension: Q32(index as i64 * 10_000 - 60_000),
                }),
            },
            palette: Palette {
                back: rgb(1),
                primary: rgb(2),
                primary_lighter: rgb(3),
                primary_darkish: rgb(4),
                primary_darker: rgb(5),
                primary_darkest: rgb(6),
                harmonic1: rgb(7),
                harmonic1_darkish: rgb(8),
                harmonic1_darkest: rgb(9),
                harmonic2: rgb(10),
                harmonic3: rgb(11),
                harmonic4: rgb(12),
                secondary_darker: rgb(13),
                dark: rgb(14),
                white: rgb(15),
            },
            face: Face {
                eyes: Eyes {
                    position_mode: 4,
                    size_h: Milli(1200),
                    ipd: Milli(2400),
                    height_upper: Q32(1),
                    height_lower: Q32(2),
                    height_bias_gate: true,
                    height_bias: Q32(3),
                    vertical_pull_gate: false,
                    vertical_pull: Q32(4),
                    contour_inset1: Q32(5),
                    contour_inset2: Q32(6),
                    eyeball_radius_scale: Q32(7),
                    pupil_scale: Q32(8),
                    iris_shade_opacity: Q32(9),
                    iris_shade_reduce: Q32(10),
                    iris_saturation_reduce: Q32(11),
                    lid_close_upper_neutral: Q32(12),
                    lid_close_lower_neutral: Q32(13),
                    lid_extension_gate: 14,
                    lid_extension_side: 15,
                    noise_freq_h: Q32(16),
                    noise_freq_v: Q32(17),
                    noise_seed: 18,
                    rotation_seed: 19,
                    lid_color_gene: 20,
                    iris_color_gene: 21,
                },
                brows: Brows {
                    end_height1: Q32(22),
                    mid_x: Q32(23),
                    end_height2: Q32(24),
                    tensions: [Q32(25), Q32(26), Q32(27)],
                    width_coupling: Q32(28),
                    clearance: Q32(29),
                },
                lashes: Lashes {
                    count: 30,
                    removed: 31,
                    length: Milli(3200),
                    stroke: Milli(3300),
                    direction: 32,
                },
                mouth: Mouth {
                    family: 33,
                    width_scale_gate: true,
                    width_scale: Q32(34),
                    aspect_widen_gate: false,
                    params: std::array::from_fn(|index| Q32(index as i64 + 35)),
                },
                placement: FacePlacement {
                    width_coeff: Q32(43),
                    height_coeff: Q32(44),
                },
            },
            pattern: Pattern {
                r#class: 45,
                circler: Circler {
                    point_reduce: 46,
                    size: Q32(47),
                    y_offset_div: Q32(48),
                    tension: Q32(49),
                    pull_min: Q32(50),
                },
                voronoi: Voronoi {
                    index: 51,
                    rotation: Bam16(52_000),
                    scale: Q32(53),
                },
                cut: Cut {
                    spread: Q32(54),
                    y_adjust_div: Q32(55),
                },
            },
            animation: Animation {
                breath_period: 56,
                breath_amplitude: Q32(57),
                blink_interval: 58,
                blink_jitter_seed: 59,
                mood_transition_tempo: Q32(60),
            },
            mood_response: MoodResponse {
                lid_reactivity: Q32(61),
                brow_reactivity: Q32(62),
                eye_rotation_reactivity: Q32(63),
                mouth_reactivity: Q32(64),
            },
            asymmetry: Asymmetry {
                deltas: std::array::from_fn(|index| Q32(index as i64 + 65)),
            },
            layers: Layers {
                count: 73,
                params: std::array::from_fn(|index| Q32(index as i64 + 74)),
            },
        }
    }

    fn neutral_phenotype() -> Phenotype {
        Phenotype {
            genome_version: 0,
            body: Body {
                point_count: 0,
                size: Milli(0),
                points: [BodyPoint {
                    x: Milli(0),
                    y: Milli(0),
                    tension: Q32(0),
                }; 16],
            },
            palette: Palette {
                back: rgb(0),
                primary: rgb(0),
                primary_lighter: rgb(0),
                primary_darkish: rgb(0),
                primary_darker: rgb(0),
                primary_darkest: rgb(0),
                harmonic1: rgb(0),
                harmonic1_darkish: rgb(0),
                harmonic1_darkest: rgb(0),
                harmonic2: rgb(0),
                harmonic3: rgb(0),
                harmonic4: rgb(0),
                secondary_darker: rgb(0),
                dark: rgb(0),
                white: rgb(0),
            },
            face: Face {
                eyes: Eyes {
                    position_mode: 0,
                    size_h: Milli(0),
                    ipd: Milli(0),
                    height_upper: Q32(0),
                    height_lower: Q32(0),
                    height_bias_gate: false,
                    height_bias: Q32(0),
                    vertical_pull_gate: false,
                    vertical_pull: Q32(0),
                    contour_inset1: Q32(0),
                    contour_inset2: Q32(0),
                    eyeball_radius_scale: Q32(0),
                    pupil_scale: Q32(0),
                    iris_shade_opacity: Q32(0),
                    iris_shade_reduce: Q32(0),
                    iris_saturation_reduce: Q32(0),
                    lid_close_upper_neutral: Q32(0),
                    lid_close_lower_neutral: Q32(0),
                    lid_extension_gate: 0,
                    lid_extension_side: 0,
                    noise_freq_h: Q32(0),
                    noise_freq_v: Q32(0),
                    noise_seed: 0,
                    rotation_seed: 0,
                    lid_color_gene: 0,
                    iris_color_gene: 0,
                },
                brows: Brows {
                    end_height1: Q32(0),
                    mid_x: Q32(0),
                    end_height2: Q32(0),
                    tensions: [Q32(0); 3],
                    width_coupling: Q32(0),
                    clearance: Q32(0),
                },
                lashes: Lashes {
                    count: 0,
                    removed: 0,
                    length: Milli(0),
                    stroke: Milli(0),
                    direction: 0,
                },
                mouth: Mouth {
                    family: 0,
                    width_scale_gate: false,
                    width_scale: Q32(0),
                    aspect_widen_gate: false,
                    params: [Q32(0); 8],
                },
                placement: FacePlacement {
                    width_coeff: Q32(0),
                    height_coeff: Q32(0),
                },
            },
            pattern: Pattern {
                r#class: 0,
                circler: Circler {
                    point_reduce: 0,
                    size: Q32(0),
                    y_offset_div: Q32(0),
                    tension: Q32(0),
                    pull_min: Q32(0),
                },
                voronoi: Voronoi {
                    index: 0,
                    rotation: Bam16(0),
                    scale: Q32(0),
                },
                cut: Cut {
                    spread: Q32(0),
                    y_adjust_div: Q32(0),
                },
            },
            animation: Animation {
                breath_period: 0,
                breath_amplitude: Q32(0),
                blink_interval: 0,
                blink_jitter_seed: 0,
                mood_transition_tempo: Q32(0),
            },
            mood_response: MoodResponse {
                lid_reactivity: Q32(0),
                brow_reactivity: Q32(0),
                eye_rotation_reactivity: Q32(0),
                mouth_reactivity: Q32(0),
            },
            asymmetry: Asymmetry {
                deltas: [Q32(0); 8],
            },
            layers: Layers {
                count: 0,
                params: [Q32(0); 16],
            },
        }
    }

    fn rgb(seed: u8) -> Rgb {
        Rgb {
            r: seed,
            g: seed.wrapping_add(1),
            b: seed.wrapping_add(2),
        }
    }

    fn top_level_members(bytes: &[u8]) -> Vec<&[u8]> {
        assert_eq!(bytes.first(), Some(&b'{'));
        let mut index = 1;
        let mut members = Vec::new();
        loop {
            skip_whitespace(bytes, &mut index);
            if bytes[index] == b'}' {
                break;
            }
            let start = index;
            let _ = parse_string(bytes, &mut index);
            skip_whitespace(bytes, &mut index);
            assert_eq!(bytes[index], b':');
            index += 1;
            skip_whitespace(bytes, &mut index);
            skip_json_value(bytes, &mut index);
            members.push(&bytes[start..index]);
            skip_whitespace(bytes, &mut index);
            if bytes[index] == b',' {
                index += 1;
            } else {
                assert_eq!(bytes[index], b'}');
                break;
            }
        }
        members
    }

    fn member_key(member: &[u8]) -> &str {
        let mut index = 0;
        parse_string(member, &mut index)
    }

    fn parse_string<'a>(bytes: &'a [u8], index: &mut usize) -> &'a str {
        assert_eq!(bytes[*index], b'"');
        *index += 1;
        let start = *index;
        while bytes[*index] != b'"' {
            *index += 1;
        }
        let value = std::str::from_utf8(&bytes[start..*index]).unwrap();
        *index += 1;
        value
    }

    fn skip_json_value(bytes: &[u8], index: &mut usize) {
        match bytes[*index] {
            b'{' | b'[' => {
                let mut expected_closers = Vec::new();
                let mut in_string = false;
                let mut escaped = false;
                while !expected_closers.is_empty() || *index < bytes.len() {
                    let byte = bytes[*index];
                    *index += 1;
                    if in_string {
                        if escaped {
                            escaped = false;
                        } else if byte == b'\\' {
                            escaped = true;
                        } else if byte == b'"' {
                            in_string = false;
                        }
                        continue;
                    }
                    match byte {
                        b'"' => in_string = true,
                        b'{' => expected_closers.push(b'}'),
                        b'[' => expected_closers.push(b']'),
                        b'}' | b']' => {
                            assert_eq!(expected_closers.pop(), Some(byte));
                            if expected_closers.is_empty() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            b'"' => {
                let _ = parse_string(bytes, index);
            }
            _ => {
                while *index < bytes.len()
                    && !matches!(
                        bytes[*index],
                        b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t'
                    )
                {
                    *index += 1;
                }
            }
        }
    }

    fn skip_whitespace(bytes: &[u8], index: &mut usize) {
        while *index < bytes.len() && matches!(bytes[*index], b' ' | b'\n' | b'\r' | b'\t') {
            *index += 1;
        }
    }
}
