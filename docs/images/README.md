# README images

The creatures shown in the top-level README. Each is a 256 px PNG drawn by
`avatar-export` from a random genome, at version 1 and with the ground shadow
kept. The genomes come from `random_sheet`, which derives genome `CELL` of a
sheet from its seed (see the comment at the top of
`crates/avatar-vectors/examples/random_sheet.rs`).

| File | Type | Seed | Cell | Mood |
|---|---|---|---|---|
| `small-robots.png` | `creature.small-robots` | 1009 | 2 | neutral |
| `knitted-toys.png` | `creature.knitted-toys` | 1009 | 0 | neutral |
| `sprouts.png` | `creature.sprouts` | 1003 | 6 | neutral |
| `pixel-critters.png` | `creature.pixel-critters` | 1008 | 6 | neutral |
| `drawn-heads.png` | `creature.drawn-heads` | 1009 | 6 | neutral |
| `wild-plus.png` | `creature.wild-plus` | 1009 | 7 | neutral |
| `mood-<mood>.png` | `creature.small-robots` | 1009 | 2 | each of the seven moods |

To draw one again:

```sh
cargo run --release -p avatar-vectors --example random_sheet -- \
    --type creature.small-robots --seed 1009 --count 16 \
    --genome-out /tmp/genomes --out /tmp/sheet.png
cargo run --release -p avatar-export -- \
    --genome-file /tmp/genomes/2.hex --type creature.small-robots --version 1 \
    --mood neutral --size 256 --out small-robots.png
```

A change to a generator that moves one of these creatures shows here only
after the image is drawn again; nothing checks that they stay current.
