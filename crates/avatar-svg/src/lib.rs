//! Avatar SVG rendering: `(Phenotype, RenderParams) -> SVG bytes`.
//!
//! The renderer is the third layer of the determinism contract (design note
//! 001): it consumes the typed phenotype and render-time inputs, never DNA.
//! All geometry is integer-only — i64 milliunits on a 1_000_000 canonical
//! canvas, Q32.32 factors, and `avatar_tables::div_round_half_away` as the
//! single pinned narrowing primitive — so every port produces byte-identical
//! output.
//!
//! This crate currently covers the body pass (design note 004): mirrored
//! spline geometry, flattening, bounds, rescale, path serialization, and
//! body paint (background, base fill/stroke, radial gradient, lighting
//! filter, pattern mask). The face pass builds on the public
//! [`BodyGeometry`] in a later note.

mod fixed;
mod format;
mod geometry;
mod pattern;
mod svg;

/// The vendored Voronoi pattern table (spec data, hash-pinned in tests).
mod voronoi {
    include!("generated/voronoi.rs");
}

pub use avatar_phenotype::Q32;

pub use geometry::{body_geometry, BodyGeometry};
pub use svg::{render_svg, Mood, RenderError, RenderParams};

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
    fn render_is_byte_identical_across_repeats() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        let first = render_svg(&phenotype, &default_params()).unwrap();
        let second = render_svg(&phenotype, &default_params()).unwrap();
        assert_eq!(first, second);
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
    fn mood_is_accepted_without_changing_the_body_bytes() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        let neutral = render_svg(&phenotype, &default_params()).unwrap();
        for mood in [
            Mood::Working,
            Mood::Happy,
            Mood::Waiting,
            Mood::Blocked,
            Mood::Sad,
            Mood::Sleepy,
        ] {
            let mut params = default_params();
            params.mood = mood;
            assert_eq!(render_svg(&phenotype, &params).unwrap(), neutral);
        }
    }

    #[test]
    fn output_is_well_formed_xml_with_the_pinned_structure() {
        let phenotype = phenotype_with(sample_body(), 42, 0);
        let svg = String::from_utf8(render_svg(&phenotype, &default_params()).unwrap()).unwrap();
        assert_well_formed_xml(&svg);
        assert!(svg
            .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10000 10000\">"));
        // Pattern class 0 emits exactly one path definition.
        assert_eq!(svg.matches("<path").count(), 1);
        assert!(svg.contains("<path id=\"av-bp\" d=\"M"));
        // Gradient stops appear in offset order.
        let first = svg.find("offset=\"89%\"").unwrap();
        let second = svg.find("offset=\"100%\"").unwrap();
        assert!(first < second);
        assert!(svg.contains("<rect width=\"10000\" height=\"10000\""));
    }

    #[test]
    fn patterned_output_adds_mask_and_pattern_use() {
        let phenotype = phenotype_with(sample_body(), 42, 4);
        let svg = String::from_utf8(render_svg(&phenotype, &default_params()).unwrap()).unwrap();
        assert_well_formed_xml(&svg);
        assert_eq!(svg.matches("<path").count(), 2);
        assert!(svg.contains("<mask id=\"av-pattern\">"));
        assert!(svg.contains("mask=\"url(#av-pattern)\""));
    }

    #[test]
    fn every_pattern_class_renders_well_formed_svg() {
        for class in 0..=5u8 {
            let phenotype = phenotype_with(sample_body(), 42, class);
            let svg =
                String::from_utf8(render_svg(&phenotype, &default_params()).unwrap()).unwrap();
            assert_well_formed_xml(&svg);
        }
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
