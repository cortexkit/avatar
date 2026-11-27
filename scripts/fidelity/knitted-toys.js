#!/usr/bin/env node
// Fidelity shim for creature.knitted-toys: runs knitted-toys/toys.js from the
// reference folder. See README.md in this directory.
"use strict";

const lib = require("./lib");

// The discrete traits decode() writes into the toy's traits, T, with the
// values each pick can give (toys.js, decode). Later rules in decode only
// clear a trait to "none" or pick again from a list inside the same set. A
// key whose trait decode does not set for a toy is left out of its choices.
const TRAITS = {
  plan: ["chibi", "ball", "egg", "pear", "bean", "loaf", "snowman", "pillar"],
  tone: ["pastel", "vivid", "muted", "dark"],
  accentFamily: [0, 1, 2, 3],
  "ears.kind": ["none", "round", "cat", "bunny", "floppy", "leaf", "gills", "tentacles", "frill", "ram", "tufts"],
  "ears.inner": ["felt", "none", "yarn"],
  "ears.tip": ["none", "tipped"],
  "top.kind": ["none", "nubs", "horns", "unicorn", "antlers", "antennae", "tuft", "sprout", "spines", "plates"],
  "top.yarn": ["cream", "gold", "accent", "grey"],
  "top.count": [3, 5],
  "top.tines": [2, 3],
  "arms.kind": ["none", "nub", "long", "wing", "four", "tentacle"],
  "arms.paw": ["none", "paw"],
  "legs.kind": ["none", "feet", "legs", "tentacles"],
  "legs.pads": ["none", "pad", "beans"],
  "legs.count": [4, 6],
  "tail.kind": ["none", "pompom", "curly", "long", "bushy", "spike"],
  "tail.tip": ["none", "tipped"],
  "stripes.kind": ["none", "parts"],
  "patches.belly": ["none", "felt", "knit"],
  "patches.spots": ["none", "spots"],
  "patches.mend": ["none", "joints", "mend", "heart"],
  "eyes.kind": ["safety", "iris", "felt", "stitched", "button"],
  "eyes.count": [2, 1, 3],
  "eyes.mount": ["face", "stalks", "bumps"],
  "eyes.iris": ["amber", "brown", "green", "blue", "violet"],
  "eyes.pupil": ["round", "slit"],
  "eyes.felt": ["round", "oval", "drop"],
  "eyes.look": ["centre", "out", "cross"],
  "eyes.lid": ["none", "half", "heavy"],
  "eyes.wonk": [0, 1, -1],
  "snout.kind": ["none", "muzzle", "pig", "beak", "bill"],
  "snout.yarn": ["accent", "tint", "main"],
  "nose.kind": ["safety", "stitched", "felt", "none"],
  "nose.shape": ["triangle", "oval", "heart"],
  "mouth.kind": ["w", "smile", "v", "line", "fangs", "teeth"],
  "brows.rest": ["none", "stitched", "felt"],
  "cheeks.kind": ["none", "felt", "stitched"],
  "colour.accent": ["cream", "tint", "shade", "contrast", "charcoal"],
  "colour.second": ["cream", "accent", "complement", "analog", "dark"],
  "temper.happy": ["arches", "open"],
  "temper.working": ["tongue", "pressed"],
  "temper.blocked": ["teeth", "zigzag"],
  "temper.sad": ["frown", "wobble"],
};

const at = (obj, path) => path.split(".").reduce((o, k) => (o === undefined || o === null ? undefined : o[k]), obj);

const spec = {
  stem: "knitted-toys",
  script: "knitted-toys/toys.js",
  global: "KnitToysB",
  // The variety page draws genomeFromString("knitted-toys-2d variety n") for
  // n from 0 to 95, through its sheet's varietyGenome.
  variety(refs) {
    const sheet = lib.loadSheet(refs, "knitted-toys/sheet.js", "KTSheet");
    return Array.from({ length: 96 }, (_, n) => sheet.varietyGenome(n));
  },
  build: (api, bytes) => api.creature(bytes),
  // The size sets the detail level (below 100 and 56 px) and the width of
  // rims and shadows, so the drawing is the one the reference makes at the
  // compared size.
  draw: (api, G, mood, size) => api.draw(G, mood, { px: size }),
  // What summary() reports and the variety page counts, with every other
  // discrete trait decode picks.
  choices(api, G) {
    const out = {};
    for (const key of Object.keys(TRAITS)) {
      const v = at(G.T, key);
      if (v !== undefined) out[key] = v;
    }
    // The main yarn: wild's body colour, dyed less strongly on a vivid toy.
    out.body_rgb = lib.rgbOfHex(G.pal.main.hex, "knitted-toys main yarn");
    return out;
  },
  domains() {
    const d = {};
    for (const [key, values] of Object.entries(TRAITS)) d[key] = values.slice();
    return d;
  },
};

module.exports = spec;
if (require.main === module) lib.main(spec);
