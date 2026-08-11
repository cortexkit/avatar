import { blake3 } from "@noble/hashes/blake3";
import { bytesToHex } from "@noble/hashes/utils";

import {
  EASE_EXPO,
  EASE_IN_CIRC,
  EASE_IN_OUT_CIRC,
  EASE_IN_QUINT,
  EASE_LINEAR,
  PALETTE_BASE,
  SIN_QUARTER,
} from "./generated/tables";

export {
  EASE_EXPO,
  EASE_IN_CIRC,
  EASE_IN_OUT_CIRC,
  EASE_IN_QUINT,
  EASE_LINEAR,
  PALETTE_BASE,
  SIN_QUARTER,
} from "./generated/tables";

const QUARTER_BAM16 = 1n << 14n;
const QUARTER_SAMPLES = 4096n;

/**
 * Divides by a positive denominator using the cross-port half-away-from-zero rule.
 *
 * BigInt division truncates toward zero like Rust, but the explicit remainder test
 * is what gives negative halves their required direction rather than JS rounding.
 */
export function divRoundHalfAway(num: bigint, den: bigint): bigint {
  if (den <= 0n) {
    throw new RangeError("divRoundHalfAway requires a positive denominator");
  }

  const quotient = num / den;
  const remainder = num % den;
  const halfThreshold = den / 2n + (den % 2n);
  if (remainder >= halfThreshold) {
    return quotient + 1n;
  }
  if (remainder <= -halfThreshold) {
    return quotient - 1n;
  }
  return quotient;
}

/** Returns the pinned Q1.30 sine sample for an unsigned BAM16 angle. */
export function sinQ30(angle: bigint): bigint {
  const normalized = angle & 0xffffn;
  const quadrant = normalized / QUARTER_BAM16;
  const index = (normalized % QUARTER_BAM16) >> 2n;
  const magnitude =
    quadrant === 0n || quadrant === 2n
      ? SIN_QUARTER[Number(index)]
      : index === 0n
        ? 1n << 30n
        : SIN_QUARTER[Number(QUARTER_SAMPLES - index)];
  return quadrant >= 2n ? -magnitude : magnitude;
}

/** Returns the pinned Q1.30 cosine sample for an unsigned BAM16 angle. */
export function cosQ30(angle: bigint): bigint {
  return sinQ30(angle + QUARTER_BAM16);
}

function littleEndianTable(values: readonly bigint[], bytesPerValue: bigint): Uint8Array {
  const output = new Uint8Array(values.length * Number(bytesPerValue));
  for (let index = 0; index < values.length; index += 1) {
    for (let byte = 0n; byte < bytesPerValue; byte += 1n) {
      output[index * Number(bytesPerValue) + Number(byte)] = Number(
        (values[index] >> (byte * 8n)) & 0xffn,
      );
    }
  }
  return output;
}

/**
 * Hashes the committed little-endian table bytes, matching avatar-tables/src/lib.rs.
 * Hashing the imported data catches an accidental re-mint or manual transcription edit.
 */
export function tableHashes(): Record<string, string> {
  const hash = (values: readonly bigint[], bytesPerValue: bigint) =>
    bytesToHex(blake3(littleEndianTable(values, bytesPerValue)));
  return {
    SIN_QUARTER: hash(SIN_QUARTER, 4n),
    EASE_LINEAR: hash(EASE_LINEAR, 8n),
    EASE_EXPO: hash(EASE_EXPO, 8n),
    EASE_IN_CIRC: hash(EASE_IN_CIRC, 8n),
    EASE_IN_QUINT: hash(EASE_IN_QUINT, 8n),
    EASE_IN_OUT_CIRC: hash(EASE_IN_OUT_CIRC, 8n),
  };
}

export function paletteHash(): string {
  const bytes = new Uint8Array(PALETTE_BASE.length * 6 * 3);
  let index = 0;
  for (const slot of PALETTE_BASE) {
    for (const color of slot) {
      for (const channel of color) {
        bytes[index] = Number(channel);
        index += 1;
      }
    }
  }
  return bytesToHex(blake3(bytes));
}
