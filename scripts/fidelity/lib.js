// What every fidelity shim shares: reading the reference folder, building the
// case manifests, checking choices against their domains and writing a run's
// output. Each `<stem>.js` beside this file describes one reference and calls
// `main` here. README.md in this directory explains the formats.
"use strict";

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

const REPO = path.resolve(__dirname, "..", "..");
// Where a worktree keeps its copy of the reference folder. It is git-ignored,
// so a fresh checkout does not have it.
const DEFAULT_REFS = path.join(".cortexkit", "internal-docs", "ports", "2d-leads");
const MOODS = ["neutral", "working", "happy", "waiting", "blocked", "sad", "sleepy"];
const GENOME_BYTES = 16384;
const GENOME_HEX = 2 * GENOME_BYTES;
const MANIFEST_FORMAT = "avatar-fidelity-manifest/1";
const KINDS = ["base", "probe", "corpus"];
// The base manifest: 33 fleet-shaped and 24 random genomes from the kit, the
// type's 96 variety genomes and 16 genomes from each dice seed r1 to r7, each
// in all seven moods.
const FLEET_COUNT = 33;
const RANDOM_COUNT = 24;
const VARIETY_COUNT = 96;
const DICE_SEEDS = ["r1", "r2", "r3", "r4", "r5", "r6", "r7"];
const DICE_COUNT = 16;
const BASE_CASES = (FLEET_COUNT + RANDOM_COUNT + VARIETY_COUNT + DICE_SEEDS.length * DICE_COUNT) * MOODS.length;
const DEFAULT_SIZE = 256;
const MAX_SIZE = 2048;
// A genome or case name becomes part of a file name.
const NAME = /^[a-z0-9][a-z0-9-]*$/;

// A problem with the harness's input or with a reference's output that makes
// the run meaningless, as opposed to a difference between two drawings.
class HarnessError extends Error {}

const sha256 = bytes => crypto.createHash("sha256").update(bytes).digest("hex");

// ---------------------------------------------------------------- arguments

// Parses `--name value` pairs. `repeatable` names flags that may be given more
// than once; their values are collected into an array.
function parseFlags(argv, allowed, repeatable = []) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i], value = argv[i + 1];
    if (!flag.startsWith("--") || !allowed.includes(flag.slice(2))) throw new HarnessError("unknown argument " + JSON.stringify(flag));
    if (value === undefined) throw new HarnessError(flag + " needs a value");
    const name = flag.slice(2);
    if (repeatable.includes(name)) (out[name] = out[name] || []).push(value);
    else if (name in out) throw new HarnessError(flag + " is given twice");
    else out[name] = value;
  }
  return out;
}

function parseSize(text) {
  if (text === undefined) return DEFAULT_SIZE;
  if (!/^[0-9]+$/.test(text) || Number(text) < 1 || Number(text) > MAX_SIZE) throw new HarnessError("--size must be a whole number of pixels from 1 to " + MAX_SIZE + ", not " + JSON.stringify(text));
  return Number(text);
}

// The reference folder: --refs, resolved from the current directory, or the
// default path in this checkout. A missing folder is the usual state of a
// fresh worktree, so the message names the path that was looked at.
function refsFolder(flag) {
  const dir = flag === undefined ? path.join(REPO, DEFAULT_REFS) : path.resolve(flag);
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    throw new HarnessError("the reference folder " + dir + " does not exist. Copy the 2D reference generators there, or pass --refs DIR.");
  }
  return dir;
}

function readRefFile(refs, rel) {
  const file = path.join(refs, rel);
  if (!fs.existsSync(file)) throw new HarnessError("the reference folder " + refs + " has no " + rel);
  return file;
}

// --------------------------------------------------------- loading references

// The references are browser scripts that hang their API on `window`. Under
// Node they find `window` as the global object, and each run loads exactly one
// reference, so their globals never collide.
function loadReference(refs, spec) {
  globalThis.window = globalThis;
  require(readRefFile(refs, spec.script));
  const api = globalThis[spec.global];
  if (!api || typeof api.creature !== "function") throw new HarnessError(spec.script + " did not define " + spec.global + ".creature");
  checkMoods(spec, api);
  return api;
}

// The reference's mood names must be exactly the seven Rust moods, in order,
// so a case's mood name means the same on both sides of the comparison.
function checkMoods(spec, api) {
  if (JSON.stringify(api.MOODS) !== JSON.stringify(MOODS)) {
    throw new HarnessError(spec.script + " has moods " + JSON.stringify(api.MOODS) + ", not the seven moods " + JSON.stringify(MOODS));
  }
}

// A review page's helper script (sheet.js), loaded after its generator. It
// defines its API on `window` like the generator does.
function loadSheet(refs, rel, global) {
  require(readRefFile(refs, rel));
  const api = globalThis[global];
  if (!api) throw new HarnessError(rel + " did not define " + global);
  return api;
}

// The 96 variety genomes as a sheet's variety() lists them, checked to be
// V00 to V95 in order.
function sheetVariety(sheet, rel) {
  const list = sheet.variety();
  list.forEach((v, i) => {
    if (v.name !== "V" + pad2(i)) throw new HarnessError(rel + ": variety genome " + i + " is named " + v.name + ", not V" + pad2(i));
  });
  return list.map(v => v.bytes);
}

function hexOf(bytes, what) {
  if (!(bytes instanceof Uint8Array) || bytes.length !== GENOME_BYTES) throw new HarnessError(what + " is not " + GENOME_BYTES + " bytes");
  return Buffer.from(bytes.buffer, bytes.byteOffset, bytes.length).toString("hex");
}

function checkHex(hex, what) {
  if (typeof hex !== "string" || hex.length !== GENOME_HEX || !/^[0-9a-f]*$/.test(hex)) throw new HarnessError(what + " is not " + GENOME_HEX + " lowercase hex digits");
  return hex;
}

const bytesOf = hex => new Uint8Array(Buffer.from(hex, "hex"));

// The kit's two genome lists: the fleet-shaped genomes and the fixed random
// ones. Their names are not used: a case is named by its place in the list.
function kitGenomes(refs) {
  require(readRefFile(refs, "kit/fleet-genomes.js"));
  const G = globalThis.AVATAR_GENOMES;
  if (!G || !Array.isArray(G.agents) || !Array.isArray(G.random)) throw new HarnessError("kit/fleet-genomes.js did not define AVATAR_GENOMES.agents and .random");
  if (G.agents.length !== FLEET_COUNT || G.random.length !== RANDOM_COUNT) {
    throw new HarnessError("kit/fleet-genomes.js holds " + G.agents.length + " fleet-shaped and " + G.random.length + " random genomes, not " + FLEET_COUNT + " and " + RANDOM_COUNT);
  }
  return {
    fleet: G.agents.map((g, i) => ({ name: "fleet-" + pad2(i), source: "kit fleet-shaped genome " + i, hex: checkHex(g.genome, "fleet-shaped genome " + i) })),
    random: G.random.map((g, i) => ({ name: "random-" + pad2(i), source: "kit random genome " + g.name, hex: checkHex(g.genome, "random genome " + g.name) })),
  };
}

// The genomes dice.js rolls for a seed, as a review page with `?dice=SEED&n=N`
// in its address shows them. dice.js reads the page's address and replaces
// the kit's random list, so it runs here with a stand-in address and the
// fixed list is put back afterwards.
function diceGenomes(refs, seed, count) {
  const file = readRefFile(refs, "dice.js");
  const G = globalThis.AVATAR_GENOMES, fixed = G.random;
  globalThis.location = { search: "?dice=" + encodeURIComponent(seed) + "&n=" + count };
  delete require.cache[require.resolve(file)];
  try {
    require(file);
    const rolled = G.random;
    if (rolled === fixed || rolled.length !== count) throw new HarnessError("dice.js rolled " + (rolled === fixed ? 0 : rolled.length) + " genomes for seed " + seed + ", not " + count);
    return rolled.map((g, i) => ({ name: "dice-" + seed + "-" + pad2(i + 1), source: "dice.js seed " + seed + " genome " + g.name, hex: checkHex(g.genome, "dice genome " + seed + ":" + g.name) }));
  } finally {
    G.random = fixed;
    delete globalThis.location;
  }
}

const pad2 = n => String(n).padStart(2, "0");

// ------------------------------------------------------------------ manifests

function manifestOf(spec, kind, genomes, moodsOf = () => MOODS) {
  const cases = [];
  for (const g of genomes) for (const mood of moodsOf(g)) cases.push({ id: g.name + "__" + mood, genome: g.name, mood });
  const manifest = {
    format: MANIFEST_FORMAT,
    type: "creature." + spec.stem,
    kind,
    genomes: genomes.map(g => ({ name: g.name, source: g.source, hex: g.hex })),
    cases,
  };
  checkManifest(manifest, spec, "the new " + kind + " manifest");
  return manifest;
}

function buildBaseManifest(spec, refs) {
  const api = loadReference(refs, spec);
  const kit = kitGenomes(refs);
  const variety = spec.variety(refs, api);
  if (variety.length !== VARIETY_COUNT) throw new HarnessError("the variety page gives " + variety.length + " genomes, not " + VARIETY_COUNT);
  const genomes = [
    ...kit.fleet,
    ...kit.random,
    ...variety.map((bytes, i) => ({ name: "variety-" + pad2(i), source: "variety genome V" + pad2(i), hex: hexOf(bytes, "variety genome " + i) })),
  ];
  for (const seed of DICE_SEEDS) genomes.push(...diceGenomes(refs, seed, DICE_COUNT));
  const manifest = manifestOf(spec, "base", genomes);
  if (manifest.cases.length !== BASE_CASES) throw new HarnessError("the base manifest has " + manifest.cases.length + " cases, not " + BASE_CASES);
  return manifest;
}

// The committed golden corpus: rows `project_id TAB moods TAB version` from
// vectors/<stem>/corpus.tsv, each genome read from vectors/wild/<id>.hex.
function buildCorpusManifest(spec, tsvFile) {
  const vectors = path.join(REPO, "vectors");
  const file = tsvFile === undefined ? path.join(vectors, spec.stem, "corpus.tsv") : path.resolve(tsvFile);
  if (!fs.existsSync(file)) throw new HarnessError("no corpus file " + file);
  const genomes = [], moods = new Map();
  fs.readFileSync(file, "utf8").split("\n").forEach((line, i) => {
    if (line === "" || line.startsWith("#")) return;
    const cols = line.split("\t"), where = file + ":" + (i + 1);
    if (cols.length !== 3) throw new HarnessError(where + ": expected project_id, moods and version separated by tabs");
    const [id, list, version] = cols;
    if (version !== "1") throw new HarnessError(where + ": version " + JSON.stringify(version) + "; a new type's corpus is drawn at version 1 only");
    if (!NAME.test(id)) throw new HarnessError(where + ": " + JSON.stringify(id) + " is not a genome name");
    const rowMoods = list.split(",");
    for (const m of rowMoods) if (!MOODS.includes(m)) throw new HarnessError(where + ": unknown mood " + JSON.stringify(m));
    if (!moods.has(id)) {
      const hex = fs.readFileSync(path.join(vectors, "wild", id + ".hex"), "utf8").trim();
      genomes.push({ name: id, source: "vectors/wild/" + id + ".hex", hex: checkHex(hex, "vectors/wild/" + id + ".hex") });
      moods.set(id, []);
    }
    moods.get(id).push(...rowMoods);
  });
  return manifestOf(spec, "corpus", genomes, g => moods.get(g.name));
}

// Supplemental probe genomes: lines `name TAB hex [TAB moods]`, where moods is
// a comma-separated list and defaults to all seven. Blank lines and lines
// starting with # are skipped.
function buildProbeManifest(spec, listFile) {
  const file = path.resolve(listFile);
  if (!fs.existsSync(file)) throw new HarnessError("no probe list " + file);
  const genomes = [], moods = new Map();
  fs.readFileSync(file, "utf8").split("\n").forEach((line, i) => {
    if (line.trim() === "" || line.startsWith("#")) return;
    const cols = line.split("\t"), where = file + ":" + (i + 1);
    if (cols.length !== 2 && cols.length !== 3) throw new HarnessError(where + ": expected name, hex and optional moods separated by tabs");
    const rowMoods = cols.length === 3 ? cols[2].split(",") : MOODS;
    for (const m of rowMoods) if (!MOODS.includes(m)) throw new HarnessError(where + ": unknown mood " + JSON.stringify(m));
    genomes.push({ name: cols[0], source: "probe list line " + (i + 1), hex: checkHex(cols[1].trim().toLowerCase(), where + ": the genome") });
    moods.set(cols[0], rowMoods);
  });
  return manifestOf(spec, "probe", genomes, g => moods.get(g.name));
}

// Checks a manifest's structure. Every case ID is `<genome>__<mood>`, so it is
// a safe file name, and a repeated case ID is an error.
function checkManifest(m, spec, what) {
  const fail = msg => { throw new HarnessError(what + ": " + msg); };
  if (!m || typeof m !== "object" || Array.isArray(m)) fail("not a JSON object");
  const keys = Object.keys(m).sort().join(",");
  if (keys !== "cases,format,genomes,kind,type") fail("expected exactly the keys cases, format, genomes, kind and type, found " + keys);
  if (m.format !== MANIFEST_FORMAT) fail("format is " + JSON.stringify(m.format) + ", not " + JSON.stringify(MANIFEST_FORMAT));
  if (m.type !== "creature." + spec.stem) fail("type is " + JSON.stringify(m.type) + ", but this shim draws creature." + spec.stem);
  if (!KINDS.includes(m.kind)) fail("kind is " + JSON.stringify(m.kind) + ", not one of " + KINDS.join(", "));
  if (!Array.isArray(m.genomes) || !Array.isArray(m.cases)) fail("genomes and cases must be arrays");
  if (m.cases.length === 0) fail("lists no cases");
  const genomes = new Map();
  for (const g of m.genomes) {
    if (!g || typeof g.name !== "string" || !NAME.test(g.name)) fail("genome name " + JSON.stringify(g && g.name) + " is not lowercase letters, digits and hyphens");
    if (genomes.has(g.name)) fail("genome " + g.name + " is listed twice");
    if (typeof g.source !== "string") fail("genome " + g.name + " has no source");
    checkHex(g.hex, what + ": genome " + g.name);
    genomes.set(g.name, g.hex);
  }
  const ids = new Set();
  for (const c of m.cases) {
    if (!c || typeof c.id !== "string") fail("a case has no id");
    if (ids.has(c.id)) fail("case ID " + c.id + " is repeated");
    ids.add(c.id);
    if (!genomes.has(c.genome)) fail("case " + c.id + " names genome " + JSON.stringify(c.genome) + ", which the manifest does not list");
    if (!MOODS.includes(c.mood)) fail("case " + c.id + " has unknown mood " + JSON.stringify(c.mood));
    if (c.id !== c.genome + "__" + c.mood) fail("case ID " + c.id + " is not " + c.genome + "__" + c.mood);
  }
  return genomes;
}

function readManifest(file, spec) {
  const what = path.resolve(file);
  if (!fs.existsSync(file)) throw new HarnessError("no manifest " + what);
  const bytes = fs.readFileSync(file);
  let m;
  try { m = JSON.parse(bytes.toString("utf8")); } catch (e) { throw new HarnessError(what + ": not JSON (" + e.message + ")"); }
  const genomes = checkManifest(m, spec, what);
  return { file: what, sha256: sha256(bytes), manifest: m, genomes };
}

function writeManifest(file, manifest) {
  // One genome and one case per line keeps the file readable in a diff.
  const lines = ["{", '"format": ' + JSON.stringify(manifest.format) + ",", '"type": ' + JSON.stringify(manifest.type) + ",", '"kind": ' + JSON.stringify(manifest.kind) + ",", '"genomes": ['];
  manifest.genomes.forEach((g, i) => lines.push(JSON.stringify(g) + (i + 1 < manifest.genomes.length ? "," : "")));
  lines.push("],", '"cases": [');
  manifest.cases.forEach((c, i) => lines.push(JSON.stringify(c) + (i + 1 < manifest.cases.length ? "," : "")));
  lines.push("]", "}", "");
  const text = lines.join("\n");
  fs.mkdirSync(path.dirname(path.resolve(file)), { recursive: true });
  fs.writeFileSync(file, text);
  return sha256(Buffer.from(text));
}

// -------------------------------------------------------- choices and domains

// A choice value is an integer, a string or a boolean. The key is the value's
// type and the value, so 1 and "1" stay apart.
const valueKey = v => typeof v + ":" + String(v);

function checkValue(v, where) {
  if (typeof v === "string" || typeof v === "boolean") return;
  if (typeof v === "number") {
    if (!Number.isSafeInteger(v)) throw new HarnessError(where + " is the non-integer number " + v + "; continuous values are never choices");
    return;
  }
  throw new HarnessError(where + " is " + (v === null ? "null" : Array.isArray(v) ? "an array" : typeof v === "object" ? "an object" : "a " + typeof v) + "; a choice is an integer, a string or a boolean");
}

// domains.json: for every choices key except body_rgb, the values the
// reference's own selector can give from a valid genome.
function checkDomains(domains, spec) {
  const what = spec.stem + " domains";
  if (!domains || typeof domains !== "object" || Array.isArray(domains)) throw new HarnessError(what + " is not an object");
  if ("body_rgb" in domains) throw new HarnessError(what + " must not list body_rgb");
  for (const [key, values] of Object.entries(domains)) {
    if (!Array.isArray(values) || values.length === 0) throw new HarnessError(what + ": " + key + " has no values");
    const seen = new Set();
    for (const v of values) {
      checkValue(v, what + ": a value of " + key);
      if (seen.has(valueKey(v))) throw new HarnessError(what + ": " + key + " lists " + JSON.stringify(v) + " twice");
      seen.add(valueKey(v));
    }
  }
  return new Map(Object.entries(domains).map(([k, vs]) => [k, new Set(vs.map(valueKey))]));
}

// Checks one case's choices: a flat object whose values are integers, strings
// or booleans, plus body_rgb, three integers from 0 to 255. Every other key
// must have a domain that holds its value.
function checkChoices(choices, domainSets, caseId) {
  const where = "case " + caseId;
  if (!choices || typeof choices !== "object" || Array.isArray(choices)) throw new HarnessError(where + ": the choices are not an object");
  const rgb = choices.body_rgb;
  if (!Array.isArray(rgb) || rgb.length !== 3 || !rgb.every(c => Number.isInteger(c) && c >= 0 && c <= 255)) {
    throw new HarnessError(where + ": body_rgb is " + JSON.stringify(rgb) + ", not three integers from 0 to 255");
  }
  for (const [key, v] of Object.entries(choices)) {
    if (key === "body_rgb") continue;
    checkValue(v, where + ": choice " + key);
    const domain = domainSets.get(key);
    if (!domain) throw new HarnessError(where + ": choice " + key + " has no domain in domains.json");
    if (!domain.has(valueKey(v))) throw new HarnessError(where + ": choice " + key + " = " + JSON.stringify(v) + " is outside its domain");
  }
}

// Key order is sorted, so two runs of the same case write the same bytes.
const sortedJson = obj => JSON.stringify(Object.fromEntries(Object.keys(obj).sort().map(k => [k, obj[k]])));

function rgbOfHex(hex, what) {
  const m = /^#([0-9a-fA-F]{6})$/.exec(hex);
  if (!m) throw new HarnessError(what + " is " + JSON.stringify(hex) + ", not a #rrggbb colour");
  return [0, 2, 4].map(i => parseInt(m[1].substr(i, 2), 16));
}

// ------------------------------------------------------------------- running

// Writes `<case>.svg` and `<case>.choices.json` for every case of every
// manifest, and domains.json, into `out`. All manifests are checked first,
// together, so a repeated case ID or a probe ID that is also a base ID stops
// the run before any file is written.
function run(spec, refs, manifestFiles, out, size) {
  const loaded = manifestFiles.map(f => readManifest(f, spec));
  const owner = new Map();
  for (const m of loaded) for (const c of m.manifest.cases) {
    if (owner.has(c.id)) throw new HarnessError("case ID " + c.id + " is in both " + owner.get(c.id) + " and " + m.file);
    owner.set(c.id, m.file);
  }
  const api = loadReference(refs, spec);
  const domains = spec.domains(api);
  const domainSets = checkDomains(domains, spec);

  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, "domains.json"), JSON.stringify(Object.fromEntries(Object.keys(domains).sort().map(k => [k, domains[k]])), null, 1) + "\n");
  let count = 0;
  for (const m of loaded) {
    // Each genome is built once and drawn in every mood its cases ask for.
    const byGenome = new Map();
    for (const c of m.manifest.cases) {
      if (!byGenome.has(c.genome)) byGenome.set(c.genome, []);
      byGenome.get(c.genome).push(c);
    }
    for (const [name, cases] of byGenome) {
      const creature = spec.build(api, bytesOf(m.genomes.get(name)));
      for (const c of cases) {
        const choices = spec.choices(api, creature, c.mood);
        checkChoices(choices, domainSets, c.id);
        const svg = spec.draw(api, creature, c.mood, size);
        if (typeof svg !== "string" || !svg.startsWith("<svg")) throw new HarnessError("case " + c.id + ": the reference did not return SVG markup");
        fs.writeFileSync(path.join(out, c.id + ".svg"), svg);
        fs.writeFileSync(path.join(out, c.id + ".choices.json"), sortedJson(choices) + "\n");
        count++;
      }
    }
  }
  return { loaded, count };
}

// Random genomes for finding which branches a reference reaches: splitmix64
// seeded with the FNV-1a hash of `seed`, as dice.js rolls them, so a seed
// always gives the same genomes.
function* randomGenomes(seed, count) {
  const MASK = (1n << 64n) - 1n;
  let state = 0xcbf29ce484222325n;
  for (const ch of seed) state = ((state ^ BigInt(ch.codePointAt(0))) * 0x100000001b3n) & MASK;
  for (let i = 0; i < count; i++) {
    const bytes = new Uint8Array(GENOME_BYTES);
    for (let j = 0; j < GENOME_BYTES; j += 8) {
      state = (state + 0x9e3779b97f4a7c15n) & MASK;
      let z = state;
      z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
      z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
      z ^= z >> 31n;
      for (let k = 0; k < 8; k++) bytes[j + k] = Number((z >> BigInt(8 * k)) & 0xffn);
    }
    yield bytes;
  }
}

// Builds `count` random genomes and counts how many reach each branch (a key
// and a value of domains.json). With `list`, writes the first genome that
// reaches each branch as a probe list line, for `probes --genomes`.
function sweep(spec, refs, count, seed, list) {
  const api = loadReference(refs, spec);
  const domains = spec.domains(api);
  const domainSets = checkDomains(domains, spec);
  const hits = new Map(), first = new Map();
  let i = 0;
  for (const bytes of randomGenomes(seed, count)) {
    const name = "sweep-" + seed.toLowerCase().replace(/[^a-z0-9]+/g, "-") + "-" + i;
    const choices = spec.choices(api, spec.build(api, bytes), "neutral");
    checkChoices(choices, domainSets, name);
    for (const [k, v] of Object.entries(choices)) {
      if (k === "body_rgb") continue;
      const b = k + " = " + JSON.stringify(v);
      hits.set(b, (hits.get(b) || 0) + 1);
      if (!first.has(b)) first.set(b, name + "\t" + hexOf(bytes, name));
    }
    i++;
  }
  const lines = [];
  for (const k of Object.keys(domains).sort()) for (const v of domains[k]) {
    const b = k + " = " + JSON.stringify(v);
    lines.push((hits.get(b) || 0) + "\t" + b);
  }
  if (list) fs.writeFileSync(list, [...new Set(first.values())].join("\n") + "\n");
  return lines;
}

function sourceCommit(refs) {
  const file = path.join(refs, "SOURCE-COMMIT");
  return fs.existsSync(file) ? fs.readFileSync(file, "utf8").trim() : "unknown (no SOURCE-COMMIT)";
}

const USAGE = [
  "usage: node scripts/fidelity/STEM.js COMMAND [options]",
  "  manifest --out FILE [--refs DIR]              write the base manifest",
  "  corpus --out FILE [--tsv FILE]                write the corpus manifest from vectors/STEM/corpus.tsv",
  "  probes --genomes LIST --out FILE              write a probe manifest from 'name TAB hex [TAB moods]' lines",
  "  run --manifest FILE [--manifest FILE ...] --out DIR [--refs DIR] [--size PX]",
  "                                                draw every case and write its SVG, its choices and domains.json",
  "  sweep --count N [--seed TEXT] [--list FILE] [--refs DIR]",
  "                                                count the branches N random genomes reach",
].join("\n");

function main(spec, argv = process.argv.slice(2)) {
  try {
    const [command, ...rest] = argv;
    if (command === "manifest") {
      const f = parseFlags(rest, ["out", "refs"]);
      if (!f.out) throw new HarnessError("manifest needs --out FILE");
      const refs = refsFolder(f.refs);
      const hash = writeManifest(f.out, buildBaseManifest(spec, refs));
      console.log("creature." + spec.stem + " base manifest: " + BASE_CASES + " cases, sha256 " + hash + ", written to " + f.out);
    } else if (command === "corpus") {
      const f = parseFlags(rest, ["out", "tsv"]);
      if (!f.out) throw new HarnessError("corpus needs --out FILE");
      const m = buildCorpusManifest(spec, f.tsv);
      const hash = writeManifest(f.out, m);
      console.log("creature." + spec.stem + " corpus manifest: " + m.cases.length + " cases, sha256 " + hash + ", written to " + f.out);
    } else if (command === "probes") {
      const f = parseFlags(rest, ["out", "genomes"]);
      if (!f.out || !f.genomes) throw new HarnessError("probes needs --genomes LIST and --out FILE");
      const m = buildProbeManifest(spec, f.genomes);
      const hash = writeManifest(f.out, m);
      console.log("creature." + spec.stem + " probe manifest: " + m.cases.length + " cases, sha256 " + hash + ", written to " + f.out);
    } else if (command === "run") {
      const f = parseFlags(rest, ["manifest", "out", "refs", "size"], ["manifest"]);
      if (!f.manifest || !f.out) throw new HarnessError("run needs --manifest FILE and --out DIR");
      const size = parseSize(f.size);
      const refs = refsFolder(f.refs);
      const result = run(spec, refs, f.manifest, f.out, size);
      console.log("creature." + spec.stem + " reference run");
      console.log("  node " + process.version + ", reference " + spec.script + " at source commit " + sourceCommit(refs));
      console.log("  input: the 16,384 genome bytes as a Uint8Array; moods: " + MOODS.join(", "));
      console.log("  drawn at " + size + " px");
      for (const m of result.loaded) console.log("  " + m.manifest.kind + " manifest " + m.file + ": " + m.manifest.cases.length + " cases, sha256 " + m.sha256);
      console.log("  wrote " + result.count + " cases and domains.json to " + path.resolve(f.out));
    } else if (command === "sweep") {
      const f = parseFlags(rest, ["count", "seed", "list", "refs"]);
      if (!/^[1-9][0-9]*$/.test(f.count || "")) throw new HarnessError("sweep needs --count N, a positive whole number");
      const refs = refsFolder(f.refs);
      for (const line of sweep(spec, refs, Number(f.count), f.seed || "sweep", f.list)) console.log(line);
    } else {
      throw new HarnessError(command === undefined ? "no command given" : "unknown command " + JSON.stringify(command));
    }
  } catch (e) {
    if (!(e instanceof HarnessError)) throw e;
    console.error("fidelity " + spec.stem + ": " + e.message);
    if (/no command|unknown command|unknown argument|needs/.test(e.message)) console.error(USAGE);
    process.exit(2);
  }
}

module.exports = {
  HarnessError, MOODS, GENOME_BYTES, BASE_CASES, DEFAULT_SIZE,
  main, run, checkChoices, checkDomains, checkManifest, readManifest, writeManifest,
  buildBaseManifest, buildCorpusManifest, buildProbeManifest, rgbOfHex, refsFolder, readRefFile, loadSheet, sheetVariety,
};
