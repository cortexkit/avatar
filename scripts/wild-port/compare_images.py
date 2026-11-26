#!/usr/bin/env python3
"""Compare finished images drawn on several platforms with the golden images.

scripts/wild-cross-platform.sh image runs the image-hashes example
(crates/avatar/examples/image-hashes.rs) natively on macOS and in Linux arm64
and x86_64 containers, then calls this with each platform's output: a list of
`<sha256>  <file>` lines, one per golden image, and a directory holding the
same images as PNGs. Output from another machine, such as Windows, is passed
the same way.

Each platform must list exactly the cases in vectors/image/manifest.json, and
each hash must equal the SHA-256 of the golden PNG's decoded RGBA pixels.
Images are compared exactly. When a hash differs, the platform's dumped PNG
is decoded and compared with the golden pixel by pixel, and the report gives
the platform, the case, the largest difference in any channel and the share
of pixels that differ. It also says whether that difference is within the
loosest tolerance this project allows a maintainer to set for images, 2/255
per channel on at most 0.5% of an image's pixels. The comparison fails either
way: the report is what a maintainer reads to decide whether to keep exact
comparison.

Only the standard library is used. The PNG reader handles what the goldens
and the dumps use, 8-bit RGBA without interlacing, with any filter type.
"""

import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
# The loosest image tolerance allowed: at most this channel difference, on
# at most this share of an image's pixels.
CEILING_CHANNEL = 2
CEILING_SHARE = 0.005


def read_png(path):
    """Returns (width, height, RGBA bytes) of an 8-bit RGBA PNG."""
    data = Path(path).read_bytes()
    if data[:8] != PNG_SIGNATURE:
        raise ValueError(f"{path}: not a PNG file")
    header = None
    compressed = []
    at = 8
    while True:
        if at + 8 > len(data):
            raise ValueError(f"{path}: ends before IEND")
        length, kind = struct.unpack(">I4s", data[at:at + 8])
        body = data[at + 8:at + 8 + length]
        crc = data[at + 8 + length:at + 12 + length]
        if len(body) != length or len(crc) != 4:
            raise ValueError(f"{path}: truncated {kind!r} chunk")
        if zlib.crc32(kind + body) != struct.unpack(">I", crc)[0]:
            raise ValueError(f"{path}: {kind!r} chunk fails its CRC")
        at += 12 + length
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            compressed.append(body)
        elif kind == b"IEND":
            break
    if header is None:
        raise ValueError(f"{path}: no IHDR chunk")
    width, height, *form = header
    if form != [8, 6, 0, 0, 0]:
        raise ValueError(f"{path}: not 8-bit RGBA without interlacing (IHDR {header})")
    stride = width * 4
    raw = zlib.decompress(b"".join(compressed))
    if len(raw) != height * (stride + 1):
        raise ValueError(f"{path}: {len(raw)} bytes of scanlines for a {width}x{height} image")
    pixels = bytearray()
    previous = bytearray(stride)
    for row in range(height):
        start = row * (stride + 1)
        line = unfilter(raw[start], bytearray(raw[start + 1:start + 1 + stride]), previous, path)
        pixels += line
        previous = line
    return width, height, bytes(pixels)


def unfilter(kind, line, previous, path):
    """Reverses one scanline's PNG filter; RGBA8 has 4 bytes per pixel."""
    if kind == 0:
        return line
    if kind == 2:
        return bytearray((a + b) & 0xFF for a, b in zip(line, previous))
    if kind not in (1, 3, 4):
        raise ValueError(f"{path}: unknown filter type {kind}")
    for i in range(len(line)):
        left = line[i - 4] if i >= 4 else 0
        up = previous[i]
        if kind == 1:
            predictor = left
        elif kind == 3:
            predictor = (left + up) >> 1
        else:
            upper_left = previous[i - 4] if i >= 4 else 0
            estimate = left + up - upper_left
            to_left, to_up, to_upper_left = (abs(estimate - left), abs(estimate - up),
                                             abs(estimate - upper_left))
            if to_left <= to_up and to_left <= to_upper_left:
                predictor = left
            elif to_up <= to_upper_left:
                predictor = up
            else:
                predictor = upper_left
        line[i] = (line[i] + predictor) & 0xFF
    return line


def write_png(path, width, height, pixels):
    """Writes 8-bit RGBA pixels verbatim, unfiltered."""
    assert len(pixels) == width * height * 4
    stride = width * 4
    scanlines = b"".join(b"\0" + pixels[row * stride:(row + 1) * stride] for row in range(height))

    def chunk(kind, body):
        return (struct.pack(">I", len(body)) + kind + body
                + struct.pack(">I", zlib.crc32(kind + body)))

    Path(path).write_bytes(PNG_SIGNATURE
                           + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
                           + chunk(b"IDAT", zlib.compress(scanlines))
                           + chunk(b"IEND", b""))


def golden_cases(root):
    """Maps each manifest file name to its golden PNG's path and pixel hash."""
    image = Path(root) / "vectors/image"
    entries = json.loads((image / "manifest.json").read_text())["entries"]
    cases = {}
    for entry in entries:
        name = entry["file"]
        if name in cases:
            raise ValueError(f"vectors/image/manifest.json lists {name} twice")
        width, height, pixels = read_png(image / name)
        if (width, height) != (entry["size_px"], entry["size_px"]):
            raise ValueError(f"vectors/image/{name} is {width}x{height}, "
                             f"not {entry['size_px']} px square")
        cases[name] = (image / name, hashlib.sha256(pixels).hexdigest())
    if not cases:
        raise ValueError("vectors/image/manifest.json lists no images")
    return cases


def read_hashes(path):
    """Reads `<sha256>  <file>` lines. Returns (hashes, problems).

    Windows PowerShell may save redirected output as UTF-16 with CRLF line
    ends, so both encodings and both line ends are accepted.
    """
    data = Path(path).read_bytes()
    text = data.decode("utf-16") if data[:2] in (b"\xff\xfe", b"\xfe\xff") else data.decode("utf-8-sig")
    hashes, problems = {}, []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        digest, separator, name = line.partition("  ")
        if (not separator or len(digest) != 64
                or any(c not in "0123456789abcdef" for c in digest) or not name):
            problems.append(f"line {number} is not '<sha256>  <file>': {line!r}")
        elif name in hashes:
            problems.append(f"line {number} lists {name} again")
        else:
            hashes[name] = digest
    return hashes, problems


def difference(expected, actual):
    """Largest channel difference and number of differing pixels."""
    largest = 0
    differing = 0
    for pixel in range(0, len(expected), 4):
        a, b = expected[pixel:pixel + 4], actual[pixel:pixel + 4]
        if a != b:
            differing += 1
            largest = max(largest, *(abs(x - y) for x, y in zip(a, b)))
    return largest, differing


def describe(platform, name, golden_path, listed_hash, dump):
    """One line on how a differing case differs from its golden."""
    prefix = f"  {platform} {name}:"
    if dump is None or not (dump / name).is_file():
        return f"{prefix} hash differs; no dumped image to measure (rerun the producer with --dump)"
    width, height, actual = read_png(dump / name)
    expected_width, expected_height, expected = read_png(golden_path)
    if hashlib.sha256(actual).hexdigest() != listed_hash:
        return f"{prefix} the dumped image's pixels do not have the listed hash"
    if (width, height) != (expected_width, expected_height):
        return f"{prefix} dumped image is {width}x{height}, golden is {expected_width}x{expected_height}"
    largest, differing = difference(expected, actual)
    pixels = width * height
    share = differing / pixels
    within = largest <= CEILING_CHANNEL and share <= CEILING_SHARE
    return (f"{prefix} max channel difference {largest}/255, {differing} of {pixels} pixels "
            f"differ ({share:.4%}); {'within' if within else 'beyond'} the "
            f"{CEILING_CHANNEL}/255 on {CEILING_SHARE:.1%} ceiling")


def compare(platform, cases, hashes_path, dump):
    """Checks one platform's hash list, and its dump where hashes differ."""
    hashes, problems = read_hashes(hashes_path)
    names = set(cases)
    missing = sorted(names - set(hashes))
    extra = sorted(set(hashes) - names)
    differing = sorted(name for name in names & set(hashes) if hashes[name] != cases[name][1])
    identical = len(names & set(hashes)) - len(differing)
    print(f"{platform}: expected={len(names)} listed={len(hashes)} identical={identical} "
          f"differing={len(differing)} missing={len(missing)} extra={len(extra)}")
    for problem in problems:
        print(f"  {platform} {problem}")
    for kind, values in [("missing", missing), ("extra", extra)]:
        if values:
            print(f"  {platform} {kind} cases: {', '.join(values[:10])}")
    for name in differing:
        print(describe(platform, name, cases[name][0], hashes[name], dump))
    passed = identical == len(names) and not (problems or missing or extra or differing)
    print(f"{'PASS' if passed else 'FAIL'} {platform}")
    return passed


def platforms(root, runs):
    """runs: (platform, hashes file, dump directory or None) for each platform."""
    cases = golden_cases(root)
    print(f"golden images: {len(cases)} cases in vectors/image/manifest.json")
    return all([compare(platform, cases, Path(hashes), None if dump is None else Path(dump))
                for platform, hashes, dump in runs])


USAGE = ("usage: compare_images.py ROOT PLATFORM HASHES DUMP [PLATFORM HASHES DUMP ...]\n"
         "DUMP is the producer's --dump directory, or - if it was run without one.")

if __name__ == "__main__":
    root, *rest = sys.argv[1:] or [None]
    if root is None or not rest or len(rest) % 3:
        sys.exit(USAGE)
    runs = [(rest[i], rest[i + 1], None if rest[i + 2] == "-" else rest[i + 2])
            for i in range(0, len(rest), 3)]
    sys.exit(0 if platforms(Path(root), runs) else 1)
