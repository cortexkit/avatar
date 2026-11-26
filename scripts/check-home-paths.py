#!/usr/bin/env python3
"""Fails when a tracked text file contains an absolute home-directory path.

A path such as /Users/<name>/... or /home/<name>/... names the machine and the
person a file was written on. Such paths slip into a repository through pasted
logs, generated files and build output, and once committed they are public.
This scan covers every tracked text file in the working tree, uncommitted edits
included; git skips binary files, whose build paths are checked separately when
the embedded core binary is rebuilt.

Run with python3 scripts/check-home-paths.py; it scans the repository it sits in,
whatever the current directory.
"""

from pathlib import Path
import re
import subprocess
import sys


# What git grep looks for. The character before the leading slash must not be
# part of a path segment, so "src/home/page/" is not mistaken for a home path.
HOME_PATH = r"(^|[^A-Za-z0-9_.-])/(Users|home)/[A-Za-z0-9._-]+/"

# Home paths that are clearly generic examples rather than a real machine,
# each with the reason it is allowed. A match is exempt only when its exact
# prefix (for example the leading "/Users/<name>/" part, with a real name) is
# a key here, mapped to the reason. Nothing in the tree needs one today; add
# an entry only for a path that cannot name a real person or machine.
EXEMPT = {}

PREFIX = re.compile(r"/(?:Users|home)/[A-Za-z0-9._-]+/")


def main():
    # -I skips binary files; -n and -z give "path\0line\0text" records that
    # survive any character in a path.
    result = subprocess.run(
        ["git", "grep", "-I", "-n", "-z", "-E", HOME_PATH],
        capture_output=True, text=True, cwd=Path(__file__).resolve().parent.parent,
    )
    if result.returncode not in (0, 1):
        sys.stderr.write(result.stderr)
        return 2
    found = []
    for record in result.stdout.splitlines():
        path, line, text = record.split("\0", 2)
        prefixes = [match.group(0) for match in PREFIX.finditer(text)]
        if any(prefix not in EXEMPT for prefix in prefixes):
            found.append(f"{path}:{line}: {text.strip()[:160]}")
    if found:
        print("tracked files contain absolute home paths:")
        for entry in found:
            print("  " + entry)
        print("Use a repository-relative path, $HOME or ~, or a placeholder such as /Users/<name>/.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
