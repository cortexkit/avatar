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

# --self-test proves the toolchain-drift guard CAN fire, by re-running this
# script against a stub rustup that reports stable behind. A guard that has
# never fired is indistinguishable from a guard that cannot fire: it would
# join the family of green signals that are true about the wrong thing,
# which is the exact failure it exists to catch.
if [[ "${1:-}" == "--self-test" ]]; then
  stub=$(mktemp -d)
  printf '%s\n' '#!/usr/bin/env bash' \
    '[[ "$1" == "check" ]] && { echo "stable-stub - Update available : 1.0.0 -> 2.0.0"; exit 0; }' \
    'exit 0' > "$stub/rustup"
  chmod +x "$stub/rustup"
  if PATH="$stub:$PATH" "$0" --rust-only >/dev/null 2>&1; then
    printf '\033[31mself-test FAILED\033[0m: drift guard stayed silent on a stale stub\n'
    rm -rf "$stub"
    exit 1
  fi
  rm -rf "$stub"
  printf '\033[32mself-test ok\033[0m: drift guard fires when stable is behind\n'
  exit 0
fi

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
# CI installs dtolnay/rust-toolchain@stable, which FLOATS. A local stable
# that has fallen behind runs an older lint set, so this script can report
# every gate green while CI fails on a lint that did not exist locally —
# which is exactly what happened with clippy's chunks_exact lint: local
# 1.97.1 knew nothing about it, CI's 1.98.0 rejected the build. Mirroring
# CI's commands is not enough; the toolchain is part of the gate.
#
# This is a hard gate rather than a warning: if stable has moved, CI is
# already running rules this machine cannot see, so "green" here would be
# a measurement of the wrong thing.
if rustup check 2>/dev/null | grep -q '^stable-.*Update available'; then
  printf '\033[31m▸ toolchain drift\033[0m: CI uses floating @stable and yours is behind.\n'
  rustup check | grep '^stable-'
  printf '  run: rustup update stable\n'
  exit 1
fi

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
    # Resolve the toolchain rather than inheriting one. Apple's /usr/bin/git
    # shim exports SDKROOT (CommandLineTools) into hook environments, so as
    # a pre-push hook this gate got Xcode's compiler with CommandLineTools'
    # SDK — a half-inherited toolchain that cannot build the stdlib, and it
    # reported a Swift conformance FAILURE for a suite that passes. A gate
    # whose verdict depends on who invoked it is not a gate.
    swift_dev_dir=/Applications/Xcode.app/Contents/Developer
    [[ -d $swift_dev_dir ]] || swift_dev_dir=$(xcode-select -p 2>/dev/null)
    run "swift conformance" \
      env -u SDKROOT DEVELOPER_DIR="$swift_dev_dir" swift test
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
