## Task 6: Docker window lifecycle, refresh cadence, and active-tab routing

**Status:** COMPLETE

### Implementation

- Retained one `DockerWindow` and one `DockerController` per main window, with strong handles in `WindowState` and shared Docker row models.
- Wired open, close, focus, title-bar drag, east resize, south-east resize, theme synchronization, and monitor-centered placement.
- Active-tab changes now retarget Docker state first and only schedule a refresh when the sidebar or detached window is visible.
- Opening the detached window always requests exactly one immediate refresh for the current target.
- Added one five-second Docker timer per main window. It refreshes only while the existing visibility/zen gating permits a visible Docker surface; hidden/collapsed/zen surfaces remain idle.
- Connected and closed session events invalidate the matching remote target through the UI callback. In-place reconnect invalidation is also retained in the reconnect path.
- Existing snapshot/detail generation and target guards remain authoritative, so late results cannot repaint a newer target or selection.
- No parser, transport, domain, terminal, SFTP, process, GPU, disk, or fork-specific behavior was changed.

### TDD and focused tests

The interrupted tree already contained the brief’s visibility test; it was retained and expanded with pure lifecycle coverage:

- visibility transitions for sidebar/window combinations;
- active SSH/non-SSH target routing and target-change generation reset;
- invalidation of the current remote target, including loading state, cleared snapshot, and generation advance;
- stale snapshot and stale detail results being ignored after target/selection changes.

The requested `cargo test ... --lib` form is not supported because this crate has no library target. The equivalent binary test filter passed:

```text
cargo test app::docker
16 passed; 0 failed
```

### Verification

```text
cargo fmt --all -- --check
passed

cargo clippy --all-targets -- -D warnings
failed on the repository baseline; no Task 6-specific diagnostic

cargo test --locked
302 passed; 0 failed

cargo check
passed
```

The strict clippy baseline includes existing dead-code warnings, existing `too_many_arguments`, `drain_collect`, `module_inception`, MSRV, and other diagnostics across unrelated modules. The only Task 6-specific warning observed during the first check was the interrupted `refresh_target` method after its call sites were replaced; it was removed and did not recur.

### Changed files

- `src/app.rs`
- `src/app/core.rs`
- `src/app/docker.rs`
- `src/app/resource_ui.rs`
- `src/app/session_event.rs`
- `src/app/sidebar.rs`
- `src/app/tab_transfer.rs`
- `src/session/struct/prompts.rs`
- `ui/app.slint`

The pre-existing untracked `.superpowers/brainstorm/` directory was not touched.

### Self-review and concerns

- The detached window is hidden rather than destroyed on close, preserving its models and user-resized dimensions.
- Refresh dispatch is visibility-gated on active-tab changes and timer ticks; opening remains an explicit immediate-refresh path.
- Session invalidation is target-scoped and generation-based, preserving stale-result protection.
- Strict clippy remains a repository-level CI concern independent of Task 6; this task does not claim that baseline is clean.

### Commit

Committed as `feat: connect docker monitor to active tab lifecycle`.
