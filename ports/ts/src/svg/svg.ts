// SVG document assembly for the body pass (transcribed from
// crates/avatar-svg/src/svg.rs).
//
// Element order, attribute order, whitespace, and ids are pinned: the byte
// shape is the compatibility surface, not an accident of serialization.
// Ids use a fixed `av-` prefix with no instance number (TOG's
// `bodylayer.tsx` interpolated a per-instance id), which keeps the output
// byte-stable per phenotype; embedders needing DOM-unique ids rewrite or
// wrap them.

import type { Phenotype, Rgb } from "../phenotype";
import { divRoundHalfAway } from "../tables";

import { hexColor } from "./colors";
import { renderFaceGroup } from "./face";
import { CANVAS_MILLI, CENTER, Q32_ONE, q32Mul } from "./fixed";
import { formatMilli, formatQ32Decimal } from "./format";
import { bodyGeometry, type BodyGeometry } from "./geometry";
import { maskContent } from "./pattern";

/**
 * The v1 viewBox edge in canvas units. Geometry always runs on the
 * 1_000_000 milliunit canvas; this only fixes the serialized viewBox.
 */
export const CANVAS_UNITS = 10_000n;

/**
 * The closed mood vocabulary (design note 001 §5). The full renderer
 * applies its pinned face deltas; the body-only renderer deliberately
 * ignores mood.
 */
export type Mood =
  | "neutral"
  | "working"
  | "happy"
  | "waiting"
  | "blocked"
  | "sad"
  | "sleepy";

/** Render-time inputs that are not part of identity. */
export interface RenderParams {
  mood: Mood;
  /**
   * Fetus→adult morph factor. v1 renders only the adult shape; any other
   * value is rejected.
   */
  growth: bigint;
  /** Output viewBox edge in canvas units. v1 pins 10_000. */
  canvas: bigint;
}

/**
 * Why a render was refused. The renderer fails closed on unsupported
 * inputs rather than silently drawing the wrong shape.
 */
export class RenderError extends Error {
  constructor(
    message: string,
    public readonly kind: "unsupported-growth" | "unsupported-canvas",
    public readonly value: bigint,
  ) {
    super(message);
    this.name = "RenderError";
  }
}

const encoder = new TextEncoder();

/** Renders the complete body and mood-reactive face. */
export function renderSvg(phenotype: Phenotype, params: RenderParams): Uint8Array {
  validate(params);
  const geometry = bodyGeometry(phenotype);
  return encoder.encode(document(phenotype, geometry, params.mood));
}

/** Renders only the background and body for faceless or tiny-size variants. */
export function renderBodySvg(phenotype: Phenotype, params: RenderParams): Uint8Array {
  validate(params);
  const geometry = bodyGeometry(phenotype);
  return encoder.encode(document(phenotype, geometry, null));
}

function validate(params: RenderParams): void {
  if (params.growth !== Q32_ONE) {
    throw new RenderError(
      `growth morph is unimplemented; v1 renders growth = 1 only (got ${params.growth})`,
      "unsupported-growth",
      params.growth,
    );
  }
  if (params.canvas !== CANVAS_UNITS) {
    throw new RenderError(
      `v1 renders the pinned ${CANVAS_UNITS}-unit canvas only (got ${params.canvas})`,
      "unsupported-canvas",
      params.canvas,
    );
  }
}

function document(phenotype: Phenotype, geometry: BodyGeometry, mood: Mood | null): string {
  const palette = phenotype.palette;
  const placement = placementFor(geometry);
  const mask = maskContent(phenotype.pattern, geometry.bodyWidth, geometry.bodyHeight, geometry.top);
  const strokeWidth = strokeWidthFor();

  let svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${CANVAS_UNITS} ${CANVAS_UNITS}">\n`;
  svg += `<rect width="${CANVAS_UNITS}" height="${CANVAS_UNITS}" fill="${hexColor(palette.back)}"/>\n`;
  svg += `<g transform="translate(${formatMilli(placement.tx)},${formatMilli(placement.ty)}) scale(${formatQ32Decimal(placement.scale)}, ${formatQ32Decimal(placement.scale)})">\n`;
  svg += "<defs>\n";
  if (mask !== null) {
    svg += `  <mask id="av-pattern">${mask}</mask>\n`;
  }
  svg +=
    '  <radialGradient id="av-gradox" cx="50%" cy="50%" r="50%" fx="50%" fy="50%" spreadMethod="pad">\n';
  svg += `    <stop offset="89%" stop-color="${hexColor(palette.primary)}"/><stop offset="100%" stop-color="${hexColor(palette.primary_darkish)}"/>\n`;
  svg += "  </radialGradient>\n";
  svg += '  <filter id="av-light">\n';
  svg +=
    '    <feDiffuseLighting in="SourceGraphic" result="light" lighting-color="white">\n';
  svg += `      <fePointLight x="${formatMilli(CENTER)}" y="${formatMilli(lightY(geometry))}" z="${formatMilli(lightZ())}"/>\n`;
  svg += "    </feDiffuseLighting>\n";
  svg +=
    '    <feComposite in="SourceGraphic" in2="light" operator="arithmetic" k1="0.9" k2="1.1" k3="0" k4="0"/>\n';
  svg += "  </filter>\n";
  svg += `  <path id="av-bp" d="${geometry.path}"/>\n`;
  svg += "</defs>\n";
  svg += `<use href="#av-bp" stroke-width="${formatMilli(strokeWidth)}" stroke="${hexColor(palette.dark)}" fill="${hexColor(palette.primary)}"/>\n`;
  svg += '<g filter="url(#av-light)">\n';
  svg += `  <use href="#av-bp" fill="${hexColor(palette.primary)}"/>\n`;
  svg += '  <use href="#av-bp" fill="url(#av-gradox)"/>\n';
  if (mask !== null) {
    svg += `  <use href="#av-bp" fill="${hexColor(palette.harmonic3)}" mask="url(#av-pattern)"/>\n`;
  }
  svg += "</g>\n";
  if (mood !== null) {
    svg += renderFaceGroup(phenotype, geometry, mood);
  }
  svg += "</g>\n</svg>\n";
  return svg;
}

/**
 * Stroke width: canvas/255 in milliunits, one pinned division. Exported
 * (though module-private in svg.rs) so the port's unit tests can pin it.
 */
export function strokeWidthFor(): bigint {
  return divRoundHalfAway(CANVAS_MILLI, 255n);
}

/**
 * Point-light position: x at the canvas center, y at the body's vertical
 * midpoint, z at canvas/21 — all pinned divisions.
 */
function lightY(geometry: BodyGeometry): bigint {
  return geometry.top + divRoundHalfAway(geometry.bodyHeight, 2n);
}

/**
 * Exported (though module-private in svg.rs) so the port's unit tests can
 * pin the division.
 */
export function lightZ(): bigint {
  return divRoundHalfAway(CANVAS_MILLI, 21n);
}

/** The centering transform applied around the body group. */
interface Placement {
  tx: bigint;
  ty: bigint;
  scale: bigint;
}

/**
 * Derives the centering transform.
 *
 * When the body (plus an 8×stroke-width margin for the outline) overflows
 * the canvas, the larger dimension sets the scale and the body is centered;
 * otherwise scale stays 1 and only the vertical centering translate runs.
 * Exported (though module-private in svg.rs) so the port's unit tests can
 * pin both branches.
 */
export function placementFor(geometry: BodyGeometry): Placement {
  const strokeMargin = 8n * strokeWidthFor();
  const limit = CANVAS_MILLI - strokeMargin;
  if (geometry.bodyHeight > limit || geometry.bodyWidth > limit) {
    const dimension =
      geometry.bodyWidth > geometry.bodyHeight ? geometry.bodyWidth : geometry.bodyHeight;
    const scale = divRoundHalfAway(limit * Q32_ONE, dimension);
    const scaledWidth = q32Mul(geometry.bodyWidth, scale);
    const scaledHeight = q32Mul(geometry.bodyHeight, scale);
    return {
      tx: divRoundHalfAway(geometry.bodyWidth - scaledWidth, 2n),
      ty: divRoundHalfAway(CANVAS_MILLI - scaledHeight, 2n) - geometry.top,
      scale,
    };
  }
  return {
    tx: 0n,
    ty: divRoundHalfAway(CANVAS_MILLI - geometry.bodyHeight, 2n) - geometry.top,
    scale: Q32_ONE,
  };
}
