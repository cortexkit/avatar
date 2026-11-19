import { expect, test } from "bun:test";

import { decodeRenderTreeJson } from "../src/tree";
import type { Primitive } from "../src/tree";
import { canonicalTreeJsonBytes } from "./render-tree-json-encoder";

// Masks travel in the tree's top-level "masks" table, and a masked effect
// names its entry with "mask_ref". These documents are hand-written in that
// canonical form so each rejection below isolates one rule of the table.

function primitive(label: string, effects: string[] = []): string {
  return `{"clip":null,"effects":[${effects.join(",")}],"label":"${label}","opacity":null,"paint":{"kind":"none"},"role":null,"shape":{"cx":0,"cy":0,"kind":"circle","r":1},"stroke":null,"transform":null}`;
}

function masked(maskRef: string): string {
  return `{"kind":"masked","mask_ref":${maskRef}}`;
}

function document(masks: string[][], primitives: string[]): string {
  const table = masks.map((entry) => `[${entry.join(",")}]`).join(",");
  return `{"background":null,"canvas":240,"masks":[${table}],"primitives":[${primitives.join(",")}]}`;
}

function maskOf(primitive: Primitive): Primitive[] {
  const effect = primitive.effects[0];
  if (effect?.kind !== "masked") throw new Error(`${primitive.label} has no masked effect`);
  return effect.mask;
}

test("resolves every reference to an entry to one shared mask array", () => {
  const source = document(
    [[primitive("inner")], [primitive("outer", [masked("0")])]],
    [primitive("a", [masked("1")]), primitive("b", [masked("1")]), primitive("c", [masked("0")])],
  );
  const tree = decodeRenderTreeJson(source);
  const [a, b, c] = tree.primitives.map(maskOf);
  expect(a).toBe(b!);
  expect(a!.map((item) => item.label)).toEqual(["outer"]);
  expect(c!.map((item) => item.label)).toEqual(["inner"]);
  expect(maskOf(a![0]!)).toBe(c!);
  // The source is canonical, so re-encoding the decoded tree reproduces it.
  expect(new TextDecoder().decode(canonicalTreeJsonBytes(tree))).toBe(source);
});

test("rejects a mask_ref outside the mask table", () => {
  const entry = [primitive("m")];
  for (const outside of ["1", "-1"]) {
    const source = document([entry], [primitive("a", [masked("0")]), primitive("b", [masked(outside)])]);
    expect(() => decodeRenderTreeJson(source)).toThrow(/is outside the mask table of 1 entries/);
  }
  expect(() => decodeRenderTreeJson(document([], [primitive("a", [masked("0")])]))).toThrow(
    /is outside the mask table of 0 entries/,
  );
});

test("rejects a mask entry that refers to itself or to a later entry", () => {
  const itself = document([[primitive("m", [masked("0")])]], [primitive("a", [masked("0")])]);
  expect(() => decodeRenderTreeJson(itself)).toThrow(/must refer to an entry before mask entry 0/);
  const later = document(
    [[primitive("m", [masked("1")])], [primitive("n")]],
    [primitive("a", [masked("0")]), primitive("b", [masked("1")])],
  );
  expect(() => decodeRenderTreeJson(later)).toThrow(/must refer to an entry before mask entry 0/);
});

test("rejects an inline mask member", () => {
  const besideReference = document([[primitive("m")]], [
    primitive("a", ['{"kind":"masked","mask":[],"mask_ref":0}']),
  ]);
  expect(() => decodeRenderTreeJson(besideReference)).toThrow(/unknown field "mask"/);
  const insteadOfReference = document([], [primitive("a", [`{"kind":"masked","mask":[${primitive("m")}]}`])]);
  expect(() => decodeRenderTreeJson(insteadOfReference)).toThrow(/unknown field "mask"/);
});

test("rejects a mask entry that nothing references", () => {
  expect(() => decodeRenderTreeJson(document([[primitive("m")]], [primitive("a")]))).toThrow(
    /masks\[0\]: mask entry is never referenced/,
  );
  // When an unused entry is the only one referring to another, the unused
  // one is reported.
  const unusedChain = document([[primitive("m")], [primitive("n", [masked("0")])]], [primitive("a")]);
  expect(() => decodeRenderTreeJson(unusedChain)).toThrow(/masks\[1\]: mask entry is never referenced/);
});

test("rejects a tree without a mask table", () => {
  expect(() => decodeRenderTreeJson('{"background":null,"canvas":240,"primitives":[]}')).toThrow(
    /missing required field "masks"/,
  );
});

test("rejects a non-integer mask_ref", () => {
  const entry = [primitive("m")];
  for (const maskRef of ['"0"', "null", "true", "[0]"]) {
    expect(() => decodeRenderTreeJson(document([entry], [primitive("a", [masked(maskRef)])]))).toThrow(
      /mask_ref: expected an integer JSON token/,
    );
  }
  // Fraction and exponent spellings never get past the integer-only grammar.
  for (const maskRef of ["0.0", "0.5", "1e0"]) {
    expect(() => decodeRenderTreeJson(document([entry], [primitive("a", [masked(maskRef)])]))).toThrow(
      /must use integer JSON tokens/,
    );
  }
});
