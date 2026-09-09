# Task 5 Report: UI-thread Docker controller

## Status

Implemented on `codex/docker-sidebar`; committed after verification.

## Implementation

- Added `src/app/docker.rs` with `DockerUiState`, target routing, generation guards, background local/SSH snapshot collection, local filtering, summary/count mapping, details mapping, and Slint model updates.
- Local targets cover welcome, local shell, Telnet, and serial tabs. SSH tabs use the active tab's authenticated `SessionHandle::docker_exec`; the SSH target remains selected while connecting or disconnected.
- Target changes increment generation and clear snapshot, errors, query, selection, details, tab, and filter. Snapshot and detail callbacks require both generation and target equality; terminal results clear `in_flight`.
- Inspect details expose only approved fields. `Config.Env` is not copied, displayed, or logged.
- Added `TabStatus.is_ssh` without changing `is_local`, preserving existing Telnet/serial resource behavior.
- Wired the Docker window callbacks and sidebar properties while retaining the existing sidebar/session branches. The Task 6 timer, visibility pause, and active-tab lifecycle work were not added.
- Stored the controller in `WindowState` for the existing window ownership model.

## TDD evidence

RED:

- The brief's `cargo test ... --lib` command cannot run because this package has no library target (`error: no library targets found`).
- The equivalent binary-target focused test initially failed because the controller symbols were not implemented.

GREEN:

- `cargo test --bin meatshell app::docker::tests`
- Result: `2 passed; 0 failed; 284 filtered out`.

## Verification results

- `cargo fmt --all`: passed.
- `cargo check`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: failed only on the pre-existing repository baseline: unused resource helpers/types, the SSH prompt marker/fields, and two `missing_const_for_thread_local` suggestions in `auth_dialogs.rs`. No Task 5 file was reported.
- `cargo test --locked`: passed, `286 passed; 0 failed` (9 pre-existing dead-code warnings).

## Files

- `src/app/docker.rs` — new controller, state, views, routing, async result guards, and focused tests.
- `src/app/core.rs` — window ownership slot for the Docker controller.
- `src/app.rs` — Docker window construction/callbacks, controller wiring, SSH target flag seeding, and sidebar refresh integration.
- `src/app/sidebar.rs` — existing sidebar API preserved; controller rendering is invoked alongside existing refreshes.
- `src/resource/struct/system.rs` — per-tab `is_ssh` flag.

## Self-review

- Filtering is performed against the retained snapshot and does not issue Docker commands.
- Summary counts use the unfiltered snapshot; rendered row counts use filtered models.
- Partial container/image failures retain the successful page and expose the failed-page error.
- Missing Docker is classified as hidden; other loading, daemon, permission, command, parse, and empty states remain visible with status text.
- Stale list and detail results are discarded after a target/generation change.

## Concerns

- The requested focused command includes `--lib`, but the current binary-only crate requires `--bin meatshell`.
- Existing repository-wide Clippy warnings/errors may remain unrelated to this task; the final command output is the authority.
