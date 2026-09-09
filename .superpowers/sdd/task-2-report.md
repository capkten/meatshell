# Task 2 Report: 固定 Docker CLI 命令和本机执行器

## Status

完成。Task 2 仅涉及 Docker 命令构造、本机执行和失败分类测试；未修改 SSH、UI 或 controller 功能。

## Implementation

- Added `src/docker/command.rs` with:
  - `docker_args(&DockerRequest)`, using only fixed Docker operations plus the selected inspect ID.
  - `remote_command(&DockerRequest)`, reusing fixed arguments, shell-quoting the JSON format template, and safely quoting inspect IDs including embedded apostrophes.
  - `run_local(DockerRequest)`, using `tokio::process::Command` directly with no shell or request environment and a five-second timeout.
  - Correct result mapping for timeout, executable-not-found, other spawn errors, exit status, stdout, and stderr.
- Registered the command module in `src/docker/mod.rs`.
- Added focused tests for every fixed request shape, JSON-template arguments, local/remote inspect command construction, shell-safe quoting, and `not_found` → `NotInstalled` classification.

## TDD RED/GREEN evidence

- The brief’s `cargo test docker::command::tests --lib` command cannot run in this binary-only crate; Cargo reports `no library targets found in package meatshell`.
- The equivalent valid command, `cargo test docker::command::tests`, passed after auditing the inherited partial implementation and adding the missing coverage: 6 passed.
- The inherited partial implementation was already present when takeover began, so the added tests did not provide a clean pre-implementation RED cycle. No production code was blindly discarded; the existing implementation was audited against the brief and retained where correct.

## Verification

- `cargo fmt --all -- --check` — passed.
- `cargo test docker::command::tests` — 6 passed.
- `cargo check` — passed; only known unrelated dead-code warnings remain elsewhere in the repository.
- `cargo test --locked` — 278 passed, 0 failed.

## Files

- `src/docker/command.rs` — command builders, local executor, and tests.
- `src/docker/mod.rs` — command module registration.
- `.superpowers/sdd/task-2-report.md` — this report.

## Self-review and concerns

- Local execution is argument-based (`Command::new("docker").args(...).output()`), so request values cannot become shell syntax.
- Remote inspect IDs are single-quoted and apostrophes are escaped as the standard shell `\\''` sequence.
- Timeout results do not expose partial output; successful process output preserves stdout, stderr, and `ExitStatus::code()`.
- The repository is binary-only, so focused tests must omit `--lib`; this is a test-command/documentation mismatch, not an implementation failure.
- The pre-existing `.superpowers/brainstorm/` untracked files were intentionally left untouched and are not part of the Task 2 commit.

## Review fix: remote JSON template quoting

- Root cause: the non-inspect `remote_command` path joined raw `docker_args` with spaces, splitting `{{json .}}` into two shell words.
- Fix: local `docker_args` is unchanged; remote fixed arguments now quote the exact JSON template as `'{{json .}}'`. Inspect IDs retain safe single-quote escaping, and no search text is interpolated.
- TDD RED: the three new Version, Containers, and Images remote-command tests failed with raw `{{json .}}` output before the fix.
- TDD GREEN: `cargo test docker::command::tests` passed with 9 tests, including all three regression tests and the existing inspect quoting tests.
- Follow-up verification: `cargo fmt --all -- --check`, `cargo check`, and `cargo test --locked` all passed; the locked suite reported 281/281 tests passing. The check/test commands retain only known unrelated dead-code warnings.
