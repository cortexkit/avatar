import { describe, expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { decode, Genome } from "../src/genome";
import { fromCanonicalJson, toCanonicalJson } from "../src/phenotype";

const repoRoot = resolve(import.meta.dir, "..", "..", "..");
const vectorsRoot = resolve(repoRoot, "vectors");
const manifest = JSON.parse(await readFile(resolve(vectorsRoot, "manifest.json"), "utf8")) as {
  vectors: Array<{ name: string }>;
};
const names = manifest.vectors.map((vector) => vector.name);

function hexWindow(bytes: Uint8Array, center: number): string {
  const start = Math.max(0, center - 20);
  const end = Math.min(bytes.length, center + 20);
  return `${start}..${end}: ${Array.from(bytes.slice(start, end), (byte) => byte.toString(16).padStart(2, "0")).join(" ")}`;
}

function assertByteEqual(name: string, actual: Uint8Array, expected: Uint8Array): void {
  const commonLength = Math.min(actual.length, expected.length);
  let offset = 0;
  while (offset < commonLength && actual[offset] === expected[offset]) offset += 1;
  if (offset === actual.length && offset === expected.length) return;
  throw new Error(
    `${name}: canonical JSON differs at byte ${offset}\n` +
      `actual   ${hexWindow(actual, offset)}\n` +
      `expected ${hexWindow(expected, offset)}`,
  );
}

test("manifest lists every DNA and phenotype boundary-1 vector", async () => {
  const listed = [...names].sort();
  const dna = (await readdir(resolve(vectorsRoot, "dna")))
    .filter((file) => file.endsWith(".hex"))
    .map((file) => file.slice(0, -".hex".length))
    .sort();
  const phenotypes = (await readdir(resolve(vectorsRoot, "phenotype")))
    .filter((file) => file.endsWith(".json"))
    .map((file) => file.slice(0, -".json".length))
    .sort();

  expect(new Set(names).size).toBe(names.length);
  expect(dna).toEqual(listed);
  expect(phenotypes).toEqual(listed);
});

describe("DNA to canonical phenotype JSON", () => {
  for (const name of names) {
    test(name, async () => {
      const hex = (await readFile(resolve(vectorsRoot, "dna", `${name}.hex`), "utf8")).trim();
      const expected = new Uint8Array(await readFile(resolve(vectorsRoot, "phenotype", `${name}.json`)));
      const actual = toCanonicalJson(decode(Genome.fromHex(hex)));
      // Byte equality is deliberate: semantic JSON equality would hide wire drift.
      assertByteEqual(name, actual, expected);

      // Parsing the producer bytes and writing them again proves the strict schema
      // accepts the canonical corpus without using JSON number arithmetic.
      assertByteEqual(`${name} parser round trip`, toCanonicalJson(fromCanonicalJson(expected)), expected);
    });
  }
});
