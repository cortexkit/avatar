// Face placement and the bounded mouth-rectangle search (transcribed from
// crates/avatar-svg/src/face.rs).

import type { Face, Phenotype } from "../phenotype";
import { divRoundHalfAway } from "../tables";

import { renderBrows } from "./brows";
import { CENTER, q32Mul, CANVAS_MILLI } from "./fixed";
import type { BodyGeometry } from "./geometry";
import { moodDeltas, scaleMoodDeltas } from "./mood";
import { renderMouth } from "./mouth";
import {
  newEyePair,
  preliminaryDimensions,
  renderDefs,
  renderEye,
  type EyePair,
} from "./eyes";
import type { Mood } from "./svg";

const MOUTH_SEARCH_MAX = 3_000;

/** Axis-aligned area reserved for one mouth renderer. */
export class MouthRect {
  constructor(
    public x1: bigint = 0n,
    public y1: bigint = 0n,
    public x2: bigint = 0n,
    public y2: bigint = 0n,
  ) {}

  width(): bigint {
    return this.x2 - this.x1;
  }

  height(): bigint {
    return this.y2 - this.y1;
  }
}

/** DNA-neutral face rectangle plus the mouth-fit result. */
export interface FaceRect {
  width: bigint;
  height: bigint;
  y: bigint;
  mouth: MouthRect;
}

/** Selects the eye-line landmark exactly as face.tsx 179–195 does. */
export function eyeLineAnchor(positionMode: bigint, body: BodyGeometry): bigint {
  switch (positionMode) {
    case 0n:
      return body.minWidthY;
    case 1n:
      return body.maxWidthY;
    case 2n:
      return body.top + div(body.bodyHeight, 3n);
    case 3n:
      return body.top + div(body.bodyHeight, 2n);
    default:
      return body.maxWidthY;
  }
}

/** Uses one shared face width for preliminary eye sizing and final placement. */
export function faceWidth(bodyWidth: bigint, face: Face): bigint {
  let width = q32Mul(div(bodyWidth, 2n), face.placement.width_coeff);
  const minimum = div(div(CANVAS_MILLI * 2n, 5n), 2n); // canvas/2.5, then /2.
  width = width > minimum ? width : minimum;
  const maximum = div(CANVAS_MILLI * 10n, 11n); // canvas/1.1.
  return width < maximum ? width : maximum;
}

/** Resolves getFaceRect and its always-enabled mouth search (face.tsx 160–335). */
export function resolveFaceRect(body: BodyGeometry, face: Face, eyesHeight: bigint): FaceRect {
  let centerY = eyeLineAnchor(face.eyes.position_mode, body);
  const width = faceWidth(body.bodyWidth, face);
  let height = q32Mul(div(body.bodyHeight, 6n), face.placement.height_coeff);
  const minimumHeight = div(div(CANVAS_MILLI, 5n), 2n);
  height = height > minimumHeight ? height : minimumHeight;

  // The 1.7 rule caps height when the face would otherwise become too tall.
  if (width * 10n < height * 17n) {
    height = div(width * 10n, 17n);
  }
  const cap = eyesHeight > 1n ? eyesHeight : 1n;
  height = height < cap ? height : cap;

  // This odd subtraction is preserved from face.tsx 228–230 rather than
  // replaced by a conventional clamp; short-body placement depends on it.
  if (centerY + height > CANVAS_MILLI) {
    centerY -= CANVAS_MILLI - centerY + height;
  }

  const leftSpace = body.top + body.bodyHeight - centerY;
  const minMouthHeight = max(div(body.bodyHeight, 15n), div(CANVAS_MILLI, 20n));
  // One shared point budget across both searches: the retry from the body
  // top continues the count exactly like face.rs's `&mut counter`.
  const state: SearchState = { counter: 0 };
  let mouth = searchMouthRows(body, centerY, height, leftSpace, minMouthHeight, state);

  // face.tsx 284–318 retries from the body top without resetting either the
  // row count or the shared 3,000-point budget.
  if (mouth.y1 >= body.top + body.bodyHeight) {
    centerY = body.top;
    mouth = searchMouthRows(body, centerY, height, leftSpace, minMouthHeight, state);
  }

  if (mouth.y1 !== 0n) {
    centerY = mouth.y1 - div(height, 2n);
  }

  return {
    width,
    height,
    y: centerY - div(height, 2n),
    mouth,
  };
}

/** Assembles the visible z-order: mouth, left and right eyes with lashes, then brows. */
export function renderFaceGroup(
  phenotype: Phenotype,
  body: BodyGeometry,
  selectedMood: Mood,
): string {
  const scaledMood = scaleMoodDeltas(moodDeltas(selectedMood), phenotype.mood_response);
  const dimensions = preliminaryDimensions(phenotype.face, body.bodyWidth);
  const rect = resolveFaceRect(body, phenotype.face, dimensions.upper + dimensions.lower);
  const eyePair = newEyePair(phenotype.face, rect, dimensions, scaledMood);
  const mouthRect = renderMouthRect(body, rect, eyePair, phenotype.face);

  let output = '<g id="av-face">\n';
  output += renderDefs(eyePair, phenotype.face.eyes);
  output += renderMouth(phenotype.face.mouth, mouthRect, scaledMood, phenotype.palette);
  output += renderEye(eyePair.left, phenotype.face, phenotype.palette);
  output += renderEye(eyePair.right, phenotype.face, phenotype.palette);
  output += renderBrows(phenotype.face, rect, eyePair, scaledMood, phenotype.palette);
  output += "</g>\n";
  return output;
}

function renderMouthRect(
  body: BodyGeometry,
  faceRect: FaceRect,
  eyes: EyePair,
  face: Face,
): MouthRect {
  const eyeDistance = abs(eyes.right.centerX - eyes.left.centerX);
  let halfWidth = max(
    div(min(div(eyeDistance, 2n), div(body.bodyWidth, 7n)), 2n),
    div(faceRect.width, 12n),
  );
  halfWidth = max(halfWidth, 1n);
  if (face.mouth.width_scale_gate) {
    // face.tsx 827–830 widens by 0.8..2.0 when its feature gate is set.
    const factor = 3_435_973_837n + q32Mul(5_153_960_755n, face.mouth.width_scale);
    halfWidth = max(q32Mul(halfWidth, factor), 1n);
  }

  let rect = faceRect.mouth;
  if (rect.x1 === 0n || rect.width() <= 0n || rect.height() <= 0n) {
    const height = max(div(body.bodyHeight, 12n), div(CANVAS_MILLI, 25n));
    rect = new MouthRect(
      CENTER - halfWidth,
      eyes.lowerEdgeY + div(faceRect.height, 4n),
      CENTER + halfWidth,
      eyes.lowerEdgeY + div(faceRect.height, 4n) + height,
    );
  } else {
    rect = new MouthRect(rect.x1, rect.y1, rect.x2, rect.y2);
    rect.x1 = CENTER - halfWidth;
    rect.x2 = CENTER + halfWidth;
  }

  if (face.mouth.aspect_widen_gate && rect.width() * 20n < rect.height() * 23n) {
    const requiredWidth = div(rect.height() * 23n, 20n);
    rect.x1 = CENTER - div(requiredWidth, 2n);
    rect.x2 = CENTER + div(requiredWidth, 2n);
  }

  // face.tsx 858–865 raises the mouth only for exceptionally short bodies.
  const eyeSpan = eyes.right.outlineTop.x3 - eyes.left.outlineTop.x3;
  if (eyeSpan * 10n > rect.width() * 11n && body.bodyHeight < div(CANVAS_MILLI, 20n)) {
    rect.y1 -= div(faceRect.height, 2n);
    rect.y2 -= div(faceRect.height, 2n);
  }
  return rect;
}

interface SearchState {
  counter: number;
}

function searchMouthRows(
  body: BodyGeometry,
  centerY: bigint,
  faceHeight: bigint,
  leftSpace: bigint,
  minMouthHeight: bigint,
  state: SearchState,
): MouthRect {
  let widest = new MouthRect();
  const rows = ceilPositive(leftSpace, minMouthHeight);
  for (let row = 0n; row < rows; row += 1n) {
    const top = centerY + div(faceHeight, 2n) + row * minMouthHeight;
    const candidate = findRect(body.allPoints, body.bodyWidth, top, minMouthHeight, state);
    if (state.counter > MOUTH_SEARCH_MAX) {
      return new MouthRect();
    }
    const area = candidate.width() * candidate.height();
    const threshold = (body.bodyWidth * body.bodyHeight) / 80n;
    if (area > threshold) {
      return candidate;
    }
    if (candidate.width() > widest.width()) {
      widest = candidate;
    }
  }
  return widest;
}

/**
 * Iterative transcription of findRect (face.tsx 91–140).
 *
 * `state.counter` is incremented for every point inspected, including the
 * scan that crosses 3,000. The next loop iteration observes the overrun,
 * matching the reference recursion's entry guard while guaranteeing a
 * finite search.
 */
export function findRect(
  points: ReadonlyArray<[bigint, bigint]>,
  bodyWidth: bigint,
  top: bigint,
  minMouthHeight: bigint,
  state: SearchState,
): MouthRect {
  let active: Array<[bigint, bigint]> = [...points];
  let reductionPercent = 5n;

  for (;;) {
    const inset = div(bodyWidth * reductionPercent, 100n);
    const rect = new MouthRect(
      CENTER - div(bodyWidth, 2n) + inset,
      top,
      CENTER + div(bodyWidth, 2n) - inset,
      top + minMouthHeight,
    );
    if (rect.x1 > rect.x2 || state.counter > MOUTH_SEARCH_MAX) {
      return rect;
    }

    const inside: Array<[bigint, bigint]> = [];
    for (const [x, y] of active) {
      state.counter += 1;
      if (x >= rect.x1 && x <= rect.x2 && y >= rect.y1 && y <= rect.y2) {
        inside.push([x, y]);
      }
    }
    if (inside.length === 0) {
      return rect;
    }
    active = inside;
    reductionPercent += 2n;
  }
}

function ceilPositive(value: bigint, divisor: bigint): bigint {
  if (value <= 0n || divisor <= 0n) {
    return 0n;
  }
  return (value + divisor - 1n) / divisor;
}

function div(value: bigint, divisor: bigint): bigint {
  return divRoundHalfAway(value, divisor);
}

function min(a: bigint, b: bigint): bigint {
  return a < b ? a : b;
}

function max(a: bigint, b: bigint): bigint {
  return a > b ? a : b;
}

function abs(value: bigint): bigint {
  return value < 0n ? -value : value;
}
