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
  "fmt" "clippy" "clippy release" "test" "doc tests"
  "corpus coverage" "playground fmt" "playground clippy" "playground tests"
  "wild invariants" "renderer consts" "no ignored internal files tracked"
  "no home paths tracked" "cross-platform comparison" "gate scheduler"
)
# STAGES is also the order logs are printed in. The first PREAMBLE entries
# run one after another before the parallel lanes start.
case $WHERE in
  linux)
    STAGES=("mint is current" "${CARGO_STAGES[@]}" "lab probes")
    PREAMBLE=1 ;;
  local)
    STAGES=("mint is current" "core binary current"
      "${CARGO_STAGES[@]}" "lab matches canonical")
    PREAMBLE=2 ;;
  remote)
    STAGES=("core binary current" "linux gates" "lab matches canonical")
    PREAMBLE=1 ;;
esac
if [[ $RUST_ONLY -eq 0 && $WHERE != linux ]]; then
  STAGES+=("ts typecheck" "ts conformance")
  if command -v swift >/dev/null 2>&1; then
    STAGES+=("swift conformance")
  else
    echo "swift conformance: skipped, no swift toolchain on PATH"
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

# Compare content as well as status: rewriting an already-dirty file need not
# change its status. Include untracked inputs, but leave ignored build output out.
tree_fingerprint() {
  {
    git status --porcelain=v1 -z &&
    git diff HEAD --binary &&
    git ls-files --others --exclude-standard -z |
      while IFS= read -r -d '' file; do
        printf '%s\0' "$file"
        if [[ -L $file ]]; then
          readlink "$file" || return
        else
          git hash-object --no-filters -- "$file" || return
        fi
      done
  } | shasum -a 256
}
tree_before=$(tree_fingerprint) || exit 1

# The committed tables must be exactly what the mint produces. --check
# compares them without writing, because build-server jobs cannot change
# tracked files.
if [[ $WHERE != remote ]]; then
  run "mint is current" cargo run --locked -p cortexkit-avatar-tables --bin mint -- --check
fi
# Host tests do not exercise the embedded iPhone slices. Verify all shipped
# slices against their build hashes and require the source stamp to match.
if [[ $WHERE != linux ]]; then
  run "core binary current" bash scripts/check-xcframework.sh
fi

rust_lane() {
  # Keep commands sharing the workspace target directory serial. Cargo's
  # build lock would serialize them anyway, with less predictable latency.
  run "fmt"             cargo fmt --all -- --check
  run "clippy"          cargo clippy --workspace --all-targets --locked -- -D warnings
  run "clippy release"  cargo clippy --workspace --release --locked -- -D warnings
  run "test"            cargo test --workspace --all-targets --locked
  run "doc tests"       cargo test --workspace --doc --locked
  # Does the corpus still cover what real genomes reach? The hand-picked
  # vectors once missed nine decode selectors that live agents hit, including
  # a mouth dispatch arm no port had ever been byte-compared on. Uniform draws
  # have the same distribution as blake3-derived fleet genomes, so a synthetic
  # population re-asks the question here without needing registry data — and
  # fails when a decode change opens a branch no vector exercises.
  run "corpus coverage" cargo run --locked -q -p cortexkit-avatar-genome \
    --example fleet_coverage -- --population 1000

}

web_lane() {
  # The playground is its own workspace, so the gates above never reach it.
  # It draws through the shipped wild generator, and its placement invariants
  # (eyes inside the body, eyes clear of each other, mouth inside the body)
  # are checked over 300 seeds here. A proof nobody runs is a proof nobody
  # has.
  run "playground fmt"    bash -c 'cd playground && cargo fmt --all -- --check'
  run "playground clippy" bash -c 'cd playground && cargo clippy --all-targets --locked -- -D warnings'
  run "playground tests"  bash -c 'cd playground && cargo test --locked'
  run "wild invariants"   bash -c 'cd playground && cargo run --locked -q --bin grid -- --check 300'
  # The constant scan needs no browser, so keep it separate from the lab's
  # runtime comparison to ensure it also runs on browserless machines.
  run "renderer consts"   python3 scripts/const-drift.py
  # .cortexkit/ holds internal notes and worker briefs that must never reach
  # git. The ignore rule only stops new files; a file committed before the
  # rule existed stays tracked silently, so check what git actually tracks.
  run "no ignored internal files tracked" bash -c \
    'tracked=$(git ls-files -ci --exclude-standard -- .cortexkit); [ -z "$tracked" ] || { echo "$tracked"; exit 1; }'
  # An absolute home path (/Users/<name>/ or /home/<name>/) in a tracked
  # text file names the machine and person it was written on, and the
  # repository is public. Pasted logs and generated files carry them in.
  run "no home paths tracked" python3 scripts/check-home-paths.py
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
  # Require both a successful drift process and its agreement line: neither a
  # skipped comparison nor a late process failure can count as agreement.
  # grep consumes the whole stream so an early match cannot close the pipe.
  case $WHERE in
    linux)
      # The build server has no browser: it produces the Rust half, and the
      # workstation compares it against the browser half (remote_lane below).
      run "lab probes" ./scripts/drift-check.sh --probes-only ;;
    local)
      run "lab matches canonical" bash -o pipefail -c \
        './scripts/drift-check.sh | tee /dev/stderr | grep "seeds: renderers agree" >/dev/null'
      ts_stages ;;
  esac
}

ts_stages() {
  if [[ $RUST_ONLY -eq 0 ]]; then
    run "ts typecheck"  bash -c 'cd ports/ts && bunx tsc --noEmit'
    run "ts conformance" bash -c 'cd ports/ts && bun test'
  fi
}

# How long the build-server half may take, counting queueing, refusals and
# retries together. A job can wait in the build server's queue before its
# command starts, and ck's --timeout bounds the command, not that wait: one
# push sat queued for 30 minutes with no output until something else ended
# it. The server can also refuse a job (exit 200) after it has queued, so a
# limit per attempt would let twelve retries hold a push for hours. The job
# itself takes under a minute.
GATE_JOB_LIMIT=${GATE_JOB_LIMIT:-600}

bounded() {
  # bounded SECONDS COMMAND...: runs COMMAND, ending it after SECONDS.
  # ck cancels its remote job when it is sent SIGTERM.
  local limit=$1
  shift
  "$@" &
  local pid=$! waited=0
  while kill -0 "$pid" 2>/dev/null; do
    if [[ $waited -ge $limit ]]; then
      kill -TERM "$pid" 2>/dev/null
      wait "$pid"
      return 124
    fi
    sleep 2
    waited=$((waited + 2))
  done
  wait "$pid"
}

linux_gates() {
  # Runs this script with --linux on the build server. Its output, including
  # every remote stage's log and the Rust lab probes, is kept so the lab
  # stage can read the probes. Exit 200 means the job was refused before
  # starting; nothing ran, so retrying is safe, within what is left of
  # GATE_JOB_LIMIT. Any other non-zero exit fails the stage, including ck's
  # own codes for a lost or timed-out job and the 124 of the spent limit.
  local attempt rc left deadline=$((SECONDS + GATE_JOB_LIMIT))
  for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
    left=$((deadline - SECONDS))
    rc=124
    if [[ $left -gt 0 ]]; then
      bounded "$left" ck motor run --vcpus 4 --timeout 1800 -- bash scripts/check.sh --linux | tee "$log_dir/linux.out"
      rc=$?
    fi
    if [[ $rc -eq 124 ]]; then
      echo "the build-server job did not finish within ${GATE_JOB_LIMIT} s, queueing and" \
        "$((attempt - 1)) refused attempts included; it was cancelled. 'ck motor attach" \
        "JOB_ID --status' shows why it waited; AVATAR_GATE_LOCAL=1 runs the gate on" \
        "this machine instead." >&2
      return 124
    fi
    [[ $rc -eq 200 ]] || return "$rc"
    echo "build server refused the job before starting it (attempt $attempt of 12); retrying in 15 s"
    sleep 15
  done
  return 200
}

remote_lane() {
  run "linux gates" linux_gates
  run "lab matches canonical" bash -o pipefail -c \
    'DRIFT_RUST_PROBES="$0" ./scripts/drift-check.sh | tee /dev/stderr | grep "seeds: renderers agree" >/dev/null' \
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
tree_after=$(tree_fingerprint) || exit 1
if [[ "$tree_after" != "$tree_before" ]]; then
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
