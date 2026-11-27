# Fidelity harness

Checks that a Rust generator draws what its JavaScript reference draws. The
references are the six 2D generators the new creature types are ported from.
They are not in this repository: a worktree that needs them gets a copy of
the reference folder at `.cortexkit/internal-docs/ports/2d-leads/` (git
ignores it), or passes another location with `--refs DIR`. Without the
folder, every command that needs it exits with status 2 and names the path
it looked at. Nothing in `scripts/check.sh` runs the harness.

| file | what it does |
|---|---|
| `wild-plus.js`, `sprouts.js`, `pixel-critters.js`, `knitted-toys.js`, `small-robots.js`, `drawn-heads.js` | one shim per type: builds its manifests and runs its reference in Node |
| `lib.js` | what the shims share: manifests, checks, output |
| `../../crates/avatar-raster/examples/fidelity_compare.rs` | compares a reference run with a port run and writes the report |
| `selftest.py` | checks that the comparison passes a run against itself and fails where it must |
| `test.js` | tests of the shared checks; needs neither the references nor cargo |

## A type's fidelity run

```sh
S=wild-plus; R=target/fidelity/$S
node scripts/fidelity/$S.js manifest --out $R/base.json          # the 1,855 base cases
node scripts/fidelity/$S.js run --manifest $R/base.json --out $R/reference
cargo run --release -p cortexkit-avatar-$S --example fidelity_port -- \
    --manifest $R/base.json --out $R/port
node scripts/fidelity/$S.js corpus --out $R/corpus.json          # from vectors/$S/corpus.tsv
node scripts/fidelity/$S.js run --manifest $R/corpus.json --out $R/corpus
cargo run --release -p cortexkit-avatar-raster --example fidelity_compare -- \
    --manifest $R/base.json --reference $R/reference --candidate $R/port \
    --corpus-manifest $R/corpus.json --corpus-choices $R/corpus --report $R/report.md
```

Branches the base cases miss are covered by probe genomes. `sweep --count N`
builds N random genomes and counts the branches they reach; with `--list FILE`
it also writes, for every branch reached, the first genome reaching it, as a
probe list. `probes --genomes LIST --out FILE` turns lines of
`name TAB hex [TAB moods]` into a probe manifest. A probe run writes into the
same two directories as the base run: give the shim both manifests
(`run --manifest base.json --manifest probes.json`), run the port once per
manifest, and give `fidelity_compare` the probes with `--probes FILE`.
Probes are never committed and never replace a base case.

Every `run` prints the Node version, the reference's source commit, the size
and each manifest's SHA-256, for the report.

## Manifests

A manifest is JSON:

```json
{
  "format": "avatar-fidelity-manifest/1",
  "type": "creature.wild-plus",
  "kind": "base",
  "genomes": [{"name": "fleet-00", "source": "kit fleet-shaped genome 0", "hex": "<32,768 hex digits>"}],
  "cases": [{"id": "fleet-00__neutral", "genome": "fleet-00", "mood": "neutral"}]
}
```

- `kind` is `base`, `probe` or `corpus`.
- A case ID is always `<genome>__<mood>`, and genome names are lowercase
  letters, digits and hyphens, so a case ID is a safe file name.
- A repeated case ID, or a probe ID that is also a base ID, stops the shim
  and the comparison before either writes anything.
- The base manifest holds 1,855 cases: 265 genomes in all seven moods. They
  are the kit's 33 fleet-shaped genomes (`fleet-00` to `fleet-32`) and 24
  random genomes (`random-00` to `random-23`), the type's 96 variety genomes
  as its variety page makes them (`variety-00` to `variety-95`), and the
  first 16 genomes `dice.js` rolls for each seed `r1` to `r7`
  (`dice-r1-01` to `dice-r7-16`).
- Fleet-shaped genomes are real stored genomes, so manifests and run
  directories stay out of the repository.

## A run's output

For every case, `<case>.svg` and `<case>.choices.json`. The shim also writes
`domains.json`.

- **`<case>.svg`** is the reference's SVG for the case's genome and mood,
  drawn at `--size` pixels (256 by default).
- **`<case>.choices.json`** is one flat JSON object of the creature's discrete
  choices, plus `body_rgb`.
  - Values are integers, strings or booleans. `body_rgb` is three integers
    from 0 to 255.
  - A key the creature does not have (a pod robot's chassis, the felt shape
    of an eye that is not felt) is left out.
  - A non-integer number, any other array, `null` or an object is a harness
    error in the shim and in the comparison.
- **`domains.json`** gives, for every key except `body_rgb`, the values the
  reference's own selector can give from a valid genome. The shim refuses to
  write a choice outside its domain.

The keys are paths into the reference's built creature. They cover the
fields the reference's own summary reports, and the named genes beside them.
For `body_rgb`, each shim takes the reference's body colour:

| type | keys | `body_rgb` |
|---|---|---|
| wild-plus | plan, anatomy read from `B.notes` (body detail, crest, arms, tail, feet, slots per limb kind), coat, surface, belly, palette recipe and scheme, eye plan and style, mouth, nose | `P.body` |
| sprouts | body, palette recipe and foliage, which shoots grow, what they bear, leaf, flower and fruit forms, lobes, roots, coat, skin, eyes, mouth | `P.body` |
| pixel-critters | the named genes of `C.G`, the palette recipe, and the eye plan, eye kind and mouth the neutral sprite draws | `P.body` |
| knitted-toys | the discrete traits `decode` writes into `T` | the main yarn, `pal.main.hex` |
| small-robots | each field `summaryOf` reports, and the palette recipe | `P.body` |
| drawn-heads | each field `summaryOf` reports, the age marks, and the palette recipe | `P.body` |

A port's `examples/fidelity_port.rs` writes the same two files per case: the
SVG from `serialize_tree`, and the choices from its `decode` mapped onto
these keys. The mapping lives in the port crate.

## Sizes

The shim asks the reference to draw at `--size` pixels, and
`fidelity_compare --size` draws both sides at the same size. The base
profile is 256 px. Ports follow one rule: `detail_px = None` draws what the
reference draws at 256 px, and `Some(n)` draws what it draws at `n` px. That
covers the discrete switches and the continuous terms below. For a
size-dependent reference, a generator slice also runs the shim and the
comparison at 48 and 96 px.

What the requested size changes in each reference:

- **wild-plus, sprouts:** only the `width` and `height` attributes.
- **pixel-critters:** with one device pixel per CSS pixel (the shim passes
  `dpr: 1`), a cell is `max(1, floor(size / 24))` pixels. The sprite is
  centred with a margin of `floor((size - 24 * cell) / 2)` pixels, which is
  avatar-raster's pixel grid at that size.
- **knitted-toys:**
  - The detail level is 2 at 100 px and up, 1 from 56 px, and 0 below. It
    is read in 26 places.
  - One output pixel is `pxu = 200 / px / fit.s` toy units. It sets rim
    widths, shadow offsets and other widths continuously, in 19 places.
  - An eye's smallest radius changes at 40 px.
- **small-robots:** with `px = size / 240`:
  - Line widths are `lineW = clamp(1.05 / px, 2.2, 8.5)`,
    `faceW = clamp(1.3 / px, 3.2, 7.8)` and `thin = 1.3 / px`, which is
    unclamped.
  - `markK = clamp(64 / size, 1, 1.5)`.
  - Fine detail is kept at 96 px and up, shading at 56 px and up.
  - A stroke with `w * px < 1.7` and a dot with `r * px < 0.9` are drawn
    simpler. A glint shows when `r * px >= 2.6`. Some parts' radius is at
    least `1.15 / px`.
  - Puffs, steam and a status mark appear only at 48 px and up.
- **drawn-heads:** with `px = size / 240`:
  - Line widths are `lineW = clamp(1.2 / px, 3.0, 8.4)` and
    `penW = clamp(1.25 / px, 2.7, 7.6)`.
  - `markK = clamp(64 / size, 1, 1.5)`.
  - Fine detail is kept at 96 px and up; `tiny` holds below 56 px.
  - Steam appears only at 48 px and up, with an edge of
    `clamp(0.85 / px, 1.3, 2.6)`.

## The comparison

`fidelity_compare` (see its module documentation for every rule):

- Each directory must hold exactly the base and probe cases, as an SVG and a
  choices file each. A missing case fails, even when it is missing from both
  sides, and so does an unexpected file.
- Both SVGs are drawn with `avatar_raster::rasterize_svg`, premultiplied, on
  a transparent background, and every RGBA channel is compared.
- A pixel fails when a channel differs by more than 8/255. An image fails
  when more than 1% of its pixels fail.
- The choices must have the same keys and equal values.
- The report lists the failing cases, the mismatches per key, and every
  branch (a key and a value of `domains.json`) with how many base, probe and
  corpus cases reach it. A branch neither the base nor the probe cases reach
  is *missed*; one the corpus does not reach is *unpinned*.
- Exit status: 0 when every case passes, 1 when a case fails, 2 for a harness
  error, which writes no report.

`python3 scripts/fidelity/selftest.py --manifest BASE --run DIR` runs the
comparison on a reference run against itself and on altered copies, and
checks each outcome.
