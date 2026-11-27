#!/usr/bin/env node
// Fidelity shim for creature.sprouts: runs sprouts/sprouts.js from the
// reference folder. See README.md in this directory.
"use strict";

const lib = require("./lib");

// Tables sprouts.js keeps to itself. Its picks choose in proportion to
// weights that are all above zero, so every entry is reachable.
const FOLIAGE = ["leaf green", "lime", "forest", "teal", "sage", "olive", "fern", "jade", "moss"];
const ROOTS = ["biped", "quad", "skirt", "stilt", "nubs", "tripod"];
const LEAVES = ["ovate", "lance", "round", "heart", "lobed", "toothed", "spoon", "strap", "trefoil", "palmate", "pinnate", "needles", "fan", "crozier"];
const FLOWERS = ["daisy", "star", "tulip", "bell", "cluster", "aster", "rose"];
const FRUIT = ["berry", "pair", "drop", "bean"];
// Each body kind picks its coat and its skin from a list of its own; these
// are the lists' union.
const COATS = ["none", "fuzz", "thorns", "spines", "bracts"];
const SKINS = ["veins", "stripes", "plain", "speckles", "dimples", "scales", "ribs", "rings", "areoles", "seam", "florets", "drupelets", "tufts"];
const EYE_PLANS = ["two", "one", "three"];
const EYE_STYLES = ["bead", "white", "sparkle", "iris", "lidded", "wonky"];
const MOUTHS = ["smile", "cat", "open", "snaggle", "flat", "fangs", "o", "grin", "wavy", "jag"];
// The shoots a body can grow: one on top, a pair beside the head, a pair of arms.
const SHOOTS = ["apex", "ear", "arm"];
// What a shoot's growth rules can bear: leaves, flowers, fruit, thorns and tendrils.
const ORGANS = ["L", "U", "K", "T", "C"];

const spec = {
  stem: "sprouts",
  script: "sprouts/sprouts.js",
  global: "Sprouts",
  variety(refs) {
    return lib.sheetVariety(lib.loadSheet(refs, "sprouts/sheet.js", "Sheet"), "sprouts/sheet.js");
  },
  build: (api, bytes) => api.creature(bytes),
  // The size changes only the width and height attributes.
  draw: (api, C, mood, size) => C.svg(mood, { size }),
  // What summaryOf reports: the body, its colours, which shoots it grows (but
  // not their grown strings, which are not a choice from a list), the lobes of
  // a rosette, what the shoots bear and the forms of leaf, flower and fruit,
  // the roots, coat, skin and face.
  choices(api, C) {
    const out = {
      body: C.B.kind,
      "palette.recipe": C.P.recipe,
      "palette.foliage": C.P.foliage,
      roots: C.Rt.layout,
      coat: C.K.kind,
      skin: C.skin.kind,
      "eyes.plan": C.F.plan,
      "eyes.style": C.F.style,
      mouth: C.F.mouth,
      body_rgb: lib.rgbOfHex(C.P.body, "sprouts P.body"),
    };
    for (const name of SHOOTS) out["shoots." + name] = C.S.some(s => s.name === name);
    for (const o of ORGANS) out["has." + o] = !!C.has[o];
    if (C.X.lobes) out.lobes = C.X.lobes.n;
    if (C.O.leaf) out["leaf.form"] = C.O.leaf.form.name;
    if (C.O.flower) out["flower.form"] = C.O.flower.form;
    if (C.O.fruit) out["fruit.form"] = C.O.fruit.form;
    return out;
  },
  domains(api) {
    const d = {
      body: api.BODY_KINDS.slice(),
      // wild's recipes: byte 160 % 8 gives pastel 0, vivid 2, muted 4, dark 3.
      "palette.recipe": [0, 2, 3, 4],
      "palette.foliage": FOLIAGE,
      roots: ROOTS,
      coat: COATS,
      skin: SKINS,
      "eyes.plan": EYE_PLANS,
      "eyes.style": EYE_STYLES,
      mouth: MOUTHS,
      // A rosette's lobes: 7 plus a pick of 0 to 5.
      lobes: [7, 8, 9, 10, 11, 12],
      "leaf.form": LEAVES,
      "flower.form": FLOWERS,
      "fruit.form": FRUIT,
    };
    for (const name of SHOOTS) d["shoots." + name] = [false, true];
    for (const o of ORGANS) d["has." + o] = [false, true];
    return d;
  },
};

module.exports = spec;
if (require.main === module) lib.main(spec);
