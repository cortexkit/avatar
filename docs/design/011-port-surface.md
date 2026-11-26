# Design note 011 — Port surface: what each language must implement

Status: DECIDED. This began as a decision brief, written before note 010
because 010's shape depended on the answer, and it keeps the order in
which the question was worked through. The decisions are in "Settled:
sampling cannot prove equivalence" (one generator, written once in Rust)
and "Determinism under single source" (IEEE f64 with no maths library).

## The question

The project requires Rust, TS and Swift to produce byte-identical SVG
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

- The cross-language constraint holds in its strongest form: any client can
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

- Port surface collapses to roughly the serializer's size. The phone app
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

Question 1 below was the one expected to settle this. **The stored-genome
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

The cost asymmetry from the type decision still stands and now bears fully
on this: under A, types x versions x languages grows forever; under B,
a new type is written once and every platform gets it by upgrading a
linked core.

## What would need to be known before choosing

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

Unprompted, from the phone app: the Swift port's value there was never
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

## The type decision changes the arithmetic (2026-08-25)

This brief was written when the redesign was understood as a
generator_version bump: the wild pipeline would REPLACE the classic
creature, so porting cost was "port the new thing once".

It has since been decided that types are different representations
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

The phone app asked that the fork be weighed on maintenance cost rather
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
tiebreak that must match exactly. Two had already diverged between the
project's OWN two implementations (the contour chaining tiebreak and the marching
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

Worth noting what the type decision does NOT change: `creature.classic` is
already ported three ways and shipping, so B would not be a rewrite of
what exists. It is a decision about where the NEXT type lands.

## Settled: sampling cannot prove equivalence (2026-08-29)

The project owner's objection, quoted below, retires the fork, and on a
stronger axis than cost:

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
draws, but it covers only the selectors someone thought to enumerate, and it
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
primitive list the phone app depends on does not have to change shape
to ship wild — which is what makes a preview on device cheap.

## The wild tree is 10x to 60x heavier than the classic one (2026-08-29)

Measured while minting the tree-JSON boundary, and it is a property of
the geometry rather than of the encoding:

| tree | primitives | path segments | JSON bytes |
|---|---|---|---|
| `seat_ava` (classic) | 41 | 200 | 35 KB |
| `pattern_3` (classic) | 31 | 368 | 45 KB |
| `wild_5004` | 12 | 1848 | 77 KB |
| `wild_5003` | 13 | 4842 | 193 KB |
| `wild_5002` | 38 | 11638 | 482 KB |

The cause is that the wild pipeline FLATTENS its curves. Classic keeps
cubic Beziers in the tree and lets the renderer tessellate; wild emits
marching-squares contours, spline flattening and voronoi cell outlines
as line segments, so the tree carries the tessellation instead of the
curve.

This matters well beyond file size. A consumer builds one path per
primitive and walks every segment, so a list of thirty wild
avatars is on the order of 350,000 segments where the same list of
classic avatars is about 6,000. It is also memory in any tree cache.

Not urgent and deliberately not fixed here — the preview path tolerates
it. But it is a real constraint on the wild pipeline.

Measured on the shipped path (2026-10-16, the phone app's release build on an
Apple-silicon Mac standing in for the phone): decoding the tree JSON in
Swift costs about 0.06 ms per KB, so 4–5 ms for a 71 KB tree and 12–15 ms
for a 220 KB one. The whole embedded call, generation plus hand-off, is
about 16 ms per creature, and a cold call costs the same as a warm one.
Thirty creatures are about 0.5 s of CPU at cold launch, run in parallel
off the main thread, plus about 1 ms of rasterizing each on the main
thread. The phone app judged that acceptable, so tree weight stays deferred.
Confirmed on real hardware (a TestFlight build on a physical iPhone): the
embedded core draws the wild reference creature byte-identical to the
corpus in 24.2 ms, putting thirty creatures near 0.7 s of background CPU at
cold launch. Revisit if that grows much worse.

Most of that cost turned out to be repeated data, not segment count. Wild
creatures clone one body mask into many primitives, and the tree JSON wrote
every copy in full: 77% of the bytes across the 18 committed wild trees.
The JSON now writes each distinct mask once, in a top-level `masks` table,
and a masked effect refers to its entry by index (design note 006). The wild
trees shrank from 3,537,942 to 820,554 bytes, and the largest from 345 KB to
54 KB. Measured by the phone app (2026-11-20, release build, iPhone 17 Pro
simulator, medians of five warm runs, its own rasterizer at 40 pt x3), decode
plus raster per creature went from 26.30 to 3.99 ms on the largest tree and
from 13.94 to 4.32 ms on a typical one, 3.2x to 6.6x faster, almost all of it
in decode. Rasterizing stays below 0.6 ms, so the segment count is not the
limit at dock sizes today.

### The obvious fix does not work, measured

This note previously asserted the remedy: fit curves to the extracted
contours instead of shipping their flattened polylines. That assertion
was wrong, and `playground/src/fit.rs` plus `--fit` exists to say so with
numbers rather than to leave it standing.

Fitting cubics to the contours the pipeline produces today, with
reparameterization and corner preservation, collapses 2388 segments to
1027 curves at a half-milliunit tolerance and 732 at two milliunits.
That is **2.3x to 3.3x**, against the 10x to 60x the weight table needs.
Worse, the true geometric deviation lands at 0.4 to 1.9 canvas units and
barely responds to tolerance at all.

The reason is that these contours are not a smooth curve sampled densely.
They are already decimated, so there is little redundancy left to remove,
and what remains is a mix of real features and marching-squares grid
noise that no smooth curve passes through. The fit bottoms out at
roughly two points per curve. One pass of 3-point smoothing before
fitting moves the ratio by under ten percent, which rules out jaggedness
as the limiter.

So the win is not downstream of the flattening; it is the flattening.
The pipeline tessellates a spline, merges the tessellation with a signed
distance field, and re-extracts a polyline by marching squares — curve
information is destroyed in the middle and cannot be recovered by fitting
afterwards. Getting a light tree means carrying curves through the merge,
or extracting the merged contour at a resolution high enough that fitting
has something to work with. That is a real piece of design, not a
transform to apply at the end, and it belongs with the fixed-point port.

Three things the spike established that are worth keeping:

- Reparameterization is not optional. Without it the fitter's error
  metric measures the wrong distance, so tightening tolerance buys more
  curves and no accuracy — counts move, deviation does not.
- Corners must be cut before fitting, and the ring seam must be tested
  for being a corner like any other point. A seam is an arbitrary index,
  so forcing a smooth closure there rounds away one feature per shape at
  a position nobody chose.
- A representation change must not change the silhouette, so the corner
  test is the load-bearing one. It caught reparameterization rounding a
  square, which is exactly the failure a ratio would have hidden.

Measured on device rather than guessed, by the phone app: all thirty
heavy trees decode and rasterize in well under the screen's first
settle on an M-series simulator, at 108pt tiles, with no list jank.
So the weight is tolerable at preview scale — but that number says
nothing about the shipped path, which will carry fitted curves and a
different tree entirely. Recorded so nobody later cites a preview
measurement as evidence the flattening is fine.

One claim from this note did hold under test: the primitive vocabulary
already covered the second type. Gradient paint, gradient-content masks
and path fill rules all drew with zero changes to the client's
rasterizer, so introducing a second creature type did not change the
contract its consumers depend on.

## Numeric primitives the port needs and does not have

The fixed-point spike proved the NEW machinery portable — SDF fields,
marching squares, Lloyd relaxation, integer sqrt. It never inventoried
what the pipeline calls that the shipped crates cannot supply, which is
a different question and the one that costs a refactor if it surfaces
mid-port. Answered here.

`avatar-tables` exposes exactly `div_round_half_away`, `sin_q30`,
`cos_q30` and the easing tables. Against that:

- **Integer sqrt is missing entirely.** Wild needs it on the render
  path in at least three places: SDF distance (`merge.rs`), arc-length
  accumulation while resampling, and the belly crown ramp. The spike
  proved a 64-step restoring algorithm exact, but it lives in a
  playground binary. It belongs in `avatar-tables`, hash-pinned with
  the other primitives, because a second implementation of sqrt is a
  second place for the rounding to differ.

- **Integer atan2 exists but is in the wrong place, and is O(16384).**
  `lashes.rs` carries a private `atan2_bam` that scans every fourth BAM
  angle and takes the maximum dot product. Correct and portable, and
  fine for the handful of lash tangents classic asks of it. A second
  consumer would not find it, and would pay 16,384 dot products per
  call if it did.

  WRONG, AND WORTH KEEPING AS A RECORD OF HOW. An earlier revision of
  this note claimed wild needs inverse trig nowhere: that `arc_to` (the
  only caller of `atan2` and `tan`) was reached only from `face::eyes`,
  a blob-era function the wild path never calls because it draws
  through `face::eyes_inked`. That was false. `eyes_inked` delegates to
  `eyes`, so the live chain is `draw_full -> eyes_inked -> eyes ->
  arc_to`, and the Lidded eye's lid band and the HalfLid arc are drawn
  through it. A review of the port found it; a call graph confirms
  it.

  The irony is specific: the "correction" was written to fix a first
  pass that counted call sites without checking reachability, and it
  made the same mistake one level up, by checking who called `arc_to`
  without checking who called its caller. Reachability has to be
  followed to an entry point, not one hop.

  Consequence for the port: `arc_to` is live and ports unchanged in
  control flow. Under the f64 decision below it needs `tan` and `atan2`
  rebuilt from basic operations, each gated against the standard
  library, rather than integer versions.

- **A caveat on this whole inventory.** It was assembled by searching
  for calls, and the `atan2` entry above shows what that misses: a
  symbol can be called from code that is itself unreachable. The
  playground is structured as a lib, so `pub` suppresses dead-code
  analysis entirely — `cargo clippy -D warnings` passes over an entire
  retired generator. Anything ported out of here must be checked for
  REACHABILITY from `decode_full`/`draw_full`, not merely for callers.

- **Forward trig is already covered.** `wild.rs` uses cos/sin for spoke
  placement, extremity roots and spines, the half-plane fold test and
  ray casting, all of which map onto the existing BAM tables.

- **The genome float is cosmetic.** `Bits::lin` returns f64, but it is
  a linear map of an integer draw and ports as a multiply and a shift.
  The splitmix64 bit source underneath is already pure integer.

## Determinism under single source: IEEE f64, no maths library (2026-10-15)

The fixed-point inventory above assumed the wild port would rewrite the
pipeline in integer arithmetic, which is how classic is built. That
discipline was chosen so THREE LANGUAGES could implement the generator
independently and still agree byte for byte. Single source removed the
language dimension: only Rust builds trees now, compiled for each
platform. What remains is whether one Rust codebase yields identical
bytes on every platform it compiles for, and that was measured rather
than assumed.

The same 206 wild trees (200 seeds, plus every mood of the first),
minted by the playground's f64 pipeline on three platforms:

| comparison | identical | differ |
| --- | --- | --- |
| macOS arm64 vs Linux arm64, libm trig | 199 | 7 |
| macOS arm64 vs Linux x86_64, libm trig | 198 | 8 |
| macOS arm64 vs Linux arm64, trig from + - * / only | 206 | 0 |
| macOS arm64 vs Linux x86_64, trig from + - * / only | 206 | 0 |

IEEE 754 requires `+ - * /` and `sqrt` to be correctly rounded, Rust
does not contract them into fused operations, and every platform Rust
targets implements them in hardware or exactly in software. The
platform maths library is the only non-portable layer, and on the wild
path it was exactly four `sin`/`cos` call sites in `wild.rs`. Replacing
them with a reduction and polynomial built only from basic operations
made every tree identical on all three platforms. The 206/206 result
corroborates the argument; the argument is what makes it hold for
inputs the sample never reached.

The divergence was not cosmetic, which is why "close enough" is not
enough. On the same machine, libm trig and basic-operation trig agree
to about 1e-14, yet 72 of the 206 trees changed, and in one a Voronoi
path moved by roughly 93 canvas units out of 240. A difference in the
14th digit crossed a branch: a half-plane fold test, a relaxation step,
a tiebreak. In this pipeline, tiny numeric differences do not stay tiny.

DECISION: the wild port keeps f64 and does not rewrite into fixed
point. Determinism comes from three rules, each enforced by a gate
rather than by care:

- No transcendental maths-library call anywhere on the generation path
  (sin, cos, tan, their inverses and hyperbolics, exp, ln, log, powf,
  powi, cbrt, hypot, mul_add). Forward trig comes from one shared
  basic-operation implementation. Enforced with clippy's
  `disallowed-methods`.
- `sqrt` is allowed: IEEE requires it correctly rounded.
- No nondeterministic iteration (randomly seeded hash maps) and no
  clocks on the generation path.

Classic keeps its fixed-point implementation unchanged. The ports
still carry an independent classic generator, which is the case
fixed-point was built for.

> Note (2026-10-26): Native Swift/TS classic builders were removed; every creature generator now lives only in Rust, and ports consume render trees.

What this removes from the inventory above: integer sqrt, the integer
atan2 question, and the fixed-point rewrite itself. The port becomes a
move of the playground pipeline into the shipped crates, with four
trig sites swapped and a gate against reintroducing them.

## Non-negotiable either way

The golden corpus remains the spec of record and the parity gate. The
only thing under discussion is which boundary the ports implement, not
whether byte-identity is proved by artifact.
