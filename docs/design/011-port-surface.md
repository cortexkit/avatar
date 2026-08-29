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

## The deciding question changed (2026-08-25, second update)

Question 1 below was the one I said would settle this. **The stored-genome
architecture retired it.** No client derives a creature from an id any
more: the registry stores the genome, the wire delivers it, and clients
render what they are given. "Offline DNA → creature with no host" is not
a capability anyone is asking for, so A's headline advantage over B is
gone.

But B does not win by default, because the same architecture sharpened
what B actually costs. A client holding genome bytes still has to turn
them into pixels. Under B it cannot, on its own — it needs a tree, and a
tree is mood-specific, so serving trees over the wire means either seven
trees per agent or a round trip on every mood change, against 1 KiB that
covers all moods forever. Nobody should want that wire.

So the real fork, stated properly at last:

> **A** — each language implements each type's generator natively.
> **B** — one Rust generator, embedded on every platform via FFI or WASM,
> with ports reduced to drawing the tree it produces.

B was never "ask the host"; it was always "one implementation, linked
everywhere". That makes the deciding question a platform question rather
than a product one:

**Can every consumer platform embed a Rust core, or does some surface need
a native implementation?** iOS can link Rust. The web can take WASM. A
future surface that cannot is what would force A.

The cost asymmetry from the type ruling still stands and now bears fully
on this: under A, types x versions x languages grows forever; under B,
a new type is written once and every platform gets it by upgrading a
linked core.

## What I would want to know before choosing

1. ~~Does any planned consumer need offline DNA → creature with no
   host?~~ RETIRED — no client derives at all under the stored-genome
   model. Superseded by the platform question above.
2. Is the tree a stable enough contract to be the parity boundary? It
   has changed twice already (rev 1 → rev 2 after the corpus
   inventory), which is evidence against freezing it early and evidence
   for the corpus staying the spec of record either way.
3. Does WASM change the question? A Rust core compiled to WASM gives
   web and possibly iOS the generator without a port, which would make
   A affordable again for exactly the consumers B was meant to help.

## The only shipped consumer removed itself as a constraint (2026-08-29)

Unprompted, from the iOS client: the Swift port's value there was never
native-ness. It was that `renderTree` returns a **flat primitive list**
that their UIKit layer rasterizes and caches. Any option preserving that
output contract is invisible to them, and they asked that the fork be
weighed on maintenance cost rather than phone-side ergonomics.

This matters more than a preference, because it retires the reframed
deciding question for the one platform that had actually shipped. "Can
every consumer embed a Rust core" was worth asking only if some surface
needed a native implementation; iOS — the hardest embedding case and the
only live one — says it does not care how the list is produced.

What survives as a real constraint is narrower and worth stating plainly:
the **primitive list is the contract**, not the language it is built in.
An option that keeps the list and moves its producer is a refactor to
consumers. An option that changes the list's shape is a breaking change
no matter which language wins.

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

## The type ruling changes the arithmetic (2026-08-25)

This brief was written when the redesign was understood as a
generator_version bump: the wild pipeline would REPLACE the classic
creature, so porting cost was "port the new thing once".

The operator has since ruled that types are different representations
of the same genome, and that a change of depiction is a NEW TYPE rather
than a version bump. So the redesign ships as `creature.wild`
alongside `creature.classic`, which keeps its faces. That is better for
users — nobody's avatar changes without them choosing it — and it
removes the flag day. It also changes this decision materially, in a
direction the brief did not consider.

**Under Option A, every type is implemented in every language, forever.**
Not once: forever, because classic does not retire when wild arrives.
Today that is ~800 new lines of branch-heavy geometry times three
languages, on top of the classic paths already there. The next type
multiplies by three again. Old versions of each type are permanent
obligations too (note 012), so the matrix is types x versions x
languages and it only grows.

## The maintenance cost, measured (2026-08-29)

The iOS client asked that the fork be weighed on maintenance cost rather
than phone-side ergonomics. That deserved a measurement rather than the
assertion above, so here is the port surface counted by role. Lines are
a proxy, not the cost itself: what actually costs is HAND-WRITTEN
algorithm code carrying iteration counts and tiebreaks that three
languages must agree on. Generated tables are near-free, and separating
them is most of what makes these numbers honest.

| role | TypeScript | Swift |
|---|---|---|
| generated tables (minted, near-zero upkeep) | 2539 | 2086 |
| **hand-written generator logic** | **3334** | **3650** |
| tree, serializer, cache, API (what B keeps) | 307 | 871 |
| total | 6180 | 6607 |

So `creature.classic` alone carries **~7000 hand-written generator lines
across two ports**, all of it duplicating canonical Rust, all of it
held correct by the corpus and nothing else.

What `creature.wild` adds: the playground pipeline is 3374 lines of f64
prototype today, and the fixed-point implementation will be larger
(integer sqrt, Q32 arithmetic, explicit tiebreaks written out). Under A
that is ported twice by hand, and it is qualitatively harder than
classic rather than merely bigger: classic is spline flattening and
shape assembly, while wild adds SDF fields, smooth-min welds, marching
squares, Lloyd relaxation, arc-length resampling, a greedy placement
search and a belly profile walk. Every one has an iteration count or a
tiebreak that must match exactly. Two have already diverged between my
OWN two implementations (the contour chaining tiebreak and the marching
squares resolution, found by the drift check) — before any third
language existed.

The recurring cost is worse than the writing cost. Under A the corpus
must prove DNA to bytes for every type x version x language, and every
new vector multiplies across all of them. Under B there is one
generator, so cross-language byte-equality of the GENERATOR is
vacuously true, and the corpus shrinks to proving that each port's
serializer turns the same tree into the same SVG — a much smaller
property, and one that only TypeScript actually needs, since iOS
consumes the primitive list directly and never parses SVG.

**Summary of the measured position.** B removes ~7000 existing
hand-written lines and avoids roughly the same again per new type,
leaves ~1180 lines of tree/serializer/cache in place, and shrinks what
the corpus must prove. The cost it adds is a build and distribution
problem — an embedded Rust core per platform — rather than a
correctness one. That is the trade to argue, and it is not close on
volume alone.

**Under Option B it does not multiply.** A new type is implemented once
in canonical Rust; the ports gain nothing, because they implement the
serializer over whatever tree arrives. Types become a canonical-side
concern and the port surface stays fixed no matter how many exist.

That is a stronger argument for B than anything in the original brief,
and it arrived from the product model rather than from engineering
taste. It does not settle the question — offline DNA-to-creature is
still A's real advantage, and question 1 below is still the one that
decides — but the cost side is now visibly super-linear where it looked
linear before.

Worth noting what the ruling does NOT change: `creature.classic` is
already ported three ways and shipping, so B would not be a rewrite of
what exists. It is a decision about where the NEXT type lands.

## Settled: sampling cannot prove equivalence (2026-08-29, operator)

The operator's objection retires the fork, and on a stronger axis than
cost:

> The problem with output comparison is that when the genome gets big
> and some features are rarer, sampling may not surface the differences.
> Instead we should have a builder from a single source.

This is correct and it invalidates the premise every argument above was
built on. The golden corpus does not prove three implementations are
equivalent. It proves they agree on a **finite sample** of an input
space of 2^8192 today and 2^131072 under the v2 layout — a measure-zero
subset either way. Sampling can only fail to find a counterexample; it
cannot establish equality.

The coverage gate added earlier this week improves the sample and does
not change its nature. It targets branch coverage rather than random
draws, but it covers only the selectors I thought to enumerate, and it
deliberately excludes continuous fields — which is exactly where
rounding boundaries live. A divergence that fires for one input in 10^9
is invisible to it, and stays invisible until it renders on somebody's
phone.

And the problem grows with the design rather than shrinking: rarer
features mean lower per-branch probability, so the sample needed to
surface a divergence grows as the vocabulary gets richer. A test whose
power decreases as the thing it tests gets more ambitious is the wrong
shape of test.

**A single source makes divergence unrepresentable rather than
undetected.** That is the same move this project has made everywhere
else: folding spokes into a half-plane made asymmetry unrepresentable
instead of tested-for; capping the eye radius by the spacing made
overlap unrepresentable instead of checked. Three hand-written
generators verified by sampled output comparison is the one place the
codebase still relied on detection.

So the answer is Option B, and the corpus keeps a real job under it: it
stops being the thing that holds three implementations together and
becomes a regression gate on the one implementation — proof that a
refactor did not change what a genome renders to.

### What the tree vocabulary already supports

Checked before assuming a contract change: `creature.wild` needs
radial-gradient paint for the voronoi face-fade, mask geometry for the
body clip, and closed paths with a fill rule. `Paint::RadialGradient`,
`Effect::Masked` and `Shape::Path` already carry all three. The
primitive list the iOS client depends on does not have to change shape
to ship wild — which is what makes a preview on device cheap.

## Non-negotiable either way

The golden corpus remains the spec of record and the parity gate. The
only thing under discussion is which boundary the ports implement, not
whether byte-identity is proved by artifact.
