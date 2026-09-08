//! Outbound proxy support for SSH / SFTP connections (issue #7).
//!
//! Establishes the TCP stream to the target host **through a proxy**, then the
//! caller hands that stream to `russh::client::connect_stream`.  Both proxy
//! kinds end up as a transparent `TcpStream`:
//!
//! * **SOCKS5** (`socks5://` / `socks5h://`) via `tokio-socks`; after the
//!   handshake we unwrap to the inner `TcpStream`.
//! * **HTTP / HTTPS CONNECT** (`http://` / `https://`): we issue an HTTP
//!   `CONNECT host:port` and reuse the same socket as the tunnel.
//!
//! The proxy is taken from the per-session setting, falling back to the standard
//! `ALL_PROXY` / `all_proxy` environment variable.

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context as TaskContext, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use super::structs::{ProxyConfig, ProxyKind};
use crate::config::Secret;

/// Resolve the proxy for a session: the explicit `session_proxy` string if set,
/// otherwise the `ALL_PROXY` / `all_proxy` environment variable.  Returns `None`
/// for a direct connection.
pub fn resolve(session_proxy: &str) -> Option<ProxyConfig> {
    let s = session_proxy.trim();
    if !s.is_empty() {
        return parse(s);
    }
    for var in ["ALL_PROXY", "all_proxy"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return parse(v.trim());
            }
        }
    }
    None
}

/// Parse a proxy URL: `scheme://[user:pass@]host:port`.
fn parse(url: &str) -> Option<ProxyConfig> {
    let (scheme, rest) = url.split_once("://").unwrap_or(("socks5", url));
    let kind = match scheme.to_ascii_lowercase().as_str() {
        "socks5" | "socks5h" | "socks" => ProxyKind::Socks5,
        "http" | "https" => ProxyKind::Http,
        _ => return None,
    };
    // Optional userinfo before '@'.
    let (auth, hostport) = match rest.rsplit_once('@') {
        Some((userinfo, hp)) => {
            let (u, p) = userinfo.split_once(':').unwrap_or((userinfo, ""));
            (Some((u.to_string(), Secret::new(p))), hp)
        }
        None => (None, rest),
    };
    let hostport = hostport.trim_end_matches('/');
    let (host, port) = hostport.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some(ProxyConfig {
        kind,
        host: host.to_string(),
        port,
        auth,
    })
}

fn parse_command(command: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut token_started = false;

    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            token_started = true;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            token_started = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            token_started = true;
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                token_started = true;
            }
            c if c.is_whitespace() => {
                if token_started {
                    args.push(std::mem::take(&mut current));
                    token_started = false;
                }
            }
            _ => {
                current.push(ch);
                token_started = true;
            }
        }
    }

    if escaped {
        bail!("ProxyCommand ends with an escape");
    }
    if quote.is_some() {
        bail!("ProxyCommand has an unterminated quote");
    }
    if token_started {
        args.push(current);
    }
    Ok(args)
}

pub(crate) fn proxy_command_enabled(command: &str) -> bool {
    let command = command.trim();
    !command.is_empty() && !command.eq_ignore_ascii_case("none")
}

#[cfg(windows)]
fn proxy_command_creation_flags() -> u32 {
    // CREATE_NO_WINDOW keeps console-based ProxyCommand executables hidden.
    0x0800_0000
}

fn configure_proxy_command(command: &mut Command) {
    #[cfg(windows)]
    command.creation_flags(proxy_command_creation_flags());
}

fn expand_command(command: &str, host: &str, port: u16, user: &str) -> Result<Vec<String>> {
    let command = command.trim();
    if !proxy_command_enabled(command) {
        return Ok(Vec::new());
    }

    parse_command(command)?
        .into_iter()
        .map(|arg| {
            let mut expanded = String::with_capacity(arg.len());
            let mut chars = arg.chars();
            while let Some(ch) = chars.next() {
                if ch != '%' {
                    expanded.push(ch);
                    continue;
                }
                let Some(next) = chars.next() else {
                    bail!("ProxyCommand has a trailing '%' placeholder");
                };
                match next {
                    '%' => expanded.push('%'),
                    'h' => expanded.push_str(host),
                    'p' => expanded.push_str(&port.to_string()),
                    'r' => expanded.push_str(user),
                    other => bail!("ProxyCommand contains unsupported placeholder '%{other}'"),
                }
            }
            Ok(expanded)
        })
        .collect()
}

pub(crate) struct ProxyCommandStream {
    child: Option<Child>,
    stdout: ChildStdout,
    stdin: Option<ChildStdin>,
}

impl AsyncRead for ProxyCommandStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ProxyCommandStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let Some(stdin) = self.get_mut().stdin.as_mut() else {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "ProxyCommand stdin is closed",
            )));
        };
        Pin::new(stdin).poll_write(cx, data)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        let Some(stdin) = self.get_mut().stdin.as_mut() else {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "ProxyCommand stdin is closed",
            )));
        };
        Pin::new(stdin).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        self.get_mut().stdin.take();
        Poll::Ready(Ok(()))
    }
}

impl Drop for ProxyCommandStream {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.start_kill();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = child.wait().await;
            });
        }
    }
}

pub(crate) async fn connect_command(
    command: &str,
    host: &str,
    port: u16,
    user: &str,
) -> Result<ProxyCommandStream> {
    let argv = expand_command(command, host, port, user)?;
    let Some(program) = argv.first() else {
        bail!("ProxyCommand is empty");
    };
    let mut command = Command::new(program);
    configure_proxy_command(&mut command);
    let mut child = command
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to start ProxyCommand executable {program}"))?;
    let stdin = child
        .stdin
        .take()
        .context("ProxyCommand stdin pipe is unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("ProxyCommand stdout pipe is unavailable")?;
    let mut stderr = child
        .stderr
        .take()
        .context("ProxyCommand stderr pipe is unavailable")?;
    tokio::spawn(async move {
        let mut buffer = [0u8; 2048];
        while stderr.read(&mut buffer).await.ok().is_some_and(|n| n > 0) {}
    });

    Ok(ProxyCommandStream {
        child: Some(child),
        stdout,
        stdin: Some(stdin),
    })
}

/// Human-readable description of where we're connecting (for status messages).
pub fn describe(cfg: &ProxyConfig) -> String {
    let scheme = match cfg.kind {
        ProxyKind::Socks5 => "socks5",
        ProxyKind::Http => "http",
    };
    format!("{}://{}:{}", scheme, cfg.host, cfg.port)
}

/// Open a TCP stream to `target_host:target_port` through the proxy.
pub async fn connect(cfg: &ProxyConfig, target_host: &str, target_port: u16) -> Result<TcpStream> {
    match cfg.kind {
        ProxyKind::Socks5 => connect_socks5(cfg, target_host, target_port).await,
        ProxyKind::Http => connect_http(cfg, target_host, target_port).await,
    }
}

async fn connect_socks5(cfg: &ProxyConfig, host: &str, port: u16) -> Result<TcpStream> {
    use tokio_socks::tcp::Socks5Stream;
    let proxy = (cfg.host.as_str(), cfg.port);
    let target = (host, port);
    let stream = match &cfg.auth {
        Some((u, p)) => Socks5Stream::connect_with_password(proxy, target, u, p.as_str())
            .await
            .context("SOCKS5 proxy connect failed")?,
        None => Socks5Stream::connect(proxy, target)
            .await
            .context("SOCKS5 proxy connect failed")?,
    };
    // After the handshake the underlying socket is a transparent tunnel.
    Ok(stream.into_inner())
}

async fn connect_http(cfg: &ProxyConfig, host: &str, port: u16) -> Result<TcpStream> {
    let mut s = TcpStream::connect((cfg.host.as_str(), cfg.port))
        .await
        .with_context(|| format!("connect to HTTP proxy {}:{} failed", cfg.host, cfg.port))?;

    let mut req = format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n");
    if let Some((u, p)) = &cfg.auth {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{u}:{}", p.as_str()));
        req.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    req.push_str("Proxy-Connection: keep-alive\r\n\r\n");
    s.write_all(req.as_bytes())
        .await
        .context("write CONNECT to proxy")?;

    // Read response headers up to the blank line, bounded.
    let mut buf = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        let n = s.read(&mut byte).await.context("read proxy response")?;
        if n == 0 {
            bail!("proxy closed the connection during CONNECT");
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            bail!("proxy CONNECT response too large");
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let status_line = head.lines().next().unwrap_or("");
    // Expect "HTTP/1.x 200 ...".
    let ok = status_line
        .split_whitespace()
        .nth(1)
        .map(|c| c == "200")
        .unwrap_or(false);
    if !ok {
        return Err(anyhow!("proxy CONNECT rejected: {}", status_line.trim()));
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        assert!(expand_command("   ", "host", 22, "user")
            .unwrap()
            .is_empty());
        assert!(expand_command("none", "host", 22, "user")
            .unwrap()
            .is_empty());
        assert!(expand_command("NONE", "host", 22, "user")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn proxy_command_is_enabled_only_for_nonempty_non_none_values() {
        assert!(!proxy_command_enabled(""));
        assert!(!proxy_command_enabled("  none  "));
        assert!(proxy_command_enabled(
            "cloudflared access ssh --hostname %h"
        ));
    }

    #[cfg(windows)]
    #[test]
    fn proxy_command_creation_flags_disable_console_window() {
        assert_eq!(proxy_command_creation_flags(), 0x0800_0000);
    }

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
        let expected = if cfg!(windows) {
            b"proxy-payload\r\n".as_slice()
        } else {
            b"proxy-payload".as_slice()
        };
        assert_eq!(received, expected);
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
}
