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
