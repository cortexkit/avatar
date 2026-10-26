// Numeric formatting for SVG output (transcribed from
// crates/avatar-svg/src/format.rs).
//
// Pinned formatters cover the decimal contracts emitted by the tree serializer.
// Coordinates are exact (milliunits divide evenly into two decimal places of a
// viewBox unit); fixed-point values use rounded nine-digit precision.
// No formatting path converts through a float.

/**
 * Formats a bigint milliunit value as an exact decimal string in viewBox
 * units.
 *
 * A milliunit is one hundredth of a viewBox unit (the canvas is 1_000_000
 * milliunits = 10_000 units), so the split `q = |v| / 100`, `r = |v| % 100`
 * is a decomposition, not a rounding: the emitted value equals `v / 100`
 * exactly. The fractional part is emitted as two digits with a trailing zero
 * stripped. `-0` is impossible by construction: the sign is only written for
 * `v < 0`, and any such `v` has a non-zero quotient or remainder.
 */
export function formatMilli(value: bigint): string {
  if (value === 0n) {
    return "0";
  }
  const magnitude = value < 0n ? -value : value;
  const whole = magnitude / 100n;
  const remainder = Number(magnitude % 100n);

  let text = value < 0n ? "-" : "";
  text += whole.toString();
  if (remainder === 0) {
    return text;
  }
  if (remainder % 10 === 0) {
    return `${text}.${remainder / 10}`;
  }
  return `${text}.${remainder.toString().padStart(2, "0")}`;
}

/**
 * Formats Q32.32 with the generic tree serializer's rounded nine-digit
 * precision.
 */
export function formatQ32Rounded(value: bigint): string {
  const SCALE = 1n << 32n;
  const DIGITS = 1_000_000_000n;
  const magnitude = value < 0n ? -value : value;
  let whole = magnitude / SCALE;
  let fraction = ((magnitude % SCALE) * DIGITS + SCALE / 2n) / SCALE;

  if (fraction === DIGITS) {
    whole += 1n;
    fraction = 0n;
  }

  const sign = value < 0n ? "-" : "";
  if (fraction === 0n) {
    return `${sign}${whole}`;
  }
  return `${sign}${whole}.${fraction.toString().padStart(9, "0").replace(/0+$/, "")}`;
}
