#!/usr/bin/env python3
"""Compare the golden-corpus mints from three platforms, byte for byte.

scripts/wild-cross-platform.sh mints every type's corpus trees natively on
macOS and inside Linux arm64 and x86_64 containers, then calls this. Every
platform must produce exactly the expected files, and every file must be
byte-identical: a creature must come out the same on every operating system
that draws it. This compares the native macOS mint with the committed trees
and with Linux on both architectures. It does not run the phone's embedded
core, and Windows is not covered here.

A mint holds one directory per type, named by the type's stem: `wild` for
creature.wild, and `wild-plus` and so on for the six newer types. The corpora
pin the same file names (every type pins wild-port-0005__neutral.json), so
each type is checked on its own, against its own committed trees, and file
names are never pooled across types: pooled names would let one type's tree
stand in for another's.
"""

import json
import sys
from pathlib import Path


WILD = "wild"
# The six newer types by stem. Each is registered as creature.<stem>, and
# its corpus list and committed trees live in vectors/<stem>/.
STEMS = ("wild-plus", "sprouts", "pixel-critters", "knitted-toys", "small-robots", "drawn-heads")
GENOME_BYTES = 16384


def tree_name(project_id, version, mood):
    """Version 1 trees keep their shipped names; later versions add __vN__."""
    return (f"{project_id}__{mood}.json" if version == 1
            else f"{project_id}__v{version}__{mood}.json")


def corpus_rows(path):
    """The rows of a corpus.tsv: (project_id, version, moods)."""
    rows = [line.split("\t") for line in path.read_text().splitlines()
            if line and not line.startswith("#")]
    assert all(len(row) == 3 for row in rows), f"{path}: rows are project_id, moods, version"
    return [(name, int(version), moods.split(",")) for name, moods, version in rows]


def wild_names(root):
    manifest = json.loads((root / "vectors/wild/manifest.json").read_text())
    assert (manifest["type"], manifest["genome_bytes"]) == ("creature.wild", GENOME_BYTES)
    assert "version" not in manifest, "the manifest records each row's version, not one for all"
    rows = corpus_rows(root / "vectors/wild/corpus.tsv")
    assert [(row["project_id"], row["version"], row["moods"]) for row in manifest["vectors"]] == [
        (name, version, moods) for name, version, moods in rows
    ], "shipped manifest differs from the frozen corpus list"
    assert sorted({version for _, version, _ in rows}) == [1, 2], "corpus versions"
    names = [tree_name(name, version, mood) for name, version, moods in rows for mood in moods]
    assert len(rows) == 13 and len(names) == len(set(names)) == 25, "short/duplicate corpus"
    return set(names)


def stem_names(root, stem):
    """A newer type's corpus: 12 genomes in neutral and one in all seven moods."""
    manifest = json.loads((root / "vectors" / stem / "manifest.json").read_text())
    assert (manifest["type"], manifest["genome_bytes"]) == (f"creature.{stem}", GENOME_BYTES), stem
    rows = corpus_rows(root / "vectors" / stem / "corpus.tsv")
    assert {version for _, version, _ in rows} == {1}, f"{stem} corpus versions"
    names = [tree_name(name, version, mood) for name, version, moods in rows for mood in moods]
    assert len(rows) == 12 and len(names) == len(set(names)) == 18, f"short/duplicate {stem} corpus"
    return set(names)


def expected_trees(root):
    """Maps each type's stem to the tree file names its corpus pins."""
    trees = {WILD: wild_names(root)}
    for stem in STEMS:
        trees[stem] = stem_names(root, stem)
    return trees


def shipped(root, stem):
    """Where a type's committed trees live. Wild's share vectors/tree with
    classic's; each newer type has its own vectors/<stem>/tree."""
    return root / "vectors/tree" if stem == WILD else root / "vectors" / stem / "tree"


def inventory(directory):
    if not directory.is_dir():
        return set()
    return {p.name for p in directory.iterdir() if p.is_file() and not p.is_symlink()}


def compare(label, reference, candidate, names, allow_classic=False):
    actual = inventory(candidate)
    missing = names - actual
    extra = actual - names
    if allow_classic:
        # vectors/tree also holds the classic corpus; only wild files count.
        extra = {name for name in extra if name.startswith("wild-")}
    differing = sorted(name for name in names & actual
                       if (candidate / name).read_bytes() != (reference / name).read_bytes())
    identical = len(names & actual) - len(differing)
    print(f"{label}: expected={len(names)} actual={len(actual)} identical={identical} "
          f"differing={len(differing)} missing={len(missing)} extra={len(extra)}")
    for kind, values in [("differing", differing), ("missing", sorted(missing)),
                         ("extra", sorted(extra))]:
        if values:
            print(f"  first {kind} files: {', '.join(values[:10])}")
    passed = identical == len(names) and not missing and not extra and not differing
    print(f"{'PASS' if passed else 'FAIL'} {label}")
    return passed


def layout(label, mint, trees):
    """Checks that a mint holds one directory per type and nothing else, so
    a type a platform skipped, or a tree written outside its type's
    directory, fails."""
    entries = {p.name: p for p in mint.iterdir()} if mint.is_dir() else {}
    missing = sorted(set(trees) - set(entries))
    stray = sorted(name for name, path in entries.items()
                   if name not in trees or path.is_symlink() or not path.is_dir())
    passed = not missing and not stray
    print(f"{'PASS' if passed else 'FAIL'} {label} layout: types={len(trees)} "
          f"missing={missing} stray={stray}")
    return passed


def platforms(root, macos, arm64, amd64):
    trees = expected_trees(root)
    print("expected trees: " + ", ".join(f"{stem}={len(names)}" for stem, names in trees.items()))
    mints = [("macOS", macos), ("linux/arm64", arm64), ("linux/amd64", amd64)]
    results = [layout(label, mint, trees) for label, mint in mints]
    for stem, names in trees.items():
        # Check the reference inventory before reading its files: an empty
        # native mint must fail, not become the expected set for two empty
        # Linux mints.
        actual = inventory(macos / stem)
        print(f"macOS {stem} inventory: expected={len(names)} actual={len(actual)}")
        if actual != names:
            print(f"FAIL macOS {stem} inventory: missing={sorted(names - actual)} "
                  f"extra={sorted(actual - names)}")
            results.append(False)
            continue
        results.append(compare(f"{stem}: macOS vs shipped fixtures", macos / stem,
                               shipped(root, stem), names, stem == WILD))
        for label, mint in mints[1:]:
            results.append(compare(f"{stem}: {label} vs macOS", macos / stem, mint / stem, names))
    return all(results)


if __name__ == "__main__":
    mode, *args = sys.argv[1:]
    if mode == "platforms" and len(args) == 4:
        sys.exit(0 if platforms(*(Path(arg) for arg in args)) else 1)
    sys.exit("usage: compare.py platforms ROOT MACOS ARM64 AMD64")
