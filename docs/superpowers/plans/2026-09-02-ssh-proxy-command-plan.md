# SSH ProxyCommand Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional, non-shell `ProxyCommand` support to SSH config import, saved sessions, the session editor, and every existing russh connection path.

**Architecture:** Store the raw command template in `Session.proxy_command`, parse it into argv with a small quote-aware parser, expand `%h`/`%p`/`%r`/`%%`, and launch it through `tokio::process::Command`. Wrap its piped stdin/stdout in an async stream owned by the SSH connection; `connect_ssh` selects it only when configured, preserving existing jump, SOCKS5/HTTP, and direct behavior otherwise.

**Tech Stack:** Rust 2021, Tokio process/IO, russh 0.49, Serde, Slint 1.8, Cargo unit tests.

## Global Constraints

- Preserve the fork-specific contracts in `docs/upstream-local-changes.md`, especially Session serialization and connection lifecycle cleanup.
- Keep the existing routing order: jump host, configured ProxyCommand, SOCKS5/HTTP proxy, direct TCP.
- Never invoke a system shell for ProxyCommand; shell operators and variable expansion are out of scope.
- Empty, whitespace-only, or case-insensitive `ProxyCommand none` must not start a process.
- Support exactly `%h`, `%p`, `%r`, and `%%`; reject any other percent placeholder.
- Do not log the complete command string because command arguments may contain credentials or tokens.
- Use TDD: each production behavior starts with a failing unit test and is implemented minimally.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --locked` in that order before completion.

---

### Task 1: Add the command-line parser and placeholder expansion

**Files:**
- Modify: `src/ssh/impls/proxy.rs`
- Test: `src/ssh/impls/proxy.rs` in the existing/new `#[cfg(test)]` module

**Interfaces:**
- Produces `fn parse_command(command: &str) -> Result<Vec<String>>` for quote-aware argv parsing.
- Produces `fn expand_command(command: &str, host: &str, port: u16, user: &str) -> Result<Vec<String>>` for parsing plus `%h`, `%p`, `%r`, and `%%` expansion.

- [ ] **Step 1: Write the failing parser tests**

Add tests that describe the complete pure behavior before adding the functions:

```rust
#[test]
fn parses_proxy_command_arguments_and_expands_supported_placeholders() {
    let args = expand_command(
        r#"cloudflared access ssh --hostname %h --port "%p" --user '%r' --label %%"#,
        "ssh.capkin.cn",
        22,
        "capkin",
    )
    .unwrap();
    assert_eq!(
        args,
        vec![
            "cloudflared",
            "access",
            "ssh",
            "--hostname",
            "ssh.capkin.cn",
            "--port",
            "22",
            "--user",
            "capkin",
            "--label",
            "%",
        ]
    );
}

#[test]
fn rejects_malformed_proxy_commands_and_unknown_placeholders() {
    assert!(expand_command("cloudflared 'access", "host", 22, "user").is_err());
    assert!(expand_command("cloudflared access\\", "host", 22, "user").is_err());
    assert!(expand_command("cloudflared --host %x", "host", 22, "user").is_err());
}

#[test]
fn empty_or_none_proxy_commands_are_disabled() {
    assert!(expand_command("   ", "host", 22, "user").unwrap().is_empty());
    assert!(expand_command("none", "host", 22, "user").unwrap().is_empty());
    assert!(expand_command("NONE", "host", 22, "user").unwrap().is_empty());
}
```

- [ ] **Step 2: Run the focused tests and verify the expected failure**

Run: `cargo test --locked ssh::proxy::tests::parses_proxy_command_arguments_and_expands_supported_placeholders`

Expected: FAIL because `expand_command` and the parser behavior do not yet exist.

- [ ] **Step 3: Implement the minimal parser and expansion**

Implement a small state machine in `src/ssh/impls/proxy.rs`:

```rust
fn parse_command(command: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if escaped {
        bail!("ProxyCommand ends with an escape");
    }
    if quote.is_some() {
        bail!("ProxyCommand has an unterminated quote");
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}
```

Expand placeholders on each parsed argument, return an empty vector for blank
or `none`, and return an error for every other `%` sequence. Keep the existing
`anyhow` result style and do not log the raw command.

- [ ] **Step 4: Run all parser tests and verify they pass**

Run: `cargo test --locked ssh::proxy::tests`

Expected: PASS with no warnings.

- [ ] **Step 5: Commit the parser behavior**

```bash
git add src/ssh/impls/proxy.rs
git commit -m "feat(ssh): parse ProxyCommand arguments"
```

### Task 2: Add the command-backed async stream

**Files:**
- Modify: `src/ssh/impls/proxy.rs`
- Test: `src/ssh/impls/proxy.rs` command-stream tests

**Interfaces:**
- Produces `pub async fn connect_command(command: &str, host: &str, port: u16, user: &str) -> Result<ProxyCommandStream>`.
- Produces `ProxyCommandStream` implementing `tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send`.

- [ ] **Step 1: Write the failing process-stream tests**

Add a platform-specific echo command helper and an async round-trip test. Use
`cat` on Unix and `cmd /C more` on Windows so the test does not need a network
service or an installed third-party executable:

```rust
#[cfg(windows)]
fn echo_proxy_command() -> &'static str {
    "cmd /C more"
}

#[cfg(not(windows))]
fn echo_proxy_command() -> &'static str {
    "cat"
}

#[tokio::test]
async fn command_stream_round_trips_bytes() {
    let mut stream = connect_command(echo_proxy_command(), "host", 22, "user")
        .await
        .unwrap();
    stream.write_all(b"proxy-payload").await.unwrap();
    stream.shutdown().await.unwrap();

    let mut received = Vec::new();
    stream.read_to_end(&mut received).await.unwrap();
    assert_eq!(received, b"proxy-payload");
}

#[tokio::test]
async fn command_stream_reports_missing_executable() {
    let error = match connect_command(
        "meatshell-command-that-does-not-exist",
        "host",
        22,
        "user",
    )
    .await
    {
        Ok(_) => panic!("missing ProxyCommand executable unexpectedly started"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("ProxyCommand"));
}
```

- [ ] **Step 2: Run the focused stream test and verify it fails for the missing interface**

Run: `cargo test --locked ssh::proxy::tests::command_stream_round_trips_bytes`

Expected: FAIL because `connect_command` and `ProxyCommandStream` do not yet exist.

- [ ] **Step 3: Implement process spawning and stream ownership**

Use `tokio::process::Command` with `Stdio::piped()` for stdin/stdout/stderr and
`kill_on_drop(true)`. Parse and expand through Task 1, reject an empty argv,
spawn the executable with `.args(&argv[1..])`, take all three pipes, and spawn a
bounded stderr-draining task that logs only diagnostic text without the command
line. Define a wrapper containing `Child`, `ChildStdout`, and `ChildStdin`; its
`AsyncRead` implementation delegates to stdout and its `AsyncWrite`
implementation delegates to stdin. In `Drop`, terminate and asynchronously reap
the child when a Tokio runtime is available.

Return errors with the executable name only, for example:

```rust
let mut child = Command::new(&argv[0])
    .args(&argv[1..])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .with_context(|| format!("failed to start ProxyCommand executable {}", argv[0]))?;
```

- [ ] **Step 4: Run the stream tests and verify they pass**

Run: `cargo test --locked ssh::proxy::tests::command_stream`

Expected: PASS on the current platform with no warnings.

- [ ] **Step 5: Commit the command stream**

```bash
git add src/ssh/impls/proxy.rs
git commit -m "feat(ssh): add ProxyCommand process stream"
```

### Task 3: Persist and import ProxyCommand

**Files:**
- Modify: `src/config/struct/session.rs`
- Modify: `src/ssh/struct/connection.rs`
- Modify: `src/ssh/impls/ssh_config.rs`
- Modify: `src/app.rs` SSH config import callback
- Test: `src/ssh/impls/ssh_config.rs`
- Test: `src/config/impls/config.rs`

**Interfaces:**
- `Session.proxy_command: String` is serde-defaulted and initialized empty by `Session::new_empty`.
- `ImportedHost.proxy_command: String` carries the parsed SSH config directive.

- [ ] **Step 1: Add failing import and compatibility tests**

Extend the SSH config parser test fixture with:

```ssh
Host cloud
    HostName ssh.capkin.cn
    User capkin
    ProxyCommand cloudflared access ssh --hostname %h

Host no-proxy
    HostName no-proxy.example.com
    User capkin

Host proxy-none
    HostName none.example.com
    ProxyCommand none
```

Assert the first imported host has the exact command, while the other two have
an empty command. Add a config test that removes `proxy_command` from a serialized
session object and deserializes it successfully with an empty field:

```rust
#[test]
fn legacy_session_without_proxy_command_defaults_to_empty() {
    let mut value = serde_json::to_value(Session::new_empty()).unwrap();
    value.as_object_mut().unwrap().remove("proxy_command");
    let session: Session = serde_json::from_value(value).unwrap();
    assert!(session.proxy_command.is_empty());
}
```

- [ ] **Step 2: Run the tests and verify they fail because the field/directive is missing**

Run: `cargo test --locked ssh::ssh_config::tests::parses_basic_blocks`

Expected: FAIL because `ImportedHost` and `Session` do not yet expose
`proxy_command` and the parser ignores `ProxyCommand`.

- [ ] **Step 3: Add the serde fields and parser mapping**

Add to `Session`:

```rust
#[serde(default)]
pub proxy_command: String,
```

Initialize it with `String::new()` in `Session::new_empty`. Add the same field to
`ImportedHost`, initialize it when a concrete `Host` starts, and handle the
lower-cased `proxycommand` key. Store `String::new()` for `none` (case
insensitive), otherwise preserve the parsed value. In `src/app.rs`'s
`on_import_ssh_config` callback, assign `proxy_command: h.proxy_command` in the
new `Session` literal.

- [ ] **Step 4: Run import and serde tests and verify they pass**

Run: `cargo test --locked ssh::ssh_config::tests` and then `cargo test --locked legacy_session_without_proxy_command_defaults_to_empty`

Expected: PASS with no warnings.

- [ ] **Step 5: Commit persistence and import**

```bash
git add src/config/struct/session.rs src/ssh/struct/connection.rs src/ssh/impls/ssh_config.rs src/app.rs src/config/impls/config.rs
git commit -m "feat(ssh): import and persist ProxyCommand"
```

### Task 4: Wire the session editor and draft mapping

**Files:**
- Modify: `ui/session_dialog.slint`
- Modify: `src/app/session_models.rs`
- Modify: `src/app.rs` new/edit dialog setup
- Test: `src/app/session_models.rs` draft mapping test if the generated draft type permits construction without UI runtime

**Interfaces:**
- `SessionDraft.proxy_command: string` carries the optional command through `submit` and `test-connection`.
- `session_from_draft` copies `draft.proxy_command` into `Session.proxy_command`.

- [ ] **Step 1: Add the failing Rust draft-mapping test**

Construct a `SessionDraft` with the existing required fields and
`proxy_command: "cloudflared access ssh --hostname %h"`, pass it to
`session_from_draft`, and assert the resulting `Session.proxy_command` matches.
Also assert a blank draft produces an empty field. Run the test before changing
the generated Slint struct so the expected failure identifies the missing
property.

- [ ] **Step 2: Run the draft test and verify the expected failure**

Run: `cargo test --locked draft_proxy_command_is_mapped`

Expected: FAIL to compile because `SessionDraft` has no `proxy_command` member.

- [ ] **Step 3: Add the Slint property and both callback payload fields**

In `ui/session_dialog.slint`:

```slint
// Optional local transport command; blank disables it.
proxy-command: string,
```

Add `draft-proxy-command` beside the existing proxy draft properties. In the
advanced SSH-only section add a `LabeledInput` bound to it, with a
`cloudflared access ssh --hostname %h` placeholder and a short warning that the
command runs locally. Add `proxy-command: root.draft-proxy-command` to both the
test and submit `SessionDraft` object literals.

- [ ] **Step 4: Wire new/edit/reset/import values in Rust**

Set the property to empty in the new-session callback, set it from
`session.proxy_command` in the edit callback, and copy
`draft.proxy_command.to_string()` in `session_from_draft`. The existing test
connection callback already calls `session_from_draft`, so it will receive the
same value after the field is added.

- [ ] **Step 5: Run the UI compile and mapping tests**

Run: `cargo test --locked draft_proxy_command_is_mapped`

Expected: PASS. Then run `cargo check` to compile the Slint property/callback
bindings and expected generated Rust accessors.

- [ ] **Step 6: Commit editor support**

```bash
git add ui/session_dialog.slint src/app/session_models.rs src/app.rs
git commit -m "feat(ui): expose ProxyCommand in SSH sessions"
```

### Task 5: Route configured commands through russh

**Files:**
- Modify: `src/ssh/impls/ssh.rs` inside `connect_ssh`
- Test: `src/ssh/impls/proxy.rs` optional-command behavior tests

**Interfaces:**
- `connect_ssh` calls `crate::ssh::proxy::connect_command(&session.proxy_command, &session.host, session.port, &session.user)` only when the command is enabled.

- [ ] **Step 1: Add a failing routing-selection test**

Add a pure proxy test for the enabled predicate used by the connection branch:

```rust
#[test]
fn proxy_command_is_enabled_only_for_nonempty_non_none_values() {
    assert!(!proxy_command_enabled(""));
    assert!(!proxy_command_enabled("  none  "));
    assert!(proxy_command_enabled("cloudflared access ssh --hostname %h"));
}
```

- [ ] **Step 2: Run the predicate test and verify it fails**

Run: `cargo test --locked ssh::proxy::tests::proxy_command_is_enabled_only_for_nonempty_non_none_values`

Expected: FAIL because the predicate does not yet exist.

- [ ] **Step 3: Add the predicate and connect branch**

Implement `pub(crate) fn proxy_command_enabled` as a trim plus case-insensitive
`none` check. Call it from `ssh.rs` through
`crate::ssh::proxy::proxy_command_enabled`.
In `connect_ssh`, keep the existing jump branch first. Before resolving the URL
proxy, add the command branch:

```rust
if crate::ssh::proxy::proxy_command_enabled(&session.proxy_command) {
    let _ = events.send(SessionEvent::Status(format!(
        "{} {}",
        t("通过 ProxyCommand 连接", "via ProxyCommand"),
        session.host
    )));
    let stream = crate::ssh::proxy::connect_command(
        &session.proxy_command,
        &session.host,
        session.port,
        &session.user,
    )
    .await
    .with_context(|| format!("ProxyCommand connect to {} failed", addr))?;
    return Ok((
        client::connect_stream(config, stream, handler)
            .await
            .with_context(|| format!("connect {} via ProxyCommand failed", addr))?,
        None,
    ));
}
```

Do not alter the existing jump or SOCKS5/HTTP branches. Because terminal,
SFTP, test authentication, and automation all call `connect_ssh`, this makes
the behavior shared without duplicating transport logic.

- [ ] **Step 4: Run routing and full SSH unit tests**

Run: `cargo test --locked ssh::proxy::tests::proxy_command_is_enabled_only_for_nonempty_non_none_values`

Expected: PASS. Then run `cargo test --locked ssh` and confirm all SSH tests pass.

- [ ] **Step 5: Commit the connection routing**

```bash
git add src/ssh/impls/proxy.rs src/ssh/impls/ssh.rs
git commit -m "feat(ssh): route connections through ProxyCommand"
```

### Task 6: Run repository verification

**Files:**
- Verify: `docs/upstream-local-changes.md`
- Verify: all modified Rust and Slint files from Tasks 1-5

- [ ] **Step 1: Check the final diff for accidental changes**

Run: `git diff --check; git status --short; git diff --stat`

Expected: no unexpected modified files; the pre-existing `.superpowers/`
untracked directory remains untouched.

- [ ] **Step 2: Run formatting in the CI order**

Run: `cargo fmt --all -- --check`

Expected: PASS with no formatting diff.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: PASS with zero warnings.

- [ ] **Step 4: Run the locked unit test suite**

Run: `cargo test --locked`

Expected: PASS with all existing and new tests green.

- [ ] **Step 5: Run the release build**

Run: `cargo build --release --locked`

Expected: PASS. Report separately if the environment lacks native GUI build
prerequisites or an installed `cloudflared`; those are manual-environment
limitations, not test failures.

- [ ] **Step 6: Commit any verification-only formatting fix if required**

If formatting changed after implementation, run `cargo fmt --all`, review the
diff, and commit only the affected feature files:

```bash
git add src/ssh/impls/proxy.rs src/ssh/impls/ssh.rs src/ssh/impls/ssh_config.rs src/ssh/struct/connection.rs src/config/struct/session.rs src/config/impls/config.rs src/app.rs src/app/session_models.rs ui/session_dialog.slint
git commit -m "style: format ProxyCommand support"
```

Do not stage `.superpowers/`.
