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
│   ├── avatar-genome/            # Genome decoding and identity derivation
│   ├── avatar-phenotype/         # Typed phenotype data model and units
│   ├── avatar-svg/               # Geometry generation, render tree, SVG serializer
│   ├── avatar-tables/            # Pinned lookup tables and division rounding logic
│   └── avatar-vectors/           # Golden vector conformance test suite
├── ports/                        # Port implementations in secondary languages
│   ├── swift/                    # Swift AvatarKit framework and CBlake3 vendor source
│   └── ts/                       # TypeScript renderer, tree decoder, and Bun test suite
├── vectors/                      # Golden vector corpus for cross-language conformance
│   ├── dna/                      # Raw 8192-bit hex DNA files
│   ├── phenotype/                # Canonical decoded phenotype JSON files
│   ├── svg/                      # Pinned golden SVG output files
│   ├── tree/                     # Canonical render-tree JSON, the consumer transport
│   └── tree-svg/                 # Generic serializer output every consumer must match
├── scripts/                      # Verification, CI check, and table import scripts
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
- Purpose: Assembles rendering geometry, builds abstract render trees, and serializes SVG output.
- Contains: Geometry generators (`face.rs`, `eyes.rs`, `brows.rs`, `mouth.rs`, `lashes.rs`, `pattern.rs`, `mood.rs`), `RenderTree` definition (`tree.rs`), version dispatch (`dispatch.rs`), SVG serializer (`serialize.rs`).
- Key files: `crates/avatar-svg/src/lib.rs`, `crates/avatar-svg/src/build.rs`, `crates/avatar-svg/src/dispatch.rs`, `crates/avatar-svg/src/tree.rs`, `crates/avatar-svg/src/serialize.rs`.

**`crates/avatar-export/`:**
- Purpose: Command-line application rasterizing agent avatars to PNG images for fleet deployment.
- Contains: Main CLI routine using `resvg` for pure Rust rasterization.
- Key files: `crates/avatar-export/src/main.rs`, `crates/avatar-export/Cargo.toml`.

**`crates/avatar-vectors/`:**
- Purpose: Test harness validating Rust pipeline outputs against stored golden vectors.
- Contains: Test fixtures reading `vectors/` corpus files and asserting byte-exact rendering.
- Key files: `crates/avatar-vectors/src/lib.rs`.

**`ports/ts/`:**
- Purpose: TypeScript port of the avatar generator targeting Web and Node runtime environments.
- Contains: TS implementations of genome decoding, phenotype model, lookup tables, geometry assembly, and Bun conformance tests.
- Key files: `ports/ts/src/svg/index.ts`, `ports/ts/src/genome.ts`, `ports/ts/src/phenotype.ts`, `ports/ts/src/tables.ts`, `ports/ts/src/svg/dispatch.ts`, `ports/ts/tests/golden-conformance.test.ts`.

**`ports/swift/`:**
- Purpose: Swift `AvatarKit` framework for iOS and macOS client applications.
- Contains: `AvatarKit` target, vendored `CBlake3` target, native `RenderTree` builder, SVG serializer, SwiftPM test suite.
- Key files: `ports/swift/Sources/AvatarKit/AvatarKit.swift`, `ports/swift/Sources/AvatarKit/RenderTree.swift`, `ports/swift/Sources/AvatarKit/TreeBuilder.swift`, `ports/swift/Sources/AvatarKit/SvgSerializer.swift`, `ports/swift/Tests/AvatarKitTests/GoldenConformanceTests.swift`.

**`vectors/`:**
- Purpose: Shared golden vector corpus pinning phenotype JSON and SVG rendering across all language implementations.
- Contains: Hex DNA vectors, JSON phenotype expectations, golden SVG files, vector manifest.
- Key files: `vectors/manifest.json`, `vectors/contact-sheet.html`, `vectors/dna/`, `vectors/phenotype/`, `vectors/svg/`, `vectors/tree/`, `vectors/tree-svg/`.
- Note: `manifest.json`'s `corpus_revision` counts re-mints. It is not the version a registry row carries, which lives in `crates/avatar-svg/src/dispatch.rs`.

**`scripts/`:**
- Purpose: Automation scripts for CI checks, parity validation, and asset updating.
- Contains: CI pipeline check script (`check.sh`), Rust-vs-JS visual probe drift check script (`drift-check.sh`), fleet avatar verification script (`verify-fleet-avatars.sh`).
- Key files: `scripts/check.sh`, `scripts/drift-check.sh`, `scripts/verify-fleet-avatars.sh`.

## Key File Locations

**Entry Points:**
- Canonical Rust Library: `crates/avatar-svg/src/lib.rs`
- PNG Export CLI Tool: `crates/avatar-export/src/main.rs`
- TypeScript Port Library: `ports/ts/src/svg/index.ts`
- Swift Framework Library: `ports/swift/Sources/AvatarKit/AvatarKit.swift`
- SwiftPM Manifest: `Package.swift`

**Configuration:**
- Workspace Cargo Manifest: `Cargo.toml`
- TypeScript Package Manifest: `ports/ts/package.json`
- TypeScript Compiler Options: `ports/ts/tsconfig.json`

**Core Logic:**
- Bit Decoding & Identity: `crates/avatar-genome/src/decode.rs`, `crates/avatar-genome/src/identity.rs`
- Fixed-Point & Tables: `crates/avatar-tables/src/lib.rs`
- Geometry & Tree Building: `crates/avatar-svg/src/build.rs`, `crates/avatar-svg/src/geometry.rs`
- Version & Type Dispatch: `crates/avatar-svg/src/dispatch.rs`
- SVG Serialization: `crates/avatar-svg/src/serialize.rs`

**Tests & Conformance:**
- Rust Golden Vector Suite: `crates/avatar-vectors/src/lib.rs`
- TypeScript Golden Vector Suite: `ports/ts/tests/golden-conformance.test.ts`
- Swift Golden Vector Suite: `ports/swift/Tests/AvatarKitTests/GoldenConformanceTests.swift`
- CI Pipeline Automation: `scripts/check.sh`

## Naming Conventions

**Files:**
- Rust Source: `snake_case.rs` (e.g., `crates/avatar-svg/src/serialize.rs`)
- TypeScript Source: `camelCase.ts` or `kebab-case.test.ts` (e.g., `ports/ts/src/genome.ts`, `ports/ts/tests/golden-conformance.test.ts`)
- Swift Source: `PascalCase.swift` (e.g., `ports/swift/Sources/AvatarKit/RenderTree.swift`)
- Spec Data & Generation: `snake_case.rs` or `kebab-case` (e.g., `crates/avatar-tables/src/bin/mint.rs`, `vectors/manifest.json`)

**Directories:**
- Cargo Workspace Crates: `kebab-case` under `crates/` (e.g., `crates/avatar-genome/`)
- Language Ports: `lowercase` under `ports/` (e.g., `ports/ts/`, `ports/swift/`)
- Test Vector Categories: `lowercase` under `vectors/` (e.g., `vectors/dna/`, `vectors/svg/`)

## Where to Add New Code

**New Avatar Type or Generator Version:**
1. Implement the geometry generator and tree builder in `crates/avatar-svg/src/`.
2. Register the type identifier and version constant in `crates/avatar-svg/src/dispatch.rs`.
3. Mirror the version dispatch and geometry logic in TypeScript (`ports/ts/src/svg/dispatch.ts`) and Swift (`ports/swift/Sources/AvatarKit/AvatarKit.swift`).

**New Numeric Table or Math Function:**
1. Add table generation or mathematical definition to `crates/avatar-tables/src/bin/mint.rs`.
2. Re-mint committed Rust table code via `cargo run -p avatar-tables --bin mint`.
3. Export table wrappers and helpers in `crates/avatar-tables/src/lib.rs`.
4. Import/generate updated table sources into TS (`ports/ts/src/generated/tables.ts`) and Swift (`ports/swift/Sources/AvatarKit/Generated/GeneratedTables.swift`).

**New Geometry Feature Pass:**
1. Add modular rendering routine in `crates/avatar-svg/src/[feature].rs`.
2. Integrate pass into tree assembly in `crates/avatar-svg/src/build.rs`.
3. Implement matching geometry routines in `ports/ts/src/svg/[feature].ts` and `ports/swift/Sources/AvatarKit/TreeBuilder.swift`.

**New Golden Conformance Vector:**
1. Place 8192-bit hex DNA file in `vectors/dna/[name].hex`.
2. Place expected decoded phenotype JSON in `vectors/phenotype/[name].json`.
3. Place expected SVG output for each mood in `vectors/svg/[name]__[mood].svg`.
4. Register the new vector entry in `vectors/manifest.json`.

**New Tests:**
- Rust unit tests: Co-located in source files inside `#[cfg(test)] mod tests`.
- Cross-language conformance: `crates/avatar-vectors/src/lib.rs`, `ports/ts/tests/golden-conformance.test.ts`, and `ports/swift/Tests/AvatarKitTests/GoldenConformanceTests.swift`.
