// Numeric formatting for SVG output (transcribed from
// crates/avatar-svg/src/format.rs).
//
// Two pinned formatters cover every number the renderer emits. Coordinates
// are exact (milliunits divide evenly into two decimal places of a viewBox
// unit); scale factors carry six fractional digits. Neither path ever sees a
// float.

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
 * Formats a Q32.32 value as a decimal string with up to six fractional
 * digits.
 *
 * Six digits is the pinned precision for scale factors: digits come from
 * repeated ×10 expansion of the fractional bits (truncating after the sixth
 * digit), with trailing zeros stripped. Coordinates never use this formatter
 * — they stay two-digit exact via `formatMilli`.
 */
export function formatQ32Decimal(value: bigint): string {
  const FRACTION_MASK = (1n << 32n) - 1n;
  const magnitude = value < 0n ? -value : value;
  const whole = magnitude >> 32n;
  let fraction = magnitude & FRACTION_MASK;

  const digits: number[] = [];
  for (let index = 0; index < 6; index += 1) {
    fraction *= 10n;
    digits.push(Number(fraction >> 32n));
    fraction &= FRACTION_MASK;
  }
  let significant = digits.length;
  while (significant > 0 && digits[significant - 1] === 0) {
    significant -= 1;
  }

  let text = "";
  if (value < 0n && (whole !== 0n || significant !== 0)) {
    text = "-";
  }
  text += whole.toString();
  if (significant !== 0) {
    text += ".";
    for (let index = 0; index < significant; index += 1) {
      text += digits[index].toString();
    }
  }
  return text;
}
