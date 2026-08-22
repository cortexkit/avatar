# Design note 011 — Port surface: what each language must implement

Status: DECISION BRIEF, for the freeze conversation. Not a decision.
Written before note 010 because 010's shape depends on the answer.

## The question

The charter requires Rust, TS and Swift to produce byte-identical SVG
from the same DNA. v1 satisfied that by having all three implement the
whole generator. v2's generator is a different size of thing.

## Measurement

Lines of code, comments and blanks excluded, of machinery v1 never had
and every port would have to reproduce byte-exactly:

| Component | Lines | Character |
| --- | ---: | --- |
| `wild.rs` — spokes, domestication, extremity capsules, ray casts | 399 | branch-heavy |
| `merge.rs` — SDF, smooth-min, marching squares, arc-length respline | 182 | iterative, tolerance-sensitive |
| `surface.rs` — Voronoi half-plane clip, Lloyd relaxation | 123 | iterative |
| SDF placement search (`place_face`) | 51 | search with tiebreaks |
| Belly profile walk (`belly_patch`) | 48 | scanline, span selection |
| **Total new** | **~800** | |

For scale, v1's entire TypeScript port is 1143 lines. The new machinery
alone is comparable to the whole previous port, and it is the hardest
category to port exactly: iteration counts, tiebreaks, span selection
and tolerance comparisons all have to match, not just arithmetic.

The fixed-point spike (note 009) proved this is POSSIBLE — zero sign
flips, ulp-scale divergence, exact integer sqrt. It did not prove it is
cheap, and it deliberately did not estimate the cost of getting three
implementations to agree on every tiebreak.

## Option A — three full generators (v1's shape, scaled up)

Every port implements all ~800 lines. Byte-parity is proved at both
boundaries by the golden corpus, exactly as today.

- The charter's constraint holds in its strongest form: any client can
  derive a creature from DNA alone, offline, with no host.
- Cost is paid three times, and again on every generator change. A
  tiebreak adjustment is a three-language change plus a re-mint.
- The failure mode is subtle: a port that gets a tiebreak wrong fails
  only on the genomes that reach that branch, so corpus coverage has to
  be argued about rather than assumed.

## Option B — one generator, ports consume the render tree

Canonical Rust produces the render tree (note 006). Ports implement
only the SERIALIZER (tree → SVG) and their native draw path. Byte
parity is proved on the serializer.

- Port surface collapses to roughly the serializer's size. CKIOS
  already consumes the tree directly rather than parsing SVG, so for
  the first real consumer this is the shape that already exists.
- Consistent with the rule the light-y bug produced: resolved values
  ride the tree, consumers never re-derive.
- But a client can no longer go from DNA to creature by itself. It
  needs a tree from somewhere — which is a host round trip, or an
  embedded Rust core, or WASM.
- The wire contract already says identity is host-authored and clients
  are pure renderers. Option B is that principle carried one layer
  further; Option A keeps a capability no shipped consumer currently
  uses.

## What I would want to know before choosing

1. Does any planned consumer need offline DNA → creature with no host?
   If yes, A. If no, B is the cheaper truth.
2. Is the tree a stable enough contract to be the parity boundary? It
   has changed twice already (rev 1 → rev 2 after the corpus
   inventory), which is evidence against freezing it early and evidence
   for the corpus staying the spec of record either way.
3. Does WASM change the question? A Rust core compiled to WASM gives
   web and possibly iOS the generator without a port, which would make
   A affordable again for exactly the consumers B was meant to help.

## Corpus coverage is now measured, not argued

Option A's stated risk was that a port getting one tiebreak wrong fails
only on the genomes reaching that branch, so corpus coverage "has to be
argued about rather than assumed". `cargo run --bin grid -- --coverage
N` replaces the argument: it counts the divergence-prone branches a
seed range reaches and greedy-solves the smallest seed set covering
them.

Over 4000 seeds:

| Branch | Reach |
| --- | ---: |
| belly span switch (stay on one mass across a split body) | 67.6% |
| chain tiebreak (two segments within eps) | 26.6% |
| belly suppressed (too small to read) | 8.8% |
| marching-squares saddle (ambiguous 4-crossing cell) | 8.4% |
| extremity curvature clamp (capsule would self-cross) | **0.5%** |
| placement eyes-only fallback | **0%** |

Two results that change how the v2 corpus gets built:

1. **The curvature clamp is reached by 1 seed in 200.** An 18-vector
   corpus sampled at random has roughly a 9% chance of containing one.
   A port that clamps differently would pass a plausible-looking corpus
   and fail in production. Vectors must be SELECTED by branch coverage,
   not sampled — the greedy set covers every reachable branch in 2
   seeds, so this costs nothing.
2. **The eyes-only placement fallback is unreachable** across 4000
   seeds, because the search tries four eye spacings and the mouth
   scales with the radius, so some candidate almost always fits.
   Inspecting why exposed a latent contradiction: the fallback drew a
   mouth the search had just failed to fit, which the invariant checker
   would have flagged as a breach. The fallback would have produced
   output its own gate rejects — invisible only because it never fires.
   Fixed: the fallback now reports `mouth_fits = false` and the renderer
   draws no mouth. A creature with eyes and no mouth is a creature; a
   mouth hanging off the body is a defect.

The second one is the argument for doing this before the freeze rather
than after: under Option A that contradiction would have been
implemented three times, tested by nothing, and discovered the first
day a vocabulary change made the branch reachable.

## Non-negotiable either way

The golden corpus remains the spec of record and the parity gate. The
only thing under discussion is which boundary the ports implement, not
whether byte-identity is proved by artifact.
