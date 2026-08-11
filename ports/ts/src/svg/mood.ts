// Pinned render-time mood deltas and per-creature reactivity scaling
// (transcribed from crates/avatar-svg/src/mood.rs).

import type { Bam16, MoodResponse, Q32 } from "../phenotype";

import { q32Mul } from "./fixed";
import type { Mood } from "./svg";

/** Geometry deltas applied after the DNA-neutral face has been resolved. */
export interface MoodDeltas {
  upperLidClose: Q32;
  lowerLidClose: Q32;
  eyeRotation: Bam16;
  eyeballShiftX: Q32;
  eyeballShiftY: Q32;
  browRotation: Bam16;
  browShiftY: Q32;
  mouthCurve: Q32;
  lowerContourRaise: Q32;
}

export const MOOD_DELTAS_ZERO: MoodDeltas = {
  upperLidClose: 0n,
  lowerLidClose: 0n,
  eyeRotation: 0n,
  eyeballShiftX: 0n,
  eyeballShiftY: 0n,
  browRotation: 0n,
  browShiftY: 0n,
  mouthCurve: 0n,
  lowerContourRaise: 0n,
};

/**
 * Scales every axis exactly once with its matching phenotype response.
 *
 * The coefficient pairings follow mood.rs: lid closures and the lower
 * contour ride lid reactivity, the eyeball shifts ride eye-rotation
 * reactivity, and brow shift rides brow reactivity.
 */
export function scaleMoodDeltas(deltas: MoodDeltas, response: MoodResponse): MoodDeltas {
  return {
    upperLidClose: q32Mul(deltas.upperLidClose, response.lid_reactivity),
    lowerLidClose: q32Mul(deltas.lowerLidClose, response.lid_reactivity),
    eyeRotation: scaleBam(deltas.eyeRotation, response.eye_rotation_reactivity),
    eyeballShiftX: q32Mul(deltas.eyeballShiftX, response.eye_rotation_reactivity),
    eyeballShiftY: q32Mul(deltas.eyeballShiftY, response.eye_rotation_reactivity),
    browRotation: scaleBam(deltas.browRotation, response.brow_reactivity),
    browShiftY: q32Mul(deltas.browShiftY, response.brow_reactivity),
    mouthCurve: q32Mul(deltas.mouthCurve, response.mouth_reactivity),
    lowerContourRaise: q32Mul(deltas.lowerContourRaise, response.lid_reactivity),
  };
}

/**
 * Returns the fixed mood-delta row for each supported mood.
 *
 * Wrapped BAM16 constants store negative angles in two's-complement form;
 * conversion to SVG degrees interprets them through a signed 16-bit value
 * first (see signedBam16 in fixed.ts).
 */
export function moodDeltas(mood: Mood): MoodDeltas {
  switch (mood) {
    // face.tsx 620–622 and 797–799 make no changes in neutral. Keeping this
    // row literally zero is what makes mood application an identity.
    case "neutral":
      return MOOD_DELTAS_ZERO;
    // New focused-work row: 0.15 upper-lid close and a 0.04 inward pupil
    // shift use the same axes centralized from face.tsx 620–817.
    case "working":
      return {
        ...MOOD_DELTAS_ZERO,
        upperLidClose: 644_245_094n, // 0.15
        eyeballShiftX: 171_798_692n, // 0.04, inward after mirroring
      };
    // face.tsx 661–681: midpoint 6.5° eye rotation = 1_183 BAM16,
    // −6° brow rotation = wrapped 64_444, lid fractions are 1/5 and 1/3,
    // the brow rises, and the lower contour rises by 1/4 of its height.
    case "happy":
      return {
        ...MOOD_DELTAS_ZERO,
        upperLidClose: 858_993_459n, // 0.20
        lowerLidClose: 1_431_655_765n, // 0.333333333
        eyeRotation: 1_183n,
        browRotation: 64_444n,
        browShiftY: -171_798_692n, // -0.04
        mouthCurve: 4_294_967_296n, // 1.0
        lowerContourRaise: 1_073_741_824n, // 0.25
      };
    // New waiting row: −0.10 opens both lids, −0.06 of face height raises
    // the brows, and −0.04 of eye height gives the slight upward look.
    case "waiting":
      return {
        ...MOOD_DELTAS_ZERO,
        upperLidClose: -429_496_730n, // -0.10
        lowerLidClose: -429_496_730n, // -0.10
        eyeballShiftY: -171_798_692n, // -0.04
        browShiftY: -257_698_038n, // -0.06
      };
    // face.tsx 623–633 and 800–803: midpoint 17.5° = 3_186 BAM16,
    // doubled brow rotation 35° = 6_372 BAM16, upper closure 1/1.8 =
    // 0.555555556, and lower closure 1/2.5 = 0.4. The downward eyeball
    // placement is normalized to 0.12 of eye height.
    case "blocked":
      return {
        ...MOOD_DELTAS_ZERO,
        upperLidClose: 2_386_092_942n, // 0.555555556
        lowerLidClose: 1_717_986_918n, // 0.40
        eyeRotation: 3_186n,
        eyeballShiftY: 515_396_076n, // 0.12
        browRotation: 6_372n,
        mouthCurve: -3_006_477_107n, // -0.70
      };
    // face.tsx 635–654 and 804–809: 1/2 upper and 1/8 lower closure,
    // −9° = wrapped 63_898 BAM16, −10° brow = wrapped 63_716, and the
    // radius/8 shifts are normalized to 0.04 inward and 0.06 downward.
    case "sad":
      return {
        ...MOOD_DELTAS_ZERO,
        upperLidClose: 2_147_483_648n, // 0.50
        lowerLidClose: 536_870_912n, // 0.125
        eyeRotation: 63_898n,
        eyeballShiftX: 171_798_692n, // 0.04, inward after mirroring
        eyeballShiftY: 257_698_038n, // 0.06
        browRotation: 63_716n,
        mouthCurve: -3_435_973_837n, // -0.80
      };
    // face.tsx 655–660 and 810–813 close the lower lid by 1/2 and put the
    // upper lid beyond the eye top. The ratio-dependent expression is
    // pinned as 0.85 here; the high eyeball placement becomes −0.12.
    case "sleepy":
      return {
        ...MOOD_DELTAS_ZERO,
        upperLidClose: 3_650_722_202n, // 0.85
        lowerLidClose: 2_147_483_648n, // 0.50
        eyeballShiftY: -515_396_076n, // -0.12
      };
  }
}

function scaleBam(angle: Bam16, reactivity: Q32): Bam16 {
  const signed = angle >= 0x8000n ? angle - 0x10000n : angle;
  const scaled = q32Mul(signed, reactivity);
  // Rust's `(scaled as i16) as u16` keeps the low 16 bits, which bigint
  // bitwise masking reproduces exactly for negative values too.
  return scaled & 0xffffn;
}
