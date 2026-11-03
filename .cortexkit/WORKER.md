# Working in avatar

Standing rules for every worker in this repository. A brief can tighten them but never loosens them. "The head" is the agent that launched you, reviews your delivery and merges it.

## Build and test remotely

- Run cargo builds and tests on the build server (`runon: "linux"`, or `linux,4c` for clippy on a crate). Only the iPhone engine binary (`scripts/build-xcframework.sh`) needs Xcode, and the head builds that.
- Keep the normal build directory. Never set your own `CARGO_TARGET_DIR`: a fresh one makes every run a cold build.
- Build-server jobs cannot change tracked files. Run anything that rewrites them, such as the mint commands below, in your worktree, not on the build server.
- In a fresh worktree, install the TypeScript dependencies with `bun install --frozen-lockfile` (in `ports/ts`) before running anything that touches the TypeScript port.

## Test what you changed, nothing more

Run only the checks for what you changed:
- the tests for the files and behaviour you changed, by name or filter (`cargo test -p <crate> <filter>`, `bun test <file>`);
- formatting and lint for what you touched (`cargo fmt --check`, `cargo clippy -p <crate> --all-targets -- -D warnings`, and `bunx tsc --noEmit` if you touched `ports/ts`);
- for a bug fix, one test shown once to fail without the fix.

Never run the repository gate (`./scripts/check.sh`, with or without `--linux`), a whole crate's or package's test suite, or the workspace tests. The head runs the full gate once per landing, over the merged deliveries.

If a test you didn't touch fails, rerun that one test by name on your base commit, never a suite. A filtered run that takes more than about 15 minutes means the filter is too broad: narrow it instead of waiting on it.

In `playground/experiments/`, no cargo test covers your change. Check your pages with the kit tools instead (`kit/measure.sh`, `kit/shot.sh`).

Put each command, its pass and fail counts, and every failing test name in your report. Say if your change touches what the head's own checks cover: the browser lab comparison, the embedded core binary's freshness, or the Swift suite.

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
