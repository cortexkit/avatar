#!/usr/bin/env python3
"""Compares the tuning constants of the two wild renderers.

This is the sampling-free half of the drift check. The geometry half
compares rendered output for a handful of seeds, and it reported the two
renderers in agreement while the lab drew every mouth 0.8r above
canonical: the divergent constant sat in a derivation downstream of
everything the probe carried, so no placement field disagreed. A constant
that no sampled seed reaches still differs.

WHAT THIS CANNOT SEE, stated because a gate whose range is implied gets
trusted past it. This compares SETS of distinct values, so it sees a
change only when one introduces or removes a distinct value. Editing a
constant to a value that already appears elsewhere in the same file
leaves the set identical and this stays silent — verified, not reasoned:
changing a spoke modulo from 12 to 11 produces no output, and changing
it to 97 names 97 immediately.

Counting occurrences instead of collecting them would close that, and it
is not viable here: only 51% of shared fractional constants occur the
same number of times in the two renderers, because the canonical side
carries sheet-rendering code the lab has no counterpart for and the two
factor their helpers differently. A gate with 38 standing mismatches
teaches people to ignore it.

The geometry half of drift-check.sh covers the complementary case: a
swap between already-present values changes rendered output, which the
per-seed comparison catches whenever a sampled seed reaches it. Neither
half is sufficient; the pair is what has range.

Only FRACTIONAL literals are GATED. Integers are reported and not gated,
which is a deliberate compromise rather than an oversight, and the report
is the point: an instrument that silently skips a span will one day be
trusted about the span it skipped.

The integer difference is currently all harness — sheet seeds, cell
coordinates, lab preview sizes. Gating it would mean a baseline that
churns on every harness edit, which trains the habit of widening a
baseline to make a gate pass. But the span is not empty of risk: the
first divergence this pair ever had was marching-squares resolution 110
against 130, an INTEGER, and a fractional-only comparison would not have
seen it. So the numbers are printed every run, and a pipeline-shaped
value appearing there is a prompt to look.

Comments are stripped; strings deliberately are NOT. Both renderers build
SVG by formatting, so a literal inside `stroke-width='2.5'` is as
load-bearing as a named binding. An earlier version stripped strings and
hid three real constants behind JS template literals, which produced
false differences and nearly buried the real one.
"""

import glob
import os
import re
import sys

# The CURRENT justified difference, each entry checked individually rather
# than labelled in bulk:
#   0.09, 0.58  blob-era face code (blush_with_color, face::belly). The
#               wild path never reaches them and the lab never implemented
#               them. These leave when that dead code does.
#   5.1, 6.3    not constants at all: the format specifiers {pct:>5.1} and
#               {:>6.3}, which a numeric extractor cannot distinguish from
#               numbers.
#   1.6         a lab slider bound. UI, not pipeline.
#
# A NEW entry is the signal this exists for. Shrink the baseline when the
# dead code goes; never grow it to make the check pass.
BASELINE_RUST = {0.09, 0.58, 5.1, 6.3}
BASELINE_JS = {1.6}

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
    sources = sorted(glob.glob(os.path.join(root, "playground/src/*.rs")))
    if not sources:
        print("const-drift: no Rust sources found", file=sys.stderr)
        return 2
    for path in sources:
        with open(path, encoding="utf-8") as handle:
            rust |= constants(handle.read())
    lab_path = os.path.join(root, "playground/web/index.html")
    with open(lab_path, encoding="utf-8") as handle:
        lab = constants(handle.read())

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

    if not new_rust and not new_lab:
        print(f"constants: {len(rust & lab)} shared, baseline unchanged")
        return 0

    print("CONSTANT DRIFT: a renderer carries tuning the other does not")
    if new_rust:
        print("  canonical only: " + " ".join(str(v) for v in new_rust))
    if new_lab:
        print("  lab only:       " + " ".join(str(v) for v in new_lab))
    print("  Fix the renderer, or justify the entry and add it above.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
