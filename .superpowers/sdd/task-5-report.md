# Task 5 Report: UI-thread Docker controller and snapshot view

## Status

Task 5 is complete on `codex/docker-sidebar`. Task 6 timer/window-lifecycle and active-tab refresh wiring was intentionally not added.

## Requirements implemented

- Added `src/app/docker.rs` with `DockerUiState`, `DockerController`, target routing, snapshot/detail dispatch, generation guards, filtering, model mapping, summaries, and focused pure-state tests.
- Welcome, local-shell, Telnet, and serial tabs route to `DockerTarget::Local`. SSH tabs route to `DockerTarget::Remote` and use that tab's authenticated `SessionHandle::docker_exec`; the SSH target remains selected while connecting or disconnected.
- Added `TabStatus.is_ssh` without changing `is_local`, preserving existing Telnet/serial resource-panel behavior.
- Local and remote version/container/image requests run in the background. Partial page failures retain successful data; missing Docker is hidden, while permission, daemon, command, parse, loading, and empty states remain visible with clear status/error text.
- Target changes increment generation and clear snapshot, errors, query, tab, filter, selection, and details. Snapshot and detail callbacks require matching generation and target; detail callbacks also require the captured selected ID and tab, preventing late inspect responses from replacing newer details. Terminal snapshot results clear `in_flight`.
- Details map only approved fields. `Config.Env` is neither copied, displayed, nor logged. No Docker environment variables are injected.
- Wired the Docker window and sidebar properties while retaining existing resource branches. The controller is owned through `WindowState`.

## TDD record

The brief's `cargo test app::docker::tests --lib` command is not runnable because this package has no library target; the initial attempt failed with Cargo's `no library targets found in package` error. The equivalent command is `cargo test --bin meatshell app::docker::tests`.

New focused tests were written before the stale-detail production fix. The red compile failure showed `apply_detail` lacked captured item/tab identity. The fix was then implemented and the focused suite passed.

Focused coverage includes local/welcome/Telnet/serial/SSH routing; filtered row/model mapping without mutating the raw snapshot; raw summary counts versus filtered rows; target-generation invalidation; stale snapshot and stale detail rejection; partial list failure with successful-page retention; not-installed visibility and ready/empty state behavior; and detail mapping without environment-variable exposure.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: nonzero due the pre-existing repository baseline (77 errors across unrelated resource, SSH, session, terminal, module-layout, and style findings). No remaining Task 5 Docker lint was reported after the narrow task-local cleanup.
- `cargo test --bin meatshell app::docker::tests`: passed, 9/9.
- `cargo test --locked`: passed, 293/293.
- `cargo check`: passed; only the existing dead-code warnings remain.

## Files

- `src/app/docker.rs` — controller, UI state, routing, async result guards, model/summary mapping, detail contract, and focused tests.
- `src/app/core.rs` — controller ownership slot in window state.
- `src/app.rs` — Docker window construction/callbacks, controller wiring, SSH target seeding, and sidebar refresh integration.
- `src/app/sidebar.rs` — unchanged existing sidebar implementation; controller-rendered properties are updated alongside it from `src/app.rs`.
- `src/resource/struct/system.rs` — per-tab `is_ssh` routing flag.
- `.superpowers/sdd/task-5-report.md` — this report.

## Self-review

- No Task 6 timer, visibility pause, or active-tab refresh lifecycle behavior was introduced.
- No changes were made to `.superpowers/brainstorm/` or unrelated features.
- Existing fork contracts for session notes, wallpaper, SFTP viewer, GPU monitoring, update URL, and russh pin remain untouched.
- Filtering is performed against the retained snapshot and does not issue Docker commands; summary counts use the unfiltered snapshot while rendered rows use filtered models.
- Partial container/image failures retain the successful page and expose the failed-page error.
- Missing Docker is classified as hidden; other loading, daemon, permission, command, parse, and empty states remain visible with status text.
- Stale list and detail results are discarded after a target/generation or selection change.
- The requested focused command includes `--lib`, but the current binary-only crate requires `--bin meatshell`.
- Compilation, formatting, focused tests, and the full locked test suite are green; the repository-wide Clippy baseline remains the only known verification limitation.

## Reviewer fix follow-up

The acceptance review identified five controller/UI contract gaps. They were fixed in this follow-up without adding Task 6 lifecycle behavior:

- `docker_summary` now selects the visible error from the active page, preserving independent container/image errors. Regression coverage verifies image-only and container-only failures on both tabs.
- `DockerUiState.detail_error` and DockerWindow's `detail-error` property now preserve and display command, timeout/channel, permission, and parse causes. Successful details still render approved fields only, with `Config.Env` omitted.
- Tab changes clear details, selected identity, and detail errors, so late detail results cannot remain visible across pages.
- Docker container/image/detail `VecModel`s are retained in the DockerWindow property and updated with `set_vec` in place; replacement is used only for initial model installation. A focused identity test covers the in-place update helper.
- DockerWindow now exposes filtered container/image counts; the controller updates them from the current query/filter view while sidebar counts remain raw snapshot counts.
- `refresh_now` renders immediately after marking the state in flight, so loading is visible before the background request completes.

### Follow-up verification

- `cargo test --bin meatshell app::docker::tests`: passed, 14/14.
- `cargo fmt --all -- --check`: passed after rustfmt normalization.
- `cargo clippy --all-targets -- -D warnings`: remains nonzero only for the pre-existing repository baseline; no Task 5 Docker diagnostics remain.
- Final post-review `cargo test --locked`: passed, 298/298.
- Final post-review `cargo check`: passed with the existing unrelated dead-code warnings.
- Final post-review strict Clippy still reports the same 77 pre-existing repository diagnostics; no reviewer-fix diagnostic was introduced.
