import { expect, test } from "bun:test";

import { formatMilli, formatQ32Rounded } from "../src/tree-format";

test("formats milliunits exactly without negative zero", () => {
  for (const [value, expected] of [
    [0n, "0"], [1n, "0.01"], [-1n, "-0.01"],
    [10n, "0.1"], [-10n, "-0.1"], [100n, "1"],
    [12345n, "123.45"], [-12345n, "-123.45"],
  ] as const) {
    expect(formatMilli(value)).toBe(expected);
  }
});

test("rounds Q32 values to nine digits with carries and trimmed zeros", () => {
  for (const [value, expected] of [
    [0n, "0"], [1n, "0"], [-1n, "-0"],
    [3n, "0.000000001"], [-3n, "-0.000000001"],
    [1n << 31n, "0.5"], [1n << 32n, "1"],
    [(1n << 32n) - 1n, "1"], [-((1n << 32n) - 1n), "-1"],
  ] as const) {
    expect(formatQ32Rounded(value)).toBe(expected);
  }
});
