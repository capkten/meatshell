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

## Review follow-up

### Findings and fixes

1. Fixed detached-window refresh gating. `dynamic_ui_active` belongs to the
   main window and can become false for focus, minimize, or occlusion while a
   detached Docker window remains visible. Docker refresh eligibility now
   treats `sidebar_visible || window_open` as the surface decision. The
   existing sidebar helper still gates the sidebar for active, expanded, and
   non-zen state; a hidden Docker window therefore remains idle.
2. Added `docker-containers`, `docker-images`, and `docker-details` model
   properties to `AppWindow`. Each is initialized once and the exact same
   `ModelRc` is attached to both AppWindow and the retained DockerWindow.
3. Extended the existing theme, scale, wallpaper, and cross-window preference
   paths to synchronize the retained DockerWindow without creating another
   window or changing unrelated detached windows.
4. Audited the additional lifecycle files. `resource_ui.rs`, `sidebar.rs`,
   `tab_transfer.rs`, `session/struct/prompts.rs`, `ui/app.slint`, and
   `session_event.rs` each retain only Docker lifecycle, model, theme, teardown,
   reconnect, or invalidation wiring required by Task 6.

### Review TDD evidence

The new regression assertion was run before the production change and failed
on the old implementation for `dynamic_ui_active=false,
sidebar_visible=false, window_open=true`. After the helper change, the focused
visibility suite passed 2/2.

### Required verification order

```
cargo fmt --all -- --check
passed

cargo clippy --all-targets -- -D warnings
failed only on the known repository-wide pre-existing baseline (77/78
diagnostics by target); no Task 6-specific diagnostic after the fix

cargo test --locked
302 passed; 0 failed

cargo check
passed
```

The focused lifecycle tests also passed:

```
cargo test app::sidebar::docker_lifecycle_tests
2 passed; 0 failed
```

The follow-up is committed as a focused fix to the Task 6 implementation.

## Re-review follow-up

### Findings and fixes

- Restored an explicit zen gate in `docker_refresh_needed`. Its first
  argument is now the Docker surface's non-zen allowance, not main-window
  focus/activity. The timer and lifecycle callbacks pass `!zen_mode`, so a
  detached visible DockerWindow continues refreshing through main-window
  unfocused, minimized, or occluded states, while zen mode pauses it. The
  sidebar visibility argument remains independently derived from
  `sidebar_updates_visible`.
- Added explicit pure coverage for zen mode with an open detached window and
  retained the visible/hidden transition cases.
- Extended `on_set_ui_scale` to synchronize the retained DockerWindow through
  `sync_docker_theme`, including related scale/theme properties.

### Re-review verification

Focused lifecycle tests:

```
cargo test app::sidebar::docker_lifecycle_tests
3 passed; 0 failed
```

Required order:

```
cargo fmt --all -- --check
passed

cargo clippy --all-targets -- -D warnings
failed only on the known repository-wide pre-existing baseline; no Task 6
diagnostics after the fix

cargo test --locked
303 passed; 0 failed

cargo check
passed
```

The strict Clippy baseline was not changed; unrelated lints were intentionally
left out of this focused fix. The re-review fix is committed separately.
