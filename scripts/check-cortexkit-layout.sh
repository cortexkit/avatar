#!/usr/bin/env bash
# Fails when git tracks anything under .cortexkit/ outside the shared list of
# trackable paths: release notes, skills, rules, the worker guide, and the
# top-level project settings files (.cortexkit/<module>.jsonc) that the agent
# daemon reads. Everything else there is internal and must stay out of git.
#
# The check lists every tracked path rather than only tracked-and-ignored
# ones: a stray "!" line in any .gitignore can un-ignore a private file, and
# a tracked file that is no longer ignored would slip past an ignore-based
# check. A failing `git ls-files` must not read as an empty list, so it fails
# with its own exit code.
set -u
if ! all="$(git ls-files -- .cortexkit)"; then
  echo "BLIND: git ls-files failed" >&2
  exit 2
fi
bad="$(printf '%s\n' "$all" \
  | grep -vE '^\.cortexkit/(release-notes/|skills/|rules/|WORKER\.md$|[a-z0-9-]+\.jsonc$)' \
  | grep -v '^$')"
if [ -n "$bad" ]; then
  printf 'tracked outside the .cortexkit allowlist:\n%s\n' "$bad" >&2
  exit 1
fi
