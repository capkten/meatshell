# SFTP Image Viewer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore a session-scoped SFTP image viewer on the v0.6.0 baseline with same-directory navigation, zoom, mouse panning, and robust loading boundaries.

**Architecture:** Extend the existing SFTP command/event channel with bounded raw-byte reads. Keep viewer state per terminal tab, derive its image list from the current sorted SFTP entries, and render a modal Slint overlay that owns zoom/pan interaction while Rust owns file loading and navigation. The feature will be isolated to SFTP/image-viewer files plus the generated application wiring.

**Tech Stack:** Rust, Tokio, russh-sftp, Slint, image crate, existing SFTP/session event models.

---

### Task 1: Define testable image-list and viewer-state behavior

**Files:**
- Modify: `src/app.rs`
- Test: `src/app.rs` unit tests

**Step 1: Write failing tests**
- Add tests for filtering only non-directory supported raster images.
- Add tests that image order follows the already sorted SFTP entry order.
- Add tests for clamping previous/next indexes after a directory refresh removes the current image.
- Add tests that navigation state includes the originating tab id.

**Step 2: Run targeted tests to verify failure**
- Run: `cargo test image_viewer -- --nocapture`
- Expected: failures because the helper/state types do not exist yet.

**Step 3: Implement minimal helpers**
- Add a shared image-extension predicate.
- Add a per-tab viewer state map keyed by tab id.
- Add helpers for deriving image paths and clamping navigation.

**Step 4: Run targeted tests**
- Run: `cargo test image_viewer -- --nocapture`
- Expected: PASS.

**Step 5: Commit**
- Commit: `test: define image viewer navigation behavior`

### Task 2: Restore bounded SFTP raw-byte reads

**Files:**
- Modify: `src/sftp.rs`
- Modify: `src/ssh.rs`
- Test: `src/sftp.rs` or `src/ssh.rs` unit tests

**Step 1: Write failing tests**
- Verify image read rejects files larger than the configured limit.
- Verify malformed/empty image payloads produce a non-success event path.
- Verify raw reads report path, name, index, and total metadata.

**Step 2: Run targeted tests**
- Run: `cargo test image_bytes -- --nocapture`
- Expected: failures because `ReadBytes` and `SftpImageLoaded` are absent.

**Step 3: Implement minimal backend**
- Add `SftpCommand::ReadBytes { remote, max_bytes }`.
- Add `SftpHandle::read_bytes`.
- Read remote metadata first, enforce a hard byte limit, then decode with `image::load_from_memory`.
- Emit a bounded RGBA payload through `SessionEvent::SftpImageLoaded`.
- Keep SVG excluded unless a dedicated renderer is added; do not pretend raster decoding supports it.

**Step 4: Run targeted tests**
- Run: `cargo test image_bytes -- --nocapture`
- Expected: PASS.

**Step 5: Commit**
- Commit: `feat: add bounded SFTP image reads`

### Task 3: Add session-scoped SFTP image metadata

**Files:**
- Modify: `ui/sftp_panel.slint`
- Modify: `src/app.rs`
- Modify: `ui/app.slint`
- Test: `src/app.rs` unit tests

**Step 1: Write failing tests**
- Verify `SftpEntry.is-image` is true only for supported raster files.
- Verify directory refresh replaces the originating tab's image list.
- Verify another tab cannot change the viewer's source list.

**Step 2: Run targeted tests**
- Run: `cargo test image_entries -- --nocapture`
- Expected: failures because the field and per-tab model are absent.

**Step 3: Implement**
- Add `is-image` to `SftpEntry`.
- Store image paths per tab id, not globally.
- Update the store on every `SftpEntries` event, preserving the UI's sorted entry order.
- Clear or clamp viewer state on directory changes and tab close.

**Step 4: Run targeted tests**
- Run: `cargo test image_entries -- --nocapture`
- Expected: PASS.

**Step 5: Commit**
- Commit: `feat: track images per SFTP session`

### Task 4: Restore viewer overlay and navigation wiring

**Files:**
- Create: `ui/image_viewer.slint`
- Modify: `ui/app.slint`
- Modify: `src/app.rs`

**Step 1: Write failing interaction/state tests**
- Verify opening an image sets the viewer tab id, index, and total.
- Verify previous/next loads from the originating SFTP handle.
- Verify empty list and out-of-range navigation close or clamp safely.

**Step 2: Run targeted tests**
- Run: `cargo test image_viewer -- --nocapture`
- Expected: failures until callbacks and state are wired.

**Step 3: Implement**
- Add modal overlay with loading and error states.
- Add previous/next controls and keyboard Escape/left/right handling.
- Keep the viewer bound to its originating tab id.
- Reset zoom/pan when a new image is loaded.

**Step 4: Run targeted tests**
- Run: `cargo test image_viewer -- --nocapture`
- Expected: PASS.

**Step 5: Commit**
- Commit: `feat: restore SFTP image viewer overlay`

### Task 5: Implement and verify zoom/pan behavior

**Files:**
- Modify: `ui/image_viewer.slint`
- Test: `ui/image_viewer.slint` behavior through build plus Rust state tests

**Step 1: Write failing state tests**
- Verify index navigation at first/last image.
- Verify load completion resets the view state.
- Verify close releases the viewer state for the originating tab.

**Step 2: Run targeted tests**
- Run: `cargo test image_viewer -- --nocapture`
- Expected: failures for missing reset/boundary behavior.

**Step 3: Implement**
- Add mouse-wheel zoom centered at cursor.
- Add min/max scale.
- Add drag panning with clamped offsets.
- Add double-click fit/2x behavior.
- Use icon buttons and tooltips for controls.

**Step 4: Run targeted tests**
- Run: `cargo test image_viewer -- --nocapture`
- Expected: PASS.

**Step 5: Commit**
- Commit: `feat: add image viewer zoom and pan`

### Task 6: Full verification and handoff

**Files:**
- Modify: `docs/release.md` only if the feature needs release-note text.
- Review: all image viewer and SFTP changes.

**Step 1: Run formatting**
- Run: `cargo fmt --all -- --check`

**Step 2: Run tests**
- Run: `cargo test`

**Step 3: Run build checks**
- Run: `cargo check --locked`

**Step 4: Inspect history and diff**
- Run: `git diff upstream/main..HEAD --stat`
- Confirm image viewer changes are separate from the upstream synchronization baseline.

**Step 5: Commit**
- Commit any final focused fixes only after verification.

