# avatar

Generated visual identities for Alfonso agents: every seat in a CortexKit
fleet gets a distinctive, deterministic, *expressive* avatar — a creature
generated from DNA, whose mood tracks the agent's live state.

Descended from TOG (Same Same but Different: Tokens of Gratitude), a fully
on-chain procedural SVG PFP project hand-built pre-LLMs. TOGs proved the
mechanism inside EVM limits; this project removes the limits.

## Pipeline

```
DNA (8192 bits)  →  Phenotype (typed integers)  →  SVG bytes / render tree
      identity           canonical JSON              7 moods, render-time
```

Three implementations produce **byte-identical SVG** from the same
DNA + mood + generator version, enforced by a golden-vector corpus at
both pipeline boundaries (`vectors/`, CI-gated in every language):

| Implementation | Where | Conformance |
| --- | --- | --- |
| Rust (canonical, mints the corpus) | `crates/` | `cargo test -p avatar-vectors` |
| TypeScript | `ports/ts/` | `bun test` |
| Swift (`AvatarKit`, SPM by git URL) | `Package.swift` → `ports/swift/` | `swift test` |

The Swift package's primary API is a flat, pre-resolved **render tree**
(SwiftUI draws it without parsing SVG); SVG is a serializer over the
same tree.

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
- `docs/design/001…006` — genome layout, determinism contract, phenotype
  schema, bit map, renderer geometry, face+mood, render tree

## Determinism rules (the short version)

Integer/fixed-point math only, one pinned rounding rule, trig and easing
from committed lookup tables, no platform libm, hand-rolled canonical
serialization. Any observable output change is a `generator_version`
bump: re-mint the corpus (`UPDATE_GOLDEN=1 cargo test -p avatar-vectors`),
review `vectors/contact-sheet.html` by eye, and every port's conformance
leg must re-pass before it ships.
