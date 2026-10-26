# Architecture

## Pattern Overview

**Overall:** Single-Source Deterministic Graphics Pipeline: each creature type has one generator, written in Rust, and every client draws the render tree it produces. The same genome, type, version and mood always produce the same bytes, on every platform.

**Key Characteristics:**
- **Deterministic Pure Pipeline:** Maps fixed-size bitstrings (DNA) through typed phenotype parameters to intermediate geometry render trees and SVG text output.
- **Platform-Invariant Arithmetic:** A creature must be byte-identical on every platform. `creature.classic` gets this from pinned integer arithmetic (`i64` milliunits, `Q32.32` fixed point, lookup tables, one half-away-from-zero rounding helper). `creature.wild` exists only in Rust, so it uses `f64` restricted to operations IEEE 754 makes exact everywhere (`+ - * /`, `sqrt`, comparisons, rounding), with trigonometry rebuilt from those in `src/wild/math.rs`. A lint (`crates/clippy.toml`) refuses maths-library calls, the one non-portable layer. Measured: wild trees are byte-identical on macOS arm64, Linux arm64 and Linux x86_64 (`scripts/wild-cross-platform.sh`).
- **Single Generator, Decoding Consumers:** Generation lives in Rust (`crates/`). Consumers receive a resolved `RenderTree` and draw it rather than reimplementing the generator, because comparing independent implementations against a corpus proves agreement on a finite sample of a 2^8192 input space and cannot prove equivalence. TypeScript (`ports/ts/`) and Swift (`ports/swift/`) each carry the tree type, a fail-closed JSON decoder, and byte gates. Every creature type, classic included, is implemented once, in Rust. The iOS app runs that Rust itself through an embedded static library. See `docs/design/011`.
- **Render-Time Mood Decoupling:** DNA and phenotype encode identity, body geometry, face composition, and color palette. Agent mood is provided at render time to modify facial feature transforms without altering identity.
- **Golden Vector Conformance:** Golden vector tests in `vectors/`, run by `scripts/check.sh` on every push, at four boundaries: DNA to phenotype JSON (`phenotype/`), phenotype to SVG (`svg/`), tree to canonical JSON (`tree/`), and tree to SVG through the generic serializer (`tree-svg/`). The last two pin the transport a consumer decodes and the rendering every consumer must reproduce.

## Layers

**Identity & Genome Layer:**
- Purpose: Derives seat identities, hashes inputs into deterministic bit genomes, and parses raw bit ranges.
- Location: `crates/avatar-genome/src/`
- Contains: Bitstring parsing (`genome.rs`), identity hashing (`identity.rs`), bit field decoders (`decode.rs`).
- Depends on: `crates/avatar-phenotype`
- Used by: `crates/avatar-svg`, `crates/avatar-export`

**Typed Phenotype Layer:**
- Purpose: Defines typed integer representations of visual attributes independent of graphics renderers.
- Location: `crates/avatar-phenotype/src/`
- Contains: Phenotype data structure, milliunit wrappers (`Milli`), fixed-point types (`Q32`), binary angle measurements (`Bam16`), color definitions (`Rgb`), JSON serialization helpers.
- Depends on: Standard library, `serde_json`
- Used by: `crates/avatar-genome`, `crates/avatar-svg`, `crates/avatar-export`, `crates/avatar-vectors`

**Pinned Lookup & Numeric Primitive Layer:**
- Purpose: Replaces platform math libraries (`libm`) with deterministic integer lookup tables and rounding primitives.
- Location: `crates/avatar-tables/src/`
- Contains: Quarter-wave sine table (`SIN_QUARTER`), committed easing tables (`EASE_*`), integer division rounding helper (`div_round_half_away`), table minting binary (`src/bin/mint.rs`).
- Depends on: `blake3`
- Used by: `crates/avatar-svg`

**Geometry Assembly & Render Tree Layer:**
- Purpose: Constructs a resolution-independent, structured tree of geometric primitives and paint definitions.
- Location: `crates/avatar-svg/src/`
- Contains: The `creature.classic` generator (`geometry.rs`, `face.rs`, `eyes.rs`, `brows.rs`, `mouth.rs`, `lashes.rs`, `pattern.rs`, `mood.rs`), the `creature.wild` generator (`src/wild/`: radial-spline bodies, extremities, signed-distance-field merging, Voronoi patterns, face placement, moods), tree data structure (`tree.rs`), canonical tree JSON (`tree_json.rs`), dispatch engine (`dispatch.rs`).
- Tree contract: a primitive's `label` is a diagnostic name with no stability promise; its `role` is what consumers may key behaviour on. So far the only role is `ground_shadow`, which a client with no ground plane hides.
- Depends on: `crates/avatar-phenotype`, `crates/avatar-tables`
- Used by: `crates/avatar-export`, `crates/avatar-vectors`

**SVG Serializer & Output Layer:**
- Purpose: Converts abstract `RenderTree` primitives into canonical SVG text bytes.
- Location: `crates/avatar-svg/src/serialize.rs`, `crates/avatar-svg/src/tree_serialize.rs`, `ports/ts/src/tree-serialize.ts`
- Contains: Number formatting, XML element emission, attribute ordering, coordinate normalization.
- Depends on: `crates/avatar-svg/src/tree.rs`
- Used by: `crates/avatar-export`, `crates/avatar-vectors`

**Embedded Core Layer:**
- Purpose: Lets a client run the Rust generator itself, so a type that exists only in Rust draws on the device.
- Location: `crates/avatar-ffi/`, built by `scripts/build-xcframework.sh` into the committed `ports/swift/AvatarCore.xcframework` (static libraries for iOS device, iOS simulator and macOS).
- Contains: One C call returning canonical tree JSON, with a distinct status per failure and panics contained at the boundary.
- Guarded by: `ports/swift/AvatarCore.source-hash`; `scripts/check.sh` refuses a committed binary built from different source, because every test builds from source and would pass on a stale binary.
- Used by: `ports/swift/Sources/AvatarKit/EmbeddedCore.swift` (`renderTreeViaCore`).

**CLI & Fleet Export Layer:**
- Purpose: Converts stored genomes (or, for backfill, stable seat IDs) into raster PNG identity assets for agent avatars.
- Location: `crates/avatar-export/src/main.rs`
- Contains: Command-line flag parsing, manifest loading, SVG rendering, `resvg` PNG rasterization.
- Depends on: `crates/avatar-genome`, `crates/avatar-svg`, `resvg`
- Used by: Fleet management and deployment scripts

## Data Flow

**Standard Generation & SVG Rendering Pipeline:**

1. Input stable seat identity string is hashed via BLAKE3 into an 8192-bit genome bitstring — `crates/avatar-genome/src/identity.rs`
2. Genome decoder unpacks bit fields into a typed `Phenotype` struct — `crates/avatar-genome/src/decode.rs`
3. Dispatcher selects generator implementation based on `avatar_type` and `version` — `crates/avatar-svg/src/dispatch.rs`
4. Geometry builder consumes `Phenotype` and render-time `RenderParams` (mood, growth) to assemble a `RenderTree` — `crates/avatar-svg/src/build.rs`
5. Serializer formats `RenderTree` into canonical SVG byte stream — `crates/avatar-svg/src/serialize.rs`

**Consumer pipeline — how a client obtains a tree:**

1. Rust emits a canonical JSON encoding of the tree — `crates/avatar-svg/src/tree_json.rs`. On iOS this Rust runs inside the app: `renderTreeViaCore(genomeBytes:type:version:mood:)` calls the embedded core — `ports/swift/Sources/AvatarKit/EmbeddedCore.swift`, `crates/avatar-ffi/src/lib.rs`
2. Consumer decodes it fail-closed: an unknown tag, a missing field or a non-integer throws, because a silently defaulted primitive draws a wrong creature that looks right — `ports/swift/Sources/AvatarKit/RenderTree.swift`, `ports/ts/src/tree.ts`
3. Consumer draws the primitives, or serializes them to SVG — `ports/ts/src/tree-serialize.ts`

This path lets a new creature type ship without being implemented more than once.

**Fleet PNG Export Pipeline:**

1. For a live agent, the CLI takes the agent's stored genome as `--genome-file <path>` (or `-` for standard input) or `--genome <hex>`, with `--type` and `--version`. A wild genome is 32,768 hex characters, longer than a Windows command line allows, so portable callers use the file form. The registry row is authoritative, and a re-roll may have replaced the genome derived from the seat id. `--stable-id` and `--manifest` remain for the one-time backfill — `crates/avatar-export/src/main.rs`
2. `avatar-export` renders through the same dispatch as every client — `crates/avatar-export/src/main.rs`
3. SVG stream passes to `resvg` pure-Rust engine to rasterize fixed-dimension PNG files — `crates/avatar-export/src/main.rs`. For a stored genome, an `--out` path ending in `.svg` or `.json` writes the SVG or the canonical render-tree JSON instead

**Golden Vector Conformance Gate:**

1. Conformance test reads golden hex DNA vectors from disk — `crates/avatar-vectors/src/lib.rs`
2. Test executes local implementation pipeline to produce phenotype JSON and SVG output
3. Output is byte-compared against expected golden assets in `vectors/phenotype/`, `vectors/svg/`, `vectors/tree/` and `vectors/tree-svg/`
4. Consumer tests decode every committed tree in TS (`ports/ts/tests/render-tree-json.test.ts`) and Swift (`ports/swift/Tests/AvatarKitTests/RenderConformanceTests.swift`) and re-encode it losslessly; TS also serializes every tree against `vectors/tree-svg/` (`ports/ts/tests/tree-svg.test.ts`). Swift proves the embedded core reproduces every classic and wild tree fixture (`ports/swift/Tests/AvatarKitTests/EmbeddedCoreTests.swift`).
5. Run by hand because it needs containers: `scripts/wild-cross-platform.sh` generates the wild corpus on macOS and in Linux arm64 and x86_64 containers and byte-compares them

## Key Abstractions

**Genome (`Genome`):**
- Purpose: Enforces 1024-byte (8192-bit) layout boundary for procedural creature DNA.
- Location: `crates/avatar-genome/src/genome.rs`
- Pattern: Value object / Newtype wrapper around byte slice with error checking.

**Phenotype (`Phenotype`):**
- Purpose: Represents canonical, generator-independent parameters governing creature appearance.
- Location: `crates/avatar-phenotype/src/lib.rs`
- Pattern: Strongly typed struct with sub-structures (`Body`, `Face`, `Palette`, `Pattern`, `MoodResponse`).

**RenderTree (`RenderTree`, `Primitive`):**
- Purpose: Provides intermediate presentation tree separating visual math from output formats.
- Location: `crates/avatar-svg/src/tree.rs`, `ports/swift/Sources/AvatarKit/RenderTree.swift`
- Pattern: Abstract Syntax Tree (AST) composed of shapes, paths, strokes, paints, transforms, and group effects.

**Fixed-Point & Canvas Units (`Milli`, `Q32`, `Bam16`):**
- Purpose: Guarantees numerical parity across platforms by avoiding native floating-point math.
- Location: `crates/avatar-phenotype/src/lib.rs`, `crates/avatar-tables/src/lib.rs`
- Pattern: Pinned integer units (`Milli` = canvas milliunits, `Q32` = Q32.32 fixed point, `Bam16` = 16-bit binary angle measurements).

**Dispatch Engine (`render_tree_for`):**
- Purpose: Decouples stored row type and version markers from implementation routines.
- Location: `crates/avatar-svg/src/dispatch.rs`
- Pattern: Versioned strategy dispatcher returning specific renderer pipeline.

## Entry Points

**Rust Workspace Library:**
- Location: `crates/avatar-svg/src/lib.rs`
- Triggers: Function calls (`render_svg`, `build_render_tree`, `render_tree_for`) from Rust services.
- Responsibilities: Main Rust API surface for phenotype-to-SVG and genome-to-tree rendering.

**Rust PNG Exporter CLI:**
- Location: `crates/avatar-export/src/main.rs`
- Triggers: Binary execution `avatar-export --stable-id <ID> --out <PATH>`.
- Responsibilities: Derives genome from stable ID, renders SVG, and rasterizes PNG files for agent avatars.

**TypeScript Port Library:**
- Location: `ports/ts/src/tree.ts`, `ports/ts/src/tree-serialize.ts`
- Triggers: Direct module imports in Node / Bun.
- Responsibilities: Decode render-tree JSON fail-closed and serialize any tree to generic SVG.

**Swift Package Framework:**
- Location: `Package.swift`, `ports/swift/Sources/AvatarKit/EmbeddedCore.swift`
- Triggers: SwiftPM dependency inclusion and `renderTreeViaCore(genomeBytes:type:version:mood:)` calls.
- Responsibilities: Run the embedded Rust generator for every type on iOS/macOS and decode its resolved render tree. `RenderTreeCache` caches the core path by bytes, type, version, and mood.

## Error Handling

**Strategy:** Fail Closed with Sentinel Error Types.

Rust parsers and renderers return explicit, typed result/error enums (`GenomeError`, `DispatchError`, `RenderError`). Swift exposes embedded-core refusals as `CoreRenderError`; the Swift and TS tree decoders throw on invalid wire data.
Unknown avatar types, unsupported generator versions, or invalid genome bit lengths immediately halt execution and return errors rather than falling back to default visuals or silent failures.

- `GenomeError::InvalidLength`: Returned when input bytes fail genome size verification.
- `DispatchError::UnsupportedType`: Returned when requested avatar type is unknown.
- `DispatchError::UnsupportedVersion`: Returned when requested version is not registered for an avatar type.
- `RenderError::UnsupportedGrowth` / `UnsupportedCanvas`: Returned when render parameters deviate from pinned constants.

## Cross-Cutting Concerns

**Determinism & Numeric Discipline:**
- No generation path calls the platform maths library; `crates/clippy.toml` refuses it under `-D warnings`.
- `creature.wild`: `f64` with IEEE-exact operations only, trigonometry from `src/wild/math.rs`, and Rust's half-away-from-zero `round`. In the wild generator a difference in the fourteenth digit can flip a branch decision, so approximately equal results are not enough.
- `creature.classic`: all trigonometric operations use `avatar_tables::sin_q30` and `cos_q30` backed by committed quarter-wave lookup tables.
- Easing calculations look up pre-computed Q32.32 values from committed tables (`EASE_LINEAR`, `EASE_EXPO`, `EASE_IN_CIRC`, `EASE_IN_QUINT`, `EASE_IN_OUT_CIRC`).
- Signed fixed-point division must use `div_round_half_away` to ensure consistent half-away-from-zero rounding across all platforms.

**Caching & State:**
- The rendering core is stateless and performs no internal caching.
- Native consumers maintain local render tree caches when drawing high-frequency UI frames.

**Storage & Assets:**
- Lookup tables are committed only in `crates/avatar-tables/src/generated/tables.rs`.
- Voronoi pattern mesh data is committed only in `crates/avatar-svg/src/generated/voronoi.rs`.
- Conformance test vectors are stored in `vectors/` (`dna/`, `phenotype/`, `svg/`, `tree/`, `tree-svg/`, `wild/`, `manifest.json`), generated by the `UPDATE_GOLDEN` and `UPDATE_WILD_GOLDEN` test commands in `crates/avatar-vectors` (see STRUCTURE.md) and never edited by hand.
- The embedded core binary is committed as `ports/swift/AvatarCore.xcframework` (about 1 MB zipped), so the iOS app needs no Rust toolchain.
