#!/usr/bin/env bash
# Asserts the three facts a train landing depends on, every run.
#
# All three were verified by hand the day the train model was adopted and
# then trusted. That is the shape this repo keeps finding: a fact about a
# file, measured once, while the file goes on being edited. Branch
# protection would catch a narrowed trigger the hard way, and avatar is
# private, so under the operator's hold there is nothing else watching.
#
#   1. the push trigger includes train/**
#      Without it a train branch produces NO check at all, and the whole
#      model degrades to pushing straight to master with extra steps.
#
#   2. no job or step `if` keys on github.ref or github.event_name
#      Widening the trigger makes the workflow FIRE; it does not make its
#      jobs RUN. A job gated on the default ref skips on a train, and a
#      job that never starts reads exactly like one that keeps passing.
#
#   3. concurrency.group keys on github.ref
#      A shared group makes a train push CANCEL the in-flight master run,
#      so the landing gate and the observer fight each other.
#
# Conditions are read by parsing the YAML and enumerating every `if` KEY
# with its VALUE, never by grepping for the condition text: a folded
# scalar puts the condition on the lines below the key, so
# `grep 'if:.*github.ref'` returns clean on a workflow that is full of
# them. The fleet found that one the hard way.
#
# Usage: scripts/check-train-preconditions.sh [workflow.yml]

set -uo pipefail
cd "$(dirname "$0")/.."

WORKFLOW="${1:-.github/workflows/ci.yml}"

python3 - "$WORKFLOW" <<'PY'
import sys
import yaml

path = sys.argv[1]
try:
    with open(path, encoding="utf-8") as handle:
        doc = yaml.safe_load(handle)
except FileNotFoundError:
    print(f"train preconditions: no workflow at {path}")
    sys.exit(2)

# PyYAML resolves the bare key `on` to the boolean True (YAML 1.1), so the
# trigger section can arrive under either key depending on quoting.
trigger = doc.get(True, doc.get("on"))
failures = []

branches = []
if isinstance(trigger, dict):
    push = trigger.get("push")
    if isinstance(push, dict):
        branches = push.get("branches") or []
if not any(str(b).startswith("train/") for b in branches):
    failures.append(
        f"push trigger does not include train/** (has: {branches or 'nothing'}) "
        "- a train branch would produce no check at all"
    )

# Enumerate every `if` key and READ its value. A folded scalar arrives as
# a plain string here, which is the whole reason for parsing.
conditions = []


def walk(node, where):
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "if":
                conditions.append((where, " ".join(str(value).split())))
            else:
                walk(value, f"{where}.{key}")
    elif isinstance(node, list):
        for index, value in enumerate(node):
            walk(value, f"{where}[{index}]")


walk(doc.get("jobs", {}), "jobs")
for where, condition in conditions:
    if "github.ref" in condition or "github.event_name" in condition:
        failures.append(
            f"{where} is gated on the push path: `{condition}` "
            "- it fires on a train and skips, which reads like passing"
        )

group = str((doc.get("concurrency") or {}).get("group", ""))
if "github.ref" not in group:
    failures.append(
        f"concurrency.group does not key on github.ref (is: {group or 'unset'}) "
        "- a train push would cancel the in-flight default-branch run"
    )

if failures:
    print(f"train preconditions FAILED for {path}:")
    for failure in failures:
        print(f"  {failure}")
    sys.exit(1)

print(
    f"train preconditions: {path} ok "
    f"({len(conditions)} if-keys read, none path-dependent)"
)
PY
