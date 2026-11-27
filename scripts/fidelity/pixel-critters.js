#!/usr/bin/env node
// Fidelity shim for creature.pixel-critters: runs
// pixel-critters/pixelcritters.js from the reference folder. See README.md in
// this directory.
"use strict";

const lib = require("./lib");

// The gene tables the reference's readGenes picks from. Every table entry is
// reachable: each table has as many entries as its bits can address. Keys
// name the field of the creature's genes, C.G.
const GENES = {
  "body.plan": ["blob", "mound", "chibi", "pear"],
  "body.top": ["round", "boxy", "pointed", "square"],
  "body.bot": ["round", "boxy", "pointed", "square"],
  "top.kind": ["none", "cat", "round", "mouse", "long", "goblin", "bat", "lynx", "nub", "horns", "bull", "knobs", "antlers", "antennae", "unicorn", "frills"],
  // Read only for ears.
  "top.detail": ["plain", "inner", "dark", "knobs"],
  "side.kind": ["nub", "short", "long", "claw", "flipper", "tentacle", "wings", "none"],
  // Read only for wings.
  "side.wing": ["bat", "feather", "insect", "tiny"],
  "legs.kind": ["feet", "legs", "bird", "quad", "tentacles", "wisp", "crab", "none"],
  "tail.kind": ["none", "stub", "curl", "bushy", "devil", "lizard", "fluke", "rat"],
  "coat.kind": ["matte", "flat", "glossy", "sleek", "furry", "spiky", "woolly", "warty"],
  "pattern.belly": ["none", "round", "low", "small"],
  "pattern.marks": ["none", "spots", "pale spots", "patches", "one patch", "points", "spots and points", "patches and points"],
  "face.plan": ["pair", "wide", "close", "big", "cyclops", "three", "frog", "stalks"],
  "face.style": ["bead", "iris", "sparkle", "tall", "sclera", "dot", "slit", "lazy"],
  "face.lid": ["open", "lidded", "heavy", "stern"],
  "face.mouth": ["smile", "flat", "cat", "fangs", "buck", "beak", "maw", "tiny"],
  // Not read under a beak.
  "face.muzzle": ["none", "nose", "muzzle", "muzzle and nose"],
  "face.cheeks": ["none", "blush", "big blush", "whiskers"],
  "face.brows": ["none", "thin", "thick", "short"],
  "face.quirk": ["none", "smirk", "snaggle", "blep"],
};

const at = (obj, path) => path.split(".").reduce((o, k) => (o === undefined || o === null ? undefined : o[k]), obj);

const spec = {
  stem: "pixel-critters",
  script: "pixel-critters/pixelcritters.js",
  global: "PixelCritters",
  variety(refs) {
    return lib.sheetVariety(lib.loadSheet(refs, "pixel-critters/sheet.js", "Sheet"), "pixel-critters/sheet.js");
  },
  build: (api, bytes) => api.creature(bytes),
  // The size sets the cell size and the margins (one device pixel per CSS
  // pixel), so the drawing matches avatar-raster's pixel grid at that size.
  draw: (api, C, mood, size) => C.svg(mood, { size, dpr: 1 }),
  // The genes summaryOf reports, every other named face gene, the palette
  // recipe, and how the neutral sprite draws its eyes and mouth (which a
  // small or dark face can change from what the genes ask for).
  choices(api, C) {
    const out = {};
    for (const key of Object.keys(GENES)) {
      const v = at(C.G, key);
      if (v !== undefined) out[key] = v;
    }
    const F = C.sprite("neutral").ctx.F;
    out["drawn.eyes.plan"] = F.plan;
    out["drawn.eyes.kind"] = F.kind;
    out["drawn.mouth"] = F.mouth.kind;
    out["palette.recipe"] = C.P.recipe;
    out.body_rgb = lib.rgbOfHex(C.P.body, "pixel-critters P.body");
    return out;
  },
  domains() {
    const d = {};
    for (const [key, values] of Object.entries(GENES)) d[key] = values.slice();
    // layFace keeps the plan or, with no room for the eyes, falls back to close.
    d["drawn.eyes.plan"] = GENES["face.plan"].slice();
    // eyeKind gives one of the eye styles.
    d["drawn.eyes.kind"] = GENES["face.style"].slice();
    // A mouth too narrow for its kind becomes smile, fangs or tiny.
    d["drawn.mouth"] = GENES["face.mouth"].slice();
    // wild's recipes: byte 160 % 8 gives pastel 0, vivid 2, muted 4, dark 3.
    d["palette.recipe"] = [0, 2, 3, 4];
    return d;
  },
};

module.exports = spec;
if (require.main === module) lib.main(spec);
