# Codebase Structure

A Rust workspace (`crates/`) with two consumer ports (`ports/`), a golden corpus (`vectors/`) and a review playground (`playground/`). This file is a map; to find a specific symbol, search the code rather than this document.

## Workspace

```text
crates/
  avatar-tables/     committed sine and easing tables, rounding division, table mint (--check)
  avatar-phenotype/  classic typed units, Phenotype struct, canonical phenotype JSON
  avatar-genome/     classic 1 KiB Genome, decoder, palette table, id-derived genomes at any length
  avatar-svg/        every generator (classic at the top level, wild in src/wild/), the tree, dispatch, serializers
  avatar-ffi/        the C boundary built into the Swift package's embedded core
  avatar-export/     CLI: stored genome to PNG, SVG or tree JSON; genome manifests
  avatar-vectors/    golden corpus tests and the corpus mint
  clippy.toml        refuses maths-library calls in every crate under crates/
ports/
  swift/             AvatarKit sources and tests, AvatarCore.xcframework, AvatarCore.source-hash
  ts/                tree decoder and generic SVG serializer, with Bun tests
vectors/             golden corpus: dna/, phenotype/, svg/, tree/, tree-svg/, wild/, manifest.json, contact-sheet.html
playground/          review harness (its own Cargo workspace) and the browser lab
scripts/             the gate and its checks, the core build, the cross-platform check
docs/                design/ notes (working notes go in the git-ignored .cortexkit/internal-docs/)
Package.swift        the Swift package, at the root so it can be consumed by git URL
LICENSE              MIT
```

`avatar-genome` builds on `avatar-phenotype` and `avatar-tables`; `avatar-svg` builds on all three, and uses `avatar-genome` only in classic dispatch. `avatar-ffi` and `avatar-export` sit on `avatar-svg`.

Directory names are the short crate names. The four library crates' package names carry a `cortexkit-` prefix (`cortexkit-avatar-svg` and so on), which is the name `cargo -p` and a crates.io dependency use; each library keeps its short Rust name, so code imports `avatar_svg`. `avatar-ffi`, `avatar-export` and `avatar-vectors` keep their names and are never published.

## `crates/avatar-svg/src`

- `dispatch.rs`: `render_tree_for_bytes` and `render_tree_for`, the type and version arms, and each type's genome length.
- `tree.rs`: `RenderTree`, `Primitive`, `Role` and the other tree types; `tree_json.rs`: canonical tree JSON.
- `tree_serialize.rs`: generic SVG for any tree. `serialize.rs`: classic's historical SVG layout, which branches on classic labels; never use it for another type.
- Classic generator: `build.rs` assembles the tree; `geometry.rs` (body spline and flattening), `face.rs`, `eyes.rs`, `brows.rs`, `mouth.rs`, `lashes.rs`, `pattern.rs`, `mood.rs`; `fixed.rs` and `format.rs` hold fixed-point and number-formatting helpers; `svg.rs` holds `RenderParams`, `Mood` and `RenderError`.
- `wild/`: `mod.rs` decodes the genome (`decode_genome`), places the face and draws the creature (`draw_full`); `wild.rs` (body silhouette), `merge.rs` (welding limbs into the body by signed distance fields and marching squares), `surface.rs` (Voronoi patterns), `face.rs`, `mood.rs`, `palette.rs`, `body.rs` (face placement profile), `render.rs` (tree helpers), `genome.rs` (`GENOME_LEN`, probe genomes), and `math.rs`, the deterministic trigonometry.
- `experimental.rs`: an unstable preview of classic's dormant asymmetry and layer regions, used by one example. No corpus and no compatibility promise.
- `generated/voronoi.rs`: classic's committed Voronoi masks; `bin/mint_voronoi.rs` regenerates them.

## Other crates

- `crates/avatar-genome`: `genome.rs`, `decode.rs`, `identity.rs`, `generated/palette.rs` with `bin/mint_palette.rs`. `examples/fleet_coverage.rs` is the classic corpus coverage check the gate runs; the other examples render sample sheets.
- `crates/avatar-vectors/src`: `lib.rs` holds the classic vector list and every boundary test; `tests/wild.rs` the wild corpus; `tree_fixture.rs` an independent strict decoder for the tree files.
- `crates/avatar-ffi`: `src/lib.rs`, the C header in `include/`, and `README.md` on how the boundary is used.

## Ports

- `ports/swift/Sources/AvatarKit`: `EmbeddedCore.swift` (`renderTreeViaCore`), `RenderTree.swift` (tree types and `decodeRenderTreeJson`), `RenderTreeJSON.swift` (numeric wire types and the strict parser), `RenderTreeCache.swift`, `TreeAffine.swift`, `Mood.swift` (moods and the type names), `Generated/SinQuarter.swift`.
- `ports/swift/Tests/AvatarKitTests`: corpus decode and re-encode, embedded-core reproduction of every tree, cache tests.
- `ports/ts/src`: `tree.ts` (types and `decodeRenderTreeJson`), `tree-serialize.ts` (`serializeTree`), `tree-format.ts`. Tests in `ports/ts/tests`.

## `playground/`

- `src/`: the `grid` binary (`main.rs`: review sheets such as `--review`, `--moods` and `--icons`, `--check N` for the placement invariants, `--probe` for the lab comparison, `--bench`), `drawn_probe.rs` (what the lab comparison reads from Rust), `fit.rs`, and two bins (`probe`, `fixedspike`). It draws through `avatar_svg::wild`; it holds no generator of its own.
- `web/index.html`: the browser lab, a JavaScript twin of the wild generator for design review. `scripts/drift-check.sh` and `scripts/const-drift.py` keep it in step.

## `scripts/`

- `check.sh`: the gate. `test-check.py` tests its scheduling and its small leaf checks.
- `check-home-paths.py`: the gate stage that refuses an absolute home-directory path in a tracked text file.
- `drift-check.sh`, `const-drift.py`: the browser lab comparisons.
- `build-xcframework.sh`, `xcframework-source-hash.sh`: build the embedded core and record what it was built from.
- `wild-cross-platform.sh` with `wild-port/` (`mint.rs`, `compare.py` and its tests): the by-hand cross-platform byte check.

## Where to add new code

- **A new creature type:** a new module under `crates/avatar-svg/src/`, as `wild/` is. Write it once, in Rust; never in Swift or TypeScript. Keep it deterministic: basic arithmetic, `sqrt`, `round`, and `wild/math.rs` or `avatar-tables` for anything else. Register it in `dispatch.rs`, keeping the type, version, length order. Give it a corpus, rebuild the embedded core, and run the cross-platform check. A new type needs the maintainer's approval before agents can be switched to it.
- **A new version of a type:** a new arm in `dispatch.rs`. Never change what an existing version draws once an agent uses that type; add its vectors beside the old ones.
- **A tree feature** (a new primitive, paint, effect or role): `tree.rs`, `tree_json.rs`, `tree_serialize.rs`, and both decoders and the TypeScript serializer, each failing closed on unknown values. Then rebuild the embedded core.
- **A numeric table:** `crates/avatar-tables/src/bin/mint.rs`, then regenerate in a worktree with `cargo run -p cortexkit-avatar-tables --bin mint`. Swift mirrors only `SIN_QUARTER`, for composing transforms; generator tables stay Rust-only.
- **A classic golden vector:** add it to the list in `crates/avatar-vectors/src/lib.rs`, then `UPDATE_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --exact --nocapture --ignored`.
- **A wild golden vector:** add a row to `vectors/wild/corpus.tsv`, whose three tab-separated columns are the id the genome derives from, the comma-separated moods to pin, and the wild version to render at. Then run `UPDATE_WILD_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --exact --nocapture --ignored`, which leaves classic untouched. Version 1 trees are named `<id>__<mood>.json`; later versions add the version, as in `<id>__v2__<mood>.json`.
- After either mint, paste the printed pins back into `crates/avatar-vectors`, and open `vectors/contact-sheet.html` and look at every new cell.
- **A gate stage:** `scripts/check.sh`, in the lane where it runs, plus its stub entry in `scripts/test-check.py`. Show it failing once before trusting it.

## Conventions

- Rust files are `snake_case.rs`, Swift files `PascalCase.swift`, TypeScript files `kebab-case.ts`.
- Rust unit tests sit in `#[cfg(test)]` modules next to the code. Corpus tests live in `crates/avatar-vectors`.
- Corpus files and generated tables are written by their mints, never by hand.
- `docs/` holds only public documents; working reports go in the git-ignored `.cortexkit/internal-docs/`. The gate refuses any tracked file that the ignore rules cover; `.cortexkit/WORKER.md` is the one tracked file there.
