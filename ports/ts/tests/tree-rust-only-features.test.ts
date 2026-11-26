import { expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { decodeRenderTreeJson } from "../src/tree";

// The render-tree features in vectors/tree-features/ (compound paths, focal
// radial gradients, stop opacity and the pixel grid) are drawn only by the
// Rust rasterizer: apps receive those creatures as finished images. This
// decoder deliberately does not learn them, so it must keep failing closed on
// every such tree rather than drawing it without the feature.
const repoRoot = resolve(import.meta.dir, "..", "..", "..");
const featureRoot = resolve(repoRoot, "vectors", "tree-features");

// Each fixture with the exact refusal the decoder gives today. The message
// names both where decoding stops and why, so a fixture that failed for an
// unrelated reason, such as a typo in an ordinary field, would not pass.
const expectedRefusals: Record<string, string> = {
  "compound-path.json":
    'render tree.primitives[0].shape.kind: unknown shape tag "compound_path"',
  "compound-path-clip.json":
    'render tree.masks[0][0].shape.kind: unknown shape tag "compound_path"',
  "compound-path-mixed.json":
    'render tree.primitives[0].shape.kind: unknown shape tag "compound_path"',
  "pixel-grid.json": 'render tree: unknown field "pixel_grid"',
  "radial-gradient-at.json":
    'render tree.primitives[0].paint.kind: unknown paint tag "radial_gradient_at"',
  "radial-gradient-in-box.json":
    'render tree.primitives[0].paint.kind: unknown paint tag "radial_gradient_at"',
  "stop-opacity.json": 'render tree.primitives[0].paint.stops[0]: unknown field "opacity_q32"',
};

const fixtureFiles = (await readdir(featureRoot)).filter((file) => file.endsWith(".json")).sort();

test("every tree-features fixture has an expected refusal", () => {
  // A fixture added without an entry here would otherwise never be checked.
  expect(fixtureFiles).toEqual(Object.keys(expectedRefusals).sort());
});

function refusal(bytes: Uint8Array): string {
  try {
    decodeRenderTreeJson(bytes);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  throw new Error("the decoder accepted a tree that uses a Rust-only feature");
}

for (const [file, reason] of Object.entries(expectedRefusals)) {
  test(`refuses ${file}`, async () => {
    const bytes = new Uint8Array(await readFile(resolve(featureRoot, file)));
    expect(refusal(bytes)).toBe(`invalid render-tree JSON: ${reason}`);
  });
}
