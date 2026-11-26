# Embedded AvatarCore

`avatar-ffi` is a static-library C boundary around the `avatar` crate's
versioned `render_tree_for` dispatch and canonical `serialize_tree_json`. It
does not implement generation or select a default creature for unsupported
requests.

The committed `ports/swift/AvatarCore.xcframework` contains **static `.a`
archives**, not dynamic frameworks, for arm64 iOS (13+), arm64 iOS simulator,
and arm64 macOS (11+). Consumers need neither Rust nor framework embedding or
code signing. Intel Mac/simulator slices are not included.

## Rebuilding

On macOS with full Xcode and Rust installed:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin
./scripts/build-xcframework.sh
```

The script uses the committed Cargo lockfile, release optimization, fat LTO,
one codegen unit and unwinding (required by the panic boundary). It strips
debug/local symbols while preserving external symbols required by the static
linker, then assembles the archives with `xcodebuild -create-xcframework
-library ... -headers ...`. It prefers `/Applications/Xcode.app` unless
`DEVELOPER_DIR` is supplied, and clears inherited `SDKROOT` so Git hooks cannot
accidentally mix CommandLineTools and Xcode SDKs. Rebuild and commit the binary
whenever the Rust generator, boundary, headers or locked dependencies change.

## Calling from Swift

On the included Apple platforms, every type goes through the embedded generator:

```swift
let tree = try renderTreeViaCore(
    genomeBytes: genomeBytes, type: "creature.classic", version: 2, mood: .happy)
```

The wrapper releases Rust's C string after decoding it with AvatarKit's
fail-closed canonical tree decoder. `CoreRenderError` distinguishes unsupported
type, unsupported version, wrong genome length, unknown mood, invalid argument,
render failure, and contained panic. Unknown statuses and missing success
payloads also fail closed; decoder errors are propagated unchanged.

`include/AvatarCore.h` specifies the C pointer/ownership contract and stable
status numbers. No caller memory is retained. Each successful call returns an
independent allocation, freed **once** by `avatar_string_free` rather than a
foreign allocator. There is no mutable global state or thread affinity.

## Verification

`./scripts/check.sh` includes the crate through `cargo test --workspace` and
runs the Swift suite on macOS. `EmbeddedCoreTests` compares every committed
classic and wild tree against the embedded core using complete
`RenderTree` equality, exercises error mapping, and makes concurrent calls.
The Linux Swift package decodes committed trees without importing AvatarCore;
the binary dependency is platform-conditional.
