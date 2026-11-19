# avatar

Generated visual identities for Alfonso agents: every seat in a CortexKit
fleet gets a distinctive, deterministic, *expressive* avatar — a creature
generated from DNA, whose mood tracks the agent's live state.

Descended from TOG (Same Same but Different: Tokens of Gratitude), a fully
on-chain procedural SVG PFP project hand-built pre-LLMs. TOGs proved the
mechanism inside EVM limits; this project removes the limits.

## Pipeline

```
registry row        →  dispatch by type and version  →  render tree  →  SVG / native drawing
genome bytes, type,     creature.classic: 1 KiB          7 moods, chosen
version                 creature.wild:   16 KiB          at render time
```

The genome is **stored**, not derived at render time. It lives on the
agent's registry row, and the operator or the agent can re-roll it, so the
row is the only truth and nothing may be cached by agent id alone. Every
byte pattern of the right length is a valid creature.

## One generator, several consumers

Generation lives in Rust. Consumers receive a **render tree** — a flat
list of drawing primitives with every value already resolved — and draw
it. They do not reimplement the generator.

That is a correctness decision, not a cost one. Comparing independent
implementations against a corpus proves they agree on a finite sample of
a 2^8192 input space; a divergence that fires for one genome in 10^9
stays invisible until it renders on somebody's screen, and the rarer a
feature is the larger the sample needed to catch it. One generator makes
divergence unrepresentable instead of undetected. `docs/design/011` has
the argument.

| Implementation | Where | Role | Conformance |
| --- | --- | --- | --- |
| Rust | `crates/` | generates, mints the corpus | `cargo test -p avatar-vectors` |
| TypeScript | `ports/ts/` | decodes a tree, renders SVG | `bun test` |
| Swift (`AvatarKit`, SPM by git URL) | `Package.swift` → `ports/swift/` | runs the Rust generator embedded as `AvatarCore.xcframework`, decodes the tree, draws it | `swift test` |

Every creature type, classic included, is implemented once, in Rust.

Four corpus boundaries in `vectors/`, checked in every language by
`scripts/check.sh` on every push (there is no automatic CI):

| Boundary | Directory | Pins |
| --- | --- | --- |
| DNA → phenotype | `phenotype/` | decode |
| phenotype → SVG | `svg/` | the classic renderer, byte for byte |
| tree → JSON | `tree/` | the transport a consumer decodes |
| tree → SVG | `tree-svg/` | the generic renderer every consumer must match |

## Consuming from Swift

```swift
.package(url: "https://github.com/cortexkit/avatar.git", from: "...")

// Any type, through the embedded Rust generator:
let tree = try renderTreeViaCore(genomeBytes: row.genome, type: row.type,
                                 version: row.version, mood: mood)
for primitive in tree.primitives { /* walk into SwiftUI Path */ }
```

Every type goes through `renderTreeViaCore`. Rendering is
pure and safe from any thread, and the tree types are `Sendable`.

Two things that have bitten a consumer, both worth reading before you
write the walk:

- **There is no top-level `Sources/`.** The manifest is hoisted to the
  repository root because SwiftPM only consumes a git dependency whose
  manifest sits at the clone root, while the sources stay in
  `ports/swift/` beside the other ports. A first look at `Sources/`
  finds nothing and reads as "this library has no such API" — it is a
  layout artefact, not absence.
- **Normalise coordinates by `tree.extent`, never by `tree.canvas`.**
  Primitive coordinates are 100× finer than the SVG canvas units, so
  dividing by `canvas` draws an avatar 100× too large — which looks like
  a plausible zoom rather than a bug.
- **Key behaviour on `primitive.role`, never on `label`.** Labels are
  diagnostic names that may change. A view with no ground plane hides the
  primitive whose role is `.groundShadow`.

Rendering honours the version stored on the row rather than the
library's newest, and an unknown type or version throws rather than
falling back: see `docs/design/012` for why drawing nothing is the
required behaviour.

## Documents

- `docs/charter.md` — design space, TOG inheritance, chartering constraints
- `docs/design/001…006` — shipped `creature.classic`: determinism contract,
  phenotype schema, genome bit map, renderer geometry, face+mood, render tree
- `docs/design/007…009`, `011` — the `creature.wild` redesign: expressiveness
  spikes, look-first workflow, genome v2 layout, and the single-source and
  determinism decisions. 010 was never written: wild version 1's byte map
  lives in its decoder, `crates/avatar-svg/src/wild/`, inside note 009's
  region boundaries
- `docs/design/012` — how a face is allowed to change: type vs version, and
  the procedure for each

## Contributing

**Open an issue before writing code, and wait for an explicit go.** Tag it
`design discussion` or `bug`. A pull request that arrives without one gets
asked for the issue rather than reviewed — not as a formality, but because
the conversation that belongs in ten lines of an issue costs hundreds of
lines and several review rounds once an implementation exists, and a
reviewer looking at working code is under quiet pressure to find a way to
take it.

Bug fixes with an obvious shape move fast: issue, reproduction, fix. The
gate is cheap because most issues get a go quickly.

This repo has a specific reason to hold the line. Every visual change is
somebody's agent's face, and faces do not change because we shipped — see
`docs/design/012` for which changes are a version bump, which are a new
type, and why a rendering path is added to rather than edited.

## Determinism rules (the short version)

The same genome, type, version and mood produce the same bytes on every
platform. No generation path calls the platform maths library; a lint
refuses it. `creature.classic` uses integer and fixed-point maths with
trig and easing from committed lookup tables. `creature.wild` uses `f64`
limited to the operations IEEE 754 makes exact everywhere, with its own
trig built from those, and is checked byte-identical across macOS and
Linux on ARM and Intel, and on Windows x86_64, where every golden tree
renders byte-identical to the committed corpus. Serialization is
hand-rolled and canonical.

An observable output change is **never** a silent edit to a shipped
version, because an agent's face must not change because we shipped:

- Fixing a **generation bug** adds a new version arm; the old arm stays
  byte-for-byte, keeps its corpus vectors, and rows upgrade only when
  somebody chooses to.
- Changing the **depiction** is a new type, not a version — types are
  different representations of the same genome behind one interface.

Either way: re-mint only the affected vectors by setting `UPDATE_GOLDEN=1`
(classic) or `UPDATE_WILD_GOLDEN=1` (wild) on
`cargo test -p avatar-vectors tests::mint -- --exact --nocapture --ignored`,
review `vectors/contact-sheet.html` by eye, rebuild the embedded core, and
every port's conformance leg must re-pass before it ships.
`docs/design/012` has the full procedure.

The manifest's `corpus_revision` is **not** that version. It counts
re-mints, moves when a vector or a boundary is added, and implies nothing
about what any creature looks like. The versions a row can carry are the
dispatch arms in `crates/avatar-svg/src/dispatch.rs`: `creature.classic`
version 2 and `creature.wild` version 1.
