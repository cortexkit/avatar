#!/usr/bin/env node
// Fidelity shim for creature.wild-plus: runs wild-plus/wildplus.js from the
// reference folder. See README.md in this directory.
"use strict";

const lib = require("./lib");

// The notes composeBody writes for each body plan (by plan index). A blob
// writes none.
const BODY_NOTES = [
  ["none"],
  ["torso", "necked torso"],
  [1, 2, 3].flatMap(n => [n + " segments", n + " segments with knobs"]),
  ["1 lobe pair", "2 lobe pairs", "3 lobe pairs"],
  ["long neck"],
  ["4 tentacles", "6 tentacles"],
  ["two thick legs", "bird legs", "4 arched legs", "6 arched legs", "four legs"],
];
const TAILS = ["curled", "tapering", "bushy", "spade", "puff"];
const FEET = ["feet", "toed feet", "clawed feet"];
const BELLIES = ["plated belly", "wide belly", "belly spot"];
// At most four wild limb slots (byte 64 % 5), so a limb kind is drawn on at
// most four slots.
const MAX_SLOTS = 4;

// The creature's anatomy, read back from the notes its builder keeps
// (B.notes): one per body plan, limb kinds, crest, arms, tail and feet. Every
// note must be recognised, so a note this does not know stops the run.
function anatomy(api, C) {
  const out = { body: "none", crest: "none", arms: "none", tail: "none", feet: "none", limbs: {} };
  for (const k of api.LIMB_KINDS) out.limbs[k] = 0;
  const fail = note => { throw new lib.HarnessError("wild-plus: unrecognised anatomy note " + JSON.stringify(note) + " in " + JSON.stringify(C.B.notes)); };
  const limbNote = note => {
    const counts = {};
    for (const part of note.split(", ")) {
      const m = /^(\d+) pairs of (.+)$/.exec(part);
      const kind = m ? m[2] : part, n = m ? Number(m[1]) : 1;
      if (!api.LIMB_KINDS.includes(kind) || kind in counts) return null;
      counts[kind] = n;
    }
    return counts;
  };
  for (const note of C.B.notes) {
    let m, limbs;
    if (BODY_NOTES[C.B.plan].includes(note) && out.body === "none") out.body = note;
    else if ((m = /^(.+) crest$/.exec(note)) && api.CRESTS.includes(m[1]) && out.crest === "none") out.crest = m[1];
    else if (note === "arms" && out.arms === "none") out.arms = "nub";
    else if ((m = /^(claw|flipper) arms$/.exec(note)) && out.arms === "none") out.arms = m[1];
    else if ((m = /^(.+) tail$/.exec(note)) && TAILS.includes(m[1]) && out.tail === "none") out.tail = m[1];
    else if (FEET.includes(note) && out.feet === "none") out.feet = note;
    else if ((limbs = limbNote(note)) && Object.values(out.limbs).every(n => n === 0)) Object.assign(out.limbs, limbs);
    else fail(note);
  }
  if (C.B.plan !== 0 && out.body === "none") fail("(no note for body plan " + C.B.plan + ")");
  return out;
}

function belly(C) {
  const found = C.surface.notes.filter(n => BELLIES.includes(n));
  if (found.length > 1) throw new lib.HarnessError("wild-plus: two belly notes in " + JSON.stringify(C.surface.notes));
  return found.length ? found[0] : "none";
}

const range = n => Array.from({ length: n }, (_, i) => i);

const spec = {
  stem: "wild-plus",
  script: "wild-plus/wildplus.js",
  global: "WildPlus",
  variety(refs) {
    return lib.sheetVariety(lib.loadSheet(refs, "wild-plus/sheet.js", "Sheet"), "wild-plus/sheet.js");
  },
  build: (api, bytes) => api.creature(bytes),
  // The size changes only the width and height attributes. A fixed id keeps
  // the output independent of how many creatures were drawn before.
  draw: (api, C, mood, size) => C.svg(mood, { size, id: "wp" }),
  // What C.traits reports: body plan, anatomy, coat, surface, palette, eyes,
  // mouth and nose.
  choices(api, C) {
    const A = anatomy(api, C), F = C.face;
    const out = {
      plan: api.PLANS[C.B.plan],
      body: A.body,
      crest: A.crest,
      arms: A.arms,
      tail: A.tail,
      feet: A.feet,
      coat: api.COATS[C.coat.kind],
      "surface.kind": api.SURFACES[C.surface.kind],
      "surface.second": api.SURFACES[C.surface.second],
      belly: belly(C),
      "palette.recipe": C.W.recipe,
      "palette.scheme": C.P.scheme,
      "eyes.plan": api.EYE_PLANS[F.eyePlan],
      "eyes.style": api.EYE_STYLES[F.style],
      mouth: api.MOUTH_PLANS[F.mouthPlan],
      nose: F.nose,
      body_rgb: lib.rgbOfHex(C.P.body, "wild-plus P.body"),
    };
    for (const k of api.LIMB_KINDS) out["limbs." + k] = A.limbs[k];
    return out;
  },
  domains(api) {
    const d = {
      plan: api.PLANS.slice(),
      body: BODY_NOTES.flat(),
      crest: api.CRESTS.slice(),
      arms: ["none", "nub", "claw", "flipper"],
      tail: ["none", ...TAILS],
      feet: ["none", ...FEET],
      coat: api.COATS.slice(),
      // A voronoi hide always draws pebbles (1); otherwise the surface pick
      // gives every other kind.
      "surface.kind": api.SURFACES.slice(),
      // Only a voronoi hide draws a second surface.
      "surface.second": [0, 7, 8, 10, 4, 2, 3].map(k => api.SURFACES[k]),
      belly: ["none", ...BELLIES],
      // wild's recipes: byte 160 % 8 gives pastel 0, vivid 2, muted 4, dark 3.
      "palette.recipe": [0, 2, 3, 4],
      "palette.scheme": range(8),
      "eyes.plan": api.EYE_PLANS.slice(),
      "eyes.style": api.EYE_STYLES.slice(),
      mouth: api.MOUTH_PLANS.slice(),
      nose: range(6),
    };
    for (const k of api.LIMB_KINDS) d["limbs." + k] = range(MAX_SLOTS + 1);
    return d;
  },
};

module.exports = spec;
if (require.main === module) lib.main(spec);
