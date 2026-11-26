#!/usr/bin/env bash
# Ship static archives so consumers need neither Rust nor an embedded/signable
# dynamic framework (including in notification extensions).
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

[[ $(uname -s) == Darwin ]] || { echo "Xcode on macOS is required" >&2; exit 1; }
# Resolve Xcode independently of the shell: Git hooks can inherit a
# CommandLineTools SDKROOT that is incompatible with Xcode's compiler.
developer_dir=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
[[ -d $developer_dir ]] || developer_dir=$(xcode-select -p)
export DEVELOPER_DIR=$developer_dir
unset SDKROOT
xcodebuild -version
xcrun --sdk iphoneos --show-sdk-path
xcrun --sdk iphonesimulator --show-sdk-path

targets=(aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin)
installed=$(rustup target list --installed)
for target in "${targets[@]}"; do
    if [[ $'\n'"$installed"$'\n' != *$'\n'"$target"$'\n'* ]]; then
        echo "Missing Rust target; run: rustup target add $target" >&2
        exit 1
    fi
done

mkdir -p "$root/target"
scratch=$(mktemp -d "$root/target/xcframework.XXXXXX")
# macOS /bin/bash 3.2 exits with status 0 through an EXIT trap when
# `set -eu` aborts on an unbound variable, so a build that stopped halfway
# would report success. Any exit before `finished=1` is a failure.
finished=0
cleanup() {
    local status=$?
    rm -rf "$scratch"
    if [[ $status -eq 0 && $finished -ne 1 ]]; then
        status=1
    fi
    exit "$status"
}
trap cleanup EXIT
# Keep unwinding enabled so the C boundary can turn panics into statuses.
export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_PROFILE_RELEASE_PANIC=unwind
export IPHONEOS_DEPLOYMENT_TARGET=13.0
export MACOSX_DEPLOYMENT_TARGET=11.0
# Keep build-machine paths out of the shipped archives. rustc embeds the
# source path of every panic location and of generic code it instantiates,
# and those paths start with the cargo home (dependency sources), the
# rustup home (the standard library) or this checkout. Each is rewritten to
# a fixed name, so the archives hold no user's home directory and two
# machines building the same source produce the same bytes. When several
# prefixes match, rustc applies the last one given, so the more specific
# toolchain homes follow the checkout in case one lives inside it.
# CARGO_ENCODED_RUSTFLAGS separates flags with 0x1f, so a path containing a
# space survives, and it overrides any RUSTFLAGS the caller's shell sets.
cargo_home=${CARGO_HOME:-$HOME/.cargo}
rustup_home=${RUSTUP_HOME:-$(rustup show home)}
unset RUSTFLAGS
sep=$'\x1f'
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$root=/avatar$sep--remap-path-prefix=$cargo_home=/cargo$sep--remap-path-prefix=$rustup_home=/rustup"
args=()
for target in "${targets[@]}"; do
    cargo build --locked --release -p avatar-ffi --target "$target" \
        --target-dir "$root/target/avatar-core"
    mkdir -p "$scratch/$target"
    archive="$scratch/$target/libavatar_ffi.a"
    # Preserve external symbols needed by the static linker, removing debug
    # and local symbols from the shipped archives. strip rewrites the
    # archive's symbol table with the current time as its date, so two
    # builds of the same source differed in exactly those bytes, and every
    # rebuild committed a changed binary. ZERO_AR_DATE makes it write zero,
    # as rustc already does for every other member.
    ZERO_AR_DATE=1 xcrun strip -S -x -o "$archive" \
        "$root/target/avatar-core/$target/release/libavatar_ffi.a"
    args+=(-library "$archive" -headers "$root/crates/avatar-ffi/include")
done
xcodebuild -create-xcframework "${args[@]}" -output "$scratch/AvatarCore.xcframework"
# xcodebuild lists the slices in Info.plist in a different order on each
# run, so an unchanged build still produces a diff. Sort them: a binary
# diff that appears on every rebuild teaches reviewers to skip binary
# diffs, which is exactly where a stale build would hide.
python3 - "$scratch/AvatarCore.xcframework/Info.plist" <<'PY'
import plistlib, sys
path = sys.argv[1]
with open(path, "rb") as handle:
    plist = plistlib.load(handle)
plist["AvailableLibraries"].sort(key=lambda library: library["LibraryIdentifier"])
with open(path, "wb") as handle:
    plistlib.dump(plist, handle, sort_keys=True)
PY

# Record the build inputs and the resulting libraries and headers so the gate
# can spot a changed source or a stale slice without rebuilding the framework.
bash "$root/scripts/xcframework-source-hash.sh" > "$scratch/source-hash"
bash "$root/scripts/xcframework-artifact-hashes.sh" "$scratch/AvatarCore.xcframework" \
    > "$scratch/artifact-hashes"
output="$root/ports/swift/AvatarCore.xcframework"
rm -rf "$output"
mv "$scratch/AvatarCore.xcframework" "$output"
mv "$scratch/source-hash" "$root/ports/swift/AvatarCore.source-hash"
mv "$scratch/artifact-hashes" "$root/ports/swift/AvatarCore.artifact-hashes"
echo "Built static-library xcframework: $output"
du -sh "$output"
finished=1
