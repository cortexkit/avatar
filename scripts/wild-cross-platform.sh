#!/usr/bin/env bash
# Run separately from check.sh: both Linux architectures need a container host.
#
#   scripts/wild-cross-platform.sh
#       Mint every type's corpus trees, wild's 25 and each newer type's 18,
#       natively on macOS and in Linux arm64 and x86_64 containers, and
#       compare them byte for byte, one type at a time: each platform's
#       trees with the macOS mint, and the macOS mint with the type's
#       committed trees. Trees must be identical everywhere.
#   scripts/wild-cross-platform.sh image [WINDOWS_HASHES [WINDOWS_DUMP]]
#       Draw every golden image in vectors/image on the same three platforms
#       with the image-hashes example, and compare the SHA-256 of each
#       image's RGBA pixels with its golden. Where a hash differs, the
#       platform's dumped image is measured against the golden, and passes
#       if no channel is more than 8/255 off on at most max(1, 0.1%) of its
#       pixels (scripts/wild-port/compare_images.py). Images, unlike trees,
#       may differ slightly between platforms, because the rasterizer calls
#       the platform's maths library. The report counts, for each platform,
#       the images identical to their golden and those only within the
#       tolerance.
#
#       Windows has no container here, so a Windows x86_64 machine runs the
#       example by hand from a checkout of the same commit:
#           cargo run --locked -p cortexkit-avatar --example image-hashes -- --dump DIR > HASHES
#       Pass HASHES, and DIR if it was written, to compare Windows in the
#       same run. Without them the run reports Windows as unmeasured.
set -euo pipefail
usage() {
    echo "usage: $0 [image [WINDOWS_HASHES [WINDOWS_DUMP]]]" >&2
    exit 2
}
mode=${1:-trees}
windows_hashes=
windows_dump=
case $mode:$# in
    trees:0 | image:1) ;;
    image:2 | image:3)
        # Resolve the paths before changing directory, so relative ones work.
        [[ -f $2 ]] || { echo "No such file: $2" >&2; exit 2; }
        windows_hashes="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
        if [[ $# -eq 3 ]]; then
            [[ -d $3 ]] || { echo "No such directory: $3" >&2; exit 2; }
            windows_dump=$(cd "$3" && pwd)
        fi
        ;;
    *) usage ;;
esac
cd "$(dirname "$0")/.."
root=$PWD
[[ $(uname -s) == Darwin ]] || { echo "The reference run must be native macOS" >&2; exit 1; }
command -v docker >/dev/null
command -v python3 >/dev/null
# Evidence goes under the git-ignored .cortexkit/, one directory per run.
logs="$root/.cortexkit/internal-docs/cross-platform"
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

if [[ $mode == image ]]; then
    # The producer draws each case in vectors/image/manifest.json with
    # avatar::render_image and prints the SHA-256 of its RGBA pixels. --dump
    # also writes every image as a PNG, so a differing hash can be measured
    # against its golden.
    host="macos-$(uname -m)"
    mkdir "$run/$host"
    cargo run --locked -j 4 -p cortexkit-avatar --example image-hashes -- \
        --dump "$run/$host/dump" >"$run/$host/image.sha256"
    for arch in arm64 x86_64; do
        mkdir "$run/linux-$arch"
        # Source is read-only. Cargo artifacts live only in the container's
        # /tmp; /out is the sole writable host mount.
        docker run --rm --platform "linux/${arch/x86_64/amd64}" --cpus 4 --memory 14g \
            --mount "type=bind,source=$root,target=/src,readonly" \
            --mount "type=bind,source=$run/linux-$arch,target=/out" \
            -e CARGO_TARGET_DIR=/tmp/image-target -e CARGO_INCREMENTAL=0 \
            rust:1.99 bash -euc '
                echo "Container architecture: $(uname -m)"
                case "$1:$(uname -m)" in arm64:aarch64|x86_64:x86_64) ;; *) exit 1 ;; esac
                rustc --version --verbose
                cargo run --manifest-path /src/Cargo.toml --locked -j 4 -p cortexkit-avatar \
                    --example image-hashes -- --dump /out/dump >/out/image.sha256
            ' bash "$arch"
    done
    runs=("$host" "$run/$host/image.sha256" "$run/$host/dump")
    for arch in arm64 x86_64; do
        runs+=("linux-$arch" "$run/linux-$arch/image.sha256" "$run/linux-$arch/dump")
    done
    if [[ -n $windows_hashes ]]; then
        # compare_images.py takes - for a producer run without --dump.
        runs+=(windows-x86_64 "$windows_hashes" "${windows_dump:--}")
    fi
    status=0
    python3 scripts/wild-port/compare_images.py "$root" "${runs[@]}" || status=$?
    echo "Platforms compared: $host, linux-arm64, linux-x86_64${windows_hashes:+, windows-x86_64}"
    [[ -n $windows_hashes ]] || echo "windows-x86_64: unmeasured (no Windows producer output was given)"
    if [[ $status -ne 0 ]]; then
        echo "Cross-platform image comparison FAILED; dumps kept as evidence; full log: $run/output.log"
        exit "$status"
    fi
    # Each dump holds tens of megabytes of uncompressed PNGs. Once every
    # platform passes, the report above already gives each image that
    # differs from its golden, with its largest channel difference and the
    # number of pixels that differ.
    rm -r "$run/$host/dump" "$run/linux-arm64/dump" "$run/linux-x86_64/dump"
    echo "Cross-platform image comparison passed; full log: $run/output.log"
    exit 0
fi

# The mint renders through the avatar crate, the one a Rust host depends on.
# Its library is named avatar, so cargo writes libavatar.rlib. It writes one
# directory per type, because the corpora share file names.
cargo build --locked -j 4 -p cortexkit-avatar --lib
rustc --edition=2021 scripts/wild-port/mint.rs \
    --extern "avatar=$CARGO_TARGET_DIR/debug/libavatar.rlib" \
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
            cargo build --manifest-path /src/Cargo.toml --locked -j 4 -p cortexkit-avatar --lib
            rustc --edition=2021 /src/scripts/wild-port/mint.rs \
                --extern avatar=/tmp/wild-target/debug/libavatar.rlib \
                -L dependency=/tmp/wild-target/debug/deps -o /tmp/wild-mint
            /tmp/wild-mint /src /out/trees
        ' bash "$arch"
done
python3 scripts/wild-port/compare.py platforms "$root" \
    "$run/macos" "$run/linux-arm64/trees" "$run/linux-amd64/trees"
echo "Cross-platform tree comparison passed; full log: $run/output.log"
