# Task 1 Report

## Implementation summary

Implemented the Docker domain layer without command execution, SSH transport, UI, or controller code.

- Added the requested Docker request/result, target, status/tab, error, snapshot, summary, and detail types.
- Added JSON-lines parsing for container and image summaries.
- Added inspect parsing for container and image details using `serde_json::Value` for nested/version-dependent fields.
- Normalized one leading slash from container names and mapped non-`running` states to `Stopped`.
- Added case-insensitive container and image search plus combined container state filtering, returning cloned results without mutating snapshots.
- Added shared failure classification for not-installed, permission, daemon, and generic command failures.
- Kept environment variables out of the detail model and parser output.

## TDD RED/GREEN evidence

RED:

- Added the four required focused tests before production model/parser implementation.
- The brief’s exact `cargo test docker::parse::tests --lib` command failed before compilation because this package has no library target.
- The equivalent binary-test invocation, `cargo test docker::parse::tests`, then failed with missing Docker types and functions, confirming the intended missing-interface RED state.

GREEN:

- Implemented the minimal model and pure parser/filter/classification APIs.
- Focused tests passed: 7 passed, 0 failed.
- Added coverage for image search, inspect mapping, environment-variable omission, and malformed/empty JSON parse failures.

## Tests and validation

- `cargo fmt --all -- --check`: passed.
- `cargo test docker::parse::tests`: passed, 7/7.
- `cargo test --locked`: passed, 271/271.
- `cargo check`: passed.
- `cargo clippy --all-targets -- -D warnings`: blocked by 91 pre-existing repository lint errors under the current toolchain, including unrelated dead code, module-inception, argument-count, MSRV, and other Clippy findings. No Docker-module finding remained after the scoped staged-API allowance.

## Changed files

- `src/main.rs` — registered `mod docker;`.
- `src/docker/mod.rs` — Docker module wiring and crate-visible re-exports.
- `src/docker/model.rs` — Docker domain types.
- `src/docker/parse.rs` — pure parsing, filtering, classification, and unit tests.
- `.superpowers/sdd/task-1-report.md` — this required report only.

## Self-review

- Scope is limited to Task 1 responsibilities and the requested files, plus the required report.
- No Docker command is executed and no user search text is interpolated into a command.
- List parsing accepts blank lines, reports malformed JSON as `ParseFailed`, and preserves stopped containers.
- Inspect parsing consumes the first object from the standard inspect array and safely maps missing fields to empty strings.
- `Config.Env` is not read or copied into any domain type.
- Filtering is pure and preserves input ordering.
- Fork-specific contracts and unrelated source files were not changed.
- Formatting and whitespace checks are clean.

## Concerns

- The repository’s prescribed focused command includes `--lib`, but this binary-only crate cannot run it; the equivalent filter without `--lib` was used and recorded above.
- Full strict Clippy remains red because of pre-existing repository findings; fixing those would exceed Task 1 scope.
- Detail fields containing nested Docker objects are represented as compact JSON text for downstream UI formatting; later tasks should keep that representation stable unless the UI contract explicitly changes.

## Reviewer fix follow-up

### Fix summary

- `first_inspect_value` now scans an inspect array and returns the first object element, rather than requiring the first array element to be an object.
- Added `parses_first_object_after_non_object_inspect_entry` covering a leading `null` before the valid inspect object.
- Changed all Docker model struct fields from `pub` to the requested `pub(crate)` visibility.
- Retained `ParseFailed` when an inspect array contains no object element.

### Fix TDD evidence and validation

- RED: the new regression test failed before the fix with `DockerError { kind: ParseFailed, message: "expected a non-empty inspect array" }`.
- GREEN: `cargo test docker::parse::tests` passed, 8/8.
- `cargo fmt --all -- --check`: passed.
- `cargo check`: passed, with the repository’s existing warnings.
- `cargo test --locked`: passed, 272/272.

### Fix self-review

- The parser now handles arbitrary non-object entries before the first inspect object and still rejects empty/all-non-object arrays without panicking.
- The visibility change is limited to the Docker model fields and does not change the crate-visible API used by the focused tests.
- No unrelated source or scratch files were changed or staged.
