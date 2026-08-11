import { describe, expect, test } from "bun:test";

import { deriveGenome } from "../src/genome";
import { divRoundHalfAway, paletteHash, tableHashes } from "../src/tables";

describe("divRoundHalfAway", () => {
  // These are the Rust falsifying vectors from crates/avatar-tables/src/lib.rs.
  test.each([
    [-3n, 2n, -2n],
    [-1n, 2n, -1n],
    [3n, 2n, 2n],
    [1n, 2n, 1n],
    [-4n, 2n, -2n],
    [4n, 2n, 2n],
    [0n, 7n, 0n],
  ])("rounds %s / %s to %s", (num, den, expected) => {
    expect(divRoundHalfAway(num, den)).toBe(expected);
  });
});

test("imported table hashes equal the Rust pins", () => {
  // Pin source: crates/avatar-tables/src/lib.rs, table_hashes_are_pinned.
  expect(tableHashes()).toEqual({
    SIN_QUARTER: "4bd5cfa26052a9ddc2daffeffcd4ddd35e27cb9b3ce6da53c1d6364b48c30a4a",
    EASE_LINEAR: "d8e9e650133191b2ced42e6ff826e793dc382e3a6677a7cef8ad920dd09af19c",
    EASE_EXPO: "428838f70e47b6c28fbb15fbcd94ec6af0ad04ce315eaa8c3876261d6f5784f1",
    EASE_IN_CIRC: "9a5f6ff31360612ccc3b76fd903e62ac520e8dbf2627f8392e4ea3bf9cb28528",
    EASE_IN_QUINT: "1d0af9d20330ac7bd15ffc75c18c853af8ef97b832ec05d45285ecefee71aeb5",
    EASE_IN_OUT_CIRC: "af3d7c927cf3a59ecc867e4512e8aeb2a568c2ad70aabe96337c076656fa317d",
  });
  // Pin source: crates/avatar-genome/src/decode.rs, palette_table_hash_is_pinned.
  expect(paletteHash()).toBe("c5f14dae266fc9d875d3d4b6727f2cff228ac6c01eb012fe5813b1e9005198f8");
});

test("identity prefixes are compatibility pins", () => {
  // Vectors copied verbatim from crates/avatar-genome/src/identity.rs.
  expect(
    deriveGenome("project-42").toHex().slice(0, 64),
  ).toBe("497c376676efcfbabd57a41123143dbb72df26ad5f2b09c6d7331410ea13c289");
  expect(
    deriveGenome({ kind: "path-fallback", canonicalPath: "project-42" }).toHex().slice(0, 64),
  ).toBe("70e579fc8f565c4563aeccb739a43eb9867e2264e6cb2a658adc8621edb63cd3");
  expect(
    deriveGenome("").toHex().slice(0, 64),
  ).toBe("ac844c9c9582832925ef89a4b581d9f31d23d5b69702fd408f5e1a2167d5bfcb");
});
