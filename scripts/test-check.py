#!/usr/bin/env python3
"""Exercise the real gate scheduler with stub leaf tools, not compilers/browsers.

Run with python3 scripts/test-check.py. Each run uses a disposable git tree so
status comparisons, mint fingerprints, command dispatch and log collection are
real, while a deliberate compiler failure costs milliseconds rather than minutes.
"""

import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest


GATE = Path(__file__).with_name("check.sh")
STAGES = [
    "mint tables", "mint is reproducible", "core binary current",
    "fmt", "clippy", "clippy release", "test", "doc tests",
    "corpus coverage", "playground fmt", "playground clippy", "playground tests",
    "wild invariants", "renderer consts", "no ignored internal files tracked",
    "cross-platform comparison", "gate scheduler", "lab matches canonical", "ts typecheck",
    "ts conformance", "swift conformance",
]
# Record the expected command for each stage so losing a flag or a check
# cannot pass just because the scheduler still prints the right headings.
COMMANDS = [
    ("mint tables", ".", "cargo", "run --locked -p avatar-tables --bin mint"),
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
if stage == "mint tables":
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
    print("stale" if failed else "current")
if stage == "lab matches canonical" and not os.environ.get("OMIT_AGREEMENT"):
    print("24 seeds: renderers agree")
event("end")
(scratch / (stage + ".done")).touch()
sys.exit(23 if failed else 0)
'''


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
        (self.root / "ports/swift/AvatarCore.source-hash").write_text("current\n")
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
             write_path="stray.txt"):
        # AVATAR_GATE_LOCAL pins the run-everything-here layout these tests
        # describe; without it, and with the build-server client on PATH, the
        # gate sends its build-server stages to the Linux build server as one
        # job, on any operating system.
        env = dict(self.env, GATE_TEST_ROOT=str(self.root), AVATAR_GATE_LOCAL="1",
                   PATH=str(self.root / "tools") + os.pathsep + os.environ["PATH"],
                   FAIL_STAGES=",".join(failures), WRITE_STAGE=write, WRITE_PATH=write_path,
                   PARALLEL_PROBE="1" if parallel_probe else "",
                   OMIT_AGREEMENT="1" if omit_agreement else "")
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

    def test_mint_change_trips_both_guards_and_other_stages_still_run(self):
        self.gate(failures=("mint is reproducible",), write="mint tables")

    def test_stale_core_does_not_skip_lanes(self):
        self.gate(failures=("core binary current",))

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


if __name__ == "__main__":
    unittest.main(verbosity=2)
