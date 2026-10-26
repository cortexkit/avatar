# Codebase Structure

## Directory Layout

```
.
├── Cargo.toml                    # Root Cargo workspace manifest
├── Package.swift                 # SwiftPM package manifest (hoisted to root for git dependencies)
├── ARCHITECTURE.md               # System architecture specification
├── STRUCTURE.md                  # Codebase directory layout and navigation guide
├── README.md                     # Project overview and usage guidelines
├── crates/                       # Canonical Rust implementation (workspace crates)
│   ├── avatar-export/            # PNG exporter CLI binary
│   ├── avatar-ffi/               # C boundary the iOS app calls; built into AvatarCore.xcframework
│   ├── avatar-genome/            # Genome decoding and identity derivation
│   ├── avatar-phenotype/         # Typed phenotype data model and units
│   ├── avatar-svg/               # Every generator (classic, and wild/ in src/wild), render tree, serializers
│   ├── avatar-tables/            # Pinned lookup tables and division rounding logic
│   └── avatar-vectors/           # Golden vector conformance test suite
├── ports/                        # Port implementations in secondary languages
│   ├── swift/                    # Swift AvatarKit and the embedded AvatarCore.xcframework
│   └── ts/                       # TypeScript tree decoder, generic SVG serializer, and Bun tests
├── vectors/                      # Golden vector corpus for cross-language conformance
│   ├── dna/                      # Raw 8192-bit hex DNA files
│   ├── phenotype/                # Canonical decoded phenotype JSON files
│   ├── svg/                      # Pinned golden SVG output files
│   ├── tree/                     # Canonical render-tree JSON, the consumer transport
│   ├── tree-svg/                 # Generic serializer output every consumer must match
│   └── wild/                     # Wild genomes, their manifest, and corpus.tsv (the frozen genome list)
├── scripts/                      # The repository gate, drift checks, xcframework build, cross-platform check
├── docs/                         # Architecture charter and design decision notes
└── playground/                   # Exploratory prototypes and visual probe web app
```

## Directory Purposes

**`crates/avatar-genome/`:**
- Purpose: Converts seat identity strings to genomes and parses genome bitfields.
- Contains: Bitstring unpacking, BLAKE3 identity derivation, raw bit decoding.
- Key files: `crates/avatar-genome/src/lib.rs`, `crates/avatar-genome/src/identity.rs`, `crates/avatar-genome/src/decode.rs`, `crates/avatar-genome/src/genome.rs`.

**`crates/avatar-phenotype/`:**
- Purpose: Defines typed parameter structs and integer unit representations.
- Contains: Strongly typed integer units (`Milli`, `Q32`, `Bam16`, `Rgb`), `Phenotype` struct, Serde JSON implementations.
- Key files: `crates/avatar-phenotype/src/lib.rs`.

**`crates/avatar-tables/`:**
- Purpose: Provides pinned trigonometric lookup tables, easing curves, and integer division primitives.
- Contains: `SIN_QUARTER` trig table, easing arrays (`EASE_*`), `div_round_half_away` function, table mint binary.
- Key files: `crates/avatar-tables/src/lib.rs`, `crates/avatar-tables/src/generated/tables.rs`, `crates/avatar-tables/src/bin/mint.rs`.

**`crates/avatar-svg/`:**
- Purpose: Every creature generator, written once in Rust; builds render trees and serializes them.
- Contains: The `creature.classic` generator (`face.rs`, `eyes.rs`, `brows.rs`, `mouth.rs`, `lashes.rs`, `pattern.rs`, `mood.rs`, `build.rs`), the `creature.wild` generator (`src/wild/`, with its deterministic maths in `src/wild/math.rs`), `RenderTree` and the primitive `role` (`tree.rs`), canonical tree JSON (`tree_json.rs`), type and version dispatch (`dispatch.rs`), the classic SVG serializer (`serialize.rs`) and the generic tree serializer (`tree_serialize.rs`).
- Key files: `crates/avatar-svg/src/lib.rs`, `crates/avatar-svg/src/dispatch.rs`, `crates/avatar-svg/src/tree.rs`, `crates/avatar-svg/src/tree_json.rs`, `crates/avatar-svg/src/wild/mod.rs`.

**`crates/avatar-ffi/`:**
- Purpose: The C boundary to the generator, built as a static library into `ports/swift/AvatarCore.xcframework` so the iOS app runs the Rust generator itself.
- Contains: One render call returning canonical tree JSON, with a distinct status per failure (unknown type, unknown version, wrong genome length, unknown mood, contained panic).
- Key files: `crates/avatar-ffi/src/lib.rs`, `crates/avatar-ffi/include/AvatarCore.h`.

**`crates/avatar-export/`:**
- Purpose: Command-line application rasterizing agent avatars to PNG images for fleet deployment.
- Contains: Main CLI routine using `resvg` for pure Rust rasterization.
- Key files: `crates/avatar-export/src/main.rs`, `crates/avatar-export/Cargo.toml`.

**`crates/avatar-vectors/`:**
- Purpose: Test harness validating Rust pipeline outputs against stored golden vectors.
- Contains: Test fixtures reading `vectors/` corpus files and asserting byte-exact rendering.
- Key files: `crates/avatar-vectors/src/lib.rs`.

**`ports/ts/`:**
- Purpose: TypeScript consumer for Web and Node: draws any type from its render tree.
- Contains: The fail-closed tree decoder and generic serializer (`src/tree.ts`, `src/tree-serialize.ts`), numeric formatting helpers, and Bun conformance tests.
- Key files: `ports/ts/src/tree.ts`, `ports/ts/src/tree-serialize.ts`, `ports/ts/src/tree-format.ts`, `ports/ts/tests/render-tree-json.test.ts`, `ports/ts/tests/tree-svg.test.ts`.

**`ports/swift/`:**
- Purpose: Swift `AvatarKit` framework for iOS and macOS client applications.
- Contains: `AvatarKit` target, the committed `AvatarCore.xcframework` (the embedded Rust generator) with `AvatarCore.source-hash` recording what it was built from, the fail-closed tree decoder, numeric wire types, host-owned tree cache, and SwiftPM test suite.
- Key files: `ports/swift/Sources/AvatarKit/EmbeddedCore.swift` (`renderTreeViaCore`, the path for every type), `ports/swift/Sources/AvatarKit/RenderTree.swift`, `ports/swift/Sources/AvatarKit/RenderTreeJSON.swift`, `ports/swift/Sources/AvatarKit/RenderTreeCache.swift`, `ports/swift/Tests/AvatarKitTests/EmbeddedCoreTests.swift`, `ports/swift/Tests/AvatarKitTests/CorpusFixtures.swift`.

**`vectors/`:**
- Purpose: Shared golden vector corpus pinning Rust generation and the tree transport and generic SVG consumed by the ports.
- Contains: Hex DNA vectors, JSON phenotype expectations, golden SVG files, vector manifest.
- Key files: `vectors/manifest.json`, `vectors/contact-sheet.html`, `vectors/dna/`, `vectors/phenotype/`, `vectors/svg/`, `vectors/tree/`, `vectors/tree-svg/`, `vectors/wild/` (wild genomes and `corpus.tsv`).
- Generated, never hand-edited: test commands in `crates/avatar-vectors/src/lib.rs` write these files and print the size and hash pins to paste back into that file (see "New Golden Conformance Vector" below).
- Note: `manifest.json`'s `corpus_revision` counts re-mints. It is not the version a registry row carries, which lives in `crates/avatar-svg/src/dispatch.rs`.

**`scripts/`:**
- Purpose: The repository gate and the checks it runs, plus tools run by hand.
- Contains: The gate (`check.sh`, run by the pre-push hook; there is no automatic CI), the browser-lab drift checks (`drift-check.sh`, `const-drift.py`), the xcframework build and its staleness hash (`build-xcframework.sh`, `xcframework-source-hash.sh`), the cross-platform byte check for `creature.wild` (`wild-cross-platform.sh` with `wild-port/`), and fleet avatar verification (`verify-fleet-avatars.sh`).
- Key files: `scripts/check.sh`, `scripts/build-xcframework.sh`, `scripts/wild-cross-platform.sh`.

## Key File Locations

**Entry Points:**
- Canonical Rust Library: `crates/avatar-svg/src/lib.rs`
- PNG Export CLI Tool: `crates/avatar-export/src/main.rs`
- TypeScript Port Library: `ports/ts/src/tree.ts`, `ports/ts/src/tree-serialize.ts`
- Swift Framework Library: `ports/swift/Sources/AvatarKit/EmbeddedCore.swift` renders any type through the embedded core; `ports/swift/Sources/AvatarKit/RenderTree.swift` decodes its tree.
- Embedded Core C Boundary: `crates/avatar-ffi/src/lib.rs`
- SwiftPM Manifest: `Package.swift`

**Configuration:**
- Workspace Cargo Manifest: `Cargo.toml`
- TypeScript Package Manifest: `ports/ts/package.json`
- TypeScript Compiler Options: `ports/ts/tsconfig.json`

**Core Logic:**
- Bit Decoding & Identity: `crates/avatar-genome/src/decode.rs`, `crates/avatar-genome/src/identity.rs`
- Fixed-Point & Tables (classic): `crates/avatar-tables/src/lib.rs`
- Deterministic Floating-Point Maths (wild): `crates/avatar-svg/src/wild/math.rs`, enforced by `crates/clippy.toml`
- Geometry & Tree Building: `crates/avatar-svg/src/build.rs` and `geometry.rs` (classic), `crates/avatar-svg/src/wild/` (wild)
- Version & Type Dispatch: `crates/avatar-svg/src/dispatch.rs`
- SVG Serialization: `crates/avatar-svg/src/serialize.rs`

**Tests & Conformance:**
- Rust Golden Vector Suite: `crates/avatar-vectors/src/lib.rs`
- TypeScript Tree Corpus Suite: `ports/ts/tests/render-tree-json.test.ts`, `ports/ts/tests/tree-svg.test.ts`
- Swift Tree Corpus Suite: `ports/swift/Tests/AvatarKitTests/RenderConformanceTests.swift`, `ports/swift/Tests/AvatarKitTests/EmbeddedCoreTests.swift`
- Swift Embedded Core Suite: `ports/swift/Tests/AvatarKitTests/EmbeddedCoreTests.swift`
- Cross-Platform Byte Check (by hand; needs containers): `scripts/wild-cross-platform.sh`
- The Repository Gate: `scripts/check.sh`

## Naming Conventions

**Files:**
- Rust Source: `snake_case.rs` (e.g., `crates/avatar-svg/src/serialize.rs`)
- TypeScript Source: `camelCase.ts` or `kebab-case.test.ts` (e.g., `ports/ts/src/tree.ts`, `ports/ts/tests/tree-svg.test.ts`)
- Swift Source: `PascalCase.swift` (e.g., `ports/swift/Sources/AvatarKit/RenderTree.swift`)
- Spec Data & Generation: `snake_case.rs` or `kebab-case` (e.g., `crates/avatar-tables/src/bin/mint.rs`, `vectors/manifest.json`)

**Directories:**
- Cargo Workspace Crates: `kebab-case` under `crates/` (e.g., `crates/avatar-genome/`)
- Language Ports: `lowercase` under `ports/` (e.g., `ports/ts/`, `ports/swift/`)
- Test Vector Categories: `lowercase` under `vectors/` (e.g., `vectors/dna/`, `vectors/svg/`)

## Where to Add New Code

**New Avatar Type or Generator Version:**
1. Implement the generator once, in Rust, as its own module under `crates/avatar-svg/src/` (as `src/wild/` does). Do NOT write it again in Swift or TypeScript: consumers draw any type from its render tree, and the iOS app runs this Rust through the embedded core. Comparing independent implementations proves agreement only on a sample (`docs/design/011`).
2. Keep it deterministic: no maths-library calls on the generation path (the `crates/clippy.toml` lint refuses them); use `src/wild/math.rs` or `avatar-tables`, `sqrt` and basic arithmetic.
3. Register it in `crates/avatar-svg/src/dispatch.rs`, which checks type, then version, then the genome length that type needs. That order is a contract: a client draws an unknown type blank but reports a malformed row.
4. A new version adds a dispatch arm and never changes what an existing version draws, because every registry row renders at the version stored with it (`docs/design/012`). A new look is a new type.
5. Add a golden corpus for it, rebuild the xcframework with `scripts/build-xcframework.sh`, and run `scripts/wild-cross-platform.sh` or an equivalent cross-platform byte check.

**New Numeric Table or Math Function:**
1. Add table generation or mathematical definition to `crates/avatar-tables/src/bin/mint.rs`.
2. Re-mint committed Rust table code via `cargo run -p avatar-tables --bin mint`.
3. Export table wrappers and helpers in `crates/avatar-tables/src/lib.rs`.
4. Tables stay Rust-only; do not add TS or Swift copies.

**New Geometry Feature Pass:**
1. Geometry belongs to a type: add it in Rust, inside that type's module.
2. Changing what an existing type draws changes agents' creatures, so it ships as a new version or a new type, never as an edit (`docs/design/012`). A type no agent has been switched to yet may still be edited in place, because no agent's creature changes.
3. All geometry changes live in Rust only; the ports consume render trees and gate their decoding against the golden corpus.

**New Golden Conformance Vector:**
1. Never write corpus files by hand; the mint generates them.
2. For a `creature.classic` vector: add it to the list in `crates/avatar-vectors/src/lib.rs`, then run `UPDATE_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --exact --nocapture`.
3. For a `creature.wild` vector: add a project id and its moods to `vectors/wild/corpus.tsv`, then run `UPDATE_WILD_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --exact --nocapture`, which leaves classic untouched.
4. Paste the printed pins back into `crates/avatar-vectors/src/lib.rs`, open `vectors/contact-sheet.html` and look at every new cell, and rebuild the xcframework if generator source changed.

**New Tests:**
- Rust unit tests: Co-located in source files inside `#[cfg(test)] mod tests`.
- Tree consumer conformance: `crates/avatar-vectors/src/lib.rs`, `ports/ts/tests/render-tree-json.test.ts`, `ports/ts/tests/tree-svg.test.ts`, `ports/swift/Tests/AvatarKitTests/RenderConformanceTests.swift`, and `ports/swift/Tests/AvatarKitTests/EmbeddedCoreTests.swift`.
- Every new check must be shown failing on a deliberately introduced defect before it is trusted, and that defect must exercise the condition the check exists to catch (a pipe bug that only appears on large output needs a large-output test, not a short one).
