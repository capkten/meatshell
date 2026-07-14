# Local Upstream Changes

This document records the functionality maintained in the local fork of
meatshell so future upstream merges do not silently remove behavior or change
stored data.

## Version Boundary

- Upstream baseline: \`v0.6.2\` at \`bdf79fb\`.
- Local merge point: \`514c348\` (\`Merge remote-tracking branch 'upstream/main'\`).
- Local fixes after that merge:
  - \`5df5621\` fixes session lifecycle, transfer cleanup, and docked-sidebar
    layout geometry.
  - \`22a1012\` restores the original compatible session-notes implementation.
- The existing \`v0.6.2\` tag belongs to the upstream baseline. The local tag
  for this state is \`v0.6.2-meatshell.1\`.

## Features Maintained By This Fork

### Session notes / memo

The original local implementation is \`b0c8332\`. Notes are intentionally
different from the later upstream \`note\` field:

- \`Session.notes\` is a multi-line string.
- The session dialog uses \`TextEdit\` with an 80 px editing area.
- The first line is shown in the Quick Connect row.
- Old configuration files use the serialized key \`notes\`.
- \`#[serde(alias = "note")]\` also reads the short-lived upstream key so an
  upgrade cannot lose data.
- Serialization always writes \`notes\`, keeping one stable format.

Do not replace this with a singular \`note\` field or a single-line input without
an explicit migration plan. The model plumbing is in \`src/config.rs\`,
\`src/app.rs\`, \`ui/session_dialog.slint\`, \`ui/app.slint\`, and
\`ui/welcome.slint\`. The compatibility test is
\`session_notes_are_backward_compatible_across_field_rename\`.

### Immersive wallpaper and default layout

Wallpaper state is persisted in \`ConfigFile\` and applied by Rust before the
main window is shown. \`src/config.rs\` owns default values and revisioned
migration; \`src/app.rs\` loads the image and derived theme; \`ui/app.slint\`
and \`ui/theme.slint\` render the wallpaper and frosted panel overlay.

The default-layout migration is deliberately revisioned. It changes only
legacy defaults and must not overwrite a user's explicit wallpaper, disabled
wallpaper, dock edge, or opacity choice. Preserve \`defaults_rev\`,
\`migrate_defaults\`, and the \`wallpaper_defaults_to_ms_but_keeps_explicit_choice\`
test when merging configuration changes.

The docked Quick Connect layout has a geometry invariant: a collapsed resource
ToolStrip is counted exactly once. In \`ui/app.slint\`, when
\`sidebar-strip-outside\` is true, \`sb-w\` / \`sb-h\` must be zero because the
36 px strip is already consumed by \`welcome-taken\`. Reintroducing a second
36 px reservation creates the transparent gap visible between Quick Connect
and the terminal.

### SFTP image viewer

The viewer is a read-only preview path for image files in SFTP:

- \`src/sftp.rs\` identifies image entries and provides guarded byte reads.
- \`src/app.rs\` loads the selected image asynchronously and guards refresh
  generations so a late response cannot replace a newer preview.
- \`ui/app.slint\` owns overlay state and callbacks.
- \`ui/image_viewer.slint\` renders zoom, pan, close, and previous/next actions.

Keep preview loading separate from the normal file download path. Preserve
generation checks and bounds checks when changing directory refresh or
pagination behavior.

### GPU resource monitoring

GPU data is normalized into \`GpuSnapshot\` in \`src/system.rs\`. Local hardware
collection and remote monitor parsing feed the same snapshot/model path;
\`src/app.rs\` converts it to \`GpuInfo\` and the sidebar plus detached system
information window consume the shared model. Parsing must remain bounded and
overflow-safe because remote monitor output is untrusted.

### Update-check repository

The update notification and release link intentionally point to the local
repository:

\`https://github.com/capkten/meatshell\`

The URL appears in the release-page action and GitHub API check in
\`src/app.rs\`. Do not blindly restore it to an upstream-owned repository during
merge. The \`update-check-enabled\` setting remains user-configurable and
persisted.

### Connection and transfer lifecycle hardening

The latest local fixes address failures that are easy to reintroduce while
merging asynchronous code:

- \`src/local.rs\`: local process exit handling is idempotent.
- \`src/ssh.rs\`: connection teardown, prompt setup, and runtime forward
  cancellation are coordinated so no task keeps a dead session alive.
- \`src/sftp.rs\`: directory-transfer children retain the parent transfer id,
  cancellation is read from the registered transfer state, and cleanup is
  performed on failure/cancel.

Keep the lifecycle tests in \`src/local.rs\`, \`src/ssh.rs\`, and \`src/sftp.rs\`
with the implementation. Avoid replacing shared cancellation state with a
per-task copy.

## Merge Checklist

1. Compare upstream changes against \`514c348\` and this document before
   accepting conflicts in the files listed above.
2. Check the serialized \`Session\` schema for both \`notes\` and \`note\`; never
   ship a build that reads one and writes an empty value for the other.
3. Run \`cargo fmt --all -- --check\`, \`cargo test --locked\`, and
   \`cargo build --release --locked\`.
4. Manually verify Quick Connect with the resource sidebar collapsed and
   expanded, edit a multi-line note, restart, and confirm the note remains.
5. Verify image preview, GPU details, update checking, transfer cancellation,
   and local-terminal close behavior before creating a release tag.
