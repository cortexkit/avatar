import { expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { serializeTree } from "../src/tree-serialize";
import { decodeRenderTreeJson } from "../src/tree";
import type { Primitive, RenderTree } from "../src/tree";

const repoRoot = resolve(import.meta.dir, "..", "..", "..");
const treeRoot = resolve(repoRoot, "vectors", "tree");
const treeSvgRoot = resolve(repoRoot, "vectors", "tree-svg");
const treeFiles = (await readdir(treeRoot)).filter((file) => file.endsWith(".json")).sort();
const treeSvgFiles = (await readdir(treeSvgRoot)).filter((file) => file.endsWith(".svg")).sort();
const encoder = new TextEncoder();

test("tree and tree-SVG corpora have the same non-empty names", () => {
  expect(treeFiles.length).toBeGreaterThan(0);
  expect(treeSvgFiles.map((file) => file.replace(/\.svg$/, ".json"))).toEqual(treeFiles);
});

function assertBytesEqual(name: string, actual: Uint8Array, expected: Uint8Array): void {
  const commonLength = Math.min(actual.length, expected.length);
  let offset = 0;
  while (offset < commonLength && actual[offset] === expected[offset]) offset += 1;
  if (offset === actual.length && offset === expected.length) return;
  throw new Error(
    `${name}: generic SVG differs at byte ${offset} (actual ${actual.length} bytes, expected ${expected.length})`,
  );
}

for (const file of treeFiles) {
  test(`projects ${file} to canonical generic SVG bytes`, async () => {
    const treeBytes = new Uint8Array(await readFile(resolve(treeRoot, file)));
    const expected = new Uint8Array(
      await readFile(resolve(treeSvgRoot, file.replace(/\.json$/, ".svg"))),
    );
    const actual = encoder.encode(serializeTree(decodeRenderTreeJson(treeBytes)));
    assertBytesEqual(file, actual, expected);
  });
}

test("derives definition ids from primitive positions", () => {
  const primitive: Primitive = {
    shape: { kind: "circle", cx: 100n, cy: 100n, r: 50n },
    transform: null,
    paint: {
      kind: "radial_gradient",
      stops: [
        { offset_q32: 0n, color: { r: 255, g: 255, b: 255 } },
        { offset_q32: 1n << 32n, color: { r: 0, g: 0, b: 0 } },
      ],
    },
    stroke: null,
    opacity: null,
    clip: null,
    effects: [],
    label: "same-content",
    role: null,
  };
  const tree: RenderTree = {
    canvas: 10,
    background: null,
    primitives: [primitive, primitive],
  };

  const svg = serializeTree(tree);
  expect(svg).toContain('id="gradient-primitive-0"');
  expect(svg).toContain('id="gradient-primitive-1"');
});
