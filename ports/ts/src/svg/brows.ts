// Three-point eyebrow splines derived from face.tsx 337–377 (transcribed
// from crates/avatar-svg/src/brows.rs).

import type { Face, Palette } from "../phenotype";

import { hexColor } from "./colors";
import type { EyePair } from "./eyes";
import type { FaceRect } from "./face";
import {
  bamDegreesQ32,
  CANVAS_MILLI,
  divI64,
  oppositeBam,
  Q32_ONE,
  q32Mul,
} from "./fixed";
import { formatMilli, formatQ32Decimal } from "./format";
import { buildSpline, pathString, type RingPoint } from "./geometry";
import type { MoodDeltas } from "./mood";

export function renderBrows(
  face: Face,
  rect: FaceRect,
  eyes: EyePair,
  mood: MoodDeltas,
  palette: Palette,
): string {
  const leftPoints = neutralPoints(face, rect, eyes);
  const rightPoints = leftPoints.map((point) => ({
    ...point,
    x: CANVAS_MILLI - point.x,
  }));
  const leftPath = pathString(buildSpline(leftPoints));
  const rightPath = pathString(buildSpline(rightPoints));
  const shiftY = q32Mul(rect.height, mood.browShiftY);
  const leftCenter = browCenter(leftPoints);
  const rightCenter = browCenter(rightPoints);
  const leftAngle = formatQ32Decimal(bamDegreesQ32(mood.browRotation));
  const rightAngle = formatQ32Decimal(bamDegreesQ32(oppositeBam(mood.browRotation)));
  return `  <path id="av-brow-left" d="${leftPath}" transform="translate(${formatMilli(0n)},${formatMilli(shiftY)}) rotate(${leftAngle},${formatMilli(leftCenter[0])},${formatMilli(leftCenter[1])})" fill="${hexColor(palette.secondary_darker)}"/>\n  <path id="av-brow-right" d="${rightPath}" transform="translate(${formatMilli(0n)},${formatMilli(shiftY)}) rotate(${rightAngle},${formatMilli(rightCenter[0])},${formatMilli(rightCenter[1])})" fill="${hexColor(palette.secondary_darker)}"/>\n`;
}

function neutralPoints(face: Face, rect: FaceRect, eyes: EyePair): RingPoint[] {
  const eyeWidth = eyes.dimensions.width;
  // face.tsx maps the brow width coefficient to 0.4..1.2 before coupling it
  // to eye width; the phenotype stores the normalized controlling axis.
  const widthFactor = 1_717_986_918n + q32Mul(3_435_973_837n, face.brows.width_coupling);
  const total = q32Mul(eyeWidth, widthFactor);
  const centerX = eyes.left.centerX - divI64(eyeWidth, 10n);
  const peak = divI64(rect.height, 12n);
  const eyeHeight = eyes.dimensions.upper + eyes.dimensions.lower;
  const lowClearance = divI64(eyeHeight, 16n);
  const highClearance = divI64(eyeHeight, 10n);
  const clearance =
    lowClearance + q32Mul(highClearance - lowClearance, face.brows.clearance) + eyes.lashLength;
  const top = eyes.left.outlineTop.cp1y - peak - clearance;
  const firstY = top + q32Mul(peak * 2n, face.brows.end_height1) - peak;
  const lastY = top + q32Mul(peak * 2n, face.brows.end_height2) - peak;
  const midMin = -divI64(total, 5n);
  const midMax = divI64(total, 4n);
  const midOffset = midMin + q32Mul(midMax - midMin, face.brows.mid_x);

  return [
    {
      x: centerX - divI64(total, 2n),
      y: firstY,
      tension: clampQ32(face.brows.tensions[0]),
    },
    {
      x: centerX + midOffset,
      y: top - peak,
      tension: clampQ32(face.brows.tensions[1]),
    },
    {
      x: centerX + divI64(total, 2n),
      y: lastY,
      tension: clampQ32(face.brows.tensions[2]),
    },
  ];
}

function browCenter(points: RingPoint[]): [bigint, bigint] {
  let minX = points[0].x;
  let maxX = points[0].x;
  let minY = points[0].y;
  let maxY = points[0].y;
  for (const point of points) {
    if (point.x < minX) minX = point.x;
    if (point.x > maxX) maxX = point.x;
    if (point.y < minY) minY = point.y;
    if (point.y > maxY) maxY = point.y;
  }
  return [divI64(minX + maxX, 2n), divI64(minY + maxY, 2n)];
}

function clampQ32(value: bigint): bigint {
  if (value < 0n) return 0n;
  if (value > Q32_ONE) return Q32_ONE;
  return value;
}
