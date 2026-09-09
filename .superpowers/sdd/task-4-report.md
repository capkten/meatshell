# Task 4 Report: Slint Docker window, sidebar summary, and translations

## Implementation

- Added `ui/docker_window.slint` with the exact exported `DockerContainerRow`, `DockerImageRow`, `DockerDetailRow`, and `DockerWindow` API required by the brief.
- Implemented the read-only Docker window layout: custom titlebar, wallpaper/theme bindings, drag and resize callbacks, container/image tabs, search, container filters, refresh, selectable rows, detail rows, loading/error/empty states, and no environment-variable field.
- Re-exported `DockerWindow` and added the exact Docker properties/callback to `AppWindow`.
- Added `DockerBlock` to both vertical and horizontal `Sidebar` layouts, conditionally shown by `docker-visible`, with target/status/error/counts and `show-docker()` forwarding.
- Added only Docker-specific English and Chinese gettext entries, preserving existing msgids and translations.
- Kept controller behavior, target routing, refresh lifecycle, detail fetching, and local/SSH integration out of scope as required.

## TDD / compile evidence

1. Added the Slint API skeleton first and ran `cargo check`; it passed and generated the expected Rust-facing surface.
2. Added the visual layout and sidebar forwarding. The first full compile caught misplaced sidebar property declarations; moved them from `StatsBlock` to `Sidebar` and re-ran the check successfully.
3. Final `cargo check` passed after all edits.
4. `git diff --check` passed.

## Verification results

- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS; existing repository dead-code warnings remain
- `cargo test --locked`: PASS — 284 passed, 0 failed
- `cargo clippy --all-targets -- -D warnings`: FAIL due to the documented pre-existing repository-wide lint set (77 errors in the binary target and 79 in the test target, including dead code, module inception, argument-count, and newer-Clippy compatibility lints). No Task 4-specific Clippy failure was reported.

## Files changed

- `ui/docker_window.slint` — new Docker window and row structs
- `ui/app.slint` — Docker type export, properties, callback, and Sidebar forwarding
- `ui/sidebar.slint` — Docker summary block, properties, and callback forwarding in both dock layouts
- `lang/en/LC_MESSAGES/meatshell.po` — Docker English strings
- `lang/zh/LC_MESSAGES/meatshell.po` — Docker Chinese translations
- `.superpowers/sdd/task-4-report.md` — this report

## Self-review and concerns

- No controller or generated `target` output was edited.
- Existing Stats, network, disk, process, session, SFTP, and fork-specific behavior was left unchanged.
- The Docker window consumes already-filtered models and does not execute commands or parse strings in Slint.
- The only remaining concern is the repository-wide strict-Clippy baseline described above; it predates this UI-only task.
