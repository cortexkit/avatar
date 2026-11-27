#!/usr/bin/env python3
"""Check that fidelity_compare passes a run against itself and fails where it must.

Takes a reference run that a shim in this directory wrote (its base manifest
and its output directory, and optionally a corpus run) and drives
crates/avatar-raster/examples/fidelity_compare.rs through these checks:

- the run compared with itself passes, with no failing pixel or choice;
- a candidate with one choice value changed fails, naming the case;
- a candidate with one drawing blanked fails on its pixels, naming the case;
- a candidate with one case removed fails, naming the case;
- the same case removed from both sides fails, naming the case;
- one case ID replaced by an unexpected ID on both sides fails, naming both;
- a manifest with one case ID repeated stops with a harness error naming it,
  although both directories match;
- a probe manifest that repeats a base case ID stops the same way;
- a choices file holding a non-integer number stops with a harness error;
- removing from the base manifest the only genome whose cases reach a branch
  reports that branch as missed.

The checks work on copies under --work (target/fidelity-selftest by default):
files are hard-linked from the run where the file system allows it, and a
changed file is replaced, never written through, so the run itself is not
changed. Nothing in scripts/check.sh calls this: it
needs the reference folder, which a checkout does not have.

usage: python3 scripts/fidelity/selftest.py --manifest BASE --run DIR
           [--corpus-manifest CORPUS --corpus-choices DIR] [--compare BIN] [--work DIR]

Without --compare it builds the example with cargo, in release.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
UNEXPECTED_ID = "selftest-unexpected__neutral"


def build_compare():
    subprocess.run(
        ["cargo", "build", "--release", "--locked", "-q", "-p", "cortexkit-avatar-raster",
         "--example", "fidelity_compare"],
        cwd=REPO, check=True,
    )
    name = "fidelity_compare.exe" if os.name == "nt" else "fidelity_compare"
    return REPO / "target" / "release" / "examples" / name


def link_or_copy(source, destination):
    """Hard-links a file, or copies it where the file system refuses links."""
    try:
        os.link(source, destination)
    except OSError:
        shutil.copy2(source, destination)


def clone(source, destination):
    """A copy of a run directory, its files hard-linked to the run's where the
    file system allows it."""
    shutil.copytree(source, destination, copy_function=link_or_copy)
    return destination


def rewrite(path, text):
    """Replaces a cloned file without writing through its link to the run."""
    path.unlink()
    path.write_text(text)


def remove_case(directory, case):
    for suffix in (".svg", ".choices.json"):
        (directory / (case + suffix)).unlink()


def write_json(path, value):
    path.write_text(json.dumps(value))
    return path


class SelfTest:
    def __init__(self, args):
        self.args = args
        self.work = Path(args.work).resolve()
        self.compare_bin = Path(args.compare).resolve() if args.compare else build_compare()
        self.run_dir = Path(args.run).resolve()
        self.manifest_path = Path(args.manifest).resolve()
        self.manifest = json.loads(self.manifest_path.read_text())
        self.cases = [c["id"] for c in self.manifest["cases"]]
        self.failures = []
        self.step = 0

    def scratch(self, name):
        self.step += 1
        return self.work / f"{self.step:02d}-{name}"

    def compare(self, name, candidate, reference=None, manifest=None, extra=()):
        report = self.work / f"{name}.md"
        command = [
            str(self.compare_bin),
            "--manifest", str(manifest or self.manifest_path),
            "--reference", str(reference or self.run_dir),
            "--candidate", str(candidate),
            "--report", str(report),
            *extra,
        ]
        return subprocess.run(command, capture_output=True, text=True)

    def expect(self, name, result, status, needles):
        output = result.stdout + result.stderr
        missing = [n for n in needles if n not in output]
        if result.returncode == status and not missing:
            print(f"PASS {name}: exit {status}; output names {', '.join(needles)}")
            return
        reason = f"exit {result.returncode}, expected {status}"
        if missing:
            reason += f"; output lacks {missing}"
        self.failures.append(name)
        print(f"FAIL {name}: {reason}\n" + "\n".join("    " + line for line in output.splitlines()[-15:]))

    def run(self):
        if self.work.exists():
            shutil.rmtree(self.work)
        self.work.mkdir(parents=True)
        first, second = self.cases[0], self.cases[1]
        domains = json.loads((self.run_dir / "domains.json").read_text())

        corpus = []
        if self.args.corpus_manifest:
            corpus = ["--corpus-manifest", str(Path(self.args.corpus_manifest).resolve()),
                      "--corpus-choices", str(Path(self.args.corpus_choices).resolve())]
        result = self.compare("self-run", self.run_dir, extra=corpus)
        self.expect("the run against itself", result, 0,
                    [f"{len(self.cases)} cases, {len(self.cases)} passed, 0 failed",
                     "0 failing pixels in all", "0 choice mismatches"])

        # One choice value changed to another value its domain allows.
        candidate = clone(self.run_dir, self.scratch("changed-choice"))
        path = candidate / f"{first}.choices.json"
        choices = json.loads(path.read_text())
        key = next(k for k in sorted(choices) if k != "body_rgb" and len(domains[k]) > 1)
        choices[key] = next(v for v in domains[key] if v != choices[key])
        rewrite(path, json.dumps(choices))
        self.expect("one choice value changed", self.compare("changed-choice", candidate), 1,
                    [f"FAIL {first}", f"choice {key}"])

        # One drawing replaced by an empty one of the same size: every pixel
        # the creature covers now differs.
        candidate = clone(self.run_dir, self.scratch("blank-drawing"))
        rewrite(candidate / f"{second}.svg",
                '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240" width="256" height="256"/>')
        self.expect("one drawing blanked", self.compare("blank-drawing", candidate), 1,
                    [f"FAIL {second}", "pixels fail"])

        candidate = clone(self.run_dir, self.scratch("removed-from-candidate"))
        remove_case(candidate, first)
        self.expect("one case removed from the candidate",
                    self.compare("removed-from-candidate", candidate), 1,
                    [f"FAIL {first}", "candidate SVG missing"])

        reference = clone(self.run_dir, self.scratch("removed-reference"))
        candidate = clone(self.run_dir, self.scratch("removed-candidate"))
        remove_case(reference, first)
        remove_case(candidate, first)
        self.expect("the same case removed from both sides",
                    self.compare("removed-from-both", candidate, reference), 1,
                    [f"FAIL {first}", "reference SVG missing", "candidate SVG missing"])

        reference = clone(self.run_dir, self.scratch("replaced-reference"))
        candidate = clone(self.run_dir, self.scratch("replaced-candidate"))
        for directory in (reference, candidate):
            for suffix in (".svg", ".choices.json"):
                (directory / (first + suffix)).rename(directory / (UNEXPECTED_ID + suffix))
        self.expect("one case ID replaced by an unexpected ID on both sides",
                    self.compare("replaced-id", candidate, reference), 1,
                    [f"FAIL {first}", f"reference: case {UNEXPECTED_ID}",
                     f"candidate: case {UNEXPECTED_ID}"])

        repeated = dict(self.manifest, cases=self.manifest["cases"] + [self.manifest["cases"][0]])
        manifest = write_json(self.work / "repeated-id.json", repeated)
        self.expect("a manifest with one case ID repeated",
                    self.compare("repeated-id", self.run_dir, manifest=manifest), 2,
                    [f"case ID {first} is repeated"])

        genome = self.manifest["cases"][0]["genome"]
        probe = dict(self.manifest, kind="probe",
                     genomes=[g for g in self.manifest["genomes"] if g["name"] == genome],
                     cases=[self.manifest["cases"][0]])
        probes = write_json(self.work / "probe-overlap.json", probe)
        self.expect("a probe ID that is also a base ID",
                    self.compare("probe-overlap", self.run_dir, extra=["--probes", str(probes)]), 2,
                    [f"probe case ID {first} is also a base case ID"])

        candidate = clone(self.run_dir, self.scratch("non-integer"))
        path = candidate / f"{second}.choices.json"
        choices = json.loads(path.read_text())
        key = next(k for k in sorted(choices) if k != "body_rgb")
        choices[key] = 0.5
        rewrite(path, json.dumps(choices))
        self.expect("a non-integer number in a choices file",
                    self.compare("non-integer", candidate), 2,
                    [f"{second}.choices.json", "non-integer number 0.5"])

        self.branch_missed(domains)
        return not self.failures

    def branch_missed(self, domains):
        """Drops the only genome whose cases reach some branch, and expects
        that branch reported missed."""
        reach = {}
        for case in self.manifest["cases"]:
            choices = json.loads((self.run_dir / f"{case['id']}.choices.json").read_text())
            for key, value in choices.items():
                if key != "body_rgb":
                    reach.setdefault((key, json.dumps(value)), set()).add(case["genome"])
        lone = sorted((k, v, next(iter(g))) for (k, v), g in reach.items() if len(g) == 1)
        if not lone:
            self.failures.append("branch missed")
            print("FAIL removing the only genome that reaches a branch: every branch is reached by two genomes or more")
            return
        key, value, genome = lone[0]
        dropped = [c["id"] for c in self.manifest["cases"] if c["genome"] == genome]
        manifest = dict(self.manifest,
                        genomes=[g for g in self.manifest["genomes"] if g["name"] != genome],
                        cases=[c for c in self.manifest["cases"] if c["genome"] != genome])
        manifest_path = write_json(self.work / "branch-missed.json", manifest)
        reference = clone(self.run_dir, self.scratch("missed-reference"))
        candidate = clone(self.run_dir, self.scratch("missed-candidate"))
        for directory in (reference, candidate):
            for case in dropped:
                remove_case(directory, case)
        result = self.compare("branch-missed", candidate, reference, manifest=manifest_path)
        self.expect(f"removing genome {genome} ({len(dropped)} cases), the only one reaching {key} = {value}",
                    result, 0, [f"missed branch: {key} = {value}"])


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--manifest", required=True, help="the base manifest of the run")
    parser.add_argument("--run", required=True, help="the shim's output directory for that manifest")
    parser.add_argument("--corpus-manifest", help="a corpus manifest, for the self-run's coverage")
    parser.add_argument("--corpus-choices", help="the shim's output directory for the corpus manifest")
    parser.add_argument("--compare", help="a built fidelity_compare example")
    parser.add_argument("--work", default=str(REPO / "target" / "fidelity-selftest"),
                        help="where the copies and reports go (emptied first)")
    args = parser.parse_args()
    if bool(args.corpus_manifest) != bool(args.corpus_choices):
        parser.error("--corpus-manifest and --corpus-choices go together")
    test = SelfTest(args)
    ok = test.run()
    print(("all checks passed" if ok else f"{len(test.failures)} checks failed") + f"; reports in {test.work}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
