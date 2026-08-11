import { blake3 } from "@noble/hashes/blake3";
import { bytesToHex } from "@noble/hashes/utils";

import type {
  Animation,
  Asymmetry,
  Body,
  BodyPoint,
  Brows,
  Circler,
  Cut,
  Eyes,
  Face,
  FacePlacement,
  Lashes,
  Layers,
  MoodResponse,
  Mouth,
  Palette,
  Pattern,
  Phenotype,
  Rgb,
  Voronoi,
} from "./phenotype";
import { PALETTE_BASE } from "./generated/tables";
import { divRoundHalfAway, EASE_EXPO, EASE_IN_CIRC, EASE_LINEAR } from "./tables";

const GENOME_BYTES = 1024;
const GENOME_BITS = GENOME_BYTES * 8;
const IDENTITY_DOMAIN = "cortexkit-avatar/identity-core/v1";
const encoder = new TextEncoder();

const BODY_START = 0;
const PALETTE_START = 512;
const FACE_START = 576;
const PATTERN_START = 1088;
const ASYMMETRY_START = 1216;
const LAYERS_START = 1280;
const ANIMATION_START = 1536;

const Q32_ONE = 1n << 32n;
const Q32_SIZE_MIN = 3006477107n;
const Q32_SIZE_MAX = 6012954214n;
const Q32_PUPIL_MIN = 644245094n;
const Q32_PUPIL_MAX = 2576980378n;
const Q32_EYEBALL_MIN = 2576980378n;
const Q32_LASH_LENGTH_MAX = 3865470566n;
const Q32_LASH_STROKE_MIN = 85899346n;
const Q32_LASH_STROKE_MAX = 515396076n;
const Q32_REACTIVITY_MIN = Q32_ONE / 2n;
const Q32_REACTIVITY_MAX = Q32_ONE + Q32_ONE / 2n;

export class Genome {
  private constructor(private readonly bytes: Uint8Array) {}

  /** Constructs a genome from its exact 1 KiB wire representation. */
  static fromBytes(bytes: Uint8Array): Genome {
    if (bytes.length !== GENOME_BYTES) {
      throw new RangeError(`genome must contain ${GENOME_BYTES} bytes, received ${bytes.length}`);
    }
    return new Genome(Uint8Array.from(bytes));
  }

  /** Parses one complete lower- or upper-case hexadecimal genome. */
  static fromHex(hex: string): Genome {
    if (hex.length !== GENOME_BYTES * 2) {
      throw new RangeError(`genome hex must contain ${GENOME_BYTES * 2} characters, received ${hex.length}`);
    }
    const bytes = new Uint8Array(GENOME_BYTES);
    for (let index = 0; index < bytes.length; index += 1) {
      const pair = hex.slice(index * 2, index * 2 + 2);
      if (!/^[0-9a-fA-F]{2}$/.test(pair)) {
        throw new SyntaxError(`invalid hexadecimal byte at index ${index * 2}`);
      }
      bytes[index] = Number.parseInt(pair, 16);
    }
    return new Genome(bytes);
  }

  asBytes(): Uint8Array {
    return Uint8Array.from(this.bytes);
  }

  toHex(): string {
    return bytesToHex(this.bytes);
  }

  /** Reads an unsigned, MSB-first field from the 8,192-bit genome. */
  readBits(offset: number, width: number): bigint {
    if (!Number.isInteger(offset) || !Number.isInteger(width) || width < 0 || width > 64) {
      throw new RangeError("genome fields must be whole widths from 0 to 64 bits");
    }
    if (offset < 0 || offset + width > GENOME_BITS) {
      throw new RangeError("genome field must fit within 8,192 bits");
    }

    let value = 0n;
    for (let bitOffset = offset; bitOffset < offset + width; bitOffset += 1) {
      const byte = BigInt(this.bytes[Math.floor(bitOffset / 8)]);
      const bit = (byte >> BigInt(7 - (bitOffset % 8))) & 1n;
      value = (value << 1n) | bit;
    }
    return value;
  }
}

export type SeatIdentity =
  | { kind: "registered"; seatName: string; projectId: string }
  | { kind: "path-fallback"; seatName: string; canonicalPath: string };

function joinedBytes(parts: readonly Uint8Array[]): Uint8Array {
  const length = parts.reduce((total, part) => total + part.length, 0);
  const output = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

/**
 * Derives the initial 1 KiB genome from stable identity data using BLAKE3 XOF.
 * The explicit prefixes and zero separators preserve Rust's domain separation;
 * a path fallback can never collide with registered identity bytes by accident.
 */
export function deriveGenome(identity: SeatIdentity): Genome {
  const zero = Uint8Array.of(0);
  const domain = encoder.encode(IDENTITY_DOMAIN);
  const parts: Uint8Array[] = [domain, zero];
  if (identity.kind === "registered") {
    parts.push(encoder.encode(identity.seatName), zero, encoder.encode(identity.projectId));
  } else {
    parts.push(
      encoder.encode("path-fallback"),
      zero,
      encoder.encode(identity.seatName),
      zero,
      encoder.encode(identity.canonicalPath),
    );
  }
  return Genome.fromBytes(blake3(joinedBytes(parts), { dkLen: GENOME_BYTES }));
}

type Mapping = { table: readonly bigint[]; lo: bigint; hi: bigint };
type MappedField =
  | "unit"
  | "body-size"
  | "body-point-coordinate"
  | "body-point-tension"
  | "eye-size"
  | "eye-ipd"
  | "pupil-scale"
  | "eyeball-radius-scale"
  | "lash-length-scale"
  | "lash-stroke-scale"
  | "placement-coefficient"
  | "breath-period"
  | "blink-interval"
  | "mood-reactivity";

function mapping(field: MappedField, bodySize: bigint): Mapping {
  switch (field) {
    case "unit":
      return { table: EASE_LINEAR, lo: 0n, hi: Q32_ONE };
    case "body-size":
      return { table: EASE_LINEAR, lo: 714285n, hi: 833333n };
    case "body-point-coordinate":
      return { table: EASE_LINEAR, lo: 0n, hi: bodySize };
    case "body-point-tension":
      return { table: EASE_LINEAR, lo: 0n, hi: (3n * Q32_ONE) / 2n };
    case "eye-size":
    case "placement-coefficient":
      return { table: EASE_LINEAR, lo: Q32_SIZE_MIN, hi: Q32_SIZE_MAX };
    case "eye-ipd":
      return { table: EASE_LINEAR, lo: Q32_ONE / 4n, hi: (3n * Q32_ONE) / 4n };
    case "pupil-scale":
      return { table: EASE_IN_CIRC, lo: Q32_PUPIL_MIN, hi: Q32_PUPIL_MAX };
    case "eyeball-radius-scale":
      return { table: EASE_LINEAR, lo: Q32_EYEBALL_MIN, hi: Q32_ONE };
    case "lash-length-scale":
      return { table: EASE_EXPO, lo: 0n, hi: Q32_LASH_LENGTH_MAX };
    case "lash-stroke-scale":
      return { table: EASE_LINEAR, lo: Q32_LASH_STROKE_MIN, hi: Q32_LASH_STROKE_MAX };
    case "breath-period":
      return { table: EASE_LINEAR, lo: 90n, hi: 240n };
    case "blink-interval":
      return { table: EASE_LINEAR, lo: 80n, hi: 400n };
    case "mood-reactivity":
      return { table: EASE_LINEAR, lo: Q32_REACTIVITY_MIN, hi: Q32_REACTIVITY_MAX };
  }
}

function map(field: MappedField, byte: bigint, bodySize = 0n): bigint {
  const { table, lo, hi } = mapping(field, bodySize);
  return lo + divRoundHalfAway((hi - lo) * table[Number(byte)], Q32_ONE);
}

function byteAt(genome: Genome, regionStart: number, byteIndex: number): bigint {
  return genome.readBits(regionStart + byteIndex * 8, 8);
}

function unitQ32(genome: Genome, regionStart: number, byteIndex: number): bigint {
  return map("unit", byteAt(genome, regionStart, byteIndex));
}

function flag(flags: bigint, bit: bigint): boolean {
  return (flags & (1n << (7n - bit))) !== 0n;
}

function rgb(channels: readonly bigint[]): Rgb {
  return { r: channels[0], g: channels[1], b: channels[2] };
}

function clampChannel(channel: bigint, amount: bigint): bigint {
  const result = channel + amount;
  if (result < 0n) return 0n;
  if (result > 255n) return 255n;
  return result;
}

function shade(color: Rgb, amount: bigint): Rgb {
  return {
    r: clampChannel(color.r, amount),
    g: clampChannel(color.g, amount),
    b: clampChannel(color.b, amount),
  };
}

function decodePalette(genome: Genome): Palette {
  const slot = Number(genome.readBits(PALETTE_START, 11));
  // Background variants remain DNA in v1 even though the current palette does not use them.
  genome.readBits(PALETTE_START + 11, 3);
  const colors = PALETTE_BASE[slot];
  const primary = rgb(colors[1]);
  const back = rgb(colors[5]);
  const harmonic1 = rgb(colors[0]);
  return {
    back,
    primary,
    primary_lighter: shade(primary, 30n),
    primary_darkish: shade(primary, -20n),
    primary_darker: shade(primary, -40n),
    primary_darkest: shade(primary, -80n),
    harmonic1,
    harmonic1_darkish: shade(harmonic1, -15n),
    harmonic1_darkest: shade(harmonic1, -30n),
    harmonic2: rgb(colors[3]),
    harmonic3: rgb(colors[4]),
    harmonic4: back,
    secondary_darker: shade(back, -80n),
    dark: shade(harmonic1, -250n),
    white: { r: 255n, g: 255n, b: 255n },
  };
}

function decodeFace(genome: Genome): Face {
  const flags = byteAt(genome, FACE_START, 5);
  const eyes: Eyes = {
    position_mode: byteAt(genome, FACE_START, 0) % 5n,
    size_h: map("eye-size", byteAt(genome, FACE_START, 1)),
    ipd: map("eye-ipd", byteAt(genome, FACE_START, 2)),
    height_upper: unitQ32(genome, FACE_START, 3),
    height_lower: unitQ32(genome, FACE_START, 4),
    height_bias_gate: flag(flags, 0n),
    height_bias: unitQ32(genome, FACE_START, 6),
    vertical_pull_gate: flag(flags, 1n),
    vertical_pull: unitQ32(genome, FACE_START, 7),
    contour_inset1: unitQ32(genome, FACE_START, 8),
    contour_inset2: unitQ32(genome, FACE_START, 9),
    eyeball_radius_scale: map("eyeball-radius-scale", byteAt(genome, FACE_START, 10)),
    pupil_scale: map("pupil-scale", byteAt(genome, FACE_START, 11)),
    iris_shade_opacity: unitQ32(genome, FACE_START, 12),
    iris_shade_reduce: unitQ32(genome, FACE_START, 13),
    iris_saturation_reduce: unitQ32(genome, FACE_START, 14),
    lid_close_upper_neutral: unitQ32(genome, FACE_START, 15),
    lid_close_lower_neutral: unitQ32(genome, FACE_START, 16),
    lid_extension_gate: byteAt(genome, FACE_START, 17),
    lid_extension_side: byteAt(genome, FACE_START, 18),
    noise_freq_h: unitQ32(genome, FACE_START, 19),
    noise_freq_v: unitQ32(genome, FACE_START, 20),
    noise_seed: byteAt(genome, FACE_START, 21),
    rotation_seed: byteAt(genome, FACE_START, 22),
    lid_color_gene: byteAt(genome, FACE_START, 23),
    iris_color_gene: byteAt(genome, FACE_START, 24),
  };
  const brows: Brows = {
    end_height1: unitQ32(genome, FACE_START, 25),
    mid_x: unitQ32(genome, FACE_START, 26),
    end_height2: unitQ32(genome, FACE_START, 27),
    tensions: Array.from({ length: 3 }, (_, index) => unitQ32(genome, FACE_START, index + 28)),
    width_coupling: unitQ32(genome, FACE_START, 31),
    clearance: unitQ32(genome, FACE_START, 32),
  };
  const lashes: Lashes = {
    count: byteAt(genome, FACE_START, 33) % 12n,
    removed: byteAt(genome, FACE_START, 34) % 4n,
    length_scale: map("lash-length-scale", byteAt(genome, FACE_START, 35)),
    stroke_scale: map("lash-stroke-scale", byteAt(genome, FACE_START, 36)),
    direction: byteAt(genome, FACE_START, 37),
  };
  const mouth: Mouth = {
    family: byteAt(genome, FACE_START, 38) % 8n,
    width_scale_gate: flag(flags, 2n),
    width_scale: unitQ32(genome, FACE_START, 39),
    aspect_widen_gate: flag(flags, 3n),
    params: Array.from({ length: 8 }, (_, index) => unitQ32(genome, FACE_START, index + 40)),
  };
  const placement: FacePlacement = {
    width_coeff: map("placement-coefficient", byteAt(genome, FACE_START, 48)),
    height_coeff: map("placement-coefficient", byteAt(genome, FACE_START, 49)),
  };
  return { eyes, brows, lashes, mouth, placement };
}

function decodePattern(genome: Genome): Pattern {
  const circler: Circler = {
    point_reduce: byteAt(genome, PATTERN_START, 1),
    size: unitQ32(genome, PATTERN_START, 2),
    y_offset_div: unitQ32(genome, PATTERN_START, 3),
    tension: unitQ32(genome, PATTERN_START, 4),
    pull_min: unitQ32(genome, PATTERN_START, 5),
  };
  const voronoi: Voronoi = {
    index: byteAt(genome, PATTERN_START, 6),
    rotation: genome.readBits(PATTERN_START + 7 * 8, 16),
    scale: unitQ32(genome, PATTERN_START, 9),
  };
  const cut: Cut = {
    spread: unitQ32(genome, PATTERN_START, 10),
    y_adjust_div: unitQ32(genome, PATTERN_START, 11),
  };
  return { class: byteAt(genome, PATTERN_START, 0) % 6n, circler, voronoi, cut };
}

function decodeAnimation(genome: Genome): Animation {
  return {
    breath_period: map("breath-period", byteAt(genome, ANIMATION_START, 0)),
    breath_amplitude: unitQ32(genome, ANIMATION_START, 1),
    blink_interval: map("blink-interval", byteAt(genome, ANIMATION_START, 2)),
    blink_jitter_seed: byteAt(genome, ANIMATION_START, 3),
    mood_transition_tempo: unitQ32(genome, ANIMATION_START, 4),
  };
}

function decodeMoodResponse(genome: Genome): MoodResponse {
  return {
    lid_reactivity: map("mood-reactivity", byteAt(genome, ANIMATION_START, 5)),
    brow_reactivity: map("mood-reactivity", byteAt(genome, ANIMATION_START, 6)),
    eye_rotation_reactivity: map("mood-reactivity", byteAt(genome, ANIMATION_START, 7)),
    mouth_reactivity: map("mood-reactivity", byteAt(genome, ANIMATION_START, 8)),
  };
}

/**
 * Decodes every possible 1 KiB genome into a complete phenotype. Mapping and
 * selector reduction make reserved and out-of-selector values valid DNA rather
 * than errors, preserving future genome space without renderer error paths.
 */
export function decode(genome: Genome): Phenotype {
  const pointCount = 2n + (genome.readBits(BODY_START, 4) % 15n);
  const bodySize = map("body-size", genome.readBits(BODY_START + 4, 8));
  const points: BodyPoint[] = Array.from({ length: 16 }, (_, index) => {
    if (BigInt(index) >= pointCount) return { x: 0n, y: 0n, tension: 0n };
    const offset = BODY_START + 12 + index * 24;
    return {
      x: map("body-point-coordinate", genome.readBits(offset, 8), bodySize),
      y: map("body-point-coordinate", genome.readBits(offset + 8, 8), bodySize),
      tension: map("body-point-tension", genome.readBits(offset + 16, 8), bodySize),
    };
  });
  const body: Body = { point_count: pointCount, size: bodySize, points };
  const asymmetry: Asymmetry = {
    deltas: Array.from({ length: 8 }, (_, index) => map("unit", byteAt(genome, ASYMMETRY_START, index))),
  };
  const layers: Layers = {
    count: byteAt(genome, LAYERS_START, 0),
    params: Array.from({ length: 16 }, (_, index) => map("unit", byteAt(genome, LAYERS_START, index + 1))),
  };
  return {
    genome_version: 1n,
    body,
    palette: decodePalette(genome),
    face: decodeFace(genome),
    pattern: decodePattern(genome),
    animation: decodeAnimation(genome),
    mood_response: decodeMoodResponse(genome),
    asymmetry,
    layers,
  };
}
