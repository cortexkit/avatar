#!/usr/bin/env bash
# Run separately from check.sh: both Linux architectures need a container host.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
[[ $(uname -s) == Darwin ]] || { echo "The reference mint must run natively on macOS" >&2; exit 1; }
command -v docker >/dev/null
command -v python3 >/dev/null
logs="$root/.cortexkit/internal-docs/s8-final"
mkdir -p "$logs"
run=$(mktemp -d "$logs/cross-platform.XXXXXX")
exec > >(tee "$run/output.log") 2>&1
echo "Evidence directory: $run"
git rev-parse HEAD
uname -sm
rustc --version --verbose
[[ $(rustc --version) == 'rustc 1.99.'* ]] || { echo "Use Rust 1.99 for the reference mint"; exit 1; }
docker version
# The mint links against debug/deps inside CARGO_TARGET_DIR. A Cargo build-dir
# setting moves deps elsewhere, so clear it to keep that layout.
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER CARGO_BUILD_RUSTC_WRAPPER CARGO_BUILD_BUILD_DIR
export CARGO_INCREMENTAL=0
# One build directory shared by every run: cargo rebuilds what changed. A
# directory per run left a full debug build behind on every run.
CARGO_TARGET_DIR="$root/target/wild-cross-platform"
export CARGO_TARGET_DIR
cargo build --locked -j 4 -p cortexkit-avatar-svg --lib
rustc --edition=2021 scripts/wild-port/mint.rs \
    --extern "avatar_svg=$CARGO_TARGET_DIR/debug/libavatar_svg.rlib" \
    -L "dependency=$CARGO_TARGET_DIR/debug/deps" -o "$CARGO_TARGET_DIR/wild-mint"
"$CARGO_TARGET_DIR/wild-mint" "$root" "$run/macos"
for arch in arm64 amd64; do
    mkdir "$run/linux-$arch"
    # Source is read-only. Cargo artifacts live only in the container's /tmp;
    # /out is the sole writable host mount and contains only this run's output.
    docker run --rm --platform "linux/$arch" --cpus 4 --memory 14g \
        --mount "type=bind,source=$root,target=/src,readonly" \
        --mount "type=bind,source=$run/linux-$arch,target=/out" \
        -e CARGO_TARGET_DIR=/tmp/wild-target -e CARGO_INCREMENTAL=0 \
        rust:1.99 bash -euc '
            echo "Container architecture: $(uname -m)"
            case "$1:$(uname -m)" in arm64:aarch64|amd64:x86_64) ;; *) exit 1 ;; esac
            rustc --version --verbose
            cargo build --manifest-path /src/Cargo.toml --locked -j 4 -p cortexkit-avatar-svg --lib
            rustc --edition=2021 /src/scripts/wild-port/mint.rs \
                --extern avatar_svg=/tmp/wild-target/debug/libavatar_svg.rlib \
                -L dependency=/tmp/wild-target/debug/deps -o /tmp/wild-mint
            /tmp/wild-mint /src /out/trees
        ' bash "$arch"
done
python3 scripts/wild-port/compare.py platforms "$root" \
    "$run/macos" "$run/linux-arm64/trees" "$run/linux-amd64/trees"
echo "Cross-platform comparison passed; full log: $run/output.log"
