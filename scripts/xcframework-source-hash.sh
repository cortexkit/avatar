#!/usr/bin/env bash
# Prints one hash over everything that determines the committed
# AvatarCore.xcframework, so a stale binary can be detected without
# rebuilding it.
#
# Why this exists: the binary is committed, and a phone runs whatever it
# contains. Change the Rust source and forget to rebuild, and every Rust
# and Swift test still passes, because they build the source rather than
# the binary. For a creature type only Rust can build, nothing else would
# notice. build-xcframework.sh records this hash beside the binary, and
# check.sh refuses when the two disagree.
#
# Inputs: the source of every crate the core links, the workspace manifest
# and lockfile, the C headers, the build script, and the compiler version.
# File CONTENT is hashed rather than git object ids, so an uncommitted edit
# counts too.
set -euo pipefail
cd "$(dirname "$0")/.."

{
  rustc --version
  find crates/avatar-ffi crates/avatar-svg crates/avatar-genome \
       crates/avatar-phenotype crates/avatar-tables \
       -type f \( -name '*.rs' -o -name '*.toml' -o -name '*.h' -o -name '*.modulemap' \) \
       -not -path '*/target/*' | LC_ALL=C sort | while read -r file; do
    printf '%s  %s\n' "$(shasum -a 256 < "$file" | cut -d' ' -f1)" "$file"
  done
  for file in Cargo.toml Cargo.lock scripts/build-xcframework.sh; do
    printf '%s  %s\n' "$(shasum -a 256 < "$file" | cut -d' ' -f1)" "$file"
  done
} | shasum -a 256 | cut -d' ' -f1
