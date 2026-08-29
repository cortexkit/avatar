#!/usr/bin/env bash
# Diffs the canonical Rust generator against the lab's JS twin.
#
# The lab is what gets reviewed, so if it drifts from canonical the review
# is of the wrong thing — the same family as every other instrument that was
# true about something else. Drift is easy to introduce: a fix applied to
# one renderer and not the other looks correct in whichever one you happen
# to open.
#
# This found real divergence the first time it ran: the lab fit creatures
# into 0.72/0.80 of the frame against canonical's 172/198 of a 240-unit
# cell, and ran marching squares at resolution 110 against canonical's 130 —
# a coarser grid is a different contour, not a faster one.
#
# Compares derived geometry rather than SVG text: number formatting differs
# between the languages and would drown real divergence in noise.
#
# Usage: scripts/drift-check.sh [start] [count]

set -uo pipefail
cd "$(dirname "$0")/.."

START="${1:-4000}"
COUNT="${2:-24}"
LAB="file://$(pwd)/playground/web/index.html"

# Resolve a browser rather than hardcoding one machine's path. The previous
# version looked only in /Applications and exited 0 when it found nothing,
# so on any other machine this guard reported success without comparing
# anything — a green result that verified nothing is worse than no result,
# because it is indistinguishable from a real pass.
# An override that does not exist must fall through to detection and then
# to the explicit failure below, never be trusted because it was non-empty.
CHROME="${CHROME_BIN:-}"
if [[ -n $CHROME && ! -x $CHROME ]]; then
  echo "drift-check: CHROME_BIN=$CHROME is not executable" >&2
  CHROME=""
fi
if [[ -z $CHROME ]]; then
  for candidate in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "$(command -v google-chrome 2>/dev/null || true)" \
    "$(command -v google-chrome-stable 2>/dev/null || true)" \
    "$(command -v chromium 2>/dev/null || true)" \
    "$(command -v chromium-browser 2>/dev/null || true)"; do
    if [[ -n $candidate && -x $candidate ]]; then
      CHROME=$candidate
      break
    fi
  done
fi

if [[ -z $CHROME ]]; then
  echo "drift-check: no chrome or chromium found; set CHROME_BIN" >&2
  exit 1
fi

cargo build --quiet --manifest-path playground/Cargo.toml --bin grid || exit 1

fails=0
for ((i = 0; i < COUNT; i++)); do
  seed=$((START + i))
  rust=$(cargo run --quiet --manifest-path playground/Cargo.toml --bin grid -- --probe "$seed" 2>/dev/null | tail -1)
  js=$("$CHROME" --headless --disable-gpu --virtual-time-budget=20000 \
        --dump-dom "$LAB?probe=$seed" 2>/dev/null | grep -o 'seed=[^<]*' | head -1)
  # Numeric comparison, not string equality. Both renderers are float
  # implementations and their formatters round differently at the last
  # decimal (Rust {:.1} is half-to-even, JS toFixed is not), so string
  # equality reports formatting noise as drift and trains you to ignore the
  # output. Counts and flags must match EXACTLY; coordinates get a 0.15-unit
  # tolerance, which is far below anything visible and far above rounding.
  # Byte-exactness is the fixed-point implementation's bar, not this one's.
  if ! python3 - "$rust" "$js" <<'PY'
import sys

def fields(line):
    out = {}
    for token in line.split():
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        out[key] = value
    return out

a, b = fields(sys.argv[1]), fields(sys.argv[2])
if a.keys() != b.keys():
    sys.exit(1)
EXACT = {"seed", "spokes", "exts", "topo", "vor", "pts", "belly"}
for key in a:
    if key in EXACT:
        if a[key] != b[key]:
            sys.exit(1)
        continue
    av, bv = a[key].split(","), b[key].split(",")
    if len(av) != len(bv):
        sys.exit(1)
    for x, y in zip(av, bv):
        try:
            if abs(float(x) - float(y)) > 0.15:
                sys.exit(1)
        except ValueError:
            if x != y:
                sys.exit(1)
sys.exit(0)
PY
  then
    printf '\033[31mDRIFT\033[0m seed %s\n  rust %s\n  js   %s\n' "$seed" "$rust" "$js"
    fails=$((fails + 1))
  fi
done

if [[ $fails -eq 0 ]]; then
  printf '\033[32m%d seeds: renderers agree\033[0m\n' "$COUNT"
  exit 0
fi
printf '\033[31m%d of %d seeds drifted\033[0m\n' "$fails" "$COUNT"
exit 1
