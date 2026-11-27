#!/usr/bin/env node
// Fidelity shim for creature.small-robots: runs small-robots/robots.js from
// the reference folder. See README.md in this directory.
"use strict";

const lib = require("./lib");

// robots.js picks through a scramble that maps each bit pattern to one entry
// in proportion to whole-number weights, so an entry is reachable exactly
// when some weight list the code uses for it is above zero.
const PLANS = ["pod", "chibi", "neck", "stack"];
const HULLS = ["box", "soft", "capsule", "dome", "cup", "bucket", "pyramid", "octagon", "hexagon", "ball", "egg", "acorn", "tee", "shield", "can"];
const CHASSIS = ["box", "barrel", "vee", "bell", "ball", "can", "octagon", "capsule"];
const NECKS = ["flush", "collar", "piston", "bellows", "ball", "spring"];
const FACES = ["screen", "lens", "lamp", "mono", "tri", "odd", "stalk"];
const SCREEN_EYES = ["dot", "pill", "square", "wide", "ring", "pupil"];
// A screen draws its mouth as a line; a hull mouth is a strip, a slot, a
// grid or none, and a face with no mouth at all writes none.
const MOUTHS = ["line", "strip", "slot", "grid", "none"];
const TOPS = ["none", "rod", "twin", "zigzag", "coil", "loop", "dish", "bulb", "beacon", "triple", "ears"];
const SIDES = ["none", "bolts", "rods", "cones", "vents", "knobs", "fins"];
// The arms pick gives "none" a weight of 0, so every robot has arms.
const ARMS = ["rod", "shell", "hose", "telescope", "stub"];
const HANDS = ["clamp", "pincer", "claw", "mitt", "ball", "magnet", "sucker", "fork"];
const GEAR = ["biped", "piston", "spring", "quad", "hexa", "tripod", "treads", "track", "wheels", "discs", "mono", "ball", "jet", "rings", "pogo", "hoses"];
const CHESTS = ["none", "light", "panel", "vent", "gauge", "leds", "hatch", "stencil", "hazard", "grille"];
const BACKS = ["none", "cable", "pipe"];
const METALS = ["steel", "gunmetal", "brass", "copper", "enamel"];

const spec = {
  stem: "small-robots",
  script: "small-robots/robots.js",
  global: "SmallRobots",
  variety(refs) {
    return lib.sheetVariety(lib.loadSheet(refs, "small-robots/sheet.js", "Sheet"), "small-robots/sheet.js");
  },
  build: (api, bytes) => api.creature(bytes),
  // The size sets the width of thin lines (1.3 / (size / 240) units,
  // unclamped), when a narrow stroke is drawn as a filled band, and the
  // detail kept below 96 and 56 px.
  draw: (api, C, mood, size) => C.svg(mood, { size }),
  // What summaryOf reports, field by field, and the palette recipe. A field
  // the robot does not have (a pod's chassis and neck, a lens face's screen)
  // is left out.
  choices(api, C) {
    const F = C.F, out = {
      plan: C.plan,
      hull: C.head.kind,
      "face.tech": F.tech,
      "face.mouth": F.mouth.kind,
      top: C.top,
      side: C.side,
      "arms.style": C.arms.style,
      "arms.hand": C.arms.hand,
      "arms.handL": C.arms.handL,
      gear: C.gear.kind,
      chest: C.chest.kind,
      back: C.back,
      metal: C.P.metalName,
      "palette.recipe": C.P.recipe,
      body_rgb: lib.rgbOfHex(C.P.body, "small-robots P.body"),
    };
    if (C.body) out.chassis = C.bodyKind;
    if (C.neck) out.neck = C.neck.kind;
    if (F.screen) {
      out["face.lcd"] = F.lcd;
      out["face.eyeStyle"] = F.eyeStyle;
    }
    return out;
  },
  domains: () => ({
    plan: PLANS, hull: HULLS, chassis: CHASSIS, neck: NECKS,
    "face.tech": FACES, "face.lcd": [false, true], "face.eyeStyle": SCREEN_EYES, "face.mouth": MOUTHS,
    top: TOPS, side: SIDES, "arms.style": ARMS, "arms.hand": HANDS, "arms.handL": HANDS,
    gear: GEAR, chest: CHESTS, back: BACKS, metal: METALS,
    // wild's recipes: byte 160 % 8 gives pastel 0, vivid 2, muted 4, dark 3.
    "palette.recipe": [0, 2, 3, 4],
  }),
};

module.exports = spec;
if (require.main === module) lib.main(spec);
