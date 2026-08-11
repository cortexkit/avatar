// Eight dense mouth-shape families transcribed from the legacy TOG
// renderer's sparsely numbered mouth variants (transcribed from
// crates/avatar-svg/src/mouth.rs).

import type { Mouth, Palette } from "../phenotype";

import { hexColor } from "./colors";
import type { MouthRect } from "./face";
import { divI64, Q32_ONE, q32Mul } from "./fixed";
import { formatMilli } from "./format";
import type { MoodDeltas } from "./mood";

export interface MouthShape {
  path: string;
  points: Array<[bigint, bigint]>;
  fill: string;
  stroke: string;
}

interface Metrics {
  rect: MouthRect;
  width: bigint;
  height: bigint;
  centerX: bigint;
  centerY: bigint;
}

export function renderMouth(
  mouth: Mouth,
  rect: MouthRect,
  mood: MoodDeltas,
  palette: Palette,
): string {
  const shape = shapeFor(mouth, rect, mood, palette);
  return `  <g id="av-mouth" data-family="${mouth.family % 8n}"><path d="${shape.path}" fill="${shape.fill}" stroke="${shape.stroke}" stroke-width="${formatMilli(196n)}" stroke-linejoin="round"/></g>\n`;
}

/**
 * Exported (though module-private in mouth.rs) so the port's unit tests can
 * pin the per-family point sets directly.
 */
export function shapeFor(
  mouth: Mouth,
  rect: MouthRect,
  mood: MoodDeltas,
  palette: Palette,
): MouthShape {
  const metrics = newMetrics(rect);
  switch (mouth.family % 8n) {
    case 0n:
      return mouth1(mouth, metrics, mood, palette);
    case 1n:
      return mouth3(mouth, metrics, mood, palette);
    case 2n:
      return mouth4(mouth, metrics, mood, palette);
    case 3n:
      return mouth5(mouth, metrics, mood, palette);
    case 4n:
      return mouth6(mouth, metrics, mood, palette);
    case 5n:
      return mouth7(mouth, metrics, mood, palette);
    case 6n:
      return mouth23(mouth, metrics, mood, palette);
    default:
      return mouth233(mouth, metrics, mood, palette);
  }
}

function newMetrics(rect: MouthRect): Metrics {
  return {
    rect,
    width: max(rect.width(), 1n),
    height: max(rect.height(), 1n),
    centerX: divI64(rect.x1 + rect.x2, 2n),
    centerY: divI64(rect.y1 + rect.y2, 2n),
  };
}

function metricX(m: Metrics, factor: bigint): bigint {
  return m.rect.x1 + q32Mul(m.width, clamp01(factor));
}

function metricY(m: Metrics, factor: bigint): bigint {
  return m.rect.y1 + q32Mul(m.height, clamp01(factor));
}

function metricClamp(m: Metrics, point: [bigint, bigint]): [bigint, bigint] {
  return [clamp(point[0], m.rect.x1, m.rect.x2), clamp(point[1], m.rect.y1, m.rect.y2)];
}

/**
 * TOG Mouth1: params[0] baseline, [1] natural bend, [2]/[3] control x;
 * params[4..7] are reserved. Positive mood curve lowers both controls,
 * making the eye-anchored line read as a smile without moving its
 * endpoints.
 */
function mouth1(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const baseline = metricY(m, 1_431_655_765n + q32Mul(1_431_655_765n, p(mouth, 0)));
  const neutral = q32Mul(divI64(m.height, 3n), p(mouth, 1) - divI64(Q32_ONE, 2n));
  const moodShift = q32Mul(divI64(m.height, 4n), mood.mouthCurve);
  const cpY = clamp(baseline + neutral + moodShift, m.rect.y1, m.rect.y2);
  const points: Array<[bigint, bigint]> = [
    [m.rect.x1, baseline],
    [metricX(m, 858_993_459n + q32Mul(1_431_655_765n, p(mouth, 2))), cpY],
    [metricX(m, 2_435_496_789n + q32Mul(1_001_390_507n, p(mouth, 3))), cpY],
    [m.rect.x2, baseline],
  ];
  return openShape(points, "none", hexColor(palette.dark));
}

/**
 * TOG Mouth3: params[0] vertical placement, [1] DNA curvature, [2]
 * left/right skew, [3] control spread. Mood curve adds to the simple
 * curve's two control heights; params[4..7] remain available for later
 * teeth detail.
 */
function mouth3(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const baseline = metricY(m, 1_288_490_189n + q32Mul(1_717_986_918n, p(mouth, 0)));
  const neutral = q32Mul(divI64(m.height, 3n), p(mouth, 1) - divI64(Q32_ONE, 2n));
  const moodShift = q32Mul(divI64(m.height, 3n), mood.mouthCurve);
  const skew = q32Mul(divI64(m.height, 6n), p(mouth, 2) - divI64(Q32_ONE, 2n));
  const spread = 1_073_741_824n + q32Mul(1_073_741_824n, p(mouth, 3));
  const points: Array<[bigint, bigint]> = [
    [m.rect.x1, baseline],
    [
      metricX(m, spread),
      clamp(baseline + neutral + moodShift + skew, m.rect.y1, m.rect.y2),
    ],
    [
      metricX(m, Q32_ONE - spread),
      clamp(baseline + neutral + moodShift - skew, m.rect.y1, m.rect.y2),
    ],
    [m.rect.x2, baseline],
  ];
  return openShape(points, "none", hexColor(palette.primary_darker));
}

/**
 * TOG Mouth4: params[0]/[1] top and bottom thickness, [2] slant, [3]
 * horizontal control inset; params[4..7] are reserved. Mood curve moves the
 * top controls down and bottom controls up, changing the blade's
 * expression.
 */
function mouth4(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const top =
    m.centerY -
    q32Mul(divI64(m.height, 3n), 1_073_741_824n + q32Mul(1_073_741_824n, p(mouth, 0)));
  const bottom =
    m.centerY +
    q32Mul(divI64(m.height, 3n), 1_073_741_824n + q32Mul(1_073_741_824n, p(mouth, 1)));
  const slant = q32Mul(divI64(m.height, 5n), p(mouth, 2) - divI64(Q32_ONE, 2n));
  const curve = q32Mul(divI64(m.height, 5n), mood.mouthCurve);
  const inset = 644_245_094n + q32Mul(644_245_094n, p(mouth, 3));
  return closedShape(
    [
      metricClamp(m, [m.rect.x1, m.centerY + slant]),
      metricClamp(m, [metricX(m, inset), top + curve]),
      metricClamp(m, [metricX(m, Q32_ONE - inset), top - curve]),
      metricClamp(m, [m.rect.x2, m.centerY - slant]),
      metricClamp(m, [metricX(m, Q32_ONE - inset), bottom - curve]),
      metricClamp(m, [metricX(m, inset), bottom + curve]),
    ],
    hexColor(palette.primary_darker),
    hexColor(palette.dark),
  );
}

/**
 * TOG Mouth5 (teeth): params[0] opening, [1] upper crown, [2] lower crown,
 * [3] corner inset; params[4..7] are reserved for individual teeth.
 * Positive mood curve lowers the lower crown and raises the upper crown
 * into a grin.
 */
function mouth5(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const opening = divI64(m.height, 8n) + q32Mul(divI64(m.height, 4n), p(mouth, 0));
  const upperCrown = q32Mul(divI64(m.height, 6n), p(mouth, 1));
  const lowerCrown = q32Mul(divI64(m.height, 6n), p(mouth, 2));
  const curve = q32Mul(divI64(m.height, 6n), mood.mouthCurve);
  const inset = 429_496_730n + q32Mul(429_496_730n, p(mouth, 3));
  return closedShape(
    [
      [metricX(m, inset), m.centerY],
      metricClamp(m, [metricX(m, 1_431_655_765n), m.centerY - opening - upperCrown - curve]),
      metricClamp(m, [metricX(m, 2_863_311_531n), m.centerY - opening - upperCrown - curve]),
      [metricX(m, Q32_ONE - inset), m.centerY],
      metricClamp(m, [metricX(m, 2_863_311_531n), m.centerY + opening + lowerCrown + curve]),
      metricClamp(m, [metricX(m, 1_431_655_765n), m.centerY + opening + lowerCrown + curve]),
    ],
    hexColor(palette.white),
    hexColor(palette.dark),
  );
}

/**
 * TOG Mouth6: params[0] top bulge, [1] bottom bulge, [2] asymmetry, [3]
 * control inset; params[4..7] are reserved. Mood curve shifts the two top
 * controls oppositely, preserving the component's characteristic skew.
 */
function mouth6(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const top = metricY(m, 1_073_741_824n);
  const bottom = metricY(m, 3_221_225_472n);
  const topBulge = q32Mul(divI64(m.height, 4n), p(mouth, 0));
  const bottomBulge = q32Mul(divI64(m.height, 4n), p(mouth, 1));
  const asymmetry = q32Mul(divI64(m.height, 5n), p(mouth, 2) - divI64(Q32_ONE, 2n));
  const curve = q32Mul(divI64(m.height, 5n), mood.mouthCurve);
  const inset = 858_993_459n + q32Mul(429_496_730n, p(mouth, 3));
  return closedShape(
    [
      [m.rect.x1, m.centerY],
      metricClamp(m, [metricX(m, inset), top - topBulge + asymmetry + curve]),
      metricClamp(m, [metricX(m, Q32_ONE - inset), top - topBulge - asymmetry - curve]),
      [m.rect.x2, m.centerY],
      metricClamp(m, [metricX(m, Q32_ONE - inset), bottom + bottomBulge - asymmetry]),
      metricClamp(m, [metricX(m, inset), bottom + bottomBulge + asymmetry]),
    ],
    hexColor(palette.harmonic1),
    hexColor(palette.dark),
  );
}

/**
 * TOG Mouth7 (open oval): params[0]/[1] top and bottom bulges, [2] side
 * height, [3] horizontal inset; params[4..7] are reserved. Mood curve
 * expands or contracts the bottom controls while all points stay inside
 * the fit rect.
 */
function mouth7(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const top =
    m.centerY - divI64(m.height, 5n) - q32Mul(divI64(m.height, 4n), p(mouth, 0));
  const bottom =
    m.centerY + divI64(m.height, 5n) + q32Mul(divI64(m.height, 4n), p(mouth, 1));
  const sideShift = q32Mul(divI64(m.height, 8n), p(mouth, 2) - divI64(Q32_ONE, 2n));
  const curve = q32Mul(divI64(m.height, 6n), mood.mouthCurve);
  const inset = 429_496_730n + q32Mul(644_245_094n, p(mouth, 3));
  return closedShape(
    [
      [metricX(m, inset), m.centerY + sideShift],
      metricClamp(m, [metricX(m, 1_431_655_765n), top]),
      metricClamp(m, [metricX(m, 2_863_311_531n), top]),
      [metricX(m, Q32_ONE - inset), m.centerY - sideShift],
      metricClamp(m, [metricX(m, 2_863_311_531n), bottom + curve]),
      metricClamp(m, [metricX(m, 1_431_655_765n), bottom + curve]),
    ],
    hexColor(palette.primary_darkest),
    hexColor(palette.dark),
  );
}

/**
 * TOG Mouth23 (wide): params[0] edge inset, [1] opening, [2] center pull,
 * [3] vertical placement; params[4..7] are reserved. Mood curve separates
 * the upper and lower controls equally so the wide corners remain fixed.
 */
function mouth23(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const inset = q32Mul(divI64(Q32_ONE, 10n), p(mouth, 0));
  const center = metricY(m, 1_288_490_189n + q32Mul(1_717_986_918n, p(mouth, 3)));
  const opening = divI64(m.height, 8n) + q32Mul(divI64(m.height, 5n), p(mouth, 1));
  const pull = q32Mul(divI64(m.height, 6n), p(mouth, 2));
  const curve = q32Mul(divI64(m.height, 5n), mood.mouthCurve);
  return closedShape(
    [
      [metricX(m, inset), center],
      metricClamp(m, [metricX(m, 1_073_741_824n), center - opening - pull - curve]),
      metricClamp(m, [metricX(m, 3_221_225_472n), center - opening - pull - curve]),
      [metricX(m, Q32_ONE - inset), center],
      metricClamp(m, [metricX(m, 3_221_225_472n), center + opening + pull + curve]),
      metricClamp(m, [metricX(m, 1_073_741_824n), center + opening + pull + curve]),
    ],
    hexColor(palette.primary_darkest),
    hexColor(palette.primary_darker),
  );
}

/**
 * TOG Mouth233 (lipped): params[0] upper lip, [1] lower lip, [2] cupid
 * bow, [3] corner height; params[4..7] are reserved. Mood curve raises the
 * cupid bow and lowers the bottom center, exaggerating this family's lip
 * profile.
 */
function mouth233(mouth: Mouth, m: Metrics, mood: MoodDeltas, palette: Palette): MouthShape {
  const upper = divI64(m.height, 8n) + q32Mul(divI64(m.height, 6n), p(mouth, 0));
  const lower = divI64(m.height, 8n) + q32Mul(divI64(m.height, 5n), p(mouth, 1));
  const bow = q32Mul(divI64(m.height, 8n), p(mouth, 2));
  const corner = q32Mul(divI64(m.height, 8n), p(mouth, 3) - divI64(Q32_ONE, 2n));
  const curve = q32Mul(divI64(m.height, 6n), mood.mouthCurve);
  const points: Array<[bigint, bigint]> = [
    metricClamp(m, [m.rect.x1, m.centerY + corner]),
    metricClamp(m, [metricX(m, 1_073_741_824n), m.centerY - upper - curve]),
    metricClamp(m, [m.centerX, m.centerY - upper + bow - curve]),
    metricClamp(m, [metricX(m, 3_221_225_472n), m.centerY - upper - curve]),
    metricClamp(m, [m.rect.x2, m.centerY - corner]),
    metricClamp(m, [metricX(m, 3_221_225_472n), m.centerY + lower + curve]),
    metricClamp(m, [m.centerX, m.centerY + lower + bow + curve]),
    metricClamp(m, [metricX(m, 1_073_741_824n), m.centerY + lower + curve]),
  ];
  return multiLipShape(points, hexColor(palette.harmonic3), hexColor(palette.secondary_darker));
}

function openShape(points: Array<[bigint, bigint]>, fill: string, stroke: string): MouthShape {
  const path = `M${formatMilli(points[0][0])} ${formatMilli(points[0][1])} C${formatMilli(points[1][0])} ${formatMilli(points[1][1])},${formatMilli(points[2][0])} ${formatMilli(points[2][1])},${formatMilli(points[3][0])} ${formatMilli(points[3][1])}`;
  return { path, points, fill, stroke };
}

function closedShape(points: Array<[bigint, bigint]>, fill: string, stroke: string): MouthShape {
  const path = `M${formatMilli(points[0][0])} ${formatMilli(points[0][1])} C${formatMilli(points[1][0])} ${formatMilli(points[1][1])},${formatMilli(points[2][0])} ${formatMilli(points[2][1])},${formatMilli(points[3][0])} ${formatMilli(points[3][1])} C${formatMilli(points[4][0])} ${formatMilli(points[4][1])},${formatMilli(points[5][0])} ${formatMilli(points[5][1])},${formatMilli(points[0][0])} ${formatMilli(points[0][1])} Z`;
  return { path, points, fill, stroke };
}

function multiLipShape(points: Array<[bigint, bigint]>, fill: string, stroke: string): MouthShape {
  const path =
    `M${formatMilli(points[0][0])} ${formatMilli(points[0][1])}` +
    ` C${formatMilli(points[1][0])} ${formatMilli(points[1][1])},${formatMilli(points[1][0])} ${formatMilli(points[1][1])},${formatMilli(points[2][0])} ${formatMilli(points[2][1])}` +
    ` C${formatMilli(points[3][0])} ${formatMilli(points[3][1])},${formatMilli(points[3][0])} ${formatMilli(points[3][1])},${formatMilli(points[4][0])} ${formatMilli(points[4][1])}` +
    ` C${formatMilli(points[5][0])} ${formatMilli(points[5][1])},${formatMilli(points[5][0])} ${formatMilli(points[5][1])},${formatMilli(points[6][0])} ${formatMilli(points[6][1])}` +
    ` C${formatMilli(points[7][0])} ${formatMilli(points[7][1])},${formatMilli(points[7][0])} ${formatMilli(points[7][1])},${formatMilli(points[0][0])} ${formatMilli(points[0][1])} Z`;
  return { path, points, fill, stroke };
}

function p(mouth: Mouth, index: number): bigint {
  return clamp01(mouth.params[index]);
}

function clamp01(value: bigint): bigint {
  return clamp(value, 0n, Q32_ONE);
}

function clamp(value: bigint, lo: bigint, hi: bigint): bigint {
  if (value < lo) return lo;
  if (value > hi) return hi;
  return value;
}

function max(value: bigint, floor: bigint): bigint {
  return value > floor ? value : floor;
}
