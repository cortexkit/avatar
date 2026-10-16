#!/usr/bin/env python3
"""Compare wild corpus mints from three platforms, byte for byte.

scripts/wild-cross-platform.sh mints the corpus natively on macOS and inside
Linux arm64 and x86_64 containers, then calls this. Every platform must
produce exactly the expected files, and every file must be byte-identical:
the phone, a Mac and a Linux export host must draw the same creature.
"""

import json
import sys
from pathlib import Path


def expected_names(root):
    manifest = json.loads((root / "vectors/wild/manifest.json").read_text())
    assert (manifest["type"], manifest["version"], manifest["genome_bytes"]) == (
        "creature.wild", 1, 16384
    )
    rows = [line.split("\t") for line in
            (root / "vectors/wild/corpus.tsv").read_text().splitlines()
            if line and not line.startswith("#")]
    assert [(row["project_id"], row["moods"]) for row in manifest["vectors"]] == [
        (name, moods.split(",")) for name, moods in rows
    ], "shipped manifest differs from the frozen corpus list"
    names = [f"{name}__{mood}.json" for name, moods in rows for mood in moods.split(",")]
    assert len(rows) == 12 and len(names) == len(set(names)) == 18, "short/duplicate corpus"
    return set(names)


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


def platforms(root, macos, arm64, amd64):
    names = expected_names(root)
    # Check the reference inventory before reading its files: an empty native
    # mint must fail, not become the expected set for two empty Linux mints.
    actual = inventory(macos)
    print(f"macOS inventory: expected={len(names)} actual={len(actual)}")
    if actual != names:
        print(f"FAIL macOS inventory: missing={sorted(names - actual)} extra={sorted(actual - names)}")
        return False
    results = [compare("macOS vs shipped fixtures", macos, root / "vectors/tree", names, True)]
    for label, directory in [("linux/arm64 vs macOS", arm64), ("linux/amd64 vs macOS", amd64)]:
        results.append(compare(label, macos, directory, names))
    return all(results)


if __name__ == "__main__":
    mode, *args = sys.argv[1:]
    if mode == "platforms" and len(args) == 4:
        sys.exit(0 if platforms(*(Path(arg) for arg in args)) else 1)
    sys.exit("usage: compare.py platforms ROOT MACOS ARM64 AMD64")
