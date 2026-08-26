# Architecture

## Pattern Overview

**Overall:** Fixed-Point Multi-Language Deterministic Graphics Pipeline

**Key Characteristics:**
- **Deterministic Pure Pipeline:** Maps fixed-size bitstrings (DNA) through typed phenotype parameters to intermediate geometry render trees and SVG text output.
- **Arithmetic Invariance across Languages:** Pinned integer arithmetic (`i64` milliunits and `Q32.32` fixed point), custom lookup tables, and a single half-away-from-zero rounding helper eliminate floating-point non-determinism across Rust, TypeScript, and Swift.
- **Canonical Rust Core with Mirror Ports:** Canonical implementation lives in Rust workspace crates (`crates/`), mirrored byte-for-byte in TypeScript (`ports/ts/`) and Swift (`ports/swift/`).
- **Render-Time Mood Decoupling:** DNA and phenotype encode identity, body geometry, face composition, and color palette. Agent mood is provided at render time to modify facial feature transforms without altering identity.
- **Golden Vector Conformance:** Enforces output parity across languages at two boundaries (phenotype JSON and rendered SVG) via CI-gated golden vector tests in `vectors/`.

## Layers

**Identity & Genome Layer:**
- Purpose: Derives seat identities, hashes inputs into deterministic bit genomes, and parses raw bit ranges.
- Location: `crates/avatar-genome/src/`
- Contains: Bitstring parsing (`genome.rs`), identity hashing (`identity.rs`), bit field decoders (`decode.rs`).
- Depends on: `crates/avatar-phenotype`
- Used by: `crates/avatar-svg`, `crates/avatar-export`, `ports/ts/src/genome.ts`, `ports/swift/Sources/AvatarKit/`

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
- Used by: `crates/avatar-svg`, `ports/ts/src/tables.ts`, `ports/swift/Sources/AvatarKit/Generated/GeneratedTables.swift`

**Geometry Assembly & Render Tree Layer:**
- Purpose: Constructs a resolution-independent, structured tree of geometric primitives and paint definitions.
- Location: `crates/avatar-svg/src/`
- Contains: Geometry generators for body, face, eyes, brows, mouth, lashes, patterns, and mood modulation (`geometry.rs`, `face.rs`, `eyes.rs`, `brows.rs`, `mouth.rs`, `lashes.rs`, `pattern.rs`, `mood.rs`), tree data structure (`tree.rs`), dispatch engine (`dispatch.rs`).
- Depends on: `crates/avatar-phenotype`, `crates/avatar-tables`
- Used by: `crates/avatar-export`, `crates/avatar-vectors`, `ports/ts/src/svg/`, `ports/swift/Sources/AvatarKit/`

**SVG Serializer & Output Layer:**
- Purpose: Converts abstract `RenderTree` primitives into canonical SVG text bytes.
- Location: `crates/avatar-svg/src/serialize.rs`, `crates/avatar-svg/src/svg.rs`
- Contains: Number formatting, XML element emission, attribute ordering, coordinate normalization.
- Depends on: `crates/avatar-svg/src/tree.rs`
- Used by: `crates/avatar-export`, `crates/avatar-vectors`, `ports/ts/src/svg/svg.ts`, `ports/swift/Sources/AvatarKit/SvgSerializer.swift`

**CLI & Fleet Export Layer:**
- Purpose: Converts stable seat IDs into raster PNG identity assets for agent avatars.
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

**Swift Native Render Tree Pipeline:**

1. Swift client supplies stored `Genome` bytes, avatar type, version, and target mood — `ports/swift/Sources/AvatarKit/AvatarKit.swift`
2. `AvatarKit` decodes genome to phenotype using native table-driven Swift logic — `ports/swift/Sources/AvatarKit/`
3. `TreeBuilder` evaluates geometry passes to construct native `RenderTree` primitives — `ports/swift/Sources/AvatarKit/TreeBuilder.swift`
4. Client walks `RenderTree` primitives directly to construct native SwiftUI paths without SVG parsing

**Fleet PNG Export Pipeline:**

1. CLI receives `--stable-id` or `--manifest` JSON file — `crates/avatar-export/src/main.rs`
2. `avatar-export` derives genome, decodes phenotype, and renders canonical SVG — `crates/avatar-export/src/main.rs`
3. SVG stream passes to `resvg` pure-Rust engine to rasterize fixed-dimension PNG files — `crates/avatar-export/src/main.rs`

**Golden Vector Conformance Gate:**

1. Conformance test reads golden hex DNA vectors from disk — `crates/avatar-vectors/src/lib.rs`
2. Test executes local implementation pipeline to produce phenotype JSON and SVG output
3. Output is byte-compared against expected golden assets in `vectors/phenotype/` and `vectors/svg/`
4. Cross-language test runners execute equivalent checks in TS (`ports/ts/tests/golden-conformance.test.ts`) and Swift (`ports/swift/Tests/AvatarKitTests/GoldenConformanceTests.swift`)

## Key Abstractions

**Genome (`Genome`):**
- Purpose: Enforces 1024-byte (8192-bit) layout boundary for procedural creature DNA.
- Location: `crates/avatar-genome/src/genome.rs`, `ports/ts/src/genome.ts`, `ports/swift/Sources/AvatarKit/`
- Pattern: Value object / Newtype wrapper around byte slice with error checking.

**Phenotype (`Phenotype`):**
- Purpose: Represents canonical, generator-independent parameters governing creature appearance.
- Location: `crates/avatar-phenotype/src/lib.rs`, `ports/ts/src/phenotype.ts`, `ports/swift/Sources/AvatarKit/`
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
- Location: `crates/avatar-svg/src/dispatch.rs`, `ports/ts/src/svg/dispatch.ts`, `ports/swift/Sources/AvatarKit/AvatarKit.swift`
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
- Location: `ports/ts/src/svg/index.ts`
- Triggers: Node / Bun imports of package `@cortexkit/avatar`.
- Responsibilities: Export TS functions (`renderSvg`, `renderSvgFor`, `bodyGeometry`) matching Rust rendering behavior.

**Swift Package Framework:**
- Location: `Package.swift`, `ports/swift/Sources/AvatarKit/AvatarKit.swift`
- Triggers: SwiftPM dependency inclusion and `renderTree(...)` calls.
- Responsibilities: Native iOS/macOS rendering surface returning pre-resolved `RenderTree` instances.

## Error Handling

**Strategy:** Fail Closed with Sentinel Error Types.

All parsers, decoders, and renderers return explicit, typed result/error enums (`GenomeError`, `DispatchError`, `RenderError`).
Unknown avatar types, unsupported generator versions, or invalid genome bit lengths immediately halt execution and return errors rather than falling back to default visuals or silent failures.

- `GenomeError::InvalidLength`: Returned when input bytes fail genome size verification.
- `DispatchError::UnsupportedType`: Returned when requested avatar type is unknown.
- `DispatchError::UnsupportedVersion`: Returned when requested version is not registered for an avatar type.
- `RenderError::UnsupportedGrowth` / `UnsupportedCanvas`: Returned when render parameters deviate from pinned constants.

## Cross-Cutting Concerns

**Determinism & Numeric Discipline:**
- All trigonometric operations use `avatar_tables::sin_q30` and `cos_q30` backed by committed quarter-wave lookup tables.
- Easing calculations look up pre-computed Q32.32 values from committed tables (`EASE_LINEAR`, `EASE_EXPO`, `EASE_IN_CIRC`, `EASE_IN_QUINT`, `EASE_IN_OUT_CIRC`).
- Signed fixed-point division must use `div_round_half_away` to ensure consistent half-away-from-zero rounding across all platforms.

**Caching & State:**
- The rendering core is stateless and performs no internal caching.
- Native consumers maintain local render tree caches when drawing high-frequency UI frames.

**Storage & Assets:**
- Lookup tables are committed in `crates/avatar-tables/src/generated/tables.rs` and mirrored in TS/Swift sources.
- Voronoi pattern mesh data is committed in `crates/avatar-svg/src/generated/voronoi.rs` and mirrored in TS/Swift sources.
- Conformance test vectors are stored in `vectors/` (`dna/`, `phenotype/`, `svg/`, `manifest.json`).
