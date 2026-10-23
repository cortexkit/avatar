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
# Usage: scripts/drift-check.sh [--probes-only] [start] [count]
#
# The two halves can run on different machines. --probes-only builds the
# Rust probe and prints one "rust-probe seed=..." line per seed, needing no
# browser; in its default mode check.sh runs it on the Linux build server so
# no Rust is compiled on the workstation. With DRIFT_RUST_PROBES naming a
# file of those lines, the Rust
# half is read from it instead of built, and only the browser half runs here.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

PROBES_ONLY=0
if [[ ${1:-} == --probes-only ]]; then
  PROBES_ONLY=1
  shift
fi
RUST_PROBES="${DRIFT_RUST_PROBES:-}"
START="${1:-4000}"
COUNT="${2:-24}"
LAB="file://$(pwd)/playground/web/index.html"

if [[ ! $COUNT =~ ^[1-9][0-9]*$ ]]; then
  echo "drift-check: count must be a positive integer" >&2
  exit 1
fi

GRID=""
build_grid() {
  # Ask cargo where it put the binary rather than assuming a target
  # directory: CARGO_TARGET_DIR or a symlinked target/ would put it elsewhere.
  GRID=$(cargo build --quiet --manifest-path playground/Cargo.toml --bin grid \
    --message-format=json | python3 -c '
import json, sys
paths = [m["executable"] for m in map(json.loads, sys.stdin)
         if m.get("reason") == "compiler-artifact" and m.get("executable")]
print(paths[-1] if paths else "")')
  if [[ -z $GRID || ! -x $GRID ]]; then
    echo "drift-check: could not build the playground grid binary" >&2
    exit 1
  fi
}

if [[ $PROBES_ONLY -eq 1 ]]; then
  build_grid
  for ((i = 0; i < COUNT; i++)); do
    seed=$((START + i))
    if ! line=$("$GRID" --probe "$seed" | tail -1); then
      echo "drift-check: Rust probe failed for seed $seed" >&2
      exit 1
    fi
    echo "rust-probe $line"
  done
  exit 0
fi

if [[ -n $RUST_PROBES && ! -r $RUST_PROBES ]]; then
  echo "drift-check: DRIFT_RUST_PROBES=$RUST_PROBES is not readable" >&2
  exit 1
fi

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
# chrome-headless-shell comes first: a plain binary with no app or window
# integration on any operating system. A full Chrome install, even with
# --headless, can register as a desktop app; on macOS every probe flashed an
# icon in the Dock and a burst of them left stale icons behind. Puppeteer and
# Playwright install the shell in per-user caches, whose location differs by
# operating system; the matching paths are version-sorted and the last is used.
newest_shell() {
  local exe
  for exe in chrome-headless-shell chrome-headless-shell.exe; do
    ls -d "$HOME"/.cache/puppeteer/chrome-headless-shell/*/chrome-headless-shell-*/"$exe" \
      "$HOME"/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-*/"$exe" \
      "$HOME"/Library/Caches/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-*/"$exe" \
      "${LOCALAPPDATA:-$HOME/AppData/Local}"/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-*/"$exe" \
      2>/dev/null
  done | sort -V | tail -1
}
if [[ -z $CHROME ]]; then
  for candidate in \
    "$(command -v chrome-headless-shell 2>/dev/null || true)" \
    "$(newest_shell)" \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "$(command -v google-chrome 2>/dev/null || true)" \
    "$(command -v google-chrome-stable 2>/dev/null || true)" \
    "$(command -v chromium 2>/dev/null || true)" \
    "$(command -v chromium-browser 2>/dev/null || true)" \
    "$(command -v chrome 2>/dev/null || true)"; do
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
case $CHROME in
  *chrome-headless-shell*) ;;
  *)
    echo "drift-check: using a full browser install, which may show a desktop icon per probe;" \
      "install chrome-headless-shell (npx @puppeteer/browsers install chrome-headless-shell@stable) to avoid it" >&2 ;;
esac

[[ -n $RUST_PROBES ]] || build_grid

# Collect every probe first, several seeds at a time, then compare in seed
# order. Each seed starts a fresh headless Chrome, so probing one seed after
# another spends most of its time waiting for browsers to start. Each Chrome
# gets its own profile directory, because concurrent instances sharing one
# profile contend for its lock.
probes=$(mktemp -d "${TMPDIR:-/tmp}/drift-check.XXXXXX") || exit 1
trap 'rm -rf "$probes"' EXIT
JOBS="${DRIFT_JOBS:-6}"
probe_seed() {
  local seed=$1
  if [[ -n $RUST_PROBES ]]; then
    # A seed missing from the file fails like a failed probe, so a short or
    # truncated file can never pass as agreement.
    grep -m1 "^rust-probe seed=$seed " "$RUST_PROBES" | sed 's/^rust-probe //' \
      > "$probes/$seed.rust" 2> "$probes/$seed.rust-err"
    echo "${PIPESTATUS[0]}" > "$probes/$seed.rust-rc"
  else
    "$GRID" --probe "$seed" > "$probes/$seed.rust" 2> "$probes/$seed.rust-err"
    echo $? > "$probes/$seed.rust-rc"
  fi
  "$CHROME" --headless --disable-gpu --user-data-dir="$probes/chrome-$seed" \
    --virtual-time-budget=20000 --dump-dom "$LAB?probe=$seed" \
    > "$probes/$seed.dom" 2>/dev/null
}
for ((i = 0; i < COUNT; i++)); do
  probe_seed $((START + i)) &
  if (((i + 1) % JOBS == 0)); then wait; fi
done
wait

fails=0
for ((i = 0; i < COUNT; i++)); do
  seed=$((START + i))
  if [[ "$(cat "$probes/$seed.rust-rc" 2>/dev/null)" != 0 ]]; then
    echo "drift-check: Rust probe failed for seed $seed" >&2
    cat "$probes/$seed.rust-err" >&2 2>/dev/null
    exit 1
  fi
  rust=$(tail -1 "$probes/$seed.rust")
  js=$(grep -o 'seed=[^<]*' "$probes/$seed.dom" 2>/dev/null | head -1)
  # Numeric comparison, not string equality. Both renderers are float
  # implementations and their formatters round differently at the last
  # decimal (Rust {:.1} is half-to-even, JS toFixed is not), so string
  # equality reports formatting noise as drift. Counts and flags must match
  # exactly; coordinates get a 0.15-unit tolerance on the 240-unit canvas,
  # allowing for both outputs rounding to one decimal place. Golden-tree
  # tests separately enforce byte-exact shipped output, without this tolerance.
  if ! python3 - "$rust" "$js" <<'PY'
import math
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
# Require the stdout contract, not just equal key sets: two blank or truncated
# probes must never masquerade as agreement.
KEYS = {"seed", "spokes", "exts", "topo", "vor", "pts", "bbox", "face", "mouthy", "belly"}
if a.keys() != KEYS or b.keys() != KEYS:
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
            ax, by = float(x), float(y)
            if not math.isfinite(ax) or not math.isfinite(by) or abs(ax - by) > 0.15:
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

# The sampling-free half. The loop above compares rendered geometry for a
# sample of seeds; this compares the tuning constants themselves, which a
# sample cannot reach exhaustively. It exists because the geometry half
# reported agreement while the lab drew every mouth 0.8r high.
if ! python3 "$(dirname "$0")/const-drift.py"; then
  fails=$((fails + 1))
fi

if [[ $fails -eq 0 ]]; then
  printf '\033[32m%d seeds: renderers agree\033[0m\n' "$COUNT"
  exit 0
fi
printf '\033[31m%d of %d seeds drifted\033[0m\n' "$fails" "$COUNT"
exit 1
