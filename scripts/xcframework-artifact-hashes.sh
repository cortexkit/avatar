#!/usr/bin/env bash
# Print a sorted inventory of hashes of the shipped libraries and headers.
set -euo pipefail
framework=${1:-"$(dirname "$0")/../ports/swift/AvatarCore.xcframework"}
cd "$framework"

for slice in ios-arm64 ios-arm64-simulator macos-arm64; do
    for file in libavatar_ffi.a Headers/AvatarCore.h Headers/module.modulemap; do
        if [[ ! -f "$slice/$file" || -L "$slice/$file" ]]; then
            echo "AvatarCore.xcframework is missing $slice/$file" >&2
            exit 1
        fi
    done
    shasum -a 256 "$slice/libavatar_ffi.a"
    find "$slice/Headers" -type f -print | LC_ALL=C sort | while IFS= read -r file; do
        shasum -a 256 "$file" || exit 1
    done
done
