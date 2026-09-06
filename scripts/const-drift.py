#!/usr/bin/env python3
"""Compares the tuning constants of the two wild renderers.

This is the sampling-free half of the drift check. The geometry half
compares rendered output for a handful of seeds, and it reported the two
renderers in agreement while the lab drew every mouth 0.8r above
canonical: the divergent constant sat in a derivation downstream of
everything the probe carried, so no placement field disagreed. A constant
that no sampled seed reaches still differs.

Only FRACTIONAL literals are compared. Integers in both languages are
dominated by indices, counts and colour components, which differ for
uninteresting reasons and would bury the signal.

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
