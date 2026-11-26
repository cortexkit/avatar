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
# Inputs: every file of every crate the core links (source, manifests, C
# headers, and any data file a crate embeds with include_str! or
# include_bytes!), the workspace manifest and lockfile, the build script, and
# the compiler version. File CONTENT is hashed rather than git object ids, so
# an uncommitted edit counts too.
set -euo pipefail
cd "$(dirname "$0")/.."

# The crates the core links are avatar-ffi and every workspace crate it
# reaches through path dependencies of normal or build kind, read from the
# workspace manifests rather than listed here, so a crate added to the core
# is hashed without editing this script. Dev-dependencies never reach the
# binary, and a crate outside the closure, such as avatar-export, cannot
# change it. --no-deps reads only the workspace manifests, so it needs no
# registry index or network. The crates are printed relative to the
# repository root: the hashed file names must not depend on where the
# checkout lives, or two checkouts of the same source would disagree.
linked_crates=$(cargo metadata --no-deps --locked --offline --format-version 1 | python3 -c '
import json, os, sys

metadata = json.load(sys.stdin)
root = os.path.realpath(metadata["workspace_root"])
packages = {
    os.path.realpath(os.path.dirname(package["manifest_path"])): package
    for package in metadata["packages"]
}
pending = [path for path, package in packages.items() if package["name"] == "avatar-ffi"]
if len(pending) != 1:
    sys.exit("avatar-ffi is not a workspace member")
linked = set()
while pending:
    path = pending.pop()
    if path in linked:
        continue
    linked.add(path)
    for dependency in packages[path]["dependencies"]:
        if dependency["kind"] not in (None, "build") or dependency.get("path") is None:
            continue
        target = os.path.realpath(dependency["path"])
        if target not in packages:
            sys.exit("%s depends on %s, which is not a workspace member"
                     % (packages[path]["name"], dependency["name"]))
        pending.append(target)
for name in sorted(os.path.relpath(path, root) for path in linked):
    if name == ".." or name.startswith("../") or "\n" in name:
        sys.exit("unexpected crate directory: %r" % name)
    print(name)
')
[[ -n $linked_crates ]] || { echo "No linked crates found" >&2; exit 1; }

{
  rustc --version
  while IFS= read -r crate; do
    find "$crate" -type d -name target -prune -o -type f -print
  done <<< "$linked_crates" | LC_ALL=C sort -u | while IFS= read -r file; do
    digest=$(shasum -a 256 < "$file" | cut -d' ' -f1) || exit 1
    printf '%s  %s\n' "$digest" "$file"
  done
  for file in Cargo.toml Cargo.lock scripts/build-xcframework.sh; do
    digest=$(shasum -a 256 < "$file" | cut -d' ' -f1) || exit 1
    printf '%s  %s\n' "$digest" "$file"
  done
} | shasum -a 256 | cut -d' ' -f1
