import { describe, expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { decode, Genome } from "../src/genome";
import { fromCanonicalJson, toCanonicalJson } from "../src/phenotype";
import { renderSvg, type Mood } from "../src/svg";

const repoRoot = resolve(import.meta.dir, "..", "..", "..");
const vectorsRoot = resolve(repoRoot, "vectors");
const manifest = JSON.parse(await readFile(resolve(vectorsRoot, "manifest.json"), "utf8")) as {
  vectors: Array<{ name: string; moods: string[] }>;
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

/** 60-byte window around the first mismatch, for the SVG byte comparison. */
function svgWindow(bytes: Uint8Array, center: number): string {
  const start = Math.max(0, center - 30);
  const end = Math.min(bytes.length, center + 30);
  const hex = Array.from(bytes.slice(start, end), (byte) => byte.toString(16).padStart(2, "0")).join(" ");
  const text = new TextDecoder().decode(bytes.slice(start, end)).replace(/[^\x20-\x7e]/g, ".");
  return `${start}..${end}\n  hex : ${hex}\n  text: ${text}`;
}

function assertSvgByteEqual(name: string, actual: Uint8Array, expected: Uint8Array): void {
  const commonLength = Math.min(actual.length, expected.length);
  let offset = 0;
  while (offset < commonLength && actual[offset] === expected[offset]) offset += 1;
  if (offset === actual.length && offset === expected.length) return;
  throw new Error(
    `${name}: SVG differs at byte ${offset} (actual ${actual.length} bytes, expected ${expected.length})\n` +
      `actual   ${svgWindow(actual, offset)}\n` +
      `expected ${svgWindow(expected, offset)}`,
  );
}

test("manifest lists every DNA, phenotype, and SVG boundary vector", async () => {
  const listed = [...names].sort();
  const dna = (await readdir(resolve(vectorsRoot, "dna")))
    .filter((file) => file.endsWith(".hex"))
    .map((file) => file.slice(0, -".hex".length))
    .sort();
  const phenotypes = (await readdir(resolve(vectorsRoot, "phenotype")))
    .filter((file) => file.endsWith(".json"))
    .map((file) => file.slice(0, -".json".length))
    .sort();
  const svgs = (await readdir(resolve(vectorsRoot, "svg")))
    .filter((file) => file.endsWith(".svg"))
    .sort();
  const expectedSvgs = manifest.vectors
    .flatMap((vector) => vector.moods.map((mood) => `${vector.name}__${mood}.svg`))
    .sort();

  expect(new Set(names).size).toBe(names.length);
  expect(dna).toEqual(listed);
  expect(phenotypes).toEqual(listed);
  expect(svgs).toEqual(expectedSvgs);
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

describe("DNA to SVG (boundary 2)", () => {
  // The full DNA→SVG parity loop: decode, then render with the TS renderer
  // and compare byte-for-byte against the corpus minted by the Rust
  // renderer (crates/avatar-vectors). Byte equality is the contract
  // (design note 001): any arithmetic or serialization drift fails here.
  for (const vector of manifest.vectors) {
    for (const mood of vector.moods) {
      test(`${vector.name}__${mood}`, async () => {
        const hex = (await readFile(resolve(vectorsRoot, "dna", `${vector.name}.hex`), "utf8")).trim();
        const expected = new Uint8Array(
          await readFile(resolve(vectorsRoot, "svg", `${vector.name}__${mood}.svg`)),
        );
        const phenotype = decode(Genome.fromHex(hex));
        const actual = renderSvg(phenotype, {
          mood: mood as Mood,
          growth: 1n << 32n,
          canvas: 10_000n,
        });
        assertSvgByteEqual(`${vector.name}__${mood}`, actual, expected);
      });
    }
  }
});
