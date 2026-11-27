#!/usr/bin/env node
// Fidelity shim for creature.drawn-heads: runs drawn-heads/heads.js from the
// reference folder. See README.md in this directory.
"use strict";

const lib = require("./lib");

// Tables heads.js keeps to itself, as [name, weight]. Its picks map each bit
// pattern to an entry in proportion to the weights, so every entry with a
// weight above zero is reachable.
const EARS = ["round", "jug", "pointed", "lobed", "small", "out", "square"];
const BROWS = ["taper", "line", "block", "arch", "bushy", "short", "wedge"];
const NOSES = ["hook", "u", "dots", "c", "angle", "bulb", "wide", "long", "dash", "potato", "v", "snub", "button", "bracket"];
const MOUTHS = ["line", "smirk", "lips", "pout", "wavy", "teeth", "wide"];
// No moustache is the empty string.
const TASH = ["", "chevron", "handlebar", "walrus", "pencil"];
// The hair colour's name: the body colour itself, a dark or a pale one, or a
// hue turned from the body's with one of four tones.
const HAIR_COLOURS = [
  "body colour",
  "black", "espresso", "charcoal", "slate",
  "white", "silver", "ash", "grey",
  ...["same hue", "next hue", "opposite hue", "far hue"].flatMap(n => [n + ", deep", n, n + ", bright", n + ", soft"]),
];

const named = (table, weight) => table.filter(k => weight(k) > 0).map(k => (Array.isArray(k) ? k[0] : k.name));

const spec = {
  stem: "drawn-heads",
  script: "drawn-heads/heads.js",
  global: "DrawnHeads",
  variety(refs) {
    return lib.sheetVariety(lib.loadSheet(refs, "drawn-heads/sheet.js", "Sheet"), "drawn-heads/sheet.js");
  },
  build: (api, bytes) => api.creature(bytes),
  // The size sets line widths below about 160 px, the detail kept below 96
  // and 56 px, and whether steam is drawn below 48 px.
  draw: (api, C, mood, size) => C.svg(mood, { size }),
  // What summaryOf reports, field by field, and the palette recipe. The hair
  // colour is named only where there is hair, as summaryOf names it.
  choices(api, C) {
    const K = C.marks, out = {
      head: C.head.kind,
      hair: C.hair.style,
      ears: C.ears.kind,
      eyes: C.E.kind,
      brows: C.B.kind,
      nose: C.N.kind,
      mouth: C.M.kind,
      beard: C.beard.kind,
      "beard.tash": C.beard.tash,
      neck: !!C.neck,
      "marks.age": K.age,
      "marks.brow": K.brow,
      "marks.fold": K.fold,
      "marks.bag": K.bag,
      "marks.crow": K.crow,
      "marks.frown": K.frown,
      "marks.gaunt": K.gaunt,
      "marks.chin": K.chin,
      "marks.cheek": K.cheek,
      "marks.mole": K.mole,
      "palette.recipe": C.P.recipe,
      body_rgb: lib.rgbOfHex(C.P.body, "drawn-heads P.body"),
    };
    if (C.hair.style !== "bald") out["hair.colour"] = C.P.hairName;
    return out;
  },
  domains: api => ({
    head: named(api.HEAD_KINDS, k => k.w),
    hair: named(api.HAIR_STYLES, k => k[1]),
    "hair.colour": HAIR_COLOURS,
    ears: EARS,
    eyes: named(api.EYE_KINDS, k => k.w),
    brows: BROWS,
    nose: NOSES,
    mouth: MOUTHS,
    beard: named(api.BEARD_KINDS, k => k[1]),
    "beard.tash": TASH,
    neck: [false, true],
    // makeMarks: age 0 to 3; forehead, nose-to-mouth and under-eye lines 0 to
    // 2; cheeks 0 to 3 (blush 1, freckles 2, both 3); a mole at one of three
    // places or none (-1); a cleft chin or a crease.
    "marks.age": [0, 1, 2, 3],
    "marks.brow": [0, 1, 2],
    "marks.fold": [0, 1, 2],
    "marks.bag": [0, 1, 2],
    "marks.crow": [false, true],
    "marks.frown": [false, true],
    "marks.gaunt": [false, true],
    "marks.chin": [0, 1, 2],
    "marks.cheek": [0, 1, 2, 3],
    "marks.mole": [-1, 0, 1, 2],
    // wild's recipes: byte 160 % 8 gives pastel 0, vivid 2, muted 4, dark 3.
    "palette.recipe": [0, 2, 3, 4],
  }),
};

module.exports = spec;
if (require.main === module) lib.main(spec);
