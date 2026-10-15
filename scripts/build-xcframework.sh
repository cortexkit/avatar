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
trap 'rm -rf "$scratch"' EXIT
# Keep unwinding enabled so the C boundary can turn panics into statuses.
export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_PROFILE_RELEASE_PANIC=unwind
export IPHONEOS_DEPLOYMENT_TARGET=13.0
export MACOSX_DEPLOYMENT_TARGET=11.0
args=()
for target in "${targets[@]}"; do
    cargo build --locked --release -p avatar-ffi --target "$target" \
        --target-dir "$root/target/avatar-core"
    mkdir -p "$scratch/$target"
    archive="$scratch/$target/libavatar_ffi.a"
    # Preserve external symbols needed by the static linker, removing debug
    # and local symbols from the shipped archives.
    xcrun strip -S -x -o "$archive" \
        "$root/target/avatar-core/$target/release/libavatar_ffi.a"
    args+=(-library "$archive" -headers "$root/crates/avatar-ffi/include")
done
xcodebuild -create-xcframework "${args[@]}" -output "$scratch/AvatarCore.xcframework"
output="$root/ports/swift/AvatarCore.xcframework"
rm -rf "$output"
mv "$scratch/AvatarCore.xcframework" "$output"
echo "Built static-library xcframework: $output"
du -sh "$output"
