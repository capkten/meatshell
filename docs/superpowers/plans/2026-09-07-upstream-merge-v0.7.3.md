# Upstream v0.7.3 Merge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge `upstream/main` at `v0.7.3` into the local fork while retaining all current local functionality and preserving the fork-specific contracts documented in `docs/upstream-local-changes.md`.

**Architecture:** Use the isolated `codex/merge-upstream-v0.7.3` worktree for all merge operations. Store the original worktree's tracked and untracked edits in a named stash, merge upstream, reapply the stash without dropping it, resolve conflicts by contract, and validate the candidate before any integration into `main`.

**Tech Stack:** Git worktrees and conflict resolution; Rust/Cargo; Slint; unit tests; Windows MSVC release build.

## Global Constraints

- Keep the original `D:\codes\meatshell` checkout and `main` ref unchanged during the attempt.
- Preserve `Session.notes` as multiline serialized `notes` with `#[serde(alias = "note")]` compatibility.
- Preserve wallpaper/default-layout migration, Quick Connect geometry, SFTP image preview guards, GPU snapshot hardening, local update-check URL, and connection/transfer lifecycle fixes.
- Keep the local ProxyCommand behavior and tests from the uncommitted worktree changes.
- Do not change the pinned `russh` 0.49 dependency without an explicit compatibility decision.
- Keep the named stash until all validation completes; use `git stash apply`, never `git stash pop`.
- Do not claim the candidate is complete unless every validation command exits successfully.

---

### Task 1: Capture and protect the original worktree

**Files:**
- Modify: Git stash metadata only in the original checkout; no source files.

**Interfaces:**
- Consumes: The original worktree at `D:\codes\meatshell`.
- Produces: A named stash containing the 13 tracked edits and `.superpowers/`.

- [ ] **Step 1: Record the pre-merge state**

Run from `D:\codes\meatshell`:

```powershell
git status --short --branch
git diff --stat
git rev-parse HEAD
git stash list
```

Expected: `main` remains at `74f0627`, the 13 tracked edits and untracked `.superpowers/` are visible, and the pre-existing stash list is recorded.

- [ ] **Step 2: Create a recoverable named stash**

```powershell
git stash push --include-untracked -m "codex pre upstream v0.7.3 merge 2026-09-07"
```

Expected: the original checkout becomes clean and the command reports the created stash. Do not drop or clear any stash.

- [ ] **Step 3: Verify the backup and clean checkout**

```powershell
git status --short --branch
git stash list
git stash show --stat --include-untracked 'stash@{0}'
```

Expected: the checkout is clean, the new named stash is present, and its stat includes the local feature files and the untracked directory.

### Task 2: Merge the upstream history in the isolated branch

**Files:**
- Modify: Files reported by Git as merge conflicts in `D:\codes\meatshell-merge-upstream-v0.7.3`.

**Interfaces:**
- Consumes: `upstream/main` at `e428f12` and the clean candidate branch.
- Produces: A merge state containing both histories, with no unresolved index entries after conflict resolution.

- [ ] **Step 1: Confirm candidate branch and upstream tip**

```powershell
git branch --show-current
git rev-parse --short HEAD
git rev-parse --short upstream/main
git status --short --branch
```

Expected: branch `codex/merge-upstream-v0.7.3`, candidate base `74f0627`, upstream tip `e428f12`, clean status.

- [ ] **Step 2: Merge without fast-forwarding**

```powershell
git merge --no-ff --no-edit upstream/main
```

Expected: Git either creates a merge commit or stops with an explicit conflict list. Never use `git reset --hard` to bypass a conflict.

- [ ] **Step 3: Inventory unresolved paths**

```powershell
git status --short
git diff --name-only --diff-filter=U
```

Expected: the conflict list is explicit and every path is assigned to one of the protected contract groups before resolution.

### Task 3: Resolve conflicts without dropping fork behavior

**Files:**
- Inspect and resolve as applicable: `src/config/impls/config.rs`, `src/config/struct/session.rs`, `src/app.rs`, `ui/app.slint`, `ui/session_dialog.slint`, `src/sftp/impls/sftp.rs`, `src/ssh/impls/proxy.rs`, `src/ssh/impls/ssh.rs`, `src/ssh/impls/ssh_config.rs`, `src/ssh/struct/connection.rs`, `Cargo.toml`, `Cargo.lock`, and any paths listed by `git diff --name-only --diff-filter=U`.
- Reference: `docs/upstream-local-changes.md`.

**Interfaces:**
- Consumes: Git conflict markers and both parent versions.
- Produces: A source tree with no conflict markers and preserved local contracts.

- [ ] **Step 1: Resolve session schema conflicts**

For every conflict touching session serialization, retain a multiline `Session.notes`, the serialized key `notes`, the `note` deserialization alias, and the compatibility test `session_notes_are_backward_compatible_across_field_rename`. Compare both sides with:

```powershell
git show HEAD:src/config/struct/session.rs
git show upstream/main:src/config/struct/session.rs
rg -n "notes|note|session_notes_are_backward" src ui tests
```

- [ ] **Step 2: Resolve UI and migration conflicts**

Retain `defaults_rev`, `migrate_defaults`, explicit wallpaper choices, the local update-check URL `https://github.com/capkten/meatshell`, the Quick Connect collapsed-sidebar geometry invariant, and the multiline note editor. Verify the relevant code and tests with:

```powershell
rg -n "defaults_rev|migrate_defaults|wallpaper_defaults_to_ms|capkten/meatshell|sidebar-strip-outside|welcome-taken" src ui tests
```

- [ ] **Step 3: Resolve SFTP, SSH, lifecycle, and proxy conflicts**

Keep image-preview generation and bounds checks, transfer cancellation state, idempotent teardown, shell setup behavior, ProxyCommand argument parsing, and their tests. Compare both parent versions before staging any path:

```powershell
git diff --cc -- src/sftp/impls/sftp.rs src/ssh/impls/proxy.rs src/ssh/impls/ssh.rs src/ssh/impls/ssh_config.rs
rg -n "generation|cancel|ProxyCommand|proxy command|lifecycle|known_hosts" src tests
```

- [ ] **Step 4: Resolve dependency and lockfile conflicts conservatively**

Keep the deliberate `russh` 0.49 pin unless the upstream merge is demonstrably compatible and the fork contract is explicitly updated. Regenerate `Cargo.lock` only through Cargo after `Cargo.toml` is resolved.

- [ ] **Step 5: Verify and stage the resolved merge**

```powershell
rg -n "^(<<<<<<<|=======|>>>>>>>)" src ui Cargo.toml Cargo.lock tests
git diff --check
git add -A
git status --short
```

Expected: no conflict markers, no whitespace errors, and no unmerged paths.

- [ ] **Step 6: Commit the merge resolution**

```powershell
git commit -m "merge: integrate upstream v0.7.3 preserving local features"
```

### Task 4: Reapply the original local feature work

**Files:**
- Modify: the paths recorded in the named stash, including the untracked `.superpowers/` directory.

**Interfaces:**
- Consumes: the completed upstream merge and the named stash from Task 1.
- Produces: a candidate worktree containing the local uncommitted feature work on top of the merge.

- [ ] **Step 1: Apply, but do not drop, the stash**

Run from the original checkout's stash reference, using the exact stash recorded in Task 1:

```powershell
git -C D:\codes\meatshell-merge-upstream-v0.7.3 stash apply 'stash@{0}'
```

Expected: local changes reappear in the candidate worktree; conflicts, if any, are reported without deleting the stash.

- [ ] **Step 2: Resolve reapplication conflicts by preserving local behavior**

For each conflict, use the pre-merge stash and merge parents as evidence:

```powershell
git -C D:\codes\meatshell-merge-upstream-v0.7.3 status --short
git -C D:\codes\meatshell-merge-upstream-v0.7.3 diff --name-only --diff-filter=U
git -C D:\codes\meatshell-merge-upstream-v0.7.3 stash show -p --include-untracked 'stash@{0}'
```

Retain all local ProxyCommand, SSH config, SFTP, session, and UI changes unless they duplicate an upstream fix; when behavior overlaps, combine the implementations and keep the stronger regression coverage.

- [ ] **Step 3: Confirm local feature signatures survived**

```powershell
rg -n "ProxyCommand|parse_proxy_command|session_notes_are_backward_compatible|wallpaper_defaults_to_ms|capkten/meatshell" src ui tests
git -C D:\codes\meatshell-merge-upstream-v0.7.3 diff --stat
```

Expected: all required local symbols/tests remain present and the candidate diff includes the intended local feature work.

### Task 5: Validate the merge candidate

**Files:**
- No intentional source changes; validation may update only generated build outputs.

**Interfaces:**
- Consumes: the conflict-free candidate with local changes reapplied.
- Produces: fresh command evidence for formatting, linting, tests, and release build.

- [ ] **Step 1: Run formatting check**

```powershell
cargo fmt --all -- --check
```

Expected: exit code 0 and no formatting differences.

- [ ] **Step 2: Run strict clippy**

```powershell
cargo clippy --all-targets -- -D warnings
```

Expected: exit code 0 with no warnings promoted to errors.

- [ ] **Step 3: Run locked tests**

```powershell
cargo test --locked
```

Expected: exit code 0, including the session notes, wallpaper, ProxyCommand, SFTP, SSH lifecycle, and upstream regression tests.

- [ ] **Step 4: Run locked release build**

```powershell
cargo build --release --locked
```

Expected: exit code 0 and a release binary built from the candidate.

- [ ] **Step 5: Inspect final state and stash retention**

```powershell
git status --short --branch
git log --oneline --decorate -6
git stash list
git diff --check
```

Expected: no unmerged files, the merge commit is present, local changes remain visible, the named stash is still available, and there are no whitespace errors.
