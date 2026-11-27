"""Tests for the cross-platform image comparison.

These prove the comparison can fail and that every difference is measured.
A platform whose image differs from the golden must be reported with the
platform, the case, the largest channel difference and the number of pixels
that differ, whether or not the difference is within the tolerance, so drift
stays visible.
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

    def changed_first(self, name, pixels, by):
        """Compares a platform whose first.png has the green channel of the
        first `pixels` pixels raised by `by` levels."""
        changed = bytearray(self.goldens["first.png"])
        for pixel in range(pixels):
            changed[pixel * 4 + 1] += by
        images = dict(self.goldens, **{"first.png": bytes(changed)})
        return self.run_compare(*self.producer_output(name, images))

    def test_green_channels_leave_room_to_change(self):
        # The changes below raise green by up to 9 in the first 2 pixels.
        for pixel in range(2):
            self.assertLessEqual(self.goldens["first.png"][pixel * 4 + 1], 255 - 9)

    def test_a_difference_within_the_tolerance_passes_and_is_measured(self):
        # A 16 by 16 image may have 1 differing pixel, by up to 8/255.
        passed, output = self.changed_first("within", 1, 8)
        self.assertTrue(passed, output)
        self.assertIn("identical=1 differing=1 missing=0 extra=0", output)
        self.assertIn(
            f"  {PLATFORM} first.png: max channel difference 8/255, 1 of 256 pixels differ "
            "(0.3906%); within the tolerance of 8/255 on at most 1 pixels\n",
            output)
        self.assertIn(f"PASS {PLATFORM}", output)

    def test_a_channel_difference_over_the_tolerance_fails(self):
        passed, output = self.changed_first("over-difference", 1, 9)
        self.assertFalse(passed, output)
        self.assertIn(
            f"  {PLATFORM} first.png: max channel difference 9/255, 1 of 256 pixels differ "
            "(0.3906%); beyond the tolerance of 8/255 on at most 1 pixels\n",
            output)
        self.assertIn(f"FAIL {PLATFORM}", output)

    def test_more_differing_pixels_than_the_tolerance_fails(self):
        passed, output = self.changed_first("over-count", 2, 1)
        self.assertFalse(passed, output)
        self.assertIn(
            f"  {PLATFORM} first.png: max channel difference 1/255, 2 of 256 pixels differ "
            "(0.7812%); beyond the tolerance of 8/255 on at most 1 pixels\n",
            output)
        self.assertIn(f"FAIL {PLATFORM}", output)

    def test_a_differing_case_without_a_dump_fails(self):
        changed = bytearray(self.goldens["first.png"])
        changed[1] += 1
        hashes, _ = self.producer_output(
            "no-dump", dict(self.goldens, **{"first.png": bytes(changed)}))
        passed, output = self.run_compare(hashes, None)
        self.assertFalse(passed, output)
        self.assertIn(f"  {PLATFORM} first.png: hash differs; no dumped image to measure", output)
        self.assertIn(f"FAIL {PLATFORM}", output)

    def test_the_pixel_allowance_is_a_tenth_of_a_percent_but_at_least_one(self):
        for pixels, allowed in [(256, 1), (1_999, 1), (65_536, 65), (1_166_400, 1_166)]:
            with self.subTest(pixels=pixels):
                self.assertEqual(compare_images.allowed_differing_pixels(pixels), allowed)
                self.assertTrue(compare_images.within_tolerance(8, allowed, pixels))
                self.assertFalse(compare_images.within_tolerance(9, 1, pixels))
                self.assertFalse(compare_images.within_tolerance(1, allowed + 1, pixels))

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
