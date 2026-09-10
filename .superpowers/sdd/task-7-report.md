# Task 7 final regression verification report

Date: 2026-09-10 (Asia/Shanghai)
Branch: `codex/docker-sidebar`
Final verified head: `c8e4c0a` (`fix: harden Docker inspect and refresh state`)
Pre-final-fix baseline: `d309a8c` (`fix: pause docker monitor in zen mode`)

## Scope and contracts reviewed

Read `task-7-brief.md`, `AGENTS.md`, and `docs/upstream-local-changes.md` before verification. The fork contracts were preserved: multi-line `Session.notes` with backward-compatible `note` alias, local update-check URL, wallpaper/default migration behavior, GPU/resource paths, SFTP viewer generation guards, connection/transfer lifecycle hardening, and docked sidebar geometry. `.superpowers/brainstorm/` was not touched.

The working tree was clean relative to `d309a8c` before this report was written; the only pre-existing untracked content is `.superpowers/brainstorm/`, which is intentionally excluded.

## Required command results

Commands were run in the requested verification order.

1. `cargo fmt --all -- --check`

   PASS, exit code 0; no formatting diff.

2. `cargo clippy --all-targets -- -D warnings`

   FAIL, exit code 101, matching the known repository-wide baseline. The output reported 78 errors for the test target and 77 for the binary target, including existing dead-code, `too_many_arguments`, module-inception, MSRV, `cmp-owned`, `items-after-test-module`, and related diagnostics across resource, auth, session, WebDAV, SSH, terminal, config, wallpaper, and test modules. No Docker-specific Clippy diagnostic was emitted. No unrelated baseline lint was changed.

3. `cargo test --locked docker`

   PASS, exit code 0: 39 passed, 0 failed, 264 filtered out.

   This includes Docker command construction/quoting, parser/error classification, search and running/stopped filters, details without environment variables, controller target changes, inactive-page errors, partial failures, stale snapshot/detail guards, model identity, SSH request enqueueing, and lifecycle visibility/zen tests.

4. `cargo test --locked`

   PASS, exit code 0: 303 passed, 0 failed, 0 ignored.

   Existing config/session/wallpaper, CPU/memory/swap/GPU/disk/process, SSH shell, SFTP, pane, Telnet/serial, reconnect/lifecycle, terminal, and fork-sensitive tests all passed. The normal test build emitted the existing 9 dead-code warnings.

5. `cargo check`

   PASS, exit code 0. Rust and generated Slint bindings compiled successfully. The normal check emitted 11 existing dead-code warnings; no Docker-specific warning was reported.

## Manual and static scenario checks

### Local Docker

- `Get-Command docker` result: `DOCKER_EXECUTABLE=ABSENT`.
- Because Docker is not installed or accessible on this host, the welcome/local-terminal live counts, popup, search/filter interaction, details view, manual refresh, five-second refresh, and daemon-stop error state could not be manually exercised.
- The no-executable branch is statically covered by `DockerStatus::NotInstalled` and `docker_summary` hiding only that state, plus `not_found_result_is_classified_as_not_installed` and `summary_counts_raw_snapshot_and_hides_only_not_installed`.

### SSH Docker and target isolation

- No SSH Docker host was available for a live connection, permission-denied test, daemon-not-running test, or tab switching test.
- Static review confirmed `target_for_tab` maps welcome/Telnet/serial/local tabs to `Local` and only an active SSH status to `Remote { tab_id, host }`; remote requests use the existing active `SessionHandle`, while local requests use local execution.
- Static review confirmed target changes and invalidation clear old data and advance the generation; result application rejects mismatched generation, target, selected ID, or active page. The focused stale-result and target-routing tests passed.

### UI behavior and lifecycle

Static review plus passing focused tests confirmed:

- hidden/not-installed Docker is omitted from the sidebar while operational errors remain visible;
- container/image tabs, case-insensitive search, running/stopped/all filters, counts, and read-only list/detail views are wired;
- container detail rows intentionally contain ID/image/command/created/ports/mounts/networks and no `Config.Env`/environment variables;
- manual refresh and the dedicated five-second timer call the controller;
- refresh runs only when the sidebar or detached Docker window is visible and is paused in zen mode;
- detached-window close hides it and clears the open flag; window teardown includes Docker state and timer ownership;
- detached Docker theme, scale, font, wallpaper image/active flag, accent, and tint are synchronized from the main window;
- Docker remote command output is bounded and timed out, and session channel closure becomes a visible error.

### Existing behavior and fork-specific regression review

`cargo test --locked` passed all 303 tests. Static contract review found no changes to the fork-specific session schema, update-check repository, wallpaper migration, SFTP image-preview generation guards, GPU normalization, local/SSH/SFTP transfer cleanup, or docked-sidebar geometry. The full suite also covered CPU/memory/swap/GPU/disk/process monitoring, system-information plumbing, SSH, SFTP, Telnet, serial, panes, reconnect/lifecycle, theme/wallpaper persistence, and terminal behavior.

## Changed files

Only this report file was changed by Task 7. No feature code or tests required a narrowly scoped fix. The pre-existing `.superpowers/brainstorm/` untracked directory remains untouched and uncommitted.

## Self-review

- Requirements were checked against the current source rather than relying only on prior task reports.
- Focused Docker tests and the full locked test suite were run fresh after the final implementation commit.
- The required formatting, Clippy, test, and check commands were run in order; the Clippy failure is clearly separated from passing compile/test evidence and contains no Docker-specific diagnostic.
- Manual limitations are reported as limitations, not as successful live checks.
- No generated files, fork-contract files, or unrelated lint fixes were introduced.

## Release blockers and disposition

1. Strict repository-wide Clippy remains red at baseline (exit 101, 77/78 existing diagnostics). This is an existing repository blocker and was intentionally not expanded into Task 7 scope.
2. Live local Docker and SSH Docker acceptance checks remain unperformed because this verification host has no Docker executable and no suitable SSH Docker server. A release candidate should repeat the brief's local and SSH manual scenarios on provisioned hosts before release sign-off.

Within the available environment, Task 7 found no concrete Docker regression requiring code changes.

## Final-fix follow-up (2026-09-10)

The whole-branch review identified and fixed the following blockers:

- Container and image inspect requests now use fixed tab-separated JSON templates containing only the approved ID/image/command/created/ports/mounts/networks or ID/repository-tags/size/created fields. The parser accepts only that restricted field shape and rejects unrestricted inspect objects. Command, parser, and UI-path tests prove no `Env` field is requested, transported, parsed, or displayed.
- Snapshot requests now carry a monotonic request identity and are deduplicated while one is in flight. Late same-target results are rejected while target/generation guards remain active; tests also prove a later refresh is allowed after completion.
- Detail requests now carry a monotonic identity, so reselecting the same item cannot be overwritten by an older inspect result. Existing target/generation/tab/selected-ID guards remain active.
- Sidebar errors now retain a concise page label when the inactive page is the only failing page.
- Docker stderr is normalized to one line and bounded to 200 characters before reaching UI state; timeout and useful reason handling remain intact.
- Successful snapshots now populate `fetched_at`, and the detached Docker window retains its shared VecModel instances without fallback replacement.

### Final-fix verification

- Initial red regression run: 6 expected failures in the old unrestricted command/parser expectations, confirming the security tests exercised the defect.
- `cargo fmt --all -- --check`: PASS, exit 0.
- `cargo clippy --all-targets -- -D warnings`: FAIL, exit 101, repository baseline only. The first post-fix run exposed three Docker-specific `needless_else` diagnostics caused by removing fallback model replacement; those branches were removed. A clean path-filtered rerun reported `DOCKER_CLIPPY_DIAGNOSTICS=NONE`; remaining Clippy failures are the pre-existing repository diagnostics.
- `cargo test --locked docker`: PASS, 46 passed, 0 failed, 264 filtered out.
- `cargo test --locked`: PASS, 310 passed, 0 failed, 0 ignored.
- `cargo check`: PASS, exit 0, with the existing 11 dead-code warnings and no Docker-specific warning.
- `git diff --check`: PASS.

Final-fix changed files: `src/docker/command.rs`, `src/docker/parse.rs`, `src/app/docker.rs`, and this report. `.superpowers/brainstorm/` was not touched. Live Docker/SSH acceptance remains unavailable on this host because Docker and a test SSH Docker server are not provisioned; this remains the only environment-based release limitation in addition to the repository Clippy baseline.
