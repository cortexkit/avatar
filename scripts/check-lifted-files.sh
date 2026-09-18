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

# The sha these files were lifted from, and the reference the comparison
# is made against.
#
# This originally diffed against the upstream WORKING TREE, which was the
# same defect it exists to catch, one level up: it answered "does my copy
# match that directory right now" while being read as "is my copy current
# with upstream". Those coincide only by accident, and they did not. The
# checkout was 16 commits ahead of origin/main, so both files carrying the
# fixes I had just verified existed nowhere but one machine.
#
# Pinning the sha makes the reference explicit and the answer stable: the
# comparison is against content, not against whatever a directory holds
# this minute, and the check reports whether that sha has reached the
# published branch yet. An unpushed pin is not an error — it is how a
# fleet fix reaches seats before it is published — but it must be VISIBLE,
# because the failure mode is a fix that lives on one machine and is
# believed to be upstream.
#
# AND THEN THE PIN ITSELF WENT STALE, which is the same defect a third
# time. Pinning fixed "what am I comparing against" and introduced "the
# comparison never moves": the check reported 4 files verbatim, green,
# every run for twelve days while upstream landed EIGHT fixes to these
# files — among them a watch-ci correction for resolving a sha to its push
# run rather than whichever trigger fired last, which matters here because
# this repo's workflow fires on both master and train/**.
#
# So upstream moving past the pin FAILS rather than warning. A warning is
# what the previous two versions effectively were, and twelve days is the
# measurement of what a warning is worth. The remedy is two commands and
# bumping the pin is an explicit, diff-visible act — which is the point:
# staleness becomes a decision rather than a default.
LIFT_SHA="${LIFT_SHA:-6aef824ef1bce91ffabff097273b54b564df2305}"
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

if ! git -C "$UPSTREAM" cat-file -e "$LIFT_SHA^{commit}" 2>/dev/null; then
  echo "lifted files: NOT VERIFIED - $UPSTREAM does not have $LIFT_SHA"
  echo "  Fetch it, or re-lift and update LIFT_SHA. This is not a pass."
  exit 0
fi

drift=0
for file in "${FILES[@]}"; do
  if ! git -C "$UPSTREAM" cat-file -e "$LIFT_SHA:$file" 2>/dev/null; then
    echo "lifted files: $file does not exist at $LIFT_SHA - renamed or retired upstream"
    drift=1
  elif ! git -C "$UPSTREAM" show "$LIFT_SHA:$file" | diff -q - "$file" >/dev/null 2>&1; then
    lines=$(git -C "$UPSTREAM" show "$LIFT_SHA:$file" | diff - "$file" | grep -c '^[<>]')
    echo "lifted files: $file differs from the pinned lift ($lines lines)"
    drift=1
  fi
done

if [[ $drift -ne 0 ]]; then
  echo "  Re-copy from $UPSTREAM rather than patching here; read the diff first,"
  echo "  since upstream changes have twice been defect fixes worth knowing about."
  exit 1
fi

# The copy matches the pin. Now ask whether the PIN is still current,
# which is the question the previous version could not ask at all.
ahead_files=()
for file in "${FILES[@]}"; do
  if ! git -C "$UPSTREAM" diff --quiet "$LIFT_SHA" HEAD -- "$file" 2>/dev/null; then
    ahead_files+=("$file")
  fi
done

if [[ ${#ahead_files[@]} -gt 0 ]]; then
  commits=$(git -C "$UPSTREAM" rev-list --count "$LIFT_SHA"..HEAD -- "${FILES[@]}" 2>/dev/null || echo '?')
  echo "lifted files: PIN IS STALE - upstream has $commits commit(s) touching these files since ${LIFT_SHA:0:9}"
  for file in "${ahead_files[@]}"; do
    echo "    moved: $file"
  done
  echo "  Read them, then re-copy and bump LIFT_SHA:"
  echo "    git -C $UPSTREAM log ${LIFT_SHA:0:9}..HEAD -- ${FILES[*]}"
  exit 1
fi

# Say what the copy is verbatim AGAINST. "Verbatim" with no reference is
# the claim that was false for an hour without anything noticing.
short="${LIFT_SHA:0:9}"
if git -C "$UPSTREAM" merge-base --is-ancestor "$LIFT_SHA" origin/main 2>/dev/null; then
  echo "lifted files: ${#FILES[@]} verbatim at $short (published on origin/main)"
else
  ahead=$(git -C "$UPSTREAM" rev-list --count origin/main.."$LIFT_SHA" 2>/dev/null || echo '?')
  echo "lifted files: ${#FILES[@]} verbatim at $short - NOT YET ON origin/main ($ahead commits ahead)"
  echo "  Fine to run; the fixes are real and verified here. But a fresh clone of"
  echo "  the upstream does not have them, so do not cite this as upstream state."
fi
