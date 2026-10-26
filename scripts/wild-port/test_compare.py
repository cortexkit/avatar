"""Tests for the cross-platform byte comparison.

These prove the comparison can fail. A comparison that passed on empty,
short or padded inventories would report three platforms as identical
without having compared anything.
"""

import contextlib
import io
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import compare


ROOT = Path(__file__).resolve().parents[2]
SHIPPED = ROOT / "vectors/tree"


class CrossPlatformComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.names = compare.expected_names(ROOT)

    def copy_trees(self, name):
        """A copy of the shipped corpus trees, so it starts out passing."""
        destination = self.directory / name
        destination.mkdir()
        for tree in self.names:
            shutil.copyfile(SHIPPED / tree, destination / tree)
        return destination

    def quiet_compare(self, candidate):
        with contextlib.redirect_stdout(io.StringIO()):
            return compare.compare("test corpus", SHIPPED, candidate, self.names)

    def test_comparison_accepts_equal_bytes(self):
        self.assertTrue(self.quiet_compare(self.copy_trees("equal")))

    def test_comparison_rejects_changed_bytes(self):
        candidate = self.copy_trees("changed")
        path = candidate / sorted(self.names)[0]
        tree = json.loads(path.read_text())
        tree["canvas"] += 1
        path.write_text(json.dumps(tree))
        self.assertFalse(self.quiet_compare(candidate))

    def test_comparison_rejects_whitespace_only_changes(self):
        candidate = self.copy_trees("whitespace")
        path = candidate / sorted(self.names)[0]
        original = path.read_bytes()
        rewritten = b" \n" + original
        self.assertEqual(json.loads(rewritten), json.loads(original))
        self.assertNotEqual(rewritten, original)
        path.write_bytes(rewritten)
        self.assertFalse(self.quiet_compare(candidate))

    def test_comparison_rejects_empty_short_and_extra_inventories(self):
        candidate = self.directory / "inventory"
        candidate.mkdir()
        self.assertFalse(self.quiet_compare(candidate))
        shutil.rmtree(candidate)
        candidate = self.copy_trees("inventory")
        removed = candidate / sorted(self.names)[0]
        saved = removed.read_bytes()
        removed.unlink()
        self.assertFalse(self.quiet_compare(candidate))
        removed.write_bytes(saved)
        (candidate / "unexpected.json").write_bytes(saved)
        self.assertFalse(self.quiet_compare(candidate))

    def test_platforms_validate_all_three_inventories(self):
        directories = [self.copy_trees(name) for name in ["macos", "arm64", "amd64"]]
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(compare.platforms(ROOT, *directories))
            for directory in directories:
                path = directory / sorted(self.names)[0]
                saved = path.read_bytes()
                path.unlink()
                self.assertFalse(compare.platforms(ROOT, *directories))
                path.write_bytes(saved)
            for directory in directories:
                shutil.rmtree(directory)
                directory.mkdir()
            self.assertFalse(compare.platforms(ROOT, *directories))


if __name__ == "__main__":
    unittest.main()
