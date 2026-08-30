# avatar

Generated visual identities for Alfonso agents: every seat in a CortexKit
fleet gets a distinctive, deterministic, *expressive* avatar — a creature
generated from DNA, whose mood tracks the agent's live state.

Descended from TOG (Same Same but Different: Tokens of Gratitude), a fully
on-chain procedural SVG PFP project hand-built pre-LLMs. TOGs proved the
mechanism inside EVM limits; this project removes the limits.

## Pipeline

```
registry row  →  DNA  →  Phenotype (typed integers)  →  SVG / render tree
 genome+type      1 KiB      canonical JSON            7 moods, render-time
 +version
```

The genome is **stored**, not derived at render time. Prefrontal seeds one
onto an agent's registry row at creation, and the operator or the agent can
re-roll it later — so the row is the only truth, and nothing may be cached
by agent id alone. Deriving a genome from an id (`avatar-export
--stable-id`) is the seeding path and the GitHub-App path, not the
consumption path.

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
| Swift (`AvatarKit`, SPM by git URL) | `Package.swift` → `ports/swift/` | decodes a tree, draws it | `swift test` |

The ports still carry a full generator for `creature.classic`, which
predates this decision and is byte-gated. New creature types are
implemented once, in Rust.

Four corpus boundaries in `vectors/`, CI-gated in every language:

| Boundary | Directory | Pins |
| --- | --- | --- |
| DNA → phenotype | `phenotype/` | decode |
| phenotype → SVG | `svg/` | the classic renderer, byte for byte |
| tree → JSON | `tree/` | the transport a consumer decodes |
| tree → SVG | `tree-svg/` | the generic renderer every consumer must match |

## Consuming from Swift

```swift
.package(url: "https://github.com/cortexkit/avatar.git", from: "...")

let tree = try renderTree(genome: genome, type: row.type,
                          version: row.version, mood: mood)
for primitive in tree.primitives { /* walk into SwiftUI Path */ }
```

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

Rendering honours the version stored on the row rather than the
library's newest, and an unknown type or version throws rather than
falling back: see `docs/design/012` for why drawing nothing is the
required behaviour.

## Documents

- `docs/charter.md` — design space, TOG inheritance, chartering constraints
- `docs/design/001…006` — shipped `creature.classic`: determinism contract,
  phenotype schema, genome bit map, renderer geometry, face+mood, render tree
- `docs/design/007…009`, `011` — the `creature.wild` redesign: expressiveness
  spikes, look-first workflow, genome v2 layout, port-surface fork. 010 is
  reserved for its phenotype schema, which waits on the look being frozen
- `docs/design/012` — how a face is allowed to change: type vs version, and
  the procedure for each

## Determinism rules (the short version)

Integer/fixed-point math only, one pinned rounding rule, trig and easing
from committed lookup tables, no platform libm, hand-rolled canonical
serialization.

An observable output change is **never** a silent edit to a shipped
version, because an agent's face must not change because we shipped:

- Fixing a **generation bug** adds a new version arm; the old arm stays
  byte-for-byte, keeps its corpus vectors, and rows upgrade only when
  somebody chooses to.
- Changing the **depiction** is a new type, not a version — types are
  different representations of the same genome behind one interface.

Either way: re-mint only the new arm's vectors (`UPDATE_GOLDEN=1 cargo test
-p avatar-vectors`), review `vectors/contact-sheet.html` by eye, and every
port's conformance leg must re-pass before it ships. `docs/design/012` has
the full procedure.

The manifest's `corpus_revision` is **not** that version. It counts
re-mints, moves when a vector or a boundary is added, and implies nothing
about what any creature looks like. The version a row carries is
`CLASSIC_VERSION` in `crates/avatar-svg/src/dispatch.rs`.
