// Public entry point of the TS SVG renderer (mirrors the API surface of
// crates/avatar-svg/src/lib.rs): `(Phenotype, RenderParams) -> SVG bytes`.
//
// The renderer is the third layer of the determinism contract (design note
// 001): it consumes the typed phenotype and render-time inputs, never DNA.
// All geometry is integer-only — bigint milliunits on a 1_000_000
// canonical canvas, Q32.32 factors, and divRoundHalfAway as the single
// pinned narrowing primitive — so every port produces byte-identical
// output.

export { renderBodySvg, renderSvg, RenderError, CANVAS_UNITS } from "./svg";
export type { Mood, RenderParams } from "./svg";
export { bodyGeometry } from "./geometry";
export type { BodyGeometry } from "./geometry";
