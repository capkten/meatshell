# SSH ProxyCommand Support Design

**Date:** 2026-09-02

**Status:** Approved for implementation

## Goal

Support optional OpenSSH `ProxyCommand` entries such as
`cloudflared access ssh --hostname %h`, both when importing `~/.ssh/config` and
when editing a saved SSH session manually. The command must participate in the
existing terminal, SFTP, connection-test, and automation connection paths.

## Scope

- Read `ProxyCommand` from concrete `Host` blocks during SSH config import.
- Persist the command on the saved session with backward-compatible serde
  defaults.
- Expose an optional SSH-only command field in the advanced session editor.
- Start the command as a local process and use its stdin/stdout as the SSH
  transport stream.
- Support command arguments, quote parsing, backslash escapes, and the common
  `%h`, `%p`, `%r`, and `%%` substitutions.
- Keep the existing behavior exactly when the command is absent, blank, or
  `none`.

The command is intentionally started without a system shell. Shell operators,
redirection, and shell variable expansion are outside this feature's scope.

## Architecture

`Session` gains an optional `proxy_command: String` field. The SSH config
importer copies the `ProxyCommand` directive into `ImportedHost`, and the app
maps it into the new session field. The Slint `SessionDraft` carries the same
value between the editor and Rust callbacks.

The existing `connect_ssh` function remains the single transport entry point.
Its routing order is:

1. Existing jump-host connection, if configured.
2. `Session.proxy_command`, if non-empty after trimming.
3. Existing SOCKS5 or HTTP proxy, if configured.
4. Direct TCP connection.

The command-backed stream lives beside the existing SOCKS5 and HTTP proxy
code. It owns the child process and its piped stdin/stdout, implements the
async read/write traits expected by `russh::client::connect_stream`, and is
dropped with the SSH connection. The child's stderr is consumed separately so
diagnostic output cannot block the SSH stream.

## Configuration Behavior

An imported block such as:

```ssh
Host ssh.capkin.cn
    HostName ssh.capkin.cn
    User capkin
    ProxyCommand cloudflared access ssh --hostname %h
```

produces a session named `ssh.capkin.cn`, with host `ssh.capkin.cn`, user
`capkin`, the existing password authentication default, and
`proxy_command` set to `cloudflared access ssh --hostname %h`.

If the directive is absent, its value is empty. `ProxyCommand none` is also
treated as empty. A blank command never starts a process and falls through to
the old jump/proxy/direct behavior.

The advanced SSH session editor shows a single-line optional ProxyCommand field.
Editing an existing session loads the persisted value. Clearing it disables the
command and allows an existing SOCKS5/HTTP proxy value to work again. If both a
command and an existing URL proxy are present, the command wins. Jump-host
routing retains its current precedence; a target command is ignored while that
target is connected through a jump session.

The UI warns that a configured command is executed on the local machine. The
command is stored with the session configuration; no new credential encryption
scheme is introduced by this feature.

## Command Parsing

The parser converts a command line into argv without invoking a shell:

- Unquoted whitespace separates arguments.
- Single and double quotes group text and are removed.
- Backslash escapes the following character.
- An unterminated quote or trailing backslash is an error.
- An empty or whitespace-only command is treated as disabled before parsing.

Substitutions happen per parsed argument so a substituted host or username
cannot create additional argv entries:

- `%h` becomes the session host.
- `%p` becomes the decimal session port.
- `%r` becomes the session user.
- `%%` becomes a literal `%`.

Any other `%` placeholder is rejected with a descriptive error rather than
being passed through silently.

## Process Lifecycle and Errors

The command process is launched with piped stdin, stdout, and stderr. The SSH
stream wrapper owns the child and enables termination when the wrapper is
dropped. The implementation also waits for the terminated child so failed or
cancelled connections do not leave unreaped processes behind.

The following conditions are reported as connection errors:

- Command line parsing failure.
- Empty argv after parsing.
- Failure to start the executable.
- Missing stdin or stdout pipes.
- Command termination or EOF before a successful SSH handshake.

Error messages may name the executable, but must not log or expose the full
command string because arguments can contain credentials or tokens. Stderr is
drained asynchronously and may be recorded at diagnostic log level without
including the full command line.

Authentication fallback reconnects through a newly spawned command process.
The previous stream and process are dropped before the replacement connection
is used. Terminal close, SFTP close, test completion, automation completion,
and cancellation all use the same cleanup path.

## Compatibility and Safety

- `#[serde(default)]` makes old session JSON readable without migration.
- Empty `proxy_command` preserves direct, SOCKS5, HTTP, jump-host, and existing
  password/key authentication behavior.
- Host config validation remains unchanged; this feature does not turn
  `HostName` into an executable fragment.
- The command is intentionally user-controlled local execution. Avoiding a
  shell limits accidental command interpretation while retaining support for
  executable-plus-argument commands such as `cloudflared` and `nc`.

## Testing and Acceptance

Unit tests cover:

- Importing a `ProxyCommand` with multiple arguments.
- Missing and `none` directives producing an empty value.
- `%h`, `%p`, `%r`, and `%%` substitutions.
- Quoted and escaped arguments.
- Unterminated quotes, trailing escapes, unknown placeholders, and empty argv.
- Reading old session JSON without `proxy_command`.
- Bidirectional command-stream I/O and child cleanup using a platform-available
  test helper.

Manual acceptance verifies that the sample `cloudflared` configuration can
authenticate with the existing username/password flow; that sessions without a
command behave as before; that clearing a command restores an existing URL
proxy; and that terminal, SFTP, connection-test, and automation paths all use
the same command transport.

## Non-Goals

- Full OpenSSH config evaluation, including wildcard precedence and every
  directive.
- Shell pipelines, redirection, shell functions, or shell variable expansion
  inside `ProxyCommand`.
- Replacing the existing SOCKS5/HTTP proxy or jump-host features.
- Delegating the entire SSH connection to the system `ssh` executable.
