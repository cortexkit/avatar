#!/usr/bin/env bash
# Check both the source revision and the actual embedded artifacts.
set -euo pipefail
cd "$(dirname "$0")/.."

expected_source=$(cat ports/swift/AvatarCore.source-hash)
actual_source=$(bash scripts/xcframework-source-hash.sh)
expected_artifacts=$(cat ports/swift/AvatarCore.artifact-hashes)
actual_artifacts=$(bash scripts/xcframework-artifact-hashes.sh)
if [[ -z $actual_source || $expected_source != "$actual_source" ||
      -z $actual_artifacts || $expected_artifacts != "$actual_artifacts" ]]; then
    echo "AvatarCore.xcframework is stale: rebuild with scripts/build-xcframework.sh" >&2
    exit 1
fi
