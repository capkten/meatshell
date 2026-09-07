# Upstream v0.7.3 Merge Design

## Goal

Integrate `upstream/main` at `v0.7.3` into the local `meatshell` fork while preserving the fork's uncommitted ProxyCommand, session, SFTP, SSH, UI, and configuration work and all documented fork-specific behavior.

## Constraints and protected behavior

- The user's current worktree changes must remain recoverable, including tracked edits and the untracked `.superpowers/` directory.
- `Session.notes` remains a multiline field serialized as `notes`, accepts the legacy `note` alias, and keeps its compatibility test.
- Wallpaper/default-layout migration, the collapsed Quick Connect geometry invariant, SFTP image preview generation/bounds checks, GPU snapshot parsing, local update-check URL, and connection/transfer lifecycle hardening remain intact.
- The local `russh` 0.49 pin and other fork-specific dependency decisions are not changed blindly.
- The current local ProxyCommand implementation and its tests must survive upstream proxy/SSH changes.

## Integration approach

1. Work in the isolated `codex/merge-upstream-v0.7.3` worktree so the user's checkout and `main` ref remain untouched during conflict resolution.
2. Record the clean baseline and create a named stash backup containing all current tracked and untracked worktree changes.
3. Merge `upstream/main` without discarding either side. Resolve conflicts by preserving the fork contracts above and then reapply the named stash with `git stash apply`, retaining the stash until validation finishes.
4. Inspect the resulting diff and conflict resolutions, especially session serialization, UI layout, SFTP/image paths, GPU monitoring, update checking, lifecycle code, and ProxyCommand parsing.
5. Run formatting, strict linting, locked unit tests, and a locked release build. Keep any unresolved failure visible rather than declaring the merge complete.

## Validation boundary

The merge candidate is acceptable only if the merge is complete, the local worktree changes are present after stash application, the compatibility tests pass, the full locked test suite passes, formatting and clippy pass, and the release build succeeds. The original `main` worktree remains unchanged until the user explicitly chooses how to integrate the validated candidate.
