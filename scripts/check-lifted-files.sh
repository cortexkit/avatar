#!/usr/bin/env bash
# Compares the lifted fleet train tooling against its upstream source.
#
# These four files are copied verbatim from cortexkit/aft rather than
# reimplemented, so that a contention-locked push protocol has one set of
# semantics across the fleet instead of one per seat. The adoption note
# says "re-copy rather than patch when they change" — which was a RULE
# WITH NO TRIGGER, and it failed within the hour.
#
# What happened: upstream fixed a real defect in watch-ci.sh (a short sha
# stored verbatim and then compared for exact equality against a full
# headSha, so the watch polled its whole budget and reported "no run
# appeared") and separately corrected a rebase-marker check. I had said I
# would wait for that fix and re-copy. Nothing was ever going to tell me
# it had landed. I found it only because another seat reported certifying
# their own lift baseline with a check that measured the wrong thing.
#
# A lift is a dependency. An unversioned dependency with no drift check
# is a copy that silently becomes a fork.
#
# NOT a CI gate: the upstream checkout only exists on a machine that has
# both repositories, so this runs in the local gate. When the upstream is
# absent it says so LOUDLY and distinctly rather than passing quietly —
# "could not verify" and "verified" must not look alike, which is the
# failure this whole script exists to correct.

set -uo pipefail
cd "$(dirname "$0")/.."

UPSTREAM="${LIFT_UPSTREAM:-../aft}"
FILES=(
  scripts/train-push.sh
  scripts/watch-ci.sh
  scripts/lib/operator-gh.sh
  scripts/lib/workflow-gates.py
)

if [[ ! -d $UPSTREAM ]]; then
  echo "lifted files: NOT VERIFIED - no upstream checkout at $UPSTREAM"
  echo "  (set LIFT_UPSTREAM, or clone cortexkit/aft beside this repo)"
  echo "  This is not a pass. The lift's currency is unknown."
  exit 0
fi

drift=0
for file in "${FILES[@]}"; do
  if [[ ! -f "$UPSTREAM/$file" ]]; then
    echo "lifted files: $file is missing UPSTREAM - it may have been renamed or retired"
    drift=1
  elif ! diff -q "$UPSTREAM/$file" "$file" >/dev/null 2>&1; then
    lines=$(diff "$UPSTREAM/$file" "$file" | grep -c '^[<>]')
    echo "lifted files: $file differs from upstream ($lines lines)"
    drift=1
  fi
done

if [[ $drift -ne 0 ]]; then
  echo "  Re-copy from $UPSTREAM rather than patching here; read the diff first,"
  echo "  since upstream changes have twice been defect fixes worth knowing about."
  exit 1
fi

echo "lifted files: ${#FILES[@]} verbatim against $UPSTREAM"
