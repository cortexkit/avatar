import { expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { decodeRenderTreeJson } from "../src/tree";
import { canonicalTreeJsonBytes } from "./render-tree-json-encoder";

const repoRoot = resolve(import.meta.dir, "..", "..", "..");
const treeRoot = resolve(repoRoot, "vectors", "tree");
const treeFiles = (await readdir(treeRoot)).filter((file) => file.endsWith(".json")).sort();

function assertBytesEqual(name: string, actual: Uint8Array, expected: Uint8Array): void {
  const commonLength = Math.min(actual.length, expected.length);
  let offset = 0;
  while (offset < commonLength && actual[offset] === expected[offset]) offset += 1;
  if (offset === actual.length && offset === expected.length) return;
  throw new Error(
    `${name}: canonical JSON differs at byte ${offset} (actual ${actual.length} bytes, expected ${expected.length})`,
  );
}

for (const file of treeFiles) {
  test(`decodes and preserves ${file}`, async () => {
    const bytes = new Uint8Array(await readFile(resolve(treeRoot, file)));
    const actual = canonicalTreeJsonBytes(decodeRenderTreeJson(bytes));

    // Byte equality proves the decoder preserved every emitted field; it does
    // not establish that the fixture's source geometry is itself correct.
    assertBytesEqual(file, actual, bytes);
  });
}

test("rejects malformed render-tree JSON instead of defaulting fields", () => {
  const primitive =
    '{"clip":null,"effects":[],"label":"x","opacity":null,"paint":{"kind":"none"},"shape":{"cx":0,"cy":0,"kind":"circle","r":1},"stroke":null,"transform":null}';
  const invalidDocuments = [
    '{"canvas":1,"primitives":[]}',
    '{"background":null,"canvas":1,"extra":0,"primitives":[]}',
    '{"background":null,"canvas":1.0,"primitives":[]}',
    '{"background":null,"canvas":4294967296,"primitives":[]}',
    `{"background":null,"canvas":1,"primitives":[${primitive.replace('"kind":"none"', '"kind":"unknown"')}]}`,
    `{"background":null,"canvas":1,"primitives":[${primitive.replace('"r":1', '"r":9223372036854775808')}]}`,
  ];

  for (const document of invalidDocuments) {
    expect(() => decodeRenderTreeJson(document)).toThrow();
  }
});
