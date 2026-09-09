# Task 3 Report: Execute Docker Requests over the Active SSH Session

## Status

Task 3 is implemented in the focused code commit `aa7e92c` (`feat: run docker queries over active ssh sessions`). This takeover audited that commit in place, corrected the stale report, and preserved the unrelated pre-existing `.superpowers/brainstorm/` worktree entry.

## Requirements and implementation

- `SessionCommand::DockerExec` carries a `DockerRequest` and a dedicated `oneshot::Sender<DockerExecResult>`.
- `SessionHandle::docker_exec` enqueues that command and returns the receiver. Docker replies are not `SessionEvent`s, so Docker output cannot enter terminal rendering or command history.
- `run_remote_docker` reuses the authenticated `Arc<russh::client::Handle<ClientHandler>>`; it never calls `execute_command` and therefore does not open a second SSH connection.
- Each request opens a short-lived session channel, executes `remote_command`, collects stdout and stderr independently with a 4 MiB bound per stream, captures the exit status, and applies a five-second timeout covering channel open, exec, and collection.
- Channel-open and exec failures return useful stderr text. Timeout results return useful stderr text and set `timed_out`; output is never logged.
- Local, Telnet, and serial workers explicitly handle `DockerExec` with an explanatory unsupported-worker reply. Existing raw input, resize, tunnel, process-control, close, and reader behavior remains unchanged, and Docker target routing is unchanged.
- The existing single `append_bounded` helper is reused; no duplicate helper remains in `src/ssh/impls/ssh.rs`.

## TDD evidence

The required no-network transport test is present as `session_handle_can_enqueue_docker_request` in `src/ssh/struct/event.rs`.

- The exact brief command, `cargo test session_handle_can_enqueue_docker_request --lib`, cannot run in this repository because `meatshell` is binary-only: Cargo reports `error: no library targets found in package 'meatshell'`.
- The equivalent valid command, `cargo test session_handle_can_enqueue_docker_request`, passes and exercises the real channel enqueue path without starting SSH.
- The implementation was already present in the inherited focused commit when this takeover began, so the original missing-API RED transition was not replayed destructively. The prior executor's report recorded that RED transition; this report does not claim an independently reproduced missing-API failure.

## Verification results

- `cargo fmt --all -- --check`: PASS.
- `cargo test session_handle_can_enqueue_docker_request`: PASS, 1 passed, 0 failed, 281 filtered.
- `cargo test --locked`: PASS, 282 passed, 0 failed.
- `cargo check`: PASS, with 11 existing dead-code warnings outside the Task 3 behavior.
- `cargo clippy --all-targets -- -D warnings`: FAILS on the existing repository-wide lint baseline. Findings include dead code, module inception, too many arguments, MSRV compatibility, and style lints across unrelated files; the Task 3 additions introduced no distinct clippy diagnostic.
- No real SSH connection was used.

## Files in the Task 3 change

- `src/ssh/struct/command.rs`
- `src/ssh/struct/event.rs`
- `src/ssh/impls/ssh.rs`
- `src/terminal/impls/local.rs`
- `src/terminal/impls/telnet.rs`
- `src/terminal/impls/serial.rs`
- `.superpowers/sdd/task-3-report.md`

## Self-review

- The transport API compiles and has a no-network enqueue test.
- The SSH worker uses the already authenticated handle and an isolated response channel.
- Collection is bounded, timed, non-panicking, and keeps sensitive Docker arguments/output out of logs.
- Non-SSH workers are exhaustive and reply instead of silently dropping the new command.
- No app/UI wiring, session routing, fork-specific behavior, or `.superpowers/brainstorm/` content was changed.

## Concerns

- There is no live SSH integration fixture in the project, so channel-open, exec, remote-output, and timeout behavior is covered by implementation inspection and compilation rather than a network test.
- The brief's `--lib` test command is incompatible with this binary-only crate; the equivalent binary test command is the valid focused evidence.
- Strict clippy remains blocked by the pre-existing repository baseline and should be handled separately from Task 3.
