use avatar_phenotype::{
    Animation, Asymmetry, Bam16, Body, BodyPoint, Brows, Circler, Cut, Eyes, Face, FacePlacement,
    Lashes, Layers, Milli, MoodResponse, Mouth, Palette, Pattern, Phenotype, Rgb, Voronoi, Q32,
};
use avatar_tables::{div_round_half_away, EASE_EXPO, EASE_IN_CIRC, EASE_LINEAR};

use crate::Genome;

include!("generated/palette.rs");

const GENOME_VERSION: u16 = 1;
const BODY_START: usize = 0;
const PALETTE_START: usize = 512;
const FACE_START: usize = 576;
const PATTERN_START: usize = 1_088;
const ASYMMETRY_START: usize = 1_216;
const LAYERS_START: usize = 1_280;
const ANIMATION_START: usize = 1_536;
const Q32_ONE: i64 = 1 << 32;
const Q32_SIZE_MIN: i64 = 3_006_477_107; // 0.7, rounded to Q32.32.
const Q32_SIZE_MAX: i64 = 6_012_954_214; // 1.4, rounded to Q32.32.
const Q32_PUPIL_MIN: i64 = 644_245_094; // 0.15, rounded to Q32.32.
const Q32_PUPIL_MAX: i64 = 2_576_980_378; // 0.6, rounded to Q32.32.
const Q32_EYEBALL_MIN: i64 = 2_576_980_378; // 0.6, rounded to Q32.32.
const Q32_LASH_LENGTH_MAX: i64 = 3_865_470_566; // 0.9, rounded to Q32.32.
const Q32_LASH_STROKE_MIN: i64 = 85_899_346; // 0.02, rounded to Q32.32.
const Q32_LASH_STROKE_MAX: i64 = 515_396_076; // 0.12, rounded to Q32.32.
const Q32_REACTIVITY_MIN: i64 = Q32_ONE / 2;
const Q32_REACTIVITY_MAX: i64 = Q32_ONE + Q32_ONE / 2;

#[derive(Clone, Copy)]
struct Mapping {
    table: &'static [u64; 256],
    lo: i64,
    hi: i64,
}

#[derive(Clone, Copy)]
enum MappedField {
    Unit,
    BodySize,
    BodyPointCoordinate,
    BodyPointTension,
    EyeSize,
    EyeIpd,
    PupilScale,
    EyeballRadiusScale,
    LashLengthScale,
    LashStrokeScale,
    PlacementCoefficient,
    BreathPeriod,
    BlinkInterval,
    MoodReactivity,
}

/// The normative v1 byte-to-value mapping table from design note 003.
///
/// These table/range pairs define observable phenotype values without changing
/// genome offsets. Altering any entry changes generated avatars and therefore
/// requires a generator-version bump and renewed compatibility vectors.
fn mapping(field: MappedField, body_size: i64) -> Mapping {
    match field {
        MappedField::Unit => Mapping {
            table: &EASE_LINEAR,
            lo: 0,
            hi: Q32_ONE,
        },
        MappedField::BodySize => Mapping {
            table: &EASE_LINEAR,
            lo: 714_285,
            hi: 833_333,
        },
        MappedField::BodyPointCoordinate => Mapping {
            table: &EASE_LINEAR,
            lo: 0,
            hi: body_size,
        },
        MappedField::BodyPointTension => Mapping {
            table: &EASE_LINEAR,
            lo: 0,
            hi: 3 * Q32_ONE / 2,
        },
        MappedField::EyeSize | MappedField::PlacementCoefficient => Mapping {
            table: &EASE_LINEAR,
            lo: Q32_SIZE_MIN,
            hi: Q32_SIZE_MAX,
        },
        MappedField::EyeIpd => Mapping {
            table: &EASE_LINEAR,
            lo: Q32_ONE / 4,
            hi: 3 * Q32_ONE / 4,
        },
        MappedField::PupilScale => Mapping {
            table: &EASE_IN_CIRC,
            lo: Q32_PUPIL_MIN,
            hi: Q32_PUPIL_MAX,
        },
        MappedField::EyeballRadiusScale => Mapping {
            table: &EASE_LINEAR,
            lo: Q32_EYEBALL_MIN,
            hi: Q32_ONE,
        },
        MappedField::LashLengthScale => Mapping {
            table: &EASE_EXPO,
            lo: 0,
            hi: Q32_LASH_LENGTH_MAX,
        },
        MappedField::LashStrokeScale => Mapping {
            table: &EASE_LINEAR,
            lo: Q32_LASH_STROKE_MIN,
            hi: Q32_LASH_STROKE_MAX,
        },
        MappedField::BreathPeriod => Mapping {
            table: &EASE_LINEAR,
            lo: 90,
            hi: 240,
        },
        MappedField::BlinkInterval => Mapping {
            table: &EASE_LINEAR,
            lo: 80,
            hi: 400,
        },
        MappedField::MoodReactivity => Mapping {
            table: &EASE_LINEAR,
            lo: Q32_REACTIVITY_MIN,
            hi: Q32_REACTIVITY_MAX,
        },
    }
}

/// Decodes every genome into one complete phenotype.
///
/// Selector reduction and byte mappings make this a total function: unassigned
/// or future-reserved bits remain DNA rather than invalid input, so no genome
/// needs an error path during rendering.
pub fn decode(genome: &Genome) -> Phenotype {
    // Floor of 4: 2- and 3-point bodies render as near-flat slivers that
    // cannot carry a recognizable face (reviewed against sample renders).
    // Mapping change history: 2 + (v % 15) in generator_version 1.
    let point_count = 4 + (genome.read_bits(BODY_START, 4) as u8 % 13);
    let body_size = map(
        MappedField::BodySize,
        byte_at_bits(genome, BODY_START + 4),
        0,
    );
    let body = Body {
        point_count,
        size: Milli(body_size),
        points: std::array::from_fn(|index| {
            if index < usize::from(point_count) {
                let offset = BODY_START + 12 + index * 24;
                BodyPoint {
                    x: Milli(map(
                        MappedField::BodyPointCoordinate,
                        byte_at_bits(genome, offset),
                        body_size,
                    )),
                    y: Milli(map(
                        MappedField::BodyPointCoordinate,
                        byte_at_bits(genome, offset + 8),
                        body_size,
                    )),
                    tension: Q32(map(
                        MappedField::BodyPointTension,
                        byte_at_bits(genome, offset + 16),
                        body_size,
                    )),
                }
            } else {
                BodyPoint {
                    x: Milli(0),
                    y: Milli(0),
                    tension: Q32(0),
                }
            }
        }),
    };

    Phenotype {
        genome_version: GENOME_VERSION,
        body,
        palette: decode_palette(genome),
        face: decode_face(genome),
        pattern: decode_pattern(genome),
        animation: decode_animation(genome),
        mood_response: decode_mood_response(genome),
        asymmetry: Asymmetry {
            deltas: std::array::from_fn(|index| {
                Q32(map(
                    MappedField::Unit,
                    byte_at_bits(genome, ASYMMETRY_START + index * 8),
                    0,
                ))
            }),
        },
        layers: Layers {
            count: byte_at(genome, LAYERS_START, 0),
            params: std::array::from_fn(|index| {
                Q32(map(
                    MappedField::Unit,
                    byte_at(genome, LAYERS_START, index + 1),
                    0,
                ))
            }),
        },
    }
}

fn decode_palette(genome: &Genome) -> Palette {
    let slot = genome.read_bits(PALETTE_START, 11) as usize;
    // Background variants are dormant in v1: preserving their DNA today means
    // a later background treatment can activate them without reshaping genomes.
    let _background_variant = genome.read_bits(PALETTE_START + 11, 3) as u8;
    let colors = PALETTE_BASE[slot];
    let primary = rgb(colors[1]);
    let harmonic1 = rgb(colors[0]);
    let back = rgb(colors[5]);

    Palette {
        back,
        primary,
        primary_lighter: shade(primary, 30),
        primary_darkish: shade(primary, -20),
        primary_darker: shade(primary, -40),
        primary_darkest: shade(primary, -80),
        harmonic1,
        harmonic1_darkish: shade(harmonic1, -15),
        harmonic1_darkest: shade(harmonic1, -30),
        harmonic2: rgb(colors[3]),
        harmonic3: rgb(colors[4]),
        harmonic4: back,
        secondary_darker: shade(back, -80),
        dark: shade(harmonic1, -250),
        white: Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
    }
}

fn decode_face(genome: &Genome) -> Face {
    let flags = byte_at(genome, FACE_START, 5);
    Face {
        eyes: Eyes {
            position_mode: byte_at(genome, FACE_START, 0) % 5,
            size_h: Q32(map(MappedField::EyeSize, byte_at(genome, FACE_START, 1), 0)),
            ipd: Q32(map(MappedField::EyeIpd, byte_at(genome, FACE_START, 2), 0)),
            height_upper: unit_q32(genome, FACE_START, 3),
            height_lower: unit_q32(genome, FACE_START, 4),
            height_bias_gate: named_flag(flags, 0),
            height_bias: unit_q32(genome, FACE_START, 6),
            vertical_pull_gate: named_flag(flags, 1),
            vertical_pull: unit_q32(genome, FACE_START, 7),
            contour_inset1: unit_q32(genome, FACE_START, 8),
            contour_inset2: unit_q32(genome, FACE_START, 9),
            eyeball_radius_scale: Q32(map(
                MappedField::EyeballRadiusScale,
                byte_at(genome, FACE_START, 10),
                0,
            )),
            pupil_scale: Q32(map(
                MappedField::PupilScale,
                byte_at(genome, FACE_START, 11),
                0,
            )),
            iris_shade_opacity: unit_q32(genome, FACE_START, 12),
            iris_shade_reduce: unit_q32(genome, FACE_START, 13),
            iris_saturation_reduce: unit_q32(genome, FACE_START, 14),
            lid_close_upper_neutral: unit_q32(genome, FACE_START, 15),
            lid_close_lower_neutral: unit_q32(genome, FACE_START, 16),
            lid_extension_gate: byte_at(genome, FACE_START, 17),
            lid_extension_side: byte_at(genome, FACE_START, 18),
            noise_freq_h: unit_q32(genome, FACE_START, 19),
            noise_freq_v: unit_q32(genome, FACE_START, 20),
            noise_seed: byte_at(genome, FACE_START, 21),
            rotation_seed: byte_at(genome, FACE_START, 22),
            lid_color_gene: byte_at(genome, FACE_START, 23),
            iris_color_gene: byte_at(genome, FACE_START, 24),
        },
        brows: Brows {
            end_height1: unit_q32(genome, FACE_START, 25),
            mid_x: unit_q32(genome, FACE_START, 26),
            end_height2: unit_q32(genome, FACE_START, 27),
            tensions: std::array::from_fn(|index| unit_q32(genome, FACE_START, index + 28)),
            width_coupling: unit_q32(genome, FACE_START, 31),
            clearance: unit_q32(genome, FACE_START, 32),
        },
        lashes: Lashes {
            count: byte_at(genome, FACE_START, 33) % 12,
            removed: byte_at(genome, FACE_START, 34) % 4,
            length_scale: Q32(map(
                MappedField::LashLengthScale,
                byte_at(genome, FACE_START, 35),
                0,
            )),
            stroke_scale: Q32(map(
                MappedField::LashStrokeScale,
                byte_at(genome, FACE_START, 36),
                0,
            )),
            direction: byte_at(genome, FACE_START, 37),
        },
        mouth: Mouth {
            family: byte_at(genome, FACE_START, 38) % 8,
            width_scale_gate: named_flag(flags, 2),
            width_scale: unit_q32(genome, FACE_START, 39),
            aspect_widen_gate: named_flag(flags, 3),
            params: std::array::from_fn(|index| unit_q32(genome, FACE_START, index + 40)),
        },
        placement: FacePlacement {
            width_coeff: Q32(map(
                MappedField::PlacementCoefficient,
                byte_at(genome, FACE_START, 48),
                0,
            )),
            height_coeff: Q32(map(
                MappedField::PlacementCoefficient,
                byte_at(genome, FACE_START, 49),
                0,
            )),
        },
    }
}

fn decode_pattern(genome: &Genome) -> Pattern {
    Pattern {
        r#class: byte_at(genome, PATTERN_START, 0) % 6,
        circler: Circler {
            point_reduce: byte_at(genome, PATTERN_START, 1),
            size: unit_q32(genome, PATTERN_START, 2),
            y_offset_div: unit_q32(genome, PATTERN_START, 3),
            tension: unit_q32(genome, PATTERN_START, 4),
            pull_min: unit_q32(genome, PATTERN_START, 5),
        },
        voronoi: Voronoi {
            index: byte_at(genome, PATTERN_START, 6),
            rotation: Bam16(genome.read_bits(PATTERN_START + 7 * 8, 16) as u16),
            scale: unit_q32(genome, PATTERN_START, 9),
        },
        cut: Cut {
            spread: unit_q32(genome, PATTERN_START, 10),
            y_adjust_div: unit_q32(genome, PATTERN_START, 11),
        },
    }
}

fn decode_animation(genome: &Genome) -> Animation {
    Animation {
        breath_period: map(
            MappedField::BreathPeriod,
            byte_at(genome, ANIMATION_START, 0),
            0,
        ) as u16,
        breath_amplitude: unit_q32(genome, ANIMATION_START, 1),
        blink_interval: map(
            MappedField::BlinkInterval,
            byte_at(genome, ANIMATION_START, 2),
            0,
        ) as u16,
        blink_jitter_seed: byte_at(genome, ANIMATION_START, 3),
        mood_transition_tempo: unit_q32(genome, ANIMATION_START, 4),
    }
}

fn decode_mood_response(genome: &Genome) -> MoodResponse {
    MoodResponse {
        lid_reactivity: Q32(map(
            MappedField::MoodReactivity,
            byte_at(genome, ANIMATION_START, 5),
            0,
        )),
        brow_reactivity: Q32(map(
            MappedField::MoodReactivity,
            byte_at(genome, ANIMATION_START, 6),
            0,
        )),
        eye_rotation_reactivity: Q32(map(
            MappedField::MoodReactivity,
            byte_at(genome, ANIMATION_START, 7),
            0,
        )),
        mouth_reactivity: Q32(map(
            MappedField::MoodReactivity,
            byte_at(genome, ANIMATION_START, 8),
            0,
        )),
    }
}

fn byte_at(genome: &Genome, region_start: usize, byte_index: usize) -> u8 {
    byte_at_bits(genome, region_start + byte_index * 8)
}

fn byte_at_bits(genome: &Genome, offset: usize) -> u8 {
    genome.read_bits(offset, 8) as u8
}

fn unit_q32(genome: &Genome, region_start: usize, byte_index: usize) -> Q32 {
    Q32(map(
        MappedField::Unit,
        byte_at(genome, region_start, byte_index),
        0,
    ))
}

fn named_flag(flags: u8, bit: u8) -> bool {
    flags & (1 << (7 - bit)) != 0
}

fn map(field: MappedField, byte: u8, body_size: i64) -> i64 {
    let Mapping { table, lo, hi } = mapping(field, body_size);
    map_byte(byte, table, lo, hi)
}

fn map_byte(byte: u8, table: &[u64; 256], lo: i64, hi: i64) -> i64 {
    let delta = i128::from(hi) - i128::from(lo);
    let scaled = div_round_half_away(
        delta * i128::from(table[byte as usize]),
        i128::from(Q32_ONE),
    );
    (i128::from(lo) + scaled) as i64
}

fn rgb(channels: [u8; 3]) -> Rgb {
    Rgb {
        r: channels[0],
        g: channels[1],
        b: channels[2],
    }
}

fn shade(color: Rgb, amount: i16) -> Rgb {
    Rgb {
        r: shade_channel(color.r, amount),
        g: shade_channel(color.g, amount),
        b: shade_channel(color.b, amount),
    }
}

fn shade_channel(channel: u8, amount: i16) -> u8 {
    (i16::from(channel) + amount).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use avatar_phenotype::{from_canonical_json, to_canonical_json};

    #[test]
    fn decoder_is_deterministic() {
        let genome = xof_genome(b"determinism");
        assert_eq!(decode(&genome), decode(&genome));
    }

    #[test]
    fn decoder_accepts_extreme_and_xof_derived_genomes() {
        let mut inputs = vec![
            Genome::from_bytes([0; 1_024]),
            Genome::from_bytes([255; 1_024]),
        ];
        inputs.extend((0..8).map(|index| xof_genome(format!("random-{index}").as_bytes())));

        for genome in inputs {
            let phenotype = decode(&genome);
            assert!((2..=16).contains(&phenotype.body.point_count));
            assert!(phenotype.face.eyes.position_mode < 5);
            assert!(phenotype.pattern.r#class < 6);
        }
    }

    #[test]
    fn body_fields_follow_the_pinned_bit_map_and_mapping_formula() {
        let mut bytes = [0; 1_024];
        set_bits(&mut bytes, BODY_START, 4, 12);
        set_bits(&mut bytes, BODY_START + 4, 8, 0x9a);
        set_bits(&mut bytes, BODY_START + 12, 8, 0x12);
        set_bits(&mut bytes, BODY_START + 20, 8, 0x34);
        set_bits(&mut bytes, BODY_START + 28, 8, 0x56);
        let phenotype = decode(&Genome::from_bytes(bytes));

        let expected_size = independently_map_linear(0x9a, 714_285, 833_333);
        let point = phenotype.body.points[0];
        assert_eq!(phenotype.body.point_count, 16);
        assert_eq!(phenotype.body.size, Milli(expected_size));
        assert_eq!(
            point.x,
            Milli(independently_map_linear(0x12, 0, expected_size))
        );
        assert_eq!(
            point.y,
            Milli(independently_map_linear(0x34, 0, expected_size))
        );
        assert_eq!(
            point.tension,
            Q32(independently_map_linear(0x56, 0, 3 * Q32_ONE / 2))
        );
    }

    #[test]
    fn canonical_json_round_trips_decoded_phenotypes() {
        let phenotype = decode(&xof_genome(b"canonical-json"));
        let json = to_canonical_json(&phenotype);
        assert_eq!(from_canonical_json(&json), Ok(phenotype));
    }

    #[test]
    fn palette_slots_decode_to_pinned_primary_and_background_colors() {
        for (slot, primary, back) in [
            (
                0,
                Rgb {
                    r: 0x00,
                    g: 0x6e,
                    b: 0xd6,
                }, // #006ed6
                Rgb {
                    r: 0x9f,
                    g: 0xff,
                    b: 0xd9,
                }, // #9fffd9
            ),
            (
                1,
                Rgb {
                    r: 0xb2,
                    g: 0x00,
                    b: 0xbe,
                }, // #b200be
                Rgb {
                    r: 0xff,
                    g: 0xe9,
                    b: 0xe9,
                }, // #ffe9e9
            ),
            (
                2,
                Rgb {
                    r: 0xf8,
                    g: 0x01,
                    b: 0x78,
                }, // #f80178
                Rgb {
                    r: 0x78,
                    g: 0x54,
                    b: 0x00,
                }, // #785400
            ),
        ] {
            let mut bytes = [0; 1_024];
            set_bits(&mut bytes, PALETTE_START, 11, slot);
            let palette = decode(&Genome::from_bytes(bytes)).palette;
            assert_eq!(palette.primary, primary, "slot {slot}");
            assert_eq!(palette.back, back, "slot {slot}");
        }
    }

    #[test]
    fn shades_clamp_each_rgb_channel() {
        assert_eq!(
            shade(
                Rgb {
                    r: 200,
                    g: 10,
                    b: 100,
                },
                100
            ),
            Rgb {
                r: 255,
                g: 110,
                b: 200,
            }
        );
        assert_eq!(
            shade(
                Rgb {
                    r: 10,
                    g: 100,
                    b: 255,
                },
                -250
            ),
            Rgb { r: 0, g: 0, b: 5 }
        );
    }

    #[test]
    fn palette_table_hash_is_pinned() {
        let mut hasher = blake3::Hasher::new();
        for slot in PALETTE_BASE {
            for color in slot {
                hasher.update(&color);
            }
        }
        assert_eq!(
            hasher.finalize().to_hex().as_str(),
            "c5f14dae266fc9d875d3d4b6727f2cff228ac6c01eb012fe5813b1e9005198f8"
        );
    }

    fn independently_map_linear(byte: u8, lo: i64, hi: i64) -> i64 {
        let scaled = i128::from(hi - lo) * i128::from(EASE_LINEAR[byte as usize]);
        let rounded = (scaled + i128::from(Q32_ONE) / 2) / i128::from(Q32_ONE);
        lo + rounded as i64
    }

    fn set_bits(bytes: &mut [u8; 1_024], offset: usize, width: usize, value: u64) {
        for index in 0..width {
            let bit = ((value >> (width - index - 1)) & 1) as u8;
            let bit_offset = offset + index;
            let byte = &mut bytes[bit_offset / 8];
            let mask = 1 << (7 - bit_offset % 8);
            *byte = (*byte & !mask) | (bit * mask);
        }
    }

    fn xof_genome(label: &[u8]) -> Genome {
        let mut bytes = [0; 1_024];
        let mut hasher = blake3::Hasher::new_derive_key("avatar-genome-test");
        hasher.update(label);
        hasher.finalize_xof().fill(&mut bytes);
        Genome::from_bytes(bytes)
    }
}
