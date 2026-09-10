# Docker Terminal Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add session-scoped Docker Tab completion for SSH Bash/Zsh terminals while preserving any native remote Docker completer.

**Architecture:** Keep the existing raw-key PTY path unchanged. Add a focused Rust module containing a single-session Bash/Zsh completion setup script, inject it into the existing hidden SSH prompt setup, and let the remote Shell own line editing and candidate rendering. The script probes native Docker completion once per interactive SSH session, latches `native`, `fallback`, or `unavailable`, and only the fallback mode queries fixed read-only Docker list commands when Tab is pressed.

**Tech Stack:** Rust, Tokio/russh SSH PTY worker, Bash completion, Zsh completion, inline `#[cfg(test)]` unit tests, optional Unix shell-process tests, existing Cargo test/lint/format commands.

## Global Constraints

- Only SSH Bash and Zsh sessions participate in the fallback; PowerShell, cmd, fish, ash/dash, local terminals, and disabled Shell integration keep their current behavior.
- Native Docker completion detection runs once per interactive SSH session; the mode is latched and never re-probed or switched during that session.
- Native completion is never overwritten; an indeterminate native probe must leave the remote completion state untouched.
- The fallback supports `docker run`, `exec`, `start`, `stop`, `rm`, `logs`, and `inspect`; `run` completes images, resource commands complete containers, and `inspect` completes both.
- Candidate queries use fixed remote Docker CLI arguments, suppress stdout/stderr from failures, never use `sudo`, and never concatenate the current input into shell or Docker command text.
- Do not modify `ui/terminal_view.slint`, the generic `src/app.rs` key forwarding path, `key_to_pty_bytes`, the Docker monitor UI, or persistent remote shell configuration.
- The embedded completion script must not contain a single quote because it is inserted into the existing single-quoted `eval` body.
- Preserve the existing OSC 7 cwd notification, OSC 697 command history capture, OSC 699 setup-complete marker, prompt echo suppression, and Bash/Zsh probe behavior.
- Run validation in repository order: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, then `cargo check`.

---

## File Map

- Create `src/ssh/impls/docker_completion.rs`
  - Own the `DOCKER_COMPLETION_SETUP` shell script.
  - Keep shell-specific completion registration, mode latching, option scanning, candidate lookup, and failure silence out of the large SSH worker file.
  - Hold focused unit tests for the script contract and Unix shell behavior tests.
- Modify `src/ssh/mod.rs`
  - Register the new private `docker_completion` module beside the existing SSH implementation modules.
- Modify `src/ssh/impls/ssh.rs`
  - Import `DOCKER_COMPLETION_SETUP`.
  - Split the existing `PROMPT_BODY` into a prefix and suffix, compose a `prompt_body() -> String`, and place the completion setup before the existing OSC 699 completion marker.
  - Update prompt setup tests to inspect the composed body rather than a static unsplit string.
- Do not modify `src/docker`, `src/app.rs`, `ui/terminal_view.slint`, or any session configuration schema.

## Interfaces Between Tasks

The implementation uses these exact internal interfaces:

```rust
// src/ssh/impls/docker_completion.rs
pub(crate) const DOCKER_COMPLETION_SETUP: &str;

// src/ssh/impls/ssh.rs
fn prompt_body() -> String;
```

`prompt_body()` returns the complete command body that is wrapped by the existing leading-space and carriage-return injection. `DOCKER_COMPLETION_SETUP` is a shell-body fragment with no surrounding `eval` quotes and no single quote characters. No Slint type or Docker monitor state crosses this boundary.

## Task 1: Establish the failing session-mode contract

**Files:**
- Modify: `src/ssh/impls/ssh.rs` in `prompt_setup_echo_tests`
- Test: `src/ssh/impls/ssh.rs` in the existing inline test module

**Interfaces:**
- Consumes: current `PROMPT_BODY` constant and existing prompt setup test helpers.
- Produces: failing assertions that define the one-time native probe, fallback registration, supported command set, and hidden setup ordering.

- [ ] **Step 1: Add the contract tests before changing production code**

Add these tests to the existing `prompt_setup_echo_tests` module and import no new production helper yet:

```rust
#[test]
fn prompt_setup_latches_docker_completion_mode_once() {
    assert!(PROMPT_BODY.contains("__ms_docker_completion_mode"));
    assert!(PROMPT_BODY.contains("complete -p docker"));
    assert!(PROMPT_BODY.contains("__ms_docker_completion_registered"));
    assert!(PROMPT_BODY.contains("__ms_docker_completion_mode=fallback"));
}

#[test]
fn prompt_setup_covers_the_supported_docker_resource_commands() {
    for command in ["run", "exec", "start", "stop", "rm", "logs", "inspect"] {
        assert!(
            PROMPT_BODY.contains(command),
            "Docker completion setup is missing command {command}"
        );
    }
    assert!(PROMPT_BODY.contains("docker image ls"));
    assert!(PROMPT_BODY.contains("docker ps -a"));
}

#[test]
fn docker_completion_setup_runs_before_the_existing_ready_marker() {
    let completion = PROMPT_BODY
        .find("__ms_docker_completion_mode")
        .expect("completion mode marker");
    let ready = PROMPT_BODY
        .find("699;ready")
        .expect("prompt setup ready marker");
    assert!(completion < ready);
}
```

- [ ] **Step 2: Run the focused tests and verify the expected RED state**

Run:

```text
cargo test --locked prompt_setup_echo_tests::prompt_setup_latches_docker_completion_mode_once
cargo test --locked prompt_setup_echo_tests::prompt_setup_covers_the_supported_docker_resource_commands
cargo test --locked prompt_setup_echo_tests::docker_completion_setup_runs_before_the_existing_ready_marker
```

Expected: each command compiles the existing project and fails its assertion because the current prompt setup has no Docker completion mode, fallback registration, or Docker list command. Do not change the tests to make the current behavior pass.

## Task 2: Implement the focused Bash/Zsh completion script

**Files:**
- Create: `src/ssh/impls/docker_completion.rs`
- Modify: `src/ssh/mod.rs`
- Test: `src/ssh/impls/docker_completion.rs` inline tests

**Interfaces:**
- Consumes: fixed remote `docker image ls` and `docker ps -a` output at completion time.
- Produces: `pub(crate) const DOCKER_COMPLETION_SETUP: &str` for the SSH prompt builder.

- [ ] **Step 1: Register the new private module and add the script contract tests**

Add this declaration to `src/ssh/mod.rs` beside the existing `ssh` implementation module:

```rust
#[path = "impls/docker_completion.rs"]
mod docker_completion;
```

Create the new file with the tests below. The tests should be written before filling in the final script body; the initial empty or incomplete constant is expected to fail the assertions.

```rust
#[cfg(test)]
mod tests {
    use super::DOCKER_COMPLETION_SETUP;

    #[test]
    fn setup_is_safe_to_embed_in_the_existing_single_quoted_eval() {
        assert!(!DOCKER_COMPLETION_SETUP.contains('\''));
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_mode"));
    }

    #[test]
    fn setup_has_one_latched_native_probe_and_one_fallback_registration_guard() {
        assert_eq!(
            DOCKER_COMPLETION_SETUP
                .matches("complete -p docker")
                .count(),
            1
        );
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_registered"));
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_mode=native"));
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_mode=fallback"));
    }

    #[test]
    fn setup_has_all_supported_command_branches_and_fixed_queries() {
        for command in ["run", "exec", "start", "stop", "rm", "logs", "inspect"] {
            assert!(
                DOCKER_COMPLETION_SETUP.contains(command),
                "missing completion branch for {command}"
            );
        }
        assert!(DOCKER_COMPLETION_SETUP.contains("docker image ls"));
        assert!(DOCKER_COMPLETION_SETUP.contains("docker ps -a"));
        assert!(DOCKER_COMPLETION_SETUP.contains("2>/dev/null"));
    }
}
```

Run:

```text
cargo test --locked docker_completion::tests
```

Expected: the new assertions fail because the script is not implemented yet.

- [ ] **Step 2: Implement the session latch and native-preserving registration**

Define `DOCKER_COMPLETION_SETUP` as a raw Rust string with these exact runtime rules:

```sh
if [ -z "${__ms_docker_completion_mode+x}" ]; then
    if [ -n "$BASH_VERSION" ]; then
        if complete -p docker >/dev/null 2>&1; then
            __ms_docker_completion_mode=native
        else
            __ms_docker_completion_mode=fallback
        fi
    elif [ -n "$ZSH_VERSION" ]; then
        if command -v compdef >/dev/null 2>&1 && compdef -p docker >/dev/null 2>&1; then
            __ms_docker_completion_mode=native
        elif command -v compdef >/dev/null 2>&1; then
            __ms_docker_completion_mode=fallback
        else
            __ms_docker_completion_mode=unavailable
        fi
    else
        __ms_docker_completion_mode=unavailable
    fi
fi
```

The Bash path must use `complete -p docker` exactly once. The Zsh path may only use the completion registry/function check inside the same `-z` latch. A later execution of the fragment must reuse the existing mode and must not run the native probe again.

When the mode is `fallback`, register only once:

```sh
if [ "$__ms_docker_completion_mode" = fallback ] && [ -z "${__ms_docker_completion_registered+x}" ]; then
    __ms_docker_completion_registered=1
    if [ -n "$BASH_VERSION" ]; then
        complete -F __ms_docker_bash_complete docker
    elif [ -n "$ZSH_VERSION" ]; then
        compdef _ms_docker_zsh_complete docker
    fi
fi
```

If the Zsh completion registry is unavailable, leave the mode as `unavailable` and do not override any remote behavior. Define all fallback functions inside the fallback branch or behind private names so native completion remains authoritative.

- [ ] **Step 3: Implement fixed candidate sources and command-position parsing**

Add private shell functions with these names and behavior:

```sh
__ms_docker_images
__ms_docker_containers
__ms_docker_candidates
__ms_docker_bash_complete
_ms_docker_zsh_complete
```

`__ms_docker_images` must run:

```sh
docker image ls --format "{{.Repository}}:{{.Tag}}" 2>/dev/null
```

and remove `<none>` repository/tag entries without showing errors. `__ms_docker_containers` must run:

```sh
docker ps -a --format "{{.ID}}\t{{.Names}}" 2>/dev/null
```

and emit both IDs and names. `__ms_docker_candidates` must return image candidates, container candidates, or a deduplicated combination for `inspect`.

The parser must walk words after the first `docker` token, skip options with values from this fixed list, and stop treating options specially after `--`:

```text
--name -e --env --env-file -h --hostname -l --label --mount --network
--publish -p --volume -v --workdir -w --user -u --restart --platform
--entrypoint --stop-timeout --memory --cpus --pull --signal -s --time -t
--since --tail --until --format -f --type --detach-keys
```

The completion target rules must be:

```text
run:     complete the first positional word after the subcommand as an image;
         return no candidates after the image.
exec:    complete container positionals.
start:   complete container positionals.
stop:    complete container positionals.
rm:      complete container positionals.
logs:    complete container positionals.
inspect: complete container and image positionals.
```

The Bash function must populate `COMPREPLY` using `COMP_WORDS`, `COMP_CWORD`, and `compgen -W`. The Zsh function must use `words`, `CURRENT`, and `compadd`; it may reuse the fixed candidate-source function. A current token beginning with `-`, an option value, or an unknown command position must return an empty candidate list. No current input value may be interpolated into the Docker command string.

- [ ] **Step 4: Run the module tests and make the script contract GREEN**

Run:

```text
cargo test --locked docker_completion::tests
```

Expected: all script contract tests pass, including the single `complete -p docker` occurrence, the session latch, fixed commands, and no-single-quote embedding rule.

- [ ] **Step 5: Commit the focused script module**

```text
git add src/ssh/mod.rs src/ssh/impls/docker_completion.rs
git commit -m "feat: add session-scoped docker completion script"
```

## Task 3: Compose the script into the existing hidden SSH prompt setup

**Files:**
- Modify: `src/ssh/impls/ssh.rs` around `PROMPT_BODY`, `prompt_setup`, and `prompt_setup_echo_tests`
- Test: `src/ssh/impls/ssh.rs` existing prompt setup tests

**Interfaces:**
- Consumes: `super::docker_completion::DOCKER_COMPLETION_SETUP`.
- Produces: `fn prompt_body() -> String` used only by the existing SSH startup injection.

- [ ] **Step 1: Add the composition regression test before changing the prompt builder**

Add this test to `prompt_setup_echo_tests` using the future `prompt_body()` interface:

```rust
#[test]
fn composed_prompt_body_keeps_docker_setup_inside_hidden_eval_before_ready() {
    let body = prompt_body();
    let completion = body
        .find("__ms_docker_completion_mode")
        .expect("Docker completion setup");
    let ready = body.find("699;ready").expect("setup ready marker");
    assert!(completion < ready);
    assert!(!body.contains("docker image ls\n"));
}
```

Run:

```text
cargo test --locked prompt_setup_echo_tests::composed_prompt_body_keeps_docker_setup_inside_hidden_eval_before_ready
```

Expected: compile/test failure because the current implementation has no `prompt_body()` function and does not compose the new module.

- [ ] **Step 2: Split and compose the existing prompt body without changing its behavior**

Replace the single `PROMPT_BODY` constant with a prefix/suffix pair and this builder shape:

```rust
const PROMPT_BODY_PREFIX: &str = "test -z \"$FISH_VERSION\" && eval '__msc(){ __c=\"$(fc -ln -1 2>/dev/null)\"; [ -n \"$__c\" ] && [ \"$__c\" != \"$__cl\" ] && { __cl=\"$__c\"; printf \"\\033]697;%s\\007\" \"$__c\"; }; }; __ms7(){ printf \"\\033]7;file://%s%s\\007\" \"$HOSTNAME\" \"$PWD\"; __msc; }; if [ -n \"$ZSH_VERSION\" ]; then autoload -Uz add-zsh-hook 2>/dev/null; add-zsh-hook precmd __ms7; else PROMPT_COMMAND=\"__ms7${PROMPT_COMMAND:+;$PROMPT_COMMAND}\"; fi; : __MEATSHELL_INTERNAL_SETUP_1; if [ -n \"$BASH_VERSION\" ]; then __md=\"$(history 2>/dev/null | { __md=\"\"; while read -r __mn __mr; do case \"$__mr\" in *\"__ms7()\"*\"PROMPT_COMMAND=\"*) __mn=\"${__mn%\\*}\"; __md=\"$__mn $__md\";; esac; done; printf \"%s\" \"$__md\"; })\"; for __mn in $__md; do history -d \"$__mn\" 2>/dev/null; done; unset __md __mn __mr; fi; __cl=\"$(fc -ln -1 2>/dev/null)\"; ";
const PROMPT_BODY_SUFFIX: &str = "printf \"\\033]699;ready\\007\"; __ms7'";

fn prompt_body() -> String {
    format!(
        "{PROMPT_BODY_PREFIX}{DOCKER_COMPLETION_SETUP}; {PROMPT_BODY_SUFFIX}"
    )
}
```

The actual prefix and suffix must preserve every existing byte of the old `PROMPT_BODY`; only the insertion point immediately before `printf "\\033]699;ready\\007"` changes. Update the runtime injection from:

```rust
let prompt_setup = format!(" {}\r", PROMPT_BODY);
```

to:

```rust
let prompt_setup = format!(" {}\r", prompt_body());
```

Update existing prompt tests to call `prompt_body()` and import the new helper instead of `PROMPT_BODY`. Keep all existing OSC echo, history cleanup, zsh redraw, and cursor-resynchronization assertions unchanged apart from obtaining the composed body.

- [ ] **Step 3: Run the complete SSH prompt test group**

Run:

```text
cargo test --locked prompt_setup_echo_tests
```

Expected: all existing prompt setup tests and the new composition test pass. In particular, the setup-complete marker remains after the Docker registration script, so the existing output suppression still hides the injected command.

- [ ] **Step 4: Commit the SSH integration**

```text
git add src/ssh/impls/ssh.rs
git commit -m "feat: inject docker completion into ssh shell setup"
```

## Task 4: Add deterministic Bash/Zsh behavior tests and session-latch regressions

**Files:**
- Modify: `src/ssh/impls/docker_completion.rs` inline tests
- Modify: `src/ssh/impls/ssh.rs` prompt setup tests if a regression belongs with existing marker tests

**Interfaces:**
- Consumes: `DOCKER_COMPLETION_SETUP`, `__ms_docker_bash_complete`, and `_ms_docker_zsh_complete` from the injected script.
- Produces: deterministic tests that do not require a running Docker daemon or network connection.

- [ ] **Step 1: Add Unix Bash tests with a fake Docker executable**

Under `#[cfg(unix)]`, add a helper that creates a unique temporary directory under `std::env::temp_dir()`, writes an executable `docker` shell script, prepends that directory to `PATH`, invokes `bash -c`, and removes the directory after the assertion. Do not add a `tempfile` dependency.

The fake Docker executable must return these fixed outputs:

```sh
if [ "$1" = image ] && [ "$2" = ls ]; then
    printf '%s\n' 'nginx:latest' 'node:20'
elif [ "$1" = ps ] && [ "$2" = -a ]; then
    printf '%s\t%s\n' 'abc123' 'web' 'def456' 'worker'
fi
```

The Bash harness must source/evaluate `DOCKER_COMPLETION_SETUP`, set completion words manually, call `__ms_docker_bash_complete`, and print `COMPREPLY`. Define the Unix-only helper as `fn bash_candidates(words: &[&str], current_index: usize) -> Vec<String>`, then add these tests:

```rust
#[test]
fn bash_run_completes_image_prefix() {
    assert_eq!(
        bash_candidates(&["docker", "run", "ng"], 2),
        vec!["nginx:latest".to_string()]
    );
}

#[test]
fn bash_exec_completes_container_prefix() {
    assert_eq!(
        bash_candidates(&["docker", "exec", "we"], 2),
        vec!["web".to_string()]
    );
}

#[test]
fn bash_inspect_merges_container_and_image_candidates() {
    assert_eq!(
        bash_candidates(&["docker", "inspect", ""], 2),
        vec![
            "abc123".to_string(),
            "def456".to_string(),
            "nginx:latest".to_string(),
            "node:20".to_string(),
            "web".to_string(),
            "worker".to_string(),
        ]
    );
}

#[test]
fn bash_run_stops_image_completion_after_the_image_position() {
    assert!(bash_candidates(&["docker", "run", "nginx", "sh"], 3).is_empty());
}
```

The helper may sort the captured output before comparing it; the shell script itself must use `sort -u` only for the combined `inspect` list and must preserve the candidate text.

- [ ] **Step 2: Add the one-time native/fallback latch tests**

Use a Bash harness that runs the setup twice. Verify both modes:

```text
native completion registered -> mode remains native after the native registration is removed and the setup fragment is evaluated again;
no native completion -> mode becomes fallback, fallback registration is present, and the second evaluation does not execute complete -p docker again or switch modes;
Docker list command exits non-zero -> completion returns no candidates and emits no stderr/stdout.
```

The assertions must inspect `__ms_docker_completion_mode`, `__ms_docker_completion_registered`, `complete -p docker`, and captured output. The native test must use a fake native completer function so it never depends on an installed Docker completion package.

- [ ] **Step 3: Add a Zsh smoke test when Zsh is available**

Run the same fake Docker fixture through `zsh -fc` only when `zsh` can be started. Verify that `_ms_docker_zsh_complete` returns `web` for `docker exec we`. If Zsh is unavailable on the test host, skip this test without failing the Cargo test run; the cross-platform script contract tests remain mandatory.

- [ ] **Step 4: Run the behavior tests and fix only implementation defects**

Run:

```text
cargo test --locked docker_completion
```

Expected: all portable contract tests pass; Unix hosts with Bash pass the four candidate and latch tests; hosts without Zsh report the explicit smoke-test skip. Do not weaken assertions to accommodate a shell parsing error; fix the script or the fixture.

- [ ] **Step 5: Commit deterministic completion tests**

```text
git add src/ssh/impls/docker_completion.rs src/ssh/impls/ssh.rs
git commit -m "test: verify docker completion candidates and latching"
```

## Task 5: Full verification and manual acceptance

**Files:**
- No new files; inspect the final diff and existing fork-specific contract files.

**Interfaces:**
- Consumes: all code and tests from Tasks 1–4.
- Produces: verified build/test evidence and a clean feature diff that leaves the pre-existing `.superpowers/brainstorm/` directory untouched.

- [ ] **Step 1: Inspect the complete diff and repository state**

Run:

```text
git diff HEAD~3..HEAD -- src/ssh/mod.rs src/ssh/impls/ssh.rs src/ssh/impls/docker_completion.rs
git status --short
```

Confirm that only the planned SSH files and tests changed, no remote configuration file was added, no Docker monitor UI code changed, and the existing untracked `.superpowers/brainstorm/` directory remains unmodified.

- [ ] **Step 2: Run formatting**

Run:

```text
cargo fmt --all -- --check
```

Expected: exit code 0 with no formatting diff.

- [ ] **Step 3: Run clippy**

Run:

```text
cargo clippy --all-targets -- -D warnings
```

Expected: exit code 0 with no warnings promoted to errors.

- [ ] **Step 4: Run all locked tests**

Run:

```text
cargo test --locked
```

Expected: all existing and new tests pass; no test requires a live Docker daemon.

- [ ] **Step 5: Run the type/build check**

Run:

```text
cargo check
```

Expected: exit code 0, including Slint build integration and the private SSH module wiring.

- [ ] **Step 6: Perform remote manual acceptance**

Use two SSH Bash/Zsh hosts:

1. On a host with native Docker completion, type `docker run ng<Tab>` and confirm the host's native behavior remains active; repeat after the setup fragment would otherwise be re-evaluated and confirm the mode does not switch.
2. On a host with Docker CLI but no native Docker completion, verify `docker run`, `docker exec`, `docker start`, `docker stop`, `docker rm`, `docker logs`, and `docker inspect` produce the expected image/container candidates.
3. Create or remove a container/image, press Tab again, and confirm candidate data refreshes without another native completion probe.
4. Verify `cd<Tab>`, arrow-key history, Ctrl+C, a simple `docker --version`, and a vim/nano session remain unchanged.
5. Verify Docker missing, daemon unavailable, permission denied, Shell integration disabled, and non-Bash/Zsh shells produce no visible completion error and do not alter typed input.

- [ ] **Step 7: Record final verification before claiming completion**

Capture the focused completion test result plus the format, clippy, full test, and check results. If a manual host is unavailable, report that exact limitation instead of claiming the remote scenario was verified.
