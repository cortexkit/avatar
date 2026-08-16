#!/usr/bin/env bash
# Runs exactly what CI runs, in one command.
#
# CI went red for seven consecutive commits without being noticed: every
# change had been verified locally with `cargo test`, but the gate that
# actually failed was `cargo fmt --check`, which local verification never
# ran. Remembering to run six separate commands is not a fix; having one
# command that mirrors the workflow is.
#
# Keep this file and .github/workflows/ci.yml in step. If a gate exists in
# only one of them, the other is lying about what "green" means.
#
# Usage: scripts/check.sh [--rust-only]

set -uo pipefail
cd "$(dirname "$0")/.."

RUST_ONLY=0
[[ "${1:-}" == "--rust-only" ]] && RUST_ONLY=1

FAILED=()
run() {
  local name="$1"
  shift
  printf '\n\033[1m▸ %s\033[0m\n' "$name"
  local start=$SECONDS
  # Every gate runs even after an earlier one fails, matching CI's
  # `if: !cancelled()`: a lint failure must not hide the test signal.
  if "$@"; then
    printf '\033[32m  ok\033[0m (%ss)\n' "$((SECONDS - start))"
  else
    printf '\033[31m  FAILED\033[0m (%ss)\n' "$((SECONDS - start))"
    FAILED+=("$name")
  fi
}

# Same property CI checks with its before/after snapshot: did the build
# rewrite a tracked file? Compared against a snapshot rather than against
# an empty tree, so a dirty working copy does not make it fire.
tree_before=$(git status --porcelain | sort)

run "fmt"             cargo fmt --all -- --check
run "clippy"          cargo clippy --workspace --all-targets --locked -- -D warnings
run "clippy release"  cargo clippy --workspace --release --locked -- -D warnings
run "test"            cargo test --workspace --all-targets --locked
run "doc tests"       cargo test --workspace --doc --locked
# CI can use `git diff --exit-code` here because it mints on a clean
# checkout. Locally the tree is usually dirty, so that check would report
# every uncommitted edit as mint drift. Hash the tree around the mint
# instead: the property is "minting changed nothing", not "the tree is
# clean".
mint_fingerprint() { { git status --porcelain; git diff; } | shasum -a 256; }
mint_before=$(mint_fingerprint)
run "mint tables"     cargo run --locked -p avatar-tables --bin mint
run "mint is reproducible" bash -c \
  "[[ \"\$( { git status --porcelain; git diff; } | shasum -a 256 )\" == \"$mint_before\" ]]"
run "golden corpus"   cargo test --locked -p avatar-vectors

if [[ $RUST_ONLY -eq 0 ]]; then
  run "ts typecheck"  bash -c 'cd ports/ts && bunx tsc --noEmit'
  run "ts conformance" bash -c 'cd ports/ts && bun test'
  # Xcode's toolchain is required on macOS; CI uses a Linux Swift image.
  if command -v swift >/dev/null 2>&1; then
    run "swift conformance" bash -c \
      'DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer} swift test'
  fi
fi

# Same property CI checks: did the build rewrite a tracked file?
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
