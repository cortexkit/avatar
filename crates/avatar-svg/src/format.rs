//! Numeric formatting for SVG output.
//!
//! Two pinned formatters cover every number the renderer emits. Coordinates
//! are exact (milliunits divide evenly into two decimal places of a viewBox
//! unit); scale factors carry six fractional digits. Neither path ever sees a
//! float.

use std::fmt::Write as _;

/// Formats an i64 milliunit value as an exact decimal string in viewBox
/// units.
///
/// A milliunit is one hundredth of a viewBox unit (the canvas is 1_000_000
/// milliunits = 10_000 units), so the split `q = |v| / 100`, `r = |v| % 100`
/// is a decomposition, not a rounding: the emitted value equals `v / 100`
/// exactly. The fractional part is emitted as two digits with a trailing zero
/// stripped. `-0` is impossible by construction: the sign is only written for
/// `v < 0`, and any such `v` has a non-zero quotient or remainder.
pub(crate) fn format_milli(value: i64) -> String {
    if value == 0 {
        return "0".to_owned();
    }
    let magnitude = value.unsigned_abs();
    let whole = magnitude / 100;
    let remainder = magnitude % 100;

    let mut text = String::new();
    if value < 0 {
        text.push('-');
    }
    write!(text, "{whole}").expect("writing to a String cannot fail");
    match remainder {
        0 => {}
        r if r % 10 == 0 => write!(text, ".{}", r / 10).expect("writing to a String cannot fail"),
        r => write!(text, ".{r:02}").expect("writing to a String cannot fail"),
    }
    text
}

/// Formats a Q32.32 value as a decimal string with up to six fractional
/// digits.
///
/// Six digits is the pinned precision for scale factors: digits come from
/// repeated ×10 expansion of the fractional bits (truncating after the sixth
/// digit), with trailing zeros stripped. Coordinates never use this formatter
/// — they stay two-digit exact via `format_milli`.
pub(crate) fn format_q32_decimal(value: i64) -> String {
    const FRACTION_MASK: u64 = (1 << 32) - 1;
    let magnitude = value.unsigned_abs();
    let whole = magnitude >> 32;
    let mut fraction = magnitude & FRACTION_MASK;

    let mut digits = [0u8; 6];
    for digit in &mut digits {
        fraction *= 10;
        *digit = (fraction >> 32) as u8;
        fraction &= FRACTION_MASK;
    }
    let significant = digits.len() - digits.iter().rev().take_while(|digit| **digit == 0).count();

    let mut text = String::new();
    if value < 0 && (whole != 0 || significant != 0) {
        text.push('-');
    }
    write!(text, "{whole}").expect("writing to a String cannot fail");
    if significant != 0 {
        text.push('.');
        for digit in &digits[..significant] {
            text.push(char::from(b'0' + digit));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed::Q32_ONE;

    #[test]
    fn milli_formatter_covers_the_pinned_cases() {
        assert_eq!(format_milli(0), "0");
        assert_eq!(format_milli(100), "1");
        assert_eq!(format_milli(150), "1.5");
        assert_eq!(format_milli(105), "1.05");
        assert_eq!(format_milli(-150), "-1.5");
        assert_eq!(format_milli(99), "0.99");
        assert_eq!(format_milli(-99), "-0.99");
        assert_eq!(format_milli(1_000_000), "10000");
    }

    #[test]
    fn milli_formatter_never_emits_negative_zero() {
        for value in -99..=99 {
            assert_ne!(format_milli(value), "-0");
        }
    }

    #[test]
    fn milli_formatter_is_exact_for_every_remainder_class() {
        // 12_345 = 123.45 units: both fractional digits survive.
        assert_eq!(format_milli(12_345), "123.45");
        // 12_340 = 123.40 units: trailing zero stripped.
        assert_eq!(format_milli(12_340), "123.4");
        // 12_300 = 123 units: no fractional part.
        assert_eq!(format_milli(12_300), "123");
        assert_eq!(format_milli(-12_345), "-123.45");
    }

    #[test]
    fn q32_formatter_emits_integer_and_fraction() {
        assert_eq!(format_q32_decimal(0), "0");
        assert_eq!(format_q32_decimal(Q32_ONE), "1");
        assert_eq!(format_q32_decimal(Q32_ONE / 2), "0.5");
        assert_eq!(format_q32_decimal(Q32_ONE / 4), "0.25");
        assert_eq!(format_q32_decimal(-Q32_ONE / 2), "-0.5");
        assert_eq!(format_q32_decimal(2 * Q32_ONE + Q32_ONE / 2), "2.5");
    }

    #[test]
    fn q32_formatter_truncates_to_six_digits() {
        // 1/3 in Q32.32 (floor): 0.333333333... truncates to six digits.
        let third = (1i128 << 32) / 3;
        assert_eq!(format_q32_decimal(third as i64), "0.333333");
    }

    #[test]
    fn q32_formatter_strips_trailing_zeros() {
        // 0.1 in Q32.32 (half-away): 0.100000000... emits one digit.
        let tenth = ((1i128 << 32) + 5) / 10;
        assert_eq!(format_q32_decimal(tenth as i64), "0.1");
    }
}
