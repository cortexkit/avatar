# Working in avatar

Standing rules for every worker in this repository. A brief can tighten them but never loosens them.

## Build and test remotely

- Run cargo builds and tests on the build server (`runon: "linux"`, `linux,8c` or more for the whole workspace). Every gate's Rust stage runs on Linux; only the iPhone engine binary (`scripts/build-xcframework.sh`) needs Xcode, and the head builds that.
- In a fresh worktree, install the TypeScript dependencies with `bun install --frozen-lockfile` (in `ports/ts`) before running anything that touches the TypeScript port.

## Gate before you deliver

Filtered tests alone are never a gate. Before delivering, run the build-server half of the repository gate on Linux:

```
./scripts/check.sh --linux
```

It runs every cargo stage (fmt, clippy in dev and release, the workspace tests and doc tests, the corpus coverage check, the playground's fmt, clippy, tests and placement invariants) and the checks beside them. If you touched `ports/ts`, also run its typecheck and tests there (`bunx tsc --noEmit`, `bun test`), as the gate does. The rest of the gate (the browser lab comparison, the embedded core binary's freshness and, where Swift is installed, the Swift suite) runs on the head's machine before merge; say in your report if your change touches what they check.

Put each command, its pass and fail counts, and every failing test name in your report. If a failure isn't yours, show it fails the same way on your base commit.

## Pre-approved, so don't ask

- Regenerating golden vectors with the mint commands (`UPDATE_GOLDEN=1` or `UPDATE_WILD_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --exact --nocapture --ignored`) and pasting the printed pins, when the new output differs only by the cause you state. Say so in the commit message.
- Updating a test that your intended change breaks, keeping what the test checks.
- In `playground/experiments/`, using the kit tools (`kit/shot.sh`, `kit/measure.sh`, `kit/compare-rows.py`) and the git-ignored `kit/fleet-genomes.js` the brief gives you.

## Never

- Change what an existing creature type and version draws. Agents' creatures render at the version stored with them, so a fix to a type's generation is a new version of that type, and a different depiction of the same genome is a new type (`docs/design/012-version-and-type-procedure.md`).
- Call a non-portable maths function on a generation path: trigonometry, exponentials, logarithms, powers, `hypot`, `cbrt` or `mul_add`. `crates/clippy.toml` lists them and refuses each one. Use `crates/avatar-svg/src/wild/math.rs` or `avatar-tables`; `sqrt` and basic arithmetic are exact everywhere and allowed.
- Write a creature generator in Swift or TypeScript. Every type is generated once, in Rust; the ports decode and draw its render tree.
- Edit files under `vectors/` by hand. The mint commands write them.
- Rebuild `ports/swift/AvatarCore.xcframework`. Building it needs Xcode, so the head rebuilds it; say in your report when you change generator source.
- Loosen an assertion, a threshold or a timeout to get green. Find the cause.
- Open, copy or write the live stores under `~/.local/share/cortexkit/`. Work on copies you were given.
- Force-add ignored `.cortexkit/` files.
- Put backticks inside `git commit -m "..."`: the shell runs them as commands. Use plain quotes.

## Ask when

A decision changes how any agent's creature looks, a wire or type contract (type names, versions, genome lengths, the render tree format), or public documents. Otherwise decide, and record the decision in your report.
