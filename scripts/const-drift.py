#!/usr/bin/env python3
"""Compare shipped wild-module constants with playground/web/index.html.

This sampling-free check complements drift-check.sh's per-seed geometry
comparison. An unmatched constant fails even if no sampled seed reaches it.
Only distinct fractional values are gated: swapping values already present
elsewhere can leave the sets identical. The geometry comparison can catch
such swaps when a sampled seed reaches the changed calculation.

Occurrence counts are not comparable: the renderers factor helpers
differently, and the browser lab omits the shipped lid-band geometry.
Integers are reported but not gated because the lab also contains UI sizes
and seeds. Inspect those differences when changing geometry; this check
alone cannot catch a contour-resolution change from 110 to 130.

Comments are stripped; strings deliberately are not. The lab builds SVG
strings, where a literal inside `stroke-width='2.5'` affects geometry just
as much as a named binding does. Static HTML slider bounds are excluded.
"""

import glob
import os
import re
import sys

# Fractional values found on only one side require matching tuning, not a
# larger exception set. Remove exceptions when their literals disappear.
# Only Rust literals used solely by atan2/tan may be added, each commented
# as lab-unimplemented: the browser retains native SVG lid arcs instead.
# Those implementations currently need no exceptions.
BASELINE_RUST = set()
BASELINE_JS = set()

NUM = re.compile(
    r"(?<![\w.])(\d[\d_]*(?:\.\d[\d_]*)?)"
    r"(?:_?(?:f32|f64|i8|i16|i32|i64|u8|u16|u32|u64|usize|isize))?"
    r"(?![\w.])"
)


def constants(text):
    text = re.sub(r"//[^\n]*", "", text)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return {float(m.group(1).replace("_", "")) for m in NUM.finditer(text)}


def main():
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    rust = set()
    sources = sorted(glob.glob(os.path.join(root, "crates/avatar-svg/src/wild/**/*.rs"), recursive=True))
    if not sources:
        print("const-drift: no Rust sources found", file=sys.stderr)
        return 2
    for path in sources:
        with open(path, encoding="utf-8") as handle:
            rust |= constants(handle.read())
    lab_path = os.path.join(root, "playground/web/index.html")
    with open(lab_path, encoding="utf-8") as handle:
        # Static HTML slider bounds are UI, not generation constants. Scan
        # every script body so the full renderer remains covered.
        scripts = re.findall(r"<script\b[^>]*>(.*?)</script>", handle.read(), re.S)
        lab = constants("\n".join(scripts))

    rust_only = sorted(v for v in rust - lab if v != int(v))
    lab_only = sorted(v for v in lab - rust if v != int(v))
    new_rust = [v for v in rust_only if v not in BASELINE_RUST]
    new_lab = [v for v in lab_only if v not in BASELINE_JS]

    int_rust = sorted(int(v) for v in rust - lab if v == int(v))
    int_lab = sorted(int(v) for v in lab - rust if v == int(v))

    def show(values, limit=14):
        head = " ".join(str(v) for v in values[:limit])
        return head + (f" ... (+{len(values) - limit})" if len(values) > limit else "")

    if int_rust or int_lab:
        print(
            f"  ungated integers: {len(int_rust)} canonical-only,"
            f" {len(int_lab)} lab-only — expected to be harness only"
        )
        if int_rust:
            print(f"    canonical: {show(int_rust)}")
        if int_lab:
            print(f"    lab:       {show(int_lab)}")

    # Removed literals must leave BASELINE_RUST/BASELINE_JS too, or those sets
    # could silently excuse unrelated tuning introduced later.
    stale_rust = sorted(BASELINE_RUST - set(rust_only))
    stale_lab = sorted(BASELINE_JS - set(lab_only))
    if stale_rust or stale_lab:
        print("BASELINE STALE: entries that no longer occur, delete them")
        if stale_rust:
            print("  canonical: " + " ".join(str(v) for v in stale_rust))
        if stale_lab:
            print("  lab:       " + " ".join(str(v) for v in stale_lab))
        return 1

    if not new_rust and not new_lab:
        print(f"constants: {len(rust & lab)} shared, baseline unchanged")
        return 0

    print("CONSTANT DRIFT: a renderer carries tuning the other does not")
    if new_rust:
        print("  canonical only: " + " ".join(str(v) for v in new_rust))
    if new_lab:
        print("  lab only:       " + " ".join(str(v) for v in new_lab))
    print("  Mirror the renderer constant; only lab-unimplemented atan2/tan literals may be exempted.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
