#!/usr/bin/env python3
"""Exercise the real gate scheduler with stub leaf tools, not compilers/browsers.

Run with python3 scripts/test-check.py. Each run uses a disposable git tree so
status comparisons, command dispatch and log collection are
real, while a deliberate compiler failure costs milliseconds rather than minutes.
"""

import contextlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import unittest


GATE = Path(__file__).with_name("check.sh")
SCRIPTS = GATE.parent
STAGES = [
    "mint is current", "core binary current",
    "fmt", "clippy", "clippy release", "test", "doc tests",
    "corpus coverage", "playground fmt", "playground clippy", "playground tests",
    "wild invariants", "renderer consts", "no ignored internal files tracked",
    "cross-platform comparison", "gate scheduler", "lab matches canonical", "ts typecheck",
    "ts conformance", "swift conformance",
]
# Record the expected command for each stage so losing a flag or a check
# cannot pass just because the scheduler still prints the right headings.
COMMANDS = [
    ("mint is current", ".", "cargo", "run --locked -p avatar-tables --bin mint -- --check"),
    ("core binary current", ".", "source-hash", ""),
    ("fmt", ".", "cargo", "fmt --all -- --check"),
    ("clippy", ".", "cargo", "clippy --workspace --all-targets --locked -- -D warnings"),
    ("clippy release", ".", "cargo", "clippy --workspace --release --locked -- -D warnings"),
    ("test", ".", "cargo", "test --workspace --all-targets --locked"),
    ("doc tests", ".", "cargo", "test --workspace --doc --locked"),
    ("corpus coverage", ".", "cargo", "run --locked -q -p avatar-genome --example fleet_coverage -- --population 1000"),
    ("playground fmt", "playground", "cargo", "fmt --all -- --check"),
    ("playground clippy", "playground", "cargo", "clippy --all-targets --locked -- -D warnings"),
    ("playground tests", "playground", "cargo", "test --locked"),
    ("wild invariants", "playground", "cargo", "run --locked -q --bin grid -- --check 300"),
    ("renderer consts", ".", "python3", "scripts/const-drift.py"),
    ("cross-platform comparison", "scripts/wild-port", "python3", "-m unittest -q test_compare"),
    ("gate scheduler", ".", "python3", "scripts/test-check.py"),
    ("lab matches canonical", ".", "drift", ""),
    ("ts typecheck", "ports/ts", "bunx", "tsc --noEmit"),
    ("ts conformance", "ports/ts", "bun", "test"),
    ("swift conformance", ".", "swift", "test --parallel"),
]

STUB = r'''
import json, os, pathlib, sys, time
root = pathlib.Path(os.environ["GATE_TEST_ROOT"])
scratch = root / ".scratch"
tool, args = sys.argv[1], sys.argv[2:]
if tool == "xcode-select":
    print("/fake/developer")
    sys.exit(0)
cwd = str(pathlib.Path.cwd().relative_to(root))
commands = json.loads((scratch / "commands.json").read_text())
stage = next(name for name, directory, exe, argv in commands
             if (directory, exe, argv.split()) == (cwd, tool, args))
def event(phase):
    # One append per record gives every process the same event order. Python
    # 3.9 on macOS has process-local monotonic clocks, not a shared timeline.
    record = (json.dumps([stage, phase]) + "\n").encode()
    fd = os.open(scratch / "events", os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        assert os.write(fd, record) == len(record)
    finally:
        os.close(fd)
event("start")
if stage == "mint is current":
    (scratch / "mint.done").touch()
elif stage == "core binary current":
    assert (scratch / "mint.done").exists()
    (scratch / "core.done").touch()
else:
    assert (scratch / "mint.done").exists()
    assert (scratch / "core.done").exists()
if os.environ.get("PARALLEL_PROBE") and stage in ("fmt", "playground fmt", "swift conformance"):
    (scratch / stage).touch()
    deadline = time.monotonic() + 10
    while not all((scratch / s).exists() for s in ("fmt", "playground fmt", "swift conformance")):
        assert time.monotonic() < deadline, "independent lanes did not overlap"
        time.sleep(0.01)
if stage != "core binary current":
    print("begin " + stage, flush=True)
    time.sleep(0.01)
    print("end " + stage, file=sys.stderr, flush=True)
if stage == os.environ.get("WRITE_STAGE"):
    if stage == "swift conformance":
        # Delay this Swift-stage write until the final commands in the root
        # and web lanes finish. A git-status guard run before all lanes join
        # would then miss the new file.
        deadline = time.monotonic() + 10
        while not all((scratch / (name + ".done")).exists()
                      for name in ("corpus coverage", "ts conformance")):
            assert time.monotonic() < deadline, "other lanes did not finish"
            time.sleep(0.01)
    (root / os.environ.get("WRITE_PATH", "stray.txt")).write_text("unexpected build output\n")
failed = stage in os.environ.get("FAIL_STAGES", "").split(",")
if stage == "core binary current":
    print("stale" if failed and not os.environ.get("MATCHING_FAILED_SOURCE_HASH") else "current")
if stage == "lab matches canonical" and not os.environ.get("OMIT_AGREEMENT"):
    print("24 seeds: renderers agree")
event("end")
(scratch / (stage + ".done")).touch()
sys.exit(23 if failed else 0)
'''


def make_framework_fixture(root):
    for name in ("check-xcframework.sh", "xcframework-artifact-hashes.sh"):
        shutil.copyfile(SCRIPTS / name, root / "scripts" / name)
    swift = root / "ports/swift"
    framework = swift / "AvatarCore.xcframework"
    for slice_name in ("ios-arm64", "ios-arm64-simulator", "macos-arm64"):
        slice_dir = framework / slice_name
        (slice_dir / "Headers").mkdir(parents=True)
        (slice_dir / "libavatar_ffi.a").write_text(slice_name)
        (slice_dir / "Headers/AvatarCore.h").write_text("header\n")
        (slice_dir / "Headers/module.modulemap").write_text("module\n")
    hashes = subprocess.check_output(["bash", "scripts/xcframework-artifact-hashes.sh"], cwd=root)
    (swift / "AvatarCore.artifact-hashes").write_bytes(hashes)
    (swift / "AvatarCore.source-hash").write_text("current\n")
    return framework


class GateTests(unittest.TestCase):
    def setUp(self):
        # Hook-provided Git paths must not redirect the disposable repository
        # or its index back into the caller's checkout.
        self.env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        self.temp = tempfile.TemporaryDirectory(prefix="avatar-gate-test-")
        self.addCleanup(self.temp.cleanup)
        # macOS temporary paths may use /var while child cwd uses /private/var.
        # Compare physical paths when matching a stub command to its stage.
        self.root = Path(self.temp.name).resolve()
        for directory in ("scripts/wild-port", "playground", "ports/ts", "ports/swift", "tools", ".scratch"):
            (self.root / directory).mkdir(parents=True, exist_ok=True)
        shutil.copyfile(GATE, self.root / "scripts/check.sh")
        make_framework_fixture(self.root)
        (self.root / ".gitignore").write_text("/.scratch/\n")
        (self.root / ".scratch/stub.py").write_text(STUB)
        (self.root / ".scratch/commands.json").write_text(json.dumps(COMMANDS))
        for tool in ("cargo", "python3", "bun", "bunx", "swift", "xcode-select", "drift", "source-hash"):
            path = self.root / "tools" / tool
            path.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{self.root}/.scratch/stub.py" {tool} "$@"\n')
            path.chmod(0o755)
        shutil.copyfile(self.root / "tools/drift", self.root / "scripts/drift-check.sh")
        shutil.copyfile(self.root / "tools/source-hash", self.root / "scripts/xcframework-source-hash.sh")
        for path in ("scripts/drift-check.sh", "scripts/xcframework-source-hash.sh"):
            (self.root / path).chmod(0o755)
        self.git("init", "-q")
        self.git("add", ".")
        self.git("-c", "user.name=Gate Test", "-c", "user.email=gate@example.invalid",
                 "commit", "-qm", "Initial gate fixture")

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, env=self.env, text=True)

    def gate(self, failures=(), write="", rust_only=False, parallel_probe=False, omit_agreement=False,
             write_path="stray.txt", matching_failed_source_hash=False, missing_framework=False):
        # AVATAR_GATE_LOCAL pins the run-everything-here layout these tests
        # describe; without it, and with the build-server client on PATH, the
        # gate sends its build-server stages to the Linux build server as one
        # job, on any operating system.
        env = dict(self.env, GATE_TEST_ROOT=str(self.root), AVATAR_GATE_LOCAL="1",
                   PATH=str(self.root / "tools") + os.pathsep + os.environ["PATH"],
                   FAIL_STAGES=",".join(failures), WRITE_STAGE=write, WRITE_PATH=write_path,
                   PARALLEL_PROBE="1" if parallel_probe else "",
                   OMIT_AGREEMENT="1" if omit_agreement else "",
                   MATCHING_FAILED_SOURCE_HASH="1" if matching_failed_source_hash else "")
        if missing_framework:
            shutil.rmtree(self.root / "ports/swift/AvatarCore.xcframework")
        with subprocess.Popen(["bash", "scripts/check.sh"] + (["--rust-only"] if rust_only else []),
                              cwd=self.root, env=env, text=True, start_new_session=True,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT) as process:
            try:
                stdout, _ = process.communicate(timeout=30)
            except subprocess.TimeoutExpired:
                # Stop every lane before removing its temporary working tree.
                os.killpg(process.pid, signal.SIGKILL)
                stdout, _ = process.communicate()
                self.fail("gate timed out:\n" + stdout)
        output = re.sub(r"\x1b\[[0-9;]*m", "", stdout)
        expected = STAGES[:-3] if rust_only else STAGES
        self.assertEqual(re.findall(r"^▸ (.+)$", output, re.M)[:len(expected)], expected, output)
        events_path = self.root / ".scratch/events"
        self.assertTrue(events_path.exists(), "stub commands produced no events:\n" + output)
        events = [json.loads(line) for line in events_path.read_text().splitlines()]
        self.assertCountEqual([stage for stage, phase in events if phase == "end"],
                              [name for name, *_ in COMMANDS if name in expected], output)
        for name, *_ in COMMANDS:
            if name not in expected or name == "core binary current":
                continue
            section = output.split("▸ " + name + "\n")[1].split("▸ ")[0]
            self.assertIn("begin " + name, section, output)
            self.assertIn("end " + name, section, output)
        failed = [stage for stage in expected if stage in failures or
                  (missing_framework and stage == "core binary current") or
                  (omit_agreement and stage == "lab matches canonical")]
        if write:
            failed.append("tracked-file guard")
        self.assertEqual(process.returncode, 1 if failed else 0, output)
        if failed:
            summary = f'{len(failed)} gate(s) failed: {" ".join(failed)}'
            self.assertEqual(output.strip().splitlines()[-1], summary, output)
            print(summary, flush=True)
        else:
            self.assertEqual(output.strip().splitlines()[-1], "all gates green", output)
        return events

    def test_all_stages_overlap_across_lanes_but_not_within_targets(self):
        events = self.gate(parallel_probe=True)
        positions = {(stage, phase): index for index, (stage, phase) in enumerate(events)}
        for lane in (("fmt", "clippy", "clippy release", "test", "doc tests", "corpus coverage"),
                     ("playground fmt", "playground clippy", "playground tests", "wild invariants", "lab matches canonical", "ts typecheck", "ts conformance")):
            for before, after in zip(lane, lane[1:]):
                self.assertLess(positions[before, "end"], positions[after, "start"],
                                f"{after} started before {before} ended: {events}")

    def test_rust_only_keeps_browser_but_skips_ports(self):
        self.gate(rust_only=True)

    def test_failure_in_rust_lane(self):
        self.gate(failures=("test",))

    def test_failure_in_web_lane(self):
        self.gate(failures=("playground tests",))

    def test_failure_in_swift_lane(self):
        self.gate(failures=("swift conformance",))

    def test_successful_browser_exit_without_agreement_still_fails(self):
        self.gate(omit_agreement=True)

    def test_browser_agreement_with_failed_exit_still_fails(self):
        self.gate(failures=("lab matches canonical",))

    def test_two_lanes_fail(self):
        self.gate(failures=("test", "swift conformance"))

    def test_stray_file_from_last_lane_trips_guard(self):
        self.gate(write="swift conformance")

    def test_stale_tables_fail_and_other_stages_still_run(self):
        self.gate(failures=("mint is current",))

    def test_stale_core_does_not_skip_lanes(self):
        self.gate(failures=("core binary current",))

    def test_failing_source_hash_with_matching_stdout_does_not_skip_lanes(self):
        self.gate(failures=("core binary current",), matching_failed_source_hash=True)

    def test_missing_framework_does_not_skip_lanes(self):
        self.gate(missing_framework=True)

    def test_existing_dirty_tree_does_not_trip_guard(self):
        (self.root / "ports/swift/AvatarCore.source-hash").write_text("current\n\n")
        (self.root / "preexisting.txt").write_text("already here\n")
        self.gate()

    def test_rewriting_already_modified_tracked_file_trips_guard(self):
        path = "playground/tracked.txt"
        (self.root / path).write_text("committed input\n")
        self.git("add", path)
        self.git("-c", "user.name=Gate Test", "-c", "user.email=gate@example.invalid",
                 "commit", "-qm", "Track build input")
        (self.root / path).write_text("preexisting edit\n")
        self.gate(write="playground tests", write_path=path)


class LeafCheckTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix="avatar-leaf-test-")
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        (self.root / "scripts").mkdir()

    def core_fixture(self):
        framework = make_framework_fixture(self.root)
        (self.root / "scripts/xcframework-source-hash.sh").write_text("echo current\n")
        return framework

    def core_check(self):
        return subprocess.run(["bash", "scripts/check-xcframework.sh"], cwd=self.root,
                              capture_output=True, text=True, timeout=10)

    def test_core_check_requires_framework_and_hashes_every_slice(self):
        framework = self.core_fixture()
        self.assertEqual(self.core_check().returncode, 0)
        for file in ("ios-arm64/libavatar_ffi.a", "ios-arm64-simulator/Headers/AvatarCore.h",
                     "macos-arm64/Headers/module.modulemap"):
            path = framework / file
            original = path.read_bytes()
            path.write_bytes(original + b"stale")
            result = self.core_check()
            self.assertNotEqual(result.returncode, 0, file)
            self.assertIn("is stale", result.stderr)
            path.write_bytes(original)
        shutil.rmtree(framework)
        self.assertNotEqual(self.core_check().returncode, 0)

    def test_core_check_rejects_failure_even_with_matching_hash_stdout(self):
        self.core_fixture()
        for name, output in (("source-hash", "echo current"),
                             ("artifact-hashes", "cat ports/swift/AvatarCore.artifact-hashes")):
            with self.subTest(name=name):
                helper = self.root / f"scripts/xcframework-{name}.sh"
                original = helper.read_text()
                helper.write_text(f"{output}\nexit 23\n")
                self.assertEqual(self.core_check().returncode, 23)
                helper.write_text(original)

    def test_drift_rejects_browser_failure_with_complete_probe(self):
        fields = ("seed=4000 spokes=8 exts=1 topo=1 vor=0 pts=32 bbox=0,0,10,10 "
                  "face=5,5,2,1,1 mouthy=7 belly=1 scale=1 drawn=0,0,10,10 "
                  "blush=none mouth=none mouthscale=none zzcount=0 zz=none")
        probes = self.root / "rust-probes"
        probes.write_text(f"rust-probe {fields}\n")
        chrome = self.root / "chrome-headless-shell-stub"
        chrome.write_text(f'''#!/bin/bash
echo '<pre id="probe">{fields}</pre>'
echo 'simulated browser failure after DOM dump' >&2
exit 23
''')
        chrome.chmod(0o755)
        env = dict(os.environ, DRIFT_RUST_PROBES=str(probes), CHROME_BIN=str(chrome),
                   TMPDIR=str(self.root), DRIFT_JOBS="1")
        result = subprocess.run(["bash", SCRIPTS / "drift-check.sh", "4000", "1"], env=env,
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("browser probe failed for seed 4000", result.stderr)
        self.assertIn("simulated browser failure after DOM dump", result.stderr)
        self.assertNotIn("renderers agree", result.stdout)

    @unittest.skipUnless(shutil.which("sqlite3"), "SQLite CLI not available")
    def test_fleet_verifier_opens_literal_hash_filename(self):
        db = self.root / "fleet#snapshot.db"
        with contextlib.closing(sqlite3.connect(db)) as conn:
            conn.execute("CREATE TABLE agent (agent_id, avatar_genome, avatar_type, avatar_version)")
        decoy = self.root / "fleet"
        result = subprocess.run(["bash", SCRIPTS / "verify-fleet-avatars.sh", db],
                                env=dict(os.environ, TMPDIR=str(self.root)),
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no rows carry a genome", result.stdout)
        self.assertFalse(decoy.exists())

    def test_constant_scan_collects_full_decimal_and_exponent_tokens(self):
        constants = runpy.run_path(str(SCRIPTS / "const-drift.py"))["constants"]
        self.assertEqual(constants("1e-3 9e-3 .125 1.e-2 5E+2 1_2.5e-1_f64"),
                         {0.001, 0.009, 0.125, 0.01, 500, 1.25})


if __name__ == "__main__":
    unittest.main(verbosity=2)
