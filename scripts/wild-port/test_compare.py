"""Tests for the cross-platform byte comparison.

These prove the comparison can fail. A comparison that passed on empty,
short or padded inventories would report three platforms as identical
without having compared anything. Every type's corpus pins the same file
names, so the comparison must also keep the types apart: a type missing
from every platform, or one type's tree differing under a file name the
other types share, must fail.
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
SHARED_NAME = "wild-port-0005__neutral.json"


class CrossPlatformComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.trees = compare.expected_trees(ROOT)
        self.names = self.trees[compare.WILD]

    def copy_trees(self, name):
        """A copy of the shipped wild corpus trees, so it starts out passing."""
        destination = self.directory / name
        destination.mkdir()
        for tree in self.names:
            shutil.copyfile(SHIPPED / tree, destination / tree)
        return destination

    def copy_mint(self, name):
        """A mint of every type, copied from the shipped trees, laid out as
        the mint writes it: one directory per type."""
        mint = self.directory / name
        mint.mkdir()
        for stem, names in self.trees.items():
            (mint / stem).mkdir()
            for tree in names:
                shutil.copyfile(compare.shipped(ROOT, stem) / tree, mint / stem / tree)
        return mint

    def quiet_compare(self, candidate):
        with contextlib.redirect_stdout(io.StringIO()):
            return compare.compare("test corpus", SHIPPED, candidate, self.names)

    def run_platforms(self, *mints):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            passed = compare.platforms(ROOT, *mints)
        return passed, output.getvalue()

    def test_every_type_has_its_own_corpus(self):
        self.assertEqual(list(self.trees), [compare.WILD, *compare.STEMS])
        self.assertEqual(len(self.names), 25)
        for stem in compare.STEMS:
            self.assertEqual(len(self.trees[stem]), 18, stem)
            self.assertIn(SHARED_NAME, self.trees[stem], stem)

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
        mints = [self.copy_mint(name) for name in ["macos", "arm64", "amd64"]]
        passed, output = self.run_platforms(*mints)
        self.assertTrue(passed, output)
        for mint in mints:
            for stem in [compare.WILD, *compare.STEMS]:
                path = mint / stem / sorted(self.trees[stem])[0]
                saved = path.read_bytes()
                path.unlink()
                passed, output = self.run_platforms(*mints)
                self.assertFalse(passed, f"{mint.name}/{stem}: {output}")
                path.write_bytes(saved)
            # A tree written beside the type directories, as a mint that
            # pooled every type's files into one directory would write it.
            stray = mint / SHARED_NAME
            stray.write_bytes((mint / compare.WILD / SHARED_NAME).read_bytes())
            passed, output = self.run_platforms(*mints)
            self.assertFalse(passed, output)
            self.assertIn(f"stray=['{SHARED_NAME}']", output)
            stray.unlink()
        for mint in mints:
            shutil.rmtree(mint)
            mint.mkdir()
        self.assertFalse(self.run_platforms(*mints)[0])

    def test_platforms_fail_when_a_type_is_absent_on_every_platform(self):
        mints = [self.copy_mint(name) for name in ["macos", "arm64", "amd64"]]
        for stem in ["sprouts", compare.WILD]:
            with self.subTest(stem=stem):
                saved = self.directory / "saved"
                for mint in mints:
                    shutil.copytree(mint / stem, saved / mint.name)
                    shutil.rmtree(mint / stem)
                # The platforms agree with each other, by leaving the type
                # out everywhere; the comparison must still miss it.
                passed, output = self.run_platforms(*mints)
                self.assertFalse(passed, output)
                self.assertIn(f"missing=['{stem}']", output)
                self.assertIn(f"FAIL macOS {stem} inventory", output)
                # An empty directory for the type is no better.
                for mint in mints:
                    (mint / stem).mkdir()
                passed, output = self.run_platforms(*mints)
                self.assertFalse(passed, output)
                self.assertIn(f"macOS {stem} inventory: expected={len(self.trees[stem])} "
                              "actual=0", output)
                for mint in mints:
                    (mint / stem).rmdir()
                    shutil.copytree(saved / mint.name, mint / stem)
                shutil.rmtree(saved)
                self.assertTrue(self.run_platforms(*mints)[0])

    def test_platforms_fail_when_one_type_differs_under_a_shared_name(self):
        # Give sprouts the tree that wild-plus pins under the same file name:
        # a real tree, and the right one for that name in another type, so
        # only a per-type comparison can see that it is wrong here.
        replacement = (compare.shipped(ROOT, "wild-plus") / SHARED_NAME).read_bytes()
        self.assertNotEqual(replacement,
                            (compare.shipped(ROOT, "sprouts") / SHARED_NAME).read_bytes())
        mints = [self.copy_mint(name) for name in ["macos", "arm64", "amd64"]]
        macos, arm64, amd64 = mints
        for label, changed, failing in [
            # One Linux platform differs from macOS in sprouts alone.
            ("linux/arm64", [arm64], "sprouts: linux/arm64 vs macOS"),
            # Every platform agrees, and only sprouts' committed tree differs.
            ("every platform", mints, "sprouts: macOS vs shipped fixtures"),
        ]:
            with self.subTest(changed=label):
                saved = (macos / "sprouts" / SHARED_NAME).read_bytes()
                for mint in changed:
                    (mint / "sprouts" / SHARED_NAME).write_bytes(replacement)
                passed, output = self.run_platforms(*mints)
                self.assertFalse(passed, output)
                self.assertIn(f"{failing}: expected=18 actual=18 identical=17 differing=1",
                              output)
                self.assertIn(f"FAIL {failing}\n", output)
                # Every other type, the same file name included, still passes.
                failures = [line for line in output.splitlines() if line.startswith("FAIL")]
                self.assertEqual(failures, [f"FAIL {failing}"], output)
                for mint in changed:
                    (mint / "sprouts" / SHARED_NAME).write_bytes(saved)
                self.assertTrue(self.run_platforms(*mints)[0])


if __name__ == "__main__":
    unittest.main()
