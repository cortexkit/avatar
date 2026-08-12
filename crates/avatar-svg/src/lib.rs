//! Geometry-primary avatar rendering with SVG as one serializer.
//!
//! The renderer is the third layer of the determinism contract (design note
//! 001): it consumes the typed phenotype and render-time inputs, never DNA.
//! All geometry is integer-only — i64 milliunits on a 1_000_000 canonical
//! canvas, Q32.32 factors, and `avatar_tables::div_round_half_away` as the
//! single pinned narrowing primitive — so every port produces byte-identical
//! output.
//!
//! The body and face passes assemble the flat render contract from design note
//! 006. Native consumers draw that tree directly; SVG preserves the established
//! byte representation as a pure projection of the same geometry.

mod brows;
mod build;
pub mod experimental;
mod eyes;
mod face;
mod fixed;
mod format;
mod geometry;
mod lashes;
mod mood;
mod mouth;
mod pattern;
mod serialize;
mod svg;
mod tree;

/// The vendored Voronoi pattern table (spec data, hash-pinned in tests).
mod voronoi {
    include!("generated/voronoi.rs");
}

pub use avatar_phenotype::{Bam16, Rgb, Q32};

pub use build::build_render_tree;
pub use geometry::{body_geometry, BodyGeometry};
pub use serialize::serialize_svg;
pub use svg::{render_body_svg, render_svg, Mood, RenderError, RenderParams};
pub use tree::{
    Affine, Effect, FillRule, GradientStop, Paint, Primitive, RenderTree, Segment, Shape, Stroke,
    StrokeCap, StrokeJoin, Transform, TransformOp,
};

#[cfg(test)]
mod tests {
    use super::*;
    use avatar_phenotype::{
        Animation, Asymmetry, Bam16, Body, BodyPoint, Brows, Circler, Cut, Eyes, Face,
        FacePlacement, Lashes, Layers, Milli, MoodResponse, Mouth, Palette, Pattern, Phenotype,
        Rgb, Voronoi,
    };

    fn default_params() -> RenderParams {
        RenderParams {
            mood: Mood::Neutral,
            growth: Q32::ONE,
            canvas: 10_000,
        }
    }

    fn point(x: i64, y: i64, tension: i64) -> BodyPoint {
        BodyPoint {
            x: Milli(x),
            y: Milli(y),
            tension: Q32(tension),
        }
    }

    /// Builds a complete phenotype around a body and pattern; every other
    /// region is deterministic filler, so render differences trace back to
    /// the fields under test.
    fn phenotype_with(body: Body, palette_seed: u8, pattern_class: u8) -> Phenotype {
        Phenotype {
            genome_version: 1,
            body,
            palette: palette(palette_seed),
            face: Face {
                eyes: Eyes {
                    position_mode: 0,
                    size_h: Q32(0),
                    ipd: Q32(0),
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
                    length_scale: Q32(0),
                    stroke_scale: Q32(0),
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
                r#class: pattern_class,
                circler: Circler {
                    point_reduce: 128,
                    size: Q32(Q32::ONE.0 / 2),
                    y_offset_div: Q32(Q32::ONE.0 / 2),
                    tension: Q32(Q32::ONE.0 / 4),
                    pull_min: Q32(Q32::ONE.0 / 2),
                },
                voronoi: Voronoi {
                    index: 2,
                    rotation: Bam16(8_192),
                    scale: Q32(Q32::ONE.0 / 2),
                },
                cut: Cut {
                    spread: Q32(Q32::ONE.0 / 4),
                    y_adjust_div: Q32(Q32::ONE.0 / 2),
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

    fn palette(seed: u8) -> Palette {
        let color = |offset: u8| Rgb {
            r: seed,
            g: seed.wrapping_add(offset),
            b: seed.wrapping_add(offset.wrapping_mul(2)),
        };
        Palette {
            back: color(1),
            primary: color(2),
            primary_lighter: color(3),
            primary_darkish: color(4),
            primary_darker: color(5),
            primary_darkest: color(6),
            harmonic1: color(7),
            harmonic1_darkish: color(8),
            harmonic1_darkest: color(9),
            harmonic2: color(10),
            harmonic3: color(11),
            harmonic4: color(12),
            secondary_darker: color(13),
            dark: color(14),
            white: color(15),
        }
    }

    fn body_with(points: &[BodyPoint]) -> Body {
        let mut body = Body {
            point_count: points.len() as u8,
            size: Milli(800_000),
            points: [BodyPoint {
                x: Milli(0),
                y: Milli(0),
                tension: Q32(Q32::ONE.0),
            }; 16],
        };
        body.points[..points.len()].copy_from_slice(points);
        body
    }

    fn sample_body() -> Body {
        body_with(&[
            point(120_000, 240_000, Q32::ONE.0),
            point(360_000, 480_000, Q32::ONE.0),
            point(560_000, 160_000, Q32::ONE.0),
        ])
    }

    #[test]
    fn every_mood_is_byte_identical_across_repeats() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        for mood in all_moods() {
            let mut params = default_params();
            params.mood = mood;
            let first = render_svg(&phenotype, &params).unwrap();
            let second = render_svg(&phenotype, &params).unwrap();
            assert_eq!(first, second, "mood {mood:?}");
        }
    }

    #[test]
    fn different_phenotypes_render_differently() {
        let params = default_params();
        let body_a = phenotype_with(sample_body(), 42, 0);
        let body_b = phenotype_with(
            body_with(&[
                point(80_000, 500_000, Q32::ONE.0),
                point(400_000, 200_000, Q32::ONE.0),
                point(640_000, 420_000, Q32::ONE.0),
            ]),
            42,
            0,
        );
        let palette_b = phenotype_with(sample_body(), 200, 0);
        let render_a = render_svg(&body_a, &params).unwrap();
        assert_ne!(render_a, render_svg(&body_b, &params).unwrap());
        assert_ne!(render_a, render_svg(&palette_b, &params).unwrap());
    }

    #[test]
    fn growth_other_than_one_is_refused() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        let mut params = default_params();
        params.growth = Q32(Q32::ONE.0 - 1);
        assert_eq!(
            render_svg(&phenotype, &params),
            Err(RenderError::UnsupportedGrowth(Q32(Q32::ONE.0 - 1)))
        );
        params.growth = Q32(0);
        assert_eq!(
            render_svg(&phenotype, &params),
            Err(RenderError::UnsupportedGrowth(Q32(0)))
        );
    }

    #[test]
    fn canvas_other_than_pinned_is_refused() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        let mut params = default_params();
        for canvas in [0u32, 9_999, 10_001, 255_000] {
            params.canvas = canvas;
            assert_eq!(
                render_svg(&phenotype, &params),
                Err(RenderError::UnsupportedCanvas(canvas))
            );
        }
    }

    #[test]
    fn body_only_render_remains_mood_independent() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        let neutral = render_body_svg(&phenotype, &default_params()).unwrap();
        for mood in all_moods() {
            let mut params = default_params();
            params.mood = mood;
            assert_eq!(render_body_svg(&phenotype, &params).unwrap(), neutral);
        }
    }

    #[test]
    fn experimental_off_state_is_tree_identical_to_the_stable_builder() {
        let phenotype = phenotype_with(sample_body(), 42, 4);
        let options = experimental::ExperimentalOptions {
            asymmetry_strength: Q32(0),
            layers: false,
        };
        assert_eq!(
            experimental::build_render_tree_experimental(&phenotype, &default_params(), &options)
                .unwrap(),
            build_render_tree(&phenotype, &default_params()).unwrap(),
        );
    }

    #[test]
    fn experimental_layers_precede_an_asymmetric_main_body_in_preview_svg() {
        let mut phenotype = phenotype_with(sample_body(), 42, 4);
        phenotype.asymmetry.deltas = [Q32::ONE; 8];
        phenotype.layers.count = 1;
        phenotype.layers.params = [Q32::ONE; 16];
        let options = experimental::ExperimentalOptions {
            asymmetry_strength: Q32::ONE,
            layers: true,
        };
        let stable = build_render_tree(&phenotype, &default_params()).unwrap();
        let preview =
            experimental::build_render_tree_experimental(&phenotype, &default_params(), &options)
                .unwrap();
        let layers: Vec<_> = preview
            .primitives
            .iter()
            .filter(|primitive| primitive.label.starts_with("av-experimental-layer-"))
            .collect();
        assert_eq!(layers.len(), 2);
        assert_eq!(
            layers[0].paint,
            Paint::Solid {
                color: phenotype.palette.harmonic2
            }
        );
        assert_eq!(
            layers[1].paint,
            Paint::Solid {
                color: phenotype.palette.harmonic1
            }
        );
        assert_eq!(preview.primitives[0].label, "av-experimental-layer-1");
        let stable_body = stable
            .primitives
            .iter()
            .find(|primitive| primitive.label == "av-body-base")
            .unwrap();
        let preview_body = preview
            .primitives
            .iter()
            .find(|primitive| primitive.label == "av-body-base")
            .unwrap();
        assert_ne!(preview_body.shape, stable_body.shape);
        let svg = String::from_utf8(experimental::serialize_svg_experimental(&preview)).unwrap();
        assert_well_formed_xml(&svg);
        assert!(svg.find("av-experimental-layer-1").unwrap() < svg.find("av-body-base").unwrap());
    }

    #[test]
    fn neutral_is_an_identity_but_happy_changes_the_face() {
        let mut phenotype = phenotype_with(sample_body(), 42, 0);
        phenotype.mood_response = MoodResponse {
            lid_reactivity: Q32::ONE,
            brow_reactivity: Q32::ONE,
            eye_rotation_reactivity: Q32::ONE,
            mouth_reactivity: Q32::ONE,
        };
        let neutral = render_svg(&phenotype, &default_params()).unwrap();
        assert_eq!(neutral, render_svg(&phenotype, &default_params()).unwrap());

        let mut zero_response = phenotype.clone();
        zero_response.mood_response = MoodResponse {
            lid_reactivity: Q32(0),
            brow_reactivity: Q32(0),
            eye_rotation_reactivity: Q32(0),
            mouth_reactivity: Q32(0),
        };
        assert_eq!(
            neutral,
            render_svg(&zero_response, &default_params()).unwrap()
        );

        let mut happy = default_params();
        happy.mood = Mood::Happy;
        assert_ne!(neutral, render_svg(&phenotype, &happy).unwrap());
    }

    #[test]
    fn output_is_well_formed_xml_with_the_pinned_structure() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        let svg = String::from_utf8(render_svg(&phenotype, &default_params()).unwrap()).unwrap();
        assert_well_formed_xml(&svg);
        assert!(svg
            .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10000 10000\">"));
        assert!(svg.contains("<path id=\"av-bp\" d=\"M"));
        assert!(svg.contains("<g id=\"av-face\">"));
        let mouth = svg.find("id=\"av-mouth\"").unwrap();
        let left_eye = svg.find("id=\"av-eye-left\"").unwrap();
        let right_eye = svg.find("id=\"av-eye-right\"").unwrap();
        let brows = svg.find("id=\"av-brow-left\"").unwrap();
        assert!(mouth < left_eye && left_eye < right_eye && right_eye < brows);
        // Gradient stops appear in offset order.
        let first = svg.find("offset=\"89%\"").unwrap();
        let second = svg.find("offset=\"100%\"").unwrap();
        assert!(first < second);
        assert!(svg.contains("<rect width=\"10000\" height=\"10000\""));
    }

    #[test]
    fn patterned_output_adds_mask_and_pattern_use() {
        let phenotype = phenotype_with(sample_body(), 42, 4);
        let svg =
            String::from_utf8(render_body_svg(&phenotype, &default_params()).unwrap()).unwrap();
        assert_well_formed_xml(&svg);
        assert_eq!(svg.matches("<path").count(), 2);
        assert!(svg.contains("<mask id=\"av-pattern\">"));
        assert!(svg.contains("mask=\"url(#av-pattern)\""));
    }

    #[test]
    fn full_render_is_well_formed_at_every_mood() {
        for mood in all_moods() {
            let phenotype = phenotype_with(sample_body(), 42, mood as u8 % 6);
            let mut params = default_params();
            params.mood = mood;
            let svg = String::from_utf8(render_svg(&phenotype, &params).unwrap()).unwrap();
            assert_well_formed_xml(&svg);
        }
    }

    #[test]
    fn two_point_sliver_face_search_is_total() {
        let phenotype = phenotype_with(
            body_with(&[point(2, 100_000, Q32::ONE.0), point(2, 700_000, Q32::ONE.0)]),
            42,
            0,
        );
        let svg = String::from_utf8(render_svg(&phenotype, &default_params()).unwrap()).unwrap();
        assert_well_formed_xml(&svg);
        assert!(svg.contains("id=\"av-mouth\""));
    }

    /// Float tripwire: source geometry must stay on the shared integer path.
    #[test]
    fn source_has_no_binary_float_type() {
        let needle = ["f", "64"].concat();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        scan_rust_sources(&source, &needle);
    }

    #[test]
    fn bounds_are_sane_across_point_counts() {
        // point_count 2 (degenerate) and 16 (max) plus a mid-range shape.
        let bodies = [
            body_with(&[
                point(200_000, 100_000, Q32::ONE.0),
                point(600_000, 500_000, Q32::ONE.0),
            ]),
            body_with(
                std::array::from_fn::<BodyPoint, 16, _>(|index| {
                    point(
                        60_000 + 50_000 * index as i64,
                        100_000 + 40_000 * ((index as i64 * 7) % 11),
                        Q32::ONE.0,
                    )
                })
                .as_slice(),
            ),
            sample_body(),
        ];
        for body in &bodies {
            let geometry = body_geometry(&phenotype_with(body.clone(), 42, 0));
            assert!(!geometry.all_points.is_empty());
            assert!(geometry.top < geometry.bottom);
            assert!(geometry.body_width > 0);
            assert!(geometry.body_height > 0);
            assert_eq!(geometry.body_height, geometry.bottom - geometry.top);
        }
    }

    fn all_moods() -> [Mood; 7] {
        [
            Mood::Neutral,
            Mood::Working,
            Mood::Happy,
            Mood::Waiting,
            Mood::Blocked,
            Mood::Sad,
            Mood::Sleepy,
        ]
    }

    fn scan_rust_sources(directory: &std::path::Path, needle: &str) {
        for entry in std::fs::read_dir(directory).expect("read source directory") {
            let path = entry.expect("read source entry").path();
            if path.is_dir() {
                scan_rust_sources(&path, needle);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read Rust source");
            let mut in_block_comment = false;
            for (line_number, line) in source.lines().enumerate() {
                let code = uncommented_code(line, &mut in_block_comment);
                assert!(
                    !code.contains(needle),
                    "binary float type in {}:{}",
                    path.display(),
                    line_number + 1,
                );
            }
        }
    }

    fn uncommented_code(line: &str, in_block_comment: &mut bool) -> String {
        let mut rest = line;
        let mut code = String::new();
        loop {
            if *in_block_comment {
                let Some(end) = rest.find("*/") else {
                    break;
                };
                rest = &rest[end + 2..];
                *in_block_comment = false;
                continue;
            }
            let line_comment = rest.find("//");
            let block_comment = rest.find("/*");
            match (line_comment, block_comment) {
                (Some(line_start), Some(block_start)) if block_start < line_start => {
                    code.push_str(&rest[..block_start]);
                    rest = &rest[block_start + 2..];
                    *in_block_comment = true;
                }
                (Some(line_start), _) => {
                    code.push_str(&rest[..line_start]);
                    break;
                }
                (None, Some(block_start)) => {
                    code.push_str(&rest[..block_start]);
                    rest = &rest[block_start + 2..];
                    *in_block_comment = true;
                }
                (None, None) => {
                    code.push_str(rest);
                    break;
                }
            }
        }
        code
    }

    #[test]
    fn voronoi_table_hash_is_pinned() {
        let mut hasher = blake3::Hasher::new();
        for pattern in voronoi::VORONOI_PATTERNS {
            hasher.update(pattern.as_bytes());
            hasher.update(b"\x1f"); // unit separator: pin the string bounds, not just the blob
        }
        assert_eq!(
            hasher.finalize().to_hex().as_str(),
            "0f080cce4b31019700389825292df78b9fe921e8d410acb2ed763952e899a150"
        );
    }

    /// Minimal tag-balance checker: enough to prove the output is well-formed
    /// XML without an XML dependency. Handles self-closing tags, closing
    /// tags, comments, and quoted attribute values.
    fn assert_well_formed_xml(xml: &str) {
        let mut stack: Vec<String> = Vec::new();
        let mut index = 0;
        let bytes = xml.as_bytes();
        while index < bytes.len() {
            if bytes[index] != b'<' {
                index += 1;
                continue;
            }
            if xml[index..].starts_with("<!--") {
                let end = xml[index..].find("-->").expect("unterminated comment");
                index += end + 3;
                continue;
            }
            let tag_end = xml[index..].find('>').expect("unterminated tag");
            let tag = &xml[index + 1..index + tag_end];
            if let Some(name) = tag.strip_prefix('/') {
                let expected = stack.pop().expect("closing tag without an open tag");
                assert_eq!(name.trim(), expected, "mismatched closing tag");
            } else {
                let name: String = tag
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, ':' | '-')
                    })
                    .collect();
                assert!(!name.is_empty(), "empty tag name at byte {index}");
                let attributes = &tag[name.len()..];
                let quotes = attributes.bytes().filter(|byte| *byte == b'"').count();
                assert_eq!(quotes % 2, 0, "unbalanced attribute quotes in <{name}>");
                if !attributes.trim_end().ends_with('/') {
                    stack.push(name);
                }
            }
            index += tag_end + 1;
        }
        assert!(stack.is_empty(), "unclosed tags: {stack:?}");
    }
}
