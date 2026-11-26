"""Tests for the cross-platform image comparison.

These prove the comparison can fail and that a failure is measured. A
platform whose image is off by one in a single channel must be reported with
the platform, the case, the largest channel difference and the share of
pixels that differ; a comparison that only said "differs" would leave nothing
to judge a tolerance by.
"""

import contextlib
import hashlib
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

import compare_images


SIZE = 16
PLATFORM = "linux-x86_64"


def pattern(seed):
    """Deterministic RGBA bytes that vary from pixel to pixel."""
    return bytes((index * 37 + seed) % 256 for index in range(SIZE * SIZE * 4))


def paeth(left, up, upper_left):
    estimate = left + up - upper_left
    distances = [abs(estimate - left), abs(estimate - up), abs(estimate - upper_left)]
    return [left, up, upper_left][distances.index(min(distances))]


def filtered_png(width, height, pixels):
    """An RGBA PNG whose rows cycle through all five filter types, as an
    adaptive encoder such as the one that wrote the goldens may choose them."""
    stride = width * 4
    previous = bytes(stride)
    scanlines = bytearray()
    for row in range(height):
        line = pixels[row * stride:(row + 1) * stride]
        kind = row % 5
        scanlines.append(kind)
        for index, value in enumerate(line):
            left = line[index - 4] if index >= 4 else 0
            up = previous[index]
            upper_left = previous[index - 4] if index >= 4 else 0
            predictor = [0, left, up, (left + up) // 2, paeth(left, up, upper_left)][kind]
            scanlines.append((value - predictor) & 0xFF)
        previous = line

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    return (compare_images.PNG_SIGNATURE
            + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(scanlines)))
            + chunk(b"IEND", b""))


class ImageComparisonTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        image = self.root / "vectors/image"
        image.mkdir(parents=True)
        self.goldens = {"first.png": pattern(11), "second.png": pattern(200)}
        for name, pixels in self.goldens.items():
            compare_images.write_png(image / name, SIZE, SIZE, pixels)
        entries = [{"file": name, "size_px": SIZE} for name in self.goldens]
        (image / "manifest.json").write_text(json.dumps({"entries": entries}))
        self.cases = compare_images.golden_cases(self.root)

    def producer_output(self, name, images):
        """A hash list and dump directory like the image-hashes example's."""
        directory = self.root / name
        dump = directory / "dump"
        dump.mkdir(parents=True)
        lines = []
        for case, pixels in images.items():
            compare_images.write_png(dump / case, SIZE, SIZE, pixels)
            lines.append(f"{hashlib.sha256(pixels).hexdigest()}  {case}\n")
        hashes = directory / "image.sha256"
        hashes.write_text("".join(lines))
        return hashes, dump

    def run_compare(self, hashes, dump):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            passed = compare_images.compare(PLATFORM, self.cases, hashes, dump)
        return passed, output.getvalue()

    def test_identical_images_pass(self):
        passed, output = self.run_compare(*self.producer_output("equal", self.goldens))
        self.assertTrue(passed, output)
        self.assertIn(f"PASS {PLATFORM}", output)

    def test_one_channel_difference_is_measured(self):
        # Pixel 37's green channel holds neither 0 nor 255, so it can move
        # either way.
        index = 37 * 4 + 1
        self.assertTrue(0 < self.goldens["first.png"][index] < 255)
        # 1 of 256 pixels is 0.39%, inside the 0.5% share the ceiling allows,
        # so the channel difference alone decides which side of it this is.
        for change, verdict in [(1, "within"), (-1, "within"), (3, "beyond")]:
            with self.subTest(change=change):
                changed = bytearray(self.goldens["first.png"])
                changed[index] += change
                images = dict(self.goldens, **{"first.png": bytes(changed)})
                passed, output = self.run_compare(
                    *self.producer_output(f"changed{change}", images))
                self.assertFalse(passed, output)
                self.assertIn("identical=1 differing=1 missing=0 extra=0", output)
                self.assertIn(
                    f"  {PLATFORM} first.png: max channel difference {abs(change)}/255, "
                    f"1 of 256 pixels differ (0.3906%); {verdict} the 2/255 on 0.5% ceiling\n",
                    output)
                self.assertIn(f"FAIL {PLATFORM}", output)

    def test_missing_case_fails(self):
        hashes, dump = self.producer_output("short", {"first.png": self.goldens["first.png"]})
        passed, output = self.run_compare(hashes, dump)
        self.assertFalse(passed, output)
        self.assertIn("identical=1 differing=0 missing=1 extra=0", output)
        self.assertIn(f"  {PLATFORM} missing cases: second.png\n", output)

    def test_reader_reverses_every_filter_type(self):
        path = self.root / "filtered.png"
        path.write_bytes(filtered_png(SIZE, SIZE, self.goldens["first.png"]))
        self.assertEqual(compare_images.read_png(path), (SIZE, SIZE, self.goldens["first.png"]))


if __name__ == "__main__":
    unittest.main()
