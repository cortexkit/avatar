// Paired eye geometry and SVG assembly (transcribed from
// crates/avatar-svg/src/eyes.rs).

import type { Eyes, Face, Palette, Rgb } from "../phenotype";
import { divRoundHalfAway } from "../tables";

import { hexColor } from "./colors";
import { faceWidth, type FaceRect } from "./face";
import {
  bamDegreesQ32,
  CANVAS_MILLI,
  CENTER,
  divI64,
  oppositeBam,
  Q32_ONE,
  q32Mul,
  ratioQ32,
} from "./fixed";
import { formatMilli, formatQ32Decimal } from "./format";
import { renderLashes, type Cubic, type EyeSide } from "./lashes";
import type { MoodDeltas } from "./mood";

const Q32_ONE_POINT_TWO = 5_153_960_755n;
const Q32_TWO_POINT_EIGHT = 12_025_908_429n;
const Q32_ZERO_POINT_THREE = 1_288_490_189n;

export interface EyeDimensions {
  width: bigint;
  upper: bigint;
  lower: bigint;
}

export interface EyeGeometry {
  outlineTop: Cubic;
  outlineBottom: Cubic;
  upperLashCurve: Cubic;
  centerX: bigint;
  centerY: bigint;
  irisRadius: bigint;
  pupilRadius: bigint;
  upperClose: bigint;
  lowerClose: bigint;
  rotation: bigint;
  side: EyeSide;
}

export interface EyePair {
  left: EyeGeometry;
  right: EyeGeometry;
  dimensions: EyeDimensions;
  lashLength: bigint;
  lowerEdgeY: bigint;
  gap: bigint;
}

/** Resolves the pre-face-rect dimensions used by getFaceRect's eye-height cap. */
export function preliminaryDimensions(face: Face, bodyWidth: bigint): EyeDimensions {
  const available = faceWidth(bodyWidth, face);
  let width = q32Mul(available, face.eyes.size_h) / 3n;
  width = max(width, divI64(available, 5n));
  width = max(width, 1n);
  const upperDivisor = Q32_ONE_POINT_TWO + q32Mul(Q32_TWO_POINT_EIGHT, face.eyes.height_upper);
  const lowerDivisor = Q32_ONE_POINT_TWO + q32Mul(Q32_TWO_POINT_EIGHT, face.eyes.height_lower);
  let upper = divideByQ32(width, max(upperDivisor, 1n));
  let lower = divideByQ32(width, max(lowerDivisor, 1n));
  if (face.eyes.height_bias_gate) {
    const bias = q32Mul(divI64(upper, 3n), face.eyes.height_bias);
    upper += bias;
    lower = max(lower - bias, 1n);
  }
  return {
    width,
    upper: max(upper, 1n),
    lower: max(lower, 1n),
  };
}

/**
 * Computes neutral outlines first, then adds lid closures, shifts the
 * eyeballs, assigns opposed group rotations, and raises the lower contour.
 */
export function newEyePair(
  face: Face,
  rect: FaceRect,
  dimensionsIn: EyeDimensions,
  mood: MoodDeltas,
): EyePair {
  const dimensions = { ...dimensionsIn };
  const totalHeight = dimensions.upper + dimensions.lower;
  if (totalHeight > rect.height) {
    const scale = ratioQ32(rect.height, totalHeight);
    dimensions.width = max(q32Mul(dimensions.width, scale), 1n);
    dimensions.upper = max(q32Mul(dimensions.upper, scale), 1n);
    dimensions.lower = max(q32Mul(dimensions.lower, scale), 1n);
  }

  const eye = face.eyes;
  const lashLength = q32Mul(dimensions.upper + dimensions.lower, face.lashes.length_scale);
  const remaining = max(rect.width - dimensions.width * 2n, 0n);
  const gap = max(q32Mul(remaining, eye.ipd), 0n);
  const faceX = CENTER - divI64(rect.width, 2n);
  const leftStartX = faceX + divI64(rect.width - (dimensions.width * 2n + gap), 2n);
  const pull = eye.vertical_pull_gate
    ? q32Mul(dimensions.upper, q32Mul(eye.vertical_pull, Q32_ZERO_POINT_THREE))
    : 0n;
  const startY = rect.y + dimensions.upper - pull;
  const endY = rect.y + dimensions.upper;
  const topY = rect.y;
  const contourRaise = q32Mul(dimensions.lower, mood.lowerContourRaise);
  const bottomY = rect.y + dimensions.upper + dimensions.lower - contourRaise;
  const inset1 = q32Mul(divI64(dimensions.width, 2n), eye.contour_inset1);
  const inset2 = q32Mul(divI64(dimensions.width, 2n), eye.contour_inset2);

  // face.tsx 569–596 defines the left cubic and obtains the right eye by
  // reflection. Keeping one source curve prevents side-specific drift.
  const leftTop: Cubic = {
    x0: leftStartX,
    y0: startY,
    cp1x: leftStartX + inset1,
    cp1y: topY,
    cp2x: leftStartX + dimensions.width - inset2,
    cp2y: topY,
    x3: leftStartX + dimensions.width,
    y3: endY,
  };
  const leftBottom: Cubic = {
    ...leftTop,
    cp1y: bottomY,
    cp2y: bottomY,
  };

  const upperFactor = clampQ32(eye.lid_close_upper_neutral + mood.upperLidClose);
  const lowerFactor = clampQ32(eye.lid_close_lower_neutral + mood.lowerLidClose);
  const upperClose = q32Mul(dimensions.upper, upperFactor);
  const lowerClose = q32Mul(dimensions.lower, lowerFactor);
  const upperLashCurve: Cubic = {
    ...leftTop,
    cp1y: leftTop.cp1y + upperClose,
    cp2y: leftTop.cp2y + upperClose,
  };

  const neutralCenterX = leftStartX + divI64(dimensions.width, 2n);
  const neutralCenterY = rect.y + divI64(dimensions.upper + dimensions.lower, 2n);
  const shiftX = q32Mul(dimensions.width, mood.eyeballShiftX);
  const shiftY = q32Mul(dimensions.upper + dimensions.lower, mood.eyeballShiftY);
  const irisRadius = max(
    q32Mul(divI64(dimensions.upper + dimensions.lower, 4n), eye.eyeball_radius_scale),
    1n,
  );
  const pupilRadius = clamp(q32Mul(irisRadius, eye.pupil_scale), 1n, irisRadius);

  const left: EyeGeometry = {
    outlineTop: leftTop,
    outlineBottom: leftBottom,
    upperLashCurve,
    centerX: neutralCenterX + shiftX,
    centerY: neutralCenterY + shiftY,
    irisRadius,
    pupilRadius,
    upperClose,
    lowerClose,
    rotation: mood.eyeRotation,
    side: "left",
  };
  const right = mirrorEye(left);

  return {
    left,
    right,
    dimensions,
    lashLength,
    lowerEdgeY: bottomY,
    gap,
  };
}

export function renderDefs(pair: EyePair, eyes: Eyes): string {
  const leftPath = outlinePath(pair.left);
  const rightPath = outlinePath(pair.right);
  // 0.01..0.10 turbulence frequency range from face.tsx, in Q32.
  const freqH = 42_949_673n + q32Mul(386_547_057n, eyes.noise_freq_h);
  const freqV = 42_949_673n + q32Mul(386_547_057n, eyes.noise_freq_v);
  const displacement = 3_000n + q32Mul(47_000n, eyes.iris_shade_reduce);
  return `  <defs>\n    <clipPath id="av-eye-left-clip"><path d="${leftPath}"/></clipPath>\n    <clipPath id="av-eye-right-clip"><path d="${rightPath}"/></clipPath>\n    <filter id="av-eye-noise"><feTurbulence type="fractalNoise" stitchTiles="stitch" baseFrequency="${formatQ32Decimal(freqH)} ${formatQ32Decimal(freqV)}" numOctaves="5" seed="${eyes.noise_seed}" result="warp"/><feDisplacementMap xChannelSelector="R" yChannelSelector="G" scale="${formatMilli(displacement)}" in="SourceGraphic" in2="warp"/></filter>\n  </defs>\n`;
}

export function renderEye(eye: EyeGeometry, genes: Face, palette: Palette): string {
  const sideName = eye.side;
  const [iris, lid] = colors(eye.side, genes.eyes, palette);
  const shadeRadius =
    eye.irisRadius - q32Mul(divI64(eye.irisRadius * 2n, 5n), genes.eyes.iris_shade_reduce);
  const rotation = formatQ32Decimal(bamDegreesQ32(eye.rotation));
  let output = `  <g id="av-eye-${sideName}" transform="rotate(${rotation},${formatMilli(eye.centerX)},${formatMilli(eye.centerY)})">\n`;
  output += `    <path class="av-eyeball" d="${outlinePath(eye)}" fill="${hexColor(palette.white)}" stroke="${hexColor(palette.dark)}" stroke-width="${formatMilli(196n)}"/>\n`;
  output += `    <g clip-path="url(#av-eye-${sideName}-clip)">\n`;
  output += `      <circle class="av-iris" cx="${formatMilli(eye.centerX)}" cy="${formatMilli(eye.centerY)}" r="${formatMilli(eye.irisRadius)}" fill="${hexColor(iris)}" stroke="${hexColor(palette.dark)}" stroke-width="${formatMilli(80n)}"/>\n`;
  output += `      <ellipse class="av-iris-shade" cx="${formatMilli(eye.centerX)}" cy="${formatMilli(eye.centerY)}" rx="${formatMilli(max(shadeRadius, 1n))}" ry="${formatMilli(divI64(max(shadeRadius, 1n) * 5n, 6n))}" fill="${hexColor(palette.primary_darkest)}" opacity="${formatQ32Decimal(genes.eyes.iris_shade_opacity)}" filter="url(#av-eye-noise)"/>\n`;
  output += `      <circle class="av-pupil" cx="${formatMilli(eye.centerX)}" cy="${formatMilli(eye.centerY)}" r="${formatMilli(eye.pupilRadius)}" fill="${hexColor(palette.dark)}"/>\n`;
  output += highlight(eye, -5n, -5n, 12n, palette.white, 3_865_470_566n);
  output += highlight(eye, 6n, 6n, 19n, palette.harmonic1, 1_288_490_189n);
  output += highlight(eye, 5n, 5n, 28n, palette.white, 3_435_973_837n);
  output += "    </g>\n";
  output += `    <path class="av-upper-lid" d="${upperLidPath(eye, genes.eyes.lid_extension_gate, genes.eyes.lid_extension_side)}" fill="${hexColor(lid)}" stroke="${hexColor(palette.dark)}" stroke-width="${formatMilli(196n)}"/>\n`;
  output += `    <path class="av-lower-lid" d="${lowerLidPath(eye, genes.eyes.lid_extension_gate, genes.eyes.lid_extension_side)}" fill="${hexColor(lid)}" stroke="${hexColor(palette.dark)}" stroke-width="${formatMilli(196n)}"/>\n`;
  output += renderLashes(
    genes.lashes,
    eye.upperLashCurve,
    eye.outlineBottom.cp1y - eye.outlineTop.cp1y,
    eye.side,
    palette.dark,
  );
  output += "  </g>\n";
  return output;
}

function mirrorEye(left: EyeGeometry): EyeGeometry {
  return {
    ...left,
    outlineTop: mirrorCurve(left.outlineTop),
    outlineBottom: mirrorCurve(left.outlineBottom),
    upperLashCurve: mirrorCurve(left.upperLashCurve),
    centerX: CANVAS_MILLI - left.centerX,
    rotation: oppositeBam(left.rotation),
    side: "right",
  };
}

function mirrorCurve(curve: Cubic): Cubic {
  return {
    ...curve,
    x0: CANVAS_MILLI - curve.x0,
    cp1x: CANVAS_MILLI - curve.cp1x,
    cp2x: CANVAS_MILLI - curve.cp2x,
    x3: CANVAS_MILLI - curve.x3,
  };
}

function outlinePath(eye: EyeGeometry): string {
  const top = eye.outlineTop;
  const bottom = eye.outlineBottom;
  return `M${formatMilli(top.x0)} ${formatMilli(top.y0)} C${formatMilli(top.cp1x)} ${formatMilli(top.cp1y)},${formatMilli(top.cp2x)} ${formatMilli(top.cp2y)},${formatMilli(top.x3)} ${formatMilli(top.y3)} C${formatMilli(bottom.cp2x)} ${formatMilli(bottom.cp2y)},${formatMilli(bottom.cp1x)} ${formatMilli(bottom.cp1y)},${formatMilli(bottom.x0)} ${formatMilli(bottom.y0)} Z`;
}

function upperLidPath(eye: EyeGeometry, gate: bigint, sideGene: bigint): string {
  const curve = eye.outlineTop;
  const [topExtension] = lidExtensions(curve, gate, sideGene);
  const signedExtension = eye.side === "left" ? -topExtension : topExtension;
  return `M${formatMilli(curve.x0 + signedExtension)} ${formatMilli(curve.y0)} C${formatMilli(curve.cp1x + signedExtension)} ${formatMilli(curve.cp1y)},${formatMilli(curve.cp2x)} ${formatMilli(curve.cp2y)},${formatMilli(curve.x3)} ${formatMilli(curve.y3)} C${formatMilli(curve.cp2x)} ${formatMilli(curve.cp2y + eye.upperClose)},${formatMilli(curve.cp1x)} ${formatMilli(curve.cp1y + eye.upperClose)},${formatMilli(curve.x0)} ${formatMilli(curve.y0)} Z`;
}

function lowerLidPath(eye: EyeGeometry, gate: bigint, sideGene: bigint): string {
  const curve = eye.outlineBottom;
  const [, lowerExtension] = lidExtensions(curve, gate, sideGene);
  const signedExtension = eye.side === "left" ? -lowerExtension : lowerExtension;
  return `M${formatMilli(curve.x0 + signedExtension)} ${formatMilli(curve.y0)} C${formatMilli(curve.cp1x + signedExtension)} ${formatMilli(curve.cp1y)},${formatMilli(curve.cp2x)} ${formatMilli(curve.cp2y)},${formatMilli(curve.x3)} ${formatMilli(curve.y3)} C${formatMilli(curve.cp2x)} ${formatMilli(curve.cp2y - eye.lowerClose)},${formatMilli(curve.cp1x)} ${formatMilli(curve.cp1y - eye.lowerClose)},${formatMilli(curve.x0)} ${formatMilli(curve.y0)} Z`;
}

function lidExtensions(curve: Cubic, gate: bigint, side: bigint): [bigint, bigint] {
  if (gate <= 220n) {
    return [0n, 0n];
  }
  const base = divI64(abs(curve.x3 - curve.x0), 30n);
  const extension = base + divI64(base * 3n * (gate - 220n), 35n);
  if (side > 127n) {
    return [divI64(extension, 3n), extension];
  }
  return [extension, divI64(extension, 3n)];
}

function highlight(
  eye: EyeGeometry,
  xDivisor: bigint,
  yDivisor: bigint,
  radiusDivisor: bigint,
  color: Rgb,
  opacity: bigint,
): string {
  return `      <circle class="av-highlight" cx="${formatMilli(eye.centerX + divI64(eye.irisRadius, xDivisor))}" cy="${formatMilli(eye.centerY + divI64(eye.irisRadius, yDivisor))}" r="${formatMilli(max(divI64(eye.irisRadius, radiusDivisor), 1n))}" fill="${hexColor(color)}" opacity="${formatQ32Decimal(opacity)}"/>\n`;
}

function colors(side: EyeSide, eyes: Eyes, palette: Palette): [Rgb, Rgb] {
  let iris: Rgb;
  const irisGene = eyes.iris_color_gene;
  if (irisGene >= 254n) {
    iris = side === "left" ? palette.harmonic3 : palette.harmonic4;
  } else if (irisGene >= 241n) {
    iris = palette.harmonic4;
  } else if (irisGene >= 236n) {
    iris = palette.harmonic3;
  } else if (irisGene >= 211n) {
    iris = palette.harmonic1;
  } else if (irisGene <= 1n) {
    iris = side === "left" ? palette.harmonic2 : palette.harmonic3;
  } else {
    iris = palette.harmonic2;
  }

  let lid: Rgb;
  const lidGene = eyes.lid_color_gene;
  if (lidGene >= 254n) {
    lid = side === "left" ? palette.harmonic1 : palette.harmonic4;
  } else if (lidGene >= 241n) {
    lid = side === "left" ? palette.primary : palette.harmonic1;
  } else if (lidGene >= 231n) {
    lid = side === "left" ? palette.harmonic1 : palette.primary;
  } else if (lidGene >= 211n) {
    lid = side === "left" ? palette.primary_darkest : palette.primary;
  } else if (lidGene <= 39n) {
    lid = palette.primary_darker;
  } else {
    lid = palette.primary;
  }
  return [reduceSaturation(iris, eyes.iris_saturation_reduce), lid];
}

function reduceSaturation(color: Rgb, amount: bigint): Rgb {
  const gray = divI64(color.r + color.g + color.b, 3n);
  const clampedAmount = clamp(amount, 0n, Q32_ONE);
  const channel = (value: bigint): bigint =>
    clamp(value + q32Mul(gray - value, clampedAmount), 0n, 255n);
  return {
    r: channel(color.r),
    g: channel(color.g),
    b: channel(color.b),
  };
}

function divideByQ32(value: bigint, divisor: bigint): bigint {
  return divRoundHalfAway(value * Q32_ONE, divisor);
}

function clampQ32(value: bigint): bigint {
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

function abs(value: bigint): bigint {
  return value < 0n ? -value : value;
}
