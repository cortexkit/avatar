//! Pinned numeric lookup tables for deterministic avatar generation.
//!
//! The committed table bytes, rather than a platform's math library, define the
//! trigonometric and easing results shared by every implementation. This keeps
//! generated creatures reproducible across languages and operating systems.

include!("generated/tables.rs");

const QUARTER_BAM16: u16 = 1 << 14;
const QUARTER_SAMPLES: usize = 4096;

/// Divides `num` by positive `den`, rounding halves away from zero.
///
/// Native signed division truncates toward zero, which would make a negative
/// half round differently from the pinned rule used by ports in other
/// languages. All fixed-point narrowing should use this helper instead.
pub fn div_round_half_away(num: i128, den: i128) -> i128 {
    assert!(
        den > 0,
        "div_round_half_away requires a positive denominator"
    );

    let quotient = num / den;
    let remainder = num % den;
    let half_threshold = den / 2 + den % 2;

    if remainder >= half_threshold {
        quotient + 1
    } else if remainder <= -half_threshold {
        quotient - 1
    } else {
        quotient
    }
}

/// Returns a Q1.30 sine value for a BAM16 angle.
///
/// A BAM16 turn contains four 16,384-unit quadrants, while the committed
/// quarter-wave has 4,096 samples. The low two angle bits select the same
/// sample, so `j = (angle within quadrant) >> 2`. Quadrants one and three
/// read `SIN_QUARTER[j]`; quadrants two and four conceptually read
/// `SIN_QUARTER[4096 - j]`. Because index 4096 is the omitted peak, the fold
/// returns `1 << 30` when `j == 0` before applying the quadrant's sign.
pub fn sin_q30(angle: u16) -> i32 {
    let quadrant = angle / QUARTER_BAM16;
    let index = ((angle % QUARTER_BAM16) >> 2) as usize;

    let magnitude = match quadrant {
        0 | 2 => SIN_QUARTER[index],
        1 | 3 if index == 0 => 1 << 30,
        1 | 3 => SIN_QUARTER[QUARTER_SAMPLES - index],
        _ => unreachable!("a u16 BAM16 angle has four quadrants"),
    } as i32;

    if quadrant >= 2 {
        -magnitude
    } else {
        magnitude
    }
}

/// Returns a Q1.30 cosine value for a BAM16 angle.
pub fn cos_q30(angle: u16) -> i32 {
    sin_q30(angle.wrapping_add(QUARTER_BAM16))
}

/// Returns BLAKE3 hashes of the canonical little-endian bytes of every table.
///
/// Explicit byte order makes these pins comparable by implementations on any
/// architecture; the names identify the corresponding committed spec data.
pub fn table_hashes() -> [(&'static str, String); 6] {
    [
        ("SIN_QUARTER", hash_u32_table(&SIN_QUARTER)),
        ("EASE_LINEAR", hash_u64_table(&EASE_LINEAR)),
        ("EASE_EXPO", hash_u64_table(&EASE_EXPO)),
        ("EASE_IN_CIRC", hash_u64_table(&EASE_IN_CIRC)),
        ("EASE_IN_QUINT", hash_u64_table(&EASE_IN_QUINT)),
        ("EASE_IN_OUT_CIRC", hash_u64_table(&EASE_IN_OUT_CIRC)),
    ]
}

fn hash_u32_table(table: &[u32]) -> String {
    let mut hasher = blake3::Hasher::new();
    for value in table {
        hasher.update(&value.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn hash_u64_table(table: &[u64]) -> String {
    let mut hasher = blake3::Hasher::new();
    for value in table {
        hasher.update(&value.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q30_ONE: i32 = 1 << 30;
    const Q32_ONE: u64 = 1 << 32;

    #[test]
    fn div_round_half_away_handles_signed_halves() {
        assert_eq!(div_round_half_away(-3, 2), -2);
        assert_eq!(div_round_half_away(-1, 2), -1);
        assert_eq!(div_round_half_away(3, 2), 2);
        assert_eq!(div_round_half_away(1, 2), 1);
        assert_eq!(div_round_half_away(-4, 2), -2);
        assert_eq!(div_round_half_away(4, 2), 2);
        assert_eq!(div_round_half_away(0, 7), 0);
    }

    #[test]
    fn trigonometric_edges_are_exact() {
        assert_eq!(sin_q30(0), 0);
        assert_eq!(sin_q30(16_384), Q30_ONE);
        assert_eq!(sin_q30(32_768), 0);
        assert_eq!(sin_q30(49_152), -Q30_ONE);
        assert_eq!(cos_q30(0), Q30_ONE);
    }

    #[test]
    fn sine_samples_match_independent_formula() {
        for index in [1, 617, 2_048, 4_095] {
            let angle = (index as f64 / 4_096.0) * std::f64::consts::FRAC_PI_2;
            assert_eq!(SIN_QUARTER[index], q30(angle.sin()));
        }
    }

    #[test]
    fn easing_samples_match_independent_formulas() {
        assert_easing_samples(&EASE_LINEAR, |t| t);
        assert_easing_samples(&EASE_EXPO, |t| {
            if t == 0.0 {
                0.0
            } else {
                2.0_f64.powf(10.0 * t - 10.0)
            }
        });
        assert_easing_samples(&EASE_IN_CIRC, |t| 1.0 - (1.0 - t * t).sqrt());
        assert_easing_samples(&EASE_IN_QUINT, |t| t.powi(5));
        assert_easing_samples(&EASE_IN_OUT_CIRC, |t| {
            if t < 0.5 {
                (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
            } else {
                ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
            }
        });
    }

    #[test]
    fn table_hashes_are_pinned() {
        let expected = [
            (
                "SIN_QUARTER",
                "4bd5cfa26052a9ddc2daffeffcd4ddd35e27cb9b3ce6da53c1d6364b48c30a4a",
            ),
            (
                "EASE_LINEAR",
                "d8e9e650133191b2ced42e6ff826e793dc382e3a6677a7cef8ad920dd09af19c",
            ),
            (
                "EASE_EXPO",
                "428838f70e47b6c28fbb15fbcd94ec6af0ad04ce315eaa8c3876261d6f5784f1",
            ),
            (
                "EASE_IN_CIRC",
                "9a5f6ff31360612ccc3b76fd903e62ac520e8dbf2627f8392e4ea3bf9cb28528",
            ),
            (
                "EASE_IN_QUINT",
                "1d0af9d20330ac7bd15ffc75c18c853af8ef97b832ec05d45285ecefee71aeb5",
            ),
            (
                "EASE_IN_OUT_CIRC",
                "af3d7c927cf3a59ecc867e4512e8aeb2a568c2ad70aabe96337c076656fa317d",
            ),
        ];

        assert_eq!(
            table_hashes(),
            expected.map(|(name, hash)| (name, hash.to_owned()))
        );
    }

    fn assert_easing_samples(table: &[u64; 256], easing: impl Fn(f64) -> f64) {
        assert_eq!(table[0], 0);
        assert_eq!(table[255], Q32_ONE);
        for index in [1, 63, 128, 254] {
            assert_eq!(table[index], q32(easing(index as f64 / 255.0)));
        }
    }

    fn q30(value: f64) -> u32 {
        (value * Q30_ONE as f64 + 0.5).floor() as u32
    }

    fn q32(value: f64) -> u64 {
        (value * Q32_ONE as f64 + 0.5).floor().min(Q32_ONE as f64) as u64
    }
}
