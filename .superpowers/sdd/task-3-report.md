# Task 3 Report: Execute Docker Requests over the Active SSH Session

## Status

Implemented and verified in the focused commit.

## Implementation

- Added `SessionCommand::DockerExec` carrying a `DockerRequest` and a dedicated oneshot reply channel.
- Added `SessionHandle::docker_exec`, which enqueues the request without routing Docker output through `SessionEvent`, terminal rendering, or command history.
- Added the SSH-only `run_remote_docker` helper. It reuses the authenticated `Arc<Handle<ClientHandler>>`, opens a short-lived session channel, executes `remote_command`, collects stdout/stderr independently up to 4 MiB, captures the exit status, and applies a 5-second timeout.
- Channel-open, exec, collection, and timeout failures return a non-panicking `DockerExecResult` with useful stderr; timeout results set `timed_out`. Command arguments and Docker output are not logged.
- Added exhaustive `DockerExec` handling to local, Telnet, and serial workers. Each returns an explicit unsupported-worker error and leaves existing raw input, resize, tunnel, process-control, close, and reader behavior unchanged.

## TDD Evidence

1. Added `session_handle_can_enqueue_docker_request` before production implementation.
2. The exact brief command, `cargo test session_handle_can_enqueue_docker_request --lib`, could not compile a test because this binary-only crate has no library target (`error: no library targets found in package 'meatshell'`).
3. The equivalent crate test command, `cargo test session_handle_can_enqueue_docker_request`, then failed for the intended missing API: `SessionHandle::docker_exec` and `SessionCommand::DockerExec` did not exist.
4. After implementing the API and worker branches, the focused test passed.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo check`: passed.
- `cargo test session_handle_can_enqueue_docker_request`: passed, 1 passed, 281 filtered.
- `cargo test --locked`: passed, 282 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: fails on the repository's pre-existing broad lint baseline (dead code, module inception, argument count, MSRV, and other unrelated findings). The Task 3 transport API's intentional unused warnings were explicitly suppressed because controller/UI wiring is out of scope for this task.

## Files Changed

- `src/ssh/struct/command.rs`
- `src/ssh/struct/event.rs`
- `src/ssh/impls/ssh.rs`
- `src/terminal/impls/local.rs`
- `src/terminal/impls/telnet.rs`
- `src/terminal/impls/serial.rs`
- `.superpowers/sdd/task-3-report.md`

## Self-Review

- The SSH Docker path uses the existing authenticated handle and never calls `execute_command`, so it cannot create a second SSH connection.
- Docker replies use a separate oneshot channel and cannot enter terminal output/history.
- Output is bounded per stream and the timeout covers channel open, exec, and collection.
- Non-SSH workers reply instead of dropping the new command, preserving exhaustive matching without changing Docker target selection.
- No Slint/UI/controller code, `.superpowers/brainstorm`, or fork-specific behavior was changed.

## Concerns

- No live SSH integration test was added because the brief requires a no-network transport test and no authenticated server fixture exists.
- The requested `--lib` focused command is structurally incompatible with this binary-only crate; the equivalent binary test command supplied the RED/GREEN evidence.
- Clippy remains red at the known repository baseline and should be revisited separately from Task 3.
