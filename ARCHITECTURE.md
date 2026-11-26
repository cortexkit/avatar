# Architecture

Avatar draws an agent as a creature. The creature comes from three fields stored on the agent's record in a host's registry: `avatar_genome` (bytes, stored as hex), `avatar_type` and `avatar_version`. A mood is added at draw time. The same genome, type, version and mood always produce the same bytes, on every platform.

Avatar is a library. It has no service, no store and no wire surface, and it never decides which genome an agent has. The registry that stores agents writes a new agent's genome when the agent is created, for example 16,384 random bytes as `creature.wild` (every byte pattern is a valid creature). Apps and tools read the record and render it with this code. Renders and caches must key on the stored record, never on the agent id: a genome can be re-rolled.

Two decisions carry the design:

- **One generator per type, written once, in Rust** (`crates/avatar-svg`). Consumers receive the finished render tree and draw it; nobody writes a second copy of a generator. Comparing independent implementations against a corpus checks a sample of a huge input space, so it cannot prove they agree on rare features (`docs/design/011-port-surface.md`). Apple-platform apps run this Rust themselves, through an embedded static library in the Swift package.
- **Platform-invariant arithmetic.** A creature must be byte-identical everywhere, so no generation path calls the platform maths library. See Determinism.

## Terms

- **Genome:** the stored bytes. `creature.classic` uses 1,024 bytes (2,048 hex characters); `creature.wild` uses 16,384 bytes (32,768 hex characters). A wild genome derived from an id begins with the classic genome derived from the same id (`derive_genome_bytes` in `crates/avatar-genome/src/identity.rs`).
- **Type:** which representation draws a genome. Registered types are `creature.classic` and `creature.wild` (`crates/avatar/src/dispatch.rs`). A new look is a new type.
- **Version:** one generation of a type's generator. It exists to fix generation bugs. Every row renders at the version it stores, so versions are added and never edited (`docs/design/012-version-and-type-procedure.md`). Today: classic 2, wild 2. Wild version 1 still draws every row that stores it; version 2 exists because version 1 draws mood lids and squints beside eyes that the creature's eye aspect or the mood has scaled (`wild::Generation` in `crates/avatar-svg/src/wild/mod.rs`). A type no agent uses yet may still change in place, because no agent has a creature of that type whose look it would change.
- **Mood:** one of seven render-time moods: neutral, working, happy, waiting, blocked, sad, sleepy. A mood changes face geometry, never identity or colour, so a render cache must key on it.
- **Phenotype:** classic only. Typed fixed-unit parameters decoded from a classic genome (`crates/avatar-phenotype`), with a canonical JSON form. Wild decodes its genome straight into its own `Creature` struct.
- **Render tree:** the output of every generator: a flat, painter-order list of primitives (shapes, paths, paints, strokes, transforms, effects) in `crates/avatar-svg/src/tree.rs`. `canvas` is the SVG viewBox edge; coordinates are 100 times finer, so divide by `RenderTree::extent()`, never by `canvas`. Classic draws on a 10,000-unit canvas, wild on 240.
- **Label and role:** a primitive's `label` is a diagnostic name with no stability promise. Its `role` is the only thing a consumer may key behaviour on. The one role so far is `ground_shadow`: each wild creature has exactly one, classic has none, and a client with no ground plane hides it.

## Code map

| Layer | Where | Role |
|---|---|---|
| Tables | `crates/avatar-tables` | Committed lookup tables (quarter-wave sine `SIN_QUARTER`, five `EASE_*` tables), `sin_q30`/`cos_q30`, the rounding division `div_round_half_away`, and the `mint` binary that regenerates or checks the tables. |
| Phenotype | `crates/avatar-phenotype` | Classic's typed units (`Milli`, `Q32`, `Bam16`, `Rgb`), the `Phenotype` struct and its canonical JSON. |
| Genome | `crates/avatar-genome` | The classic 1 KiB `Genome`, its decoder to `Phenotype`, the palette table, and identity derivation from an id at any length. |
| Generators and tree | `crates/avatar-svg` | The classic generator (`build.rs`, `geometry.rs`, `face.rs`, `eyes.rs`, `brows.rs`, `mouth.rs`, `lashes.rs`, `pattern.rs`, `mood.rs`), the wild generator (`src/wild/`), the tree, canonical tree JSON and both SVG serializers. |
| Dispatch | `crates/avatar` | The one crate a Rust host depends on. `dispatch.rs` routes a stored row's type and version to the generator that draws it, and the crate root re-exports the tree types and serializers. |
| C boundary | `crates/avatar-ffi` | One call, `avatar_render_tree_json`, returning canonical tree JSON with a distinct status per failure; panics are contained. |
| Swift | `ports/swift`, `Package.swift` | AvatarKit: calls the embedded core, decodes the tree, owns a cache, composes transforms. Ships the committed `AvatarCore.xcframework`. |
| TypeScript | `ports/ts` | A fail-closed tree decoder and the generic SVG serializer. No generator. |
| Export | `crates/avatar-export` | CLI: renders a stored genome to PNG, SVG or tree JSON, and mints genome manifests. |
| Corpus | `crates/avatar-vectors`, `vectors/` | Golden vectors and the tests that pin every boundary. |
| Review harness | `playground/` | Grids, review sheets, the placement-invariant checker and the browser lab. Not shipped. |
| Gate | `scripts/check.sh` | The only gate. See The gate. |

`STRUCTURE.md` has the directory map and where to add new code.

The five library crates have prefixed package names, the names they are published under: `cortexkit-avatar-tables`, `cortexkit-avatar-phenotype`, `cortexkit-avatar-genome`, `cortexkit-avatar-svg` and `cortexkit-avatar`. Each keeps its short library name: code imports `avatar_tables`, `avatar_phenotype`, `avatar_genome`, `avatar_svg` and `avatar`. Cargo commands that select a package use the prefixed name (`cargo test -p cortexkit-avatar-svg`). `avatar-ffi`, `avatar-export` and `avatar-vectors` are never published.

## Rendering a stored avatar

1. The caller holds the row's bytes, type and version, plus a mood.
2. `render_tree_for_bytes` checks the type, then the version, then the genome length that type needs. The order is a contract. An unknown type or version means "this build cannot draw it": the client draws nothing and stays quiet. A wrong length for a known type means the row is malformed: the client reports it. Checking length first would report every row of a newer, differently sized type as corrupt.
3. Classic v2: `avatar_genome::decode` gives a `Phenotype`, and `build_render_tree` builds the tree. Wild v1 and v2: `wild::decode_genome` reads each feature from fixed region offsets (body bytes 0..64, extremities 64..160, palette and face from 160, surface sites 240..304), and `draw_full` builds the primitives for the `wild::Generation` the version names.
4. The tree leaves through one of three projections: `serialize_tree_json` (the canonical JSON every consumer decodes), `serialize_tree` (generic SVG; `serialize_tree_with_id_prefix` when several creatures share one document), or `serialize_svg` (classic only, the historical SVG layout its corpus pins). The JSON writes each distinct mask once, in a top-level `masks` table that masked effects name by index (`mask_ref`), because wild creatures clone one body mask into many primitives and the full copies made up most of a wild tree's bytes. Decoders resolve the references back into inline masks, so the in-memory tree is the same in every language (`docs/design/006-render-tree.md`).

How each consumer gets there:

- **Apple-platform apps:** AvatarKit's `renderTreeViaCore(genomeBytes:type:version:mood:)` calls the embedded core, then decodes the JSON with `decodeRenderTreeJson` (`RenderTree.swift`), which first checks the integer-only number grammar with the strict parser in `RenderTreeJSON.swift`. Each core status maps to its own `CoreRenderError`. `RenderTreeCache` is owned by the host app, keyed by bytes, type, version and mood, and can be emptied under memory pressure. `TreeAffine.swift` composes decoded transforms with the pinned sine table and a fixed-point multiply; it builds no geometry.
- **Rust hosts:** depend only on the `cortexkit-avatar` package (its library is imported as `avatar`), from crates.io or by git at a pinned commit, and call `render_tree_for_bytes` and `serialize_tree`. It re-exports the tree types, `Mood` and the serializers, so rendering needs no other avatar crate.
- **TypeScript:** `decodeRenderTreeJson` and `serializeTree` in `ports/ts/src`.
- **Images:** `avatar-export --genome-file <path> --type <type> --version <n> --out <file>`. The `--out` extension picks the format: `.png` or none, `.svg`, or `.json`; any other extension is refused. Live agents always render their stored genome. `--mint-genomes` writes a manifest of id-derived genomes for a type. A wild genome does not fit on a Windows command line, so portable callers use `--genome-file` (`-` reads standard input) rather than `--genome <hex>`.

No consumer may normalise a creature's visual weight (cropping to alpha bounds, scale nudges, minimum sizes): how much ink a creature has is part of who it is.

## Determinism

- **creature.classic** is integer-only: `i64` milliunits on a 1,000,000-unit canonical canvas, Q32.32 factors, 16-bit binary angles, sine and cosine from the committed quarter-wave table, easing from committed tables, and one rounding helper, `div_round_half_away`.
- **creature.wild** uses `f64`, restricted to operations IEEE 754 makes exact everywhere: `+ - * /`, `sqrt`, comparisons and Rust's half-away-from-zero `round`. Sine, cosine, tangent and arctangent are rebuilt from those in `src/wild/math.rs`. `crates/clippy.toml` refuses maths-library calls (`sin`, `atan2`, `powf`, `mul_add` and the rest) under `-D warnings`; the standard functions appear only as test oracles. This matters because a wild creature takes branches on computed values: in measurement, a difference in the fourteenth digit flipped a branch and moved one creature by 93 canvas units.
- **Measured:** `scripts/wild-cross-platform.sh` mints the wild corpus on macOS arm64 and in Linux arm64 and x86_64 containers and compares the bytes. It needs Docker, so it runs by hand, and again on the exact commit before a release tag.
- **Committed data:** the lookup tables (`crates/avatar-tables/src/generated/`), the classic palette (`crates/avatar-genome/src/generated/`) and the classic Voronoi masks (`crates/avatar-svg/src/generated/`). Each has a mint binary. The gate runs the tables mint with `--check`, which compares and writes nothing.

## Conformance

The golden corpus lives in `vectors/`. The mint test in `crates/avatar-vectors` writes it; nobody edits it by hand.

- **Genome to phenotype:** `dna/` to `phenotype/`, 22 classic genomes.
- **Phenotype to classic SVG:** `svg/`, 40 genome and mood cells.
- **Tree to canonical JSON:** `tree/`, 65 trees (40 classic, 25 wild). This pins the transport every consumer decodes.
- **Tree to generic SVG:** `tree-svg/`, the same 65. This pins the drawing every consumer must reproduce.
- **Wild references:** `vectors/wild/` holds 12 genomes derived from the fixed ids in `corpus.tsv`, and its manifest. Each `corpus.tsv` row and manifest entry names the wild version its trees render at. The tests render every reference genome in every mood at version 1, and one of them, `wild-port-0022`, at version 2 too. 25 of those renders are pinned as tree files: 18 at version 1, and the seven version 2 moods of `wild-port-0022`, named `<id>__v2__<mood>.json`. A further test checks that on every reference genome in every mood, version 2 differs from version 1 only in where the lids and squints sit.
- `vectors/contact-sheet.html` is regenerated with the corpus and byte-checked, so the visual review sheet cannot drift from the vectors.
- **Classic coverage:** the `fleet_coverage` example decodes 1,000 synthetic genomes and fails when a classic decode branch has no corpus vector.

The consumers are checked against the same files. Swift decodes every tree and re-encodes it to identical bytes, and its embedded-core tests reproduce every classic and wild tree through the binary. TypeScript decodes every tree and serializes each one to its `tree-svg/` file.

The browser lab (`playground/web/index.html`) is a JavaScript twin of the wild generator, used to review designs. `scripts/drift-check.sh` compares what the lab draws with the Rust generator over 24 seeds, and `scripts/const-drift.py` compares the sets of numeric constants in both. The two checks have complementary blind spots: the constant scan cannot see two existing values swapped, and the geometry comparison sees only the seeds it samples.

## The gate

`scripts/check.sh` is the repository's only gate. CI runs only when started by hand, and the untracked `.git/hooks/pre-push` runs the gate on every push except branch deletions.

- By default every cargo stage, the Python checks and the Rust half of the lab comparison run as one job on a Linux build server (`check.sh --linux`), sent there by the build-server client `ck` (a maintainers' tool that is not published) within a 600-second limit that covers queueing and retries. The workstation runs the embedded core's freshness check, the browser half of the lab comparison (with `chrome-headless-shell`), TypeScript, and the Swift suite where Swift is installed.
- Without `ck` the gate refuses. `AVATAR_GATE_LOCAL=1` runs every stage on the workstation, and is how to run the gate without that build server; it is never chosen automatically.
- Build-server jobs cannot write git-tracked files, so no gate stage writes one. Mints that rewrite tracked files run in a worktree.

## The embedded core

`ports/swift/AvatarCore.xcframework` is committed (static libraries for ios-arm64, ios-arm64-simulator and macos-arm64), so the app needs no Rust toolchain. `scripts/build-xcframework.sh` builds it on macOS with Xcode. `ports/swift/AvatarCore.source-hash` records a hash of every source the binary is built from (`scripts/xcframework-source-hash.sh`). The gate refuses when the hash no longer matches, because every test builds from source and would pass on a stale binary. Rebuild it whenever the Rust it embeds changes; rebuilds of the same source are byte-identical.

## Error handling

Everything fails closed. No path falls back to a default or newest creature.

- `DispatchError`: `UnsupportedType`, `UnsupportedVersion` (both: draw nothing, quietly), `GenomeLength` (a malformed row: report it), and `Render(RenderError)`, where `RenderError` is `UnsupportedGrowth` or `UnsupportedCanvas` for classic render inputs outside their pinned values.
- The C boundary returns 0 for success, then one status per case: unknown type, unknown version, genome length, unknown mood, invalid argument, render error, panic. Swift maps each to its own `CoreRenderError` case, and an unrecognised status to `unknownStatus`.
- `GenomeError` covers malformed hex (`InvalidHexLength`, `InvalidHexDigit`).
- The Swift and TypeScript tree decoders throw on an unknown tag or role, a missing field, a fraction where an integer belongs, or a `mask_ref` that breaks the mask table's rules (out of range, pointing to its own or a later entry, or leaving an entry unreferenced). Both decoders also refuse unknown fields. A silently defaulted primitive would draw a wrong creature that looks right.

## Further reading

- `docs/design/`: the design notes. 006 (render tree), 009 (wild genome layout), 011 (one generator, and the determinism decision) and 012 (versions and types) bear on most changes.
- `.cortexkit/WORKER.md`: standing rules for automated contributors working in this repository.
