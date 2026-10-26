#!/usr/bin/env bash
# THE gate for this repository. Every gate runs in one command, and the
# pre-push hook runs it before anything reaches master.
#
# CI is manual-only, so nothing else checks a change. This script exists
# because remembering to run separate commands failed: the repo once went
# red for seven commits while local verification ran `cargo test` and the
# gate that broke was `cargo fmt --check`.
#
# Usage: scripts/check.sh [--rust-only] [--linux]
#
# Where the stages run. A full gate compiles and runs dozens of fresh test
# binaries, and the workstation pushing is shared with other work (on macOS
# each new binary also costs a security scan). So by default, on any
# operating system, the build-server stages (every cargo stage, plus the
# Python and git checks that sit beside them and the Rust half of the lab
# comparison) run on the Linux build server, as one `ck motor run` job that
# snapshots this working tree, uncommitted edits included, and runs this
# script there with --linux. This machine runs the browser half of the lab
# comparison, the TypeScript port, the committed core binary's freshness
# and, where Swift is installed, the Swift suite. Each mode selects its own
# set of stages; a stage that runs in more than one mode runs the same
# command in each.
#
# AVATAR_GATE_LOCAL=1 runs everything on this machine instead: without the
# build-server client (`ck`), or when the build server is down. It is a
# choice the caller makes, never an automatic fallback: a fallback would
# quietly put the whole build back on the workstation.

set -uo pipefail
cd "$(dirname "$0")/.."

RUST_ONLY=0
LINUX=0
for arg in "$@"; do
  case $arg in
    --rust-only) RUST_ONLY=1 ;;
    --linux) LINUX=1 ;;
    *) echo "usage: scripts/check.sh [--rust-only] [--linux]" >&2; exit 2 ;;
  esac
done
if [[ $LINUX -eq 1 ]]; then
  WHERE=linux
elif [[ ${AVATAR_GATE_LOCAL:-} == 1 ]]; then
  WHERE=local
elif command -v ck >/dev/null 2>&1; then
  WHERE=remote
else
  echo "check.sh: the build-server client 'ck' is not on PATH." \
    "Set AVATAR_GATE_LOCAL=1 to run every stage on this machine." >&2
  exit 2
fi

FAILED=()
CARGO_STAGES=(
  "fmt" "clippy" "clippy release" "test" "doc tests" "golden corpus"
  "corpus coverage" "playground fmt" "playground clippy" "playground tests"
  "wild invariants" "renderer consts" "no ignored internal files tracked"
  "cross-platform comparison" "gate scheduler"
)
# STAGES is also the order logs are printed in. The first PREAMBLE entries
# run one after another before the parallel lanes start.
case $WHERE in
  linux)
    STAGES=("mint tables" "mint is reproducible" "${CARGO_STAGES[@]}" "lab probes")
    PREAMBLE=2 ;;
  local)
    STAGES=("mint tables" "mint is reproducible" "core binary current"
      "${CARGO_STAGES[@]}" "lab matches canonical")
    PREAMBLE=3 ;;
  remote)
    STAGES=("core binary current" "linux gates" "lab matches canonical")
    PREAMBLE=1 ;;
esac
if [[ $RUST_ONLY -eq 0 && $WHERE != linux ]]; then
  STAGES+=("ts typecheck" "ts conformance")
  if command -v swift >/dev/null 2>&1; then
    STAGES+=("swift conformance")
  fi
fi

log_dir=$(mktemp -d "${TMPDIR:-/tmp}/avatar-check.XXXXXX") || exit 1
trap 'rm -rf "$log_dir"' EXIT
mkfifo "$log_dir/completed" || exit 1
exec 3<> "$log_dir/completed"
PARALLEL=0

publish() {
  local index="$1" result="$2"
  cat "$log_dir/$index.log"
  [[ $result -eq 0 ]] || FAILED+=("${STAGES[$index]}")
}

run() {
  local name="$1"
  shift
  local index result start
  for index in "${!STAGES[@]}"; do
    [[ ${STAGES[$index]} == "$name" ]] && break
  done
  # Every gate runs even after an earlier one fails: a lint failure must
  # not hide the test signal.
  {
    printf '\n\033[1m▸ %s\033[0m\n' "$name"
    start=$SECONDS
    # A pipe keeps /dev/stderr a stream. The drift check tees there; pointing
    # it directly at a regular log file would reopen and truncate that file.
    if "$@" 2>&1 | cat; then
      result=0
      printf '\033[32m  ok\033[0m (%ss)\n' "$((SECONDS - start))"
    else
      result=1
      printf '\033[31m  FAILED\033[0m (%ss)\n' "$((SECONDS - start))"
    fi
  } > "$log_dir/$index.log" 2>&1
  if [[ $PARALLEL -eq 1 ]]; then
    # One short completion record is atomic even when two lanes finish at
    # once. The reader buffers out-of-order results without blocking a lane.
    printf '%s %s\n' "$index" "$result" >&3
  else
    publish "$index" "$result"
  fi
}

# Did the build rewrite a tracked file? Compared against a snapshot rather
# than against an empty tree, so a dirty working copy does not make it fire.
tree_before=$(git status --porcelain | sort)

# CI can use `git diff --exit-code` here because it mints on a clean
# checkout. Locally the tree is usually dirty, so that check would report
# every uncommitted edit as mint drift. Hash the tree around the mint
# instead: the property is "minting changed nothing", not "the tree is
# clean".
if [[ $WHERE != remote ]]; then
  mint_fingerprint() { { git status --porcelain; git diff; } | shasum -a 256; }
  mint_before=$(mint_fingerprint)
  run "mint tables"     cargo run --locked -p avatar-tables --bin mint
  run "mint is reproducible" bash -c \
    "[[ \"\$( { git status --porcelain; git diff; } | shasum -a 256 )\" == \"$mint_before\" ]]"
fi
# Mint is the only stage that writes source. Finish it before any compiler
# or source-hash reader starts, so all three lanes see the same inputs.
# The committed AvatarCore binary is what phones run. Every other test
# builds from source, so a forgotten rebuild passes them all; this compares
# the source the binary was built from against the source here.
if [[ $WHERE != linux ]]; then
  run "core binary current" bash -c \
    '[ "$(cat ports/swift/AvatarCore.source-hash 2>/dev/null)" = "$(./scripts/xcframework-source-hash.sh)" ] \
     || { echo "  AvatarCore.xcframework is stale: rebuild with scripts/build-xcframework.sh"; exit 1; }'
fi

rust_lane() {
  # Keep commands sharing the workspace target directory serial. Cargo's
  # build lock would serialize them anyway, with less predictable latency.
  run "fmt"             cargo fmt --all -- --check
  run "clippy"          cargo clippy --workspace --all-targets --locked -- -D warnings
  run "clippy release"  cargo clippy --workspace --release --locked -- -D warnings
  run "test"            cargo test --workspace --all-targets --locked
  run "doc tests"       cargo test --workspace --doc --locked
  run "golden corpus"   cargo test --locked -p avatar-vectors
  # Does the corpus still cover what real genomes reach? The hand-picked
  # vectors once missed nine decode selectors that live agents hit, including
  # a mouth dispatch arm no port had ever been byte-compared on. Uniform draws
  # have the same distribution as blake3-derived fleet genomes, so a synthetic
  # population re-asks the question here without needing registry data — and
  # fails when a decode change opens a branch no vector exercises.
  run "corpus coverage" cargo run --locked -q -p avatar-genome \
    --example fleet_coverage -- --population 1000

}

web_lane() {
  # The playground is its own workspace, so the gates above never reached it.
  # That mattered once it stopped being a sketch: it is the source the second
  # creature type will be ported from, and its placement invariants (eyes
  # inside the body, eyes clear of each other, mouth inside the body) were
  # only ever proved by hand. A proof nobody runs is a proof nobody has.
  run "playground fmt"    bash -c 'cd playground && cargo fmt --all -- --check'
  run "playground clippy" bash -c 'cd playground && cargo clippy --all-targets --locked -- -D warnings'
  run "playground tests"  bash -c 'cd playground && cargo test --locked'
  run "wild invariants"   bash -c 'cd playground && cargo run --locked -q --bin grid -- --check 300'
  # NOT gated here, deliberately, and the reason is worth more than the
  # step would be. `--bin grid -- --coverage` is a REPORT: it prints how
  # often each divergence-prone branch is reached and solves a minimal
  # covering set. It has no failure condition, so wiring it in would add
  # runtime and produce a green tick that means nothing.
  #
  # It cannot honestly become pass/fail either. It currently shows the
  # placement eyes-only fallback at 0 of 4000 seeds, and that branch SHOULD
  # be unreachable: it is the guarantee that a creature always has a face,
  # for bodies the population does not contain but the genome space allows.
  # A gate on "no unreached branches" would demand deleting a safety net or
  # faking its coverage. Run it by hand when choosing corpus vectors or
  # after changing the decoder.
  # Runs here as well as inside drift-check.sh: this half needs no browser,
  # and reaching it only through the half that does made it skippable on any
  # machine without one.
  run "renderer consts"   python3 scripts/const-drift.py
  # .cortexkit/ holds internal notes and worker briefs that must never reach
  # git. The ignore rule only stops new files; a file committed before the
  # rule existed stays tracked silently, so check what git actually tracks.
  run "no ignored internal files tracked" bash -c \
    'tracked=$(git ls-files -ci --exclude-standard -- .cortexkit); [ -z "$tracked" ] || { echo "$tracked"; exit 1; }'
  # The cross-platform script needs containers, so it runs by hand, but its
  # comparison must still be proven able to fail on every push: a comparison
  # that passed on empty or short inventories would report three platforms
  # identical without comparing anything.
  run "cross-platform comparison" bash -c \
    'cd scripts/wild-port && python3 -m unittest -q test_compare'
  # This script's own scheduling, run against stub tools: stage order, lanes,
  # failure reporting and the tracked-file guard. Its stubs only answer the
  # exact tool invocations listed in its COMMANDS table, so a changed flag
  # fails it. It must run here: outside the gate it once went stale unnoticed
  # when a stage gained a flag.
  run "gate scheduler" python3 scripts/test-check.py
  # playground/web/index.html is a JavaScript twin of the Rust generator,
  # and it is what a human opens to review creature designs. If the two
  # implementations drift, the review is of something nobody ships, so
  # drift-check.sh renders the same seeds through both and compares the
  # derived geometry.
  #
  # The gate greps for the script's agreement line instead of trusting its
  # exit code, because a comparison that never ran must not be reportable as
  # a successful comparison.
  case $WHERE in
    linux)
      # The build server has no browser: it produces the Rust half, and the
      # workstation compares it against the browser half (remote_lane below).
      run "lab probes" ./scripts/drift-check.sh --probes-only ;;
    local)
      run "lab matches canonical" bash -c \
        './scripts/drift-check.sh | tee /dev/stderr | grep -q "seeds: renderers agree"'
      ts_stages ;;
  esac
}

ts_stages() {
  if [[ $RUST_ONLY -eq 0 ]]; then
    run "ts typecheck"  bash -c 'cd ports/ts && bunx tsc --noEmit'
    run "ts conformance" bash -c 'cd ports/ts && bun test'
  fi
}

linux_gates() {
  # Runs this script with --linux on the build server. Its output, including
  # every remote stage's log and the Rust lab probes, is kept so the lab
  # stage can read the probes. Exit 200 means the server refused the job
  # before starting it (for example when it is full); nothing ran, so
  # retrying is safe. Any other non-zero exit fails the stage, including
  # ck's own codes for a lost or timed-out job.
  local attempt rc
  for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
    ck motor run --vcpus 4 --timeout 1800 -- bash scripts/check.sh --linux | tee "$log_dir/linux.out"
    rc=$?
    [[ $rc -eq 200 ]] || return "$rc"
    echo "build server refused the job before starting it (attempt $attempt of 12); retrying in 15 s"
    sleep 15
  done
  return 200
}

remote_lane() {
  run "linux gates" linux_gates
  run "lab matches canonical" bash -c \
    'DRIFT_RUST_PROBES="$0" ./scripts/drift-check.sh | tee /dev/stderr | grep -q "seeds: renderers agree"' \
    "$log_dir/linux.out"
}

swift_lane() {
  # --parallel runs the corpus-wide tree and embedded-core tests in several
  # worker processes.
  if [[ $(uname -s) != Darwin ]]; then
    # Linux and Windows use the toolchain on PATH. The embedded-core tests
    # need the Apple-platform core binary, so they are compiled only there.
    run "swift conformance" swift test --parallel
    return
  fi
  # On macOS, resolve Xcode's toolchain rather than inheriting one. Apple's
  # /usr/bin/git shim exports SDKROOT (CommandLineTools) into hook
  # environments, so as a pre-push hook this gate got Xcode's compiler with
  # CommandLineTools' SDK — a half-inherited toolchain that cannot build the
  # stdlib, and it reported a Swift conformance FAILURE for a suite that
  # passes. A gate whose verdict depends on who invoked it is not a gate.
  swift_dev_dir=/Applications/Xcode.app/Contents/Developer
  [[ -d $swift_dev_dir ]] || swift_dev_dir=$(xcode-select -p 2>/dev/null)
  run "swift conformance" \
    env -u SDKROOT DEVELOPER_DIR="$swift_dev_dir" swift test --parallel
}

# The browser probe builds and runs playground's grid binary, so it belongs
# with that workspace rather than racing its Cargo lock from another lane.
# SwiftPM uses .build; neither it nor the web lane shares the root target/.
PARALLEL=1
pids=()
if [[ $WHERE == remote ]]; then
  remote_lane &
  pids+=($!)
  ts_stages &
  pids+=($!)
else
  rust_lane &
  pids+=($!)
  web_lane &
  pids+=($!)
fi
if [[ " ${STAGES[*]} " == *" swift conformance "* ]]; then
  swift_lane &
  pids+=($!)
fi

# Logs are published in a fixed order, not in scheduler-dependent completion
# order. Consume completions immediately so a later stage never holds up its
# lane just because an earlier, slower stage has not been printed yet.
results=()
next=$PREAMBLE
remaining=$((${#STAGES[@]} - next))
while [[ $remaining -gt 0 ]]; do
  read -r index result <&3
  results[index]=$result
  remaining=$((remaining - 1))
  while [[ $next -lt ${#STAGES[@]} && -n ${results[$next]:-} ]]; do
    publish "$next" "${results[$next]}"
    next=$((next + 1))
  done
done
for pid in "${pids[@]}"; do wait "$pid"; done
exec 3>&-

# Did the build rewrite a tracked file?
if [[ "$(git status --porcelain | sort)" != "$tree_before" ]]; then
  printf '\n\033[31m▸ build rewrote tracked files\033[0m\n'
  git status --porcelain
  FAILED+=("tracked-file guard")
fi

printf '\n'
if [[ ${#FAILED[@]} -eq 0 ]]; then
  printf '\033[32mall gates green\033[0m\n'
  exit 0
fi
printf '\033[31m%d gate(s) failed:\033[0m %s\n' "${#FAILED[@]}" "${FAILED[*]}"
exit 1
