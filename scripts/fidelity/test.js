#!/usr/bin/env node
// Tests of the checks the shims share (lib.js). They need neither the
// reference folder nor cargo: node scripts/fidelity/test.js
"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const lib = require("./lib");

const spec = { stem: "test", script: "test.js" };
const domains = lib.checkDomains({ kind: ["a", "b"], count: [0, 1, 2], flag: [false, true] }, spec);
const valid = { kind: "a", count: 2, flag: true, body_rgb: [0, 128, 255] };
const hex = "0".repeat(32768);

function manifest(cases, extra = {}) {
  return { format: "avatar-fidelity-manifest/1", type: "creature.test", kind: "base", genomes: [{ name: "g", source: "s", hex }], cases, ...extra };
}

test("a choices file holds only integers, strings and booleans in their domains", () => {
  lib.checkChoices(valid, domains, "g__neutral");
  for (const [change, message] of [
    [{ count: 0.5 }, /non-integer number 0.5/],
    [{ count: [1] }, /is an array/],
    [{ count: null }, /is null/],
    [{ count: {} }, /is an object/],
    [{ count: 3 }, /outside its domain/],
    [{ count: "2" }, /outside its domain/],
    [{ other: 1 }, /has no domain/],
    [{ body_rgb: [0, 0, 256] }, /body_rgb/],
    [{ body_rgb: [0, 0] }, /body_rgb/],
    [{ body_rgb: undefined }, /body_rgb/],
  ]) {
    assert.throws(() => lib.checkChoices({ ...valid, ...change }, domains, "g__neutral"), e => e instanceof lib.HarnessError && message.test(e.message));
  }
});

test("domains list each value once and never body_rgb", () => {
  for (const [bad, message] of [
    [{ body_rgb: [0] }, /must not list body_rgb/],
    [{ k: [1, 1] }, /twice/],
    [{ k: [] }, /no values/],
    [{ k: [0.5] }, /non-integer/],
  ]) {
    assert.throws(() => lib.checkDomains(bad, spec), e => e instanceof lib.HarnessError && message.test(e.message));
  }
});

test("a manifest refuses a repeated or malformed case ID", () => {
  lib.checkManifest(manifest([{ id: "g__neutral", genome: "g", mood: "neutral" }]), spec, "m");
  for (const [cases, message] of [
    [[{ id: "g__neutral", genome: "g", mood: "neutral" }, { id: "g__neutral", genome: "g", mood: "neutral" }], /case ID g__neutral is repeated/],
    [[{ id: "x__neutral", genome: "g", mood: "neutral" }], /is not g__neutral/],
    [[{ id: "h__neutral", genome: "h", mood: "neutral" }], /does not list/],
    [[{ id: "g__angry", genome: "g", mood: "angry" }], /unknown mood/],
  ]) {
    assert.throws(() => lib.checkManifest(manifest(cases), spec, "m"), e => e instanceof lib.HarnessError && message.test(e.message));
  }
  assert.throws(() => lib.checkManifest(manifest([], { type: "creature.other" }), spec, "m"), /creature.other/);
});

test("overlapping manifests stop a run before anything is written", () => {
  // Under the checkout's ignored target directory, so the test writes nothing
  // outside the checkout.
  const scratch = path.resolve(__dirname, "..", "..", "target");
  fs.mkdirSync(scratch, { recursive: true });
  const dir = fs.mkdtempSync(path.join(scratch, "fidelity-test-"));
  try {
    const base = path.join(dir, "base.json"), probes = path.join(dir, "probes.json"), out = path.join(dir, "out");
    const one = [{ id: "g__neutral", genome: "g", mood: "neutral" }];
    fs.writeFileSync(base, JSON.stringify(manifest(one)));
    fs.writeFileSync(probes, JSON.stringify(manifest(one, { kind: "probe" })));
    assert.throws(() => lib.run(spec, dir, [base, probes], out, 256), /case ID g__neutral is in both/);
    assert.ok(!fs.existsSync(out), "the output directory was created");
  } finally {
    fs.rmSync(dir, { recursive: true });
  }
});

test("a shim names the reference folder it could not find", () => {
  const missing = path.resolve(__dirname, "..", "..", "target", "no-such-reference-folder");
  const result = spawnSync(process.execPath, [path.join(__dirname, "wild-plus.js"), "run", "--manifest", "m.json", "--out", "o", "--refs", missing], { encoding: "utf8" });
  assert.strictEqual(result.status, 2);
  assert.match(result.stderr, new RegExp("the reference folder " + missing.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + " does not exist"));
});
