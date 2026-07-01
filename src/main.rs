// Entry point. Wires the Slint UI to the config store, system sampler and
// SSH session manager.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod errlog;
mod forward;
mod i18n;
mod known_hosts;
mod panes;
mod proxy;
mod serial;
mod sftp;
mod ssh;
mod ssh_config;
mod system;
mod telnet;
mod wallpaper;
mod zmodem;

fn main() -> anyhow::Result<()> {
    // macOS renderer is left at Slint's default (femtovg) and is NOT forced.
    //
    // History: 0.4.10 force-set SLINT_BACKEND=winit-skia to work around femtovg's
    // CoreText font lookup failing on macOS 26 / Tahoe (all text vanished, #108).
    // That fix shipped without on-device verification and turned out to *break* a
    // different set of Macs (Apple Silicon M5 / 26.5): Skia couldn't resolve the
    // "PingFang SC" UI font and all text vanished there instead (#129). Icons
    // survived in both cases because Material Icons is an embedded font.
    //
    // Neither renderer works for every macOS machine, so we no longer pick for the
    // user: femtovg is the known-good default for the majority. Users for whom
    // femtovg fails to render text (e.g. #108) can opt into Skia at launch with
    //     SLINT_BACKEND=winit-skia
    // The renderer-skia feature is still compiled in on macOS (see Cargo.toml) so
    // that override is available without a rebuild.

    // Auto-detect remote desktop sessions and fall back to software rendering.
    // Hardware-accelerated renderers (femtovg/skia) often white-screen over
    // remote desktop tools (RDP, Sunflower/向日葵, ToDesk, etc.) because they
    // can't access the GPU's OpenGL/Vulkan context.
    //
    // If the user already set SLINT_BACKEND we respect it unconditionally.
    if std::env::var("SLINT_BACKEND").is_err() && is_remote_desktop() {
        std::env::set_var("SLINT_BACKEND", "software");
    }

    init_tracing();

    // ── IME policy ───────────────────────────────────────────────────────────
    // NOTE: We deliberately DO **NOT** call `ImmDisableIME` here.
    //
    // An earlier version disabled the IME for the whole Slint event-loop thread
    // to work around a vim `:q!` glitch (Chinese IMEs intercept letter keys and,
    // on a Shift press, discard the in-flight pinyin).  But disabling the IME
    // also makes 中文输入 completely impossible — there is no composition window
    // at all, which is exactly the "无法输入任何中文" bug.
    //
    // Chinese input now flows through the hidden `ime-input` TextInput in
    // terminal_view.slint: composition happens there, and committed text is
    // forwarded to the PTY via the `edited` callback.  The vim/Shift side-effects
    // are handled instead by the C0-marker + 3-layer Backspace filters in
    // `app::on_send_key`, so we no longer need (and must not use) ImmDisableIME.

    app::run()
}

/// Detect whether the app is running inside a remote desktop session.
///
/// Checks Windows RDP via `GetSystemMetrics(SM_REMOTESESSION)` and scans for
/// common third-party remote desktop agents (Sunflower/向日葵, ToDesk, etc.)
/// whose OpenGL contexts are unreliable over the network.
#[cfg(windows)]
fn is_remote_desktop() -> bool {
    // 1. Native RDP session (mstsc.exe / Windows Remote Desktop).
    const SM_REMOTESESSION: i32 = 0x1000;
    #[link(name = "user32")]
    extern "system" {
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }
    if unsafe { GetSystemMetrics(SM_REMOTESESSION) } != 0 {
        return true;
    }

    // 2. Third-party remote desktop agents — check if their core processes are
    //    running.  This covers the most common tools in the Chinese market that
    //    white-screen with hardware rendering.
    let remote_agents: &[&str] = &[
        "SunloginClient",  // 向日葵
        "SunloginService", // 向日葵 service
        "ToDesk",          // ToDesk
        "ToDesk_Service",
        "AnyDesk", // AnyDesk
        "AnyDesk_Service",
        "TeamViewer", // TeamViewer
    ];

    // sysinfo is already a dependency; use it for a lightweight process scan.
    use sysinfo::System;
    let sys = System::new_all();
    for proc in sys.processes().values() {
        let name = proc.name().to_string_lossy().to_lowercase();
        for agent in remote_agents {
            if name.contains(&agent.to_lowercase()) {
                return true;
            }
        }
    }

    false
}

#[cfg(not(windows))]
fn is_remote_desktop() -> bool {
    // On Linux/macOS we don't auto-detect; users can set SLINT_BACKEND=software.
    false
}
/// `error.log` file at WARN and above so users can send diagnostics — e.g. a
/// bastion disconnect reason — without setting RUST_LOG (#86).
fn init_tracing() {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    // Third-party noise routed through `log` → tracing: ICU4X data-error warnings
    // (icu_provider dependency) and fontdb's "malformed font" warning for fonts it
    // can't parse but harmlessly skips (e.g. Windows' mstmc.ttf). Silence on every
    // layer; keep fontdb at `error` so genuine failures still surface.
    fn quiet_noise(mut f: EnvFilter) -> EnvFilter {
        for d in [
            "icu_provider=off",
            "icu_segmenter=off",
            "icu_normalizer=off",
            "fontdb=error",
        ] {
            if let Ok(dir) = d.parse() {
                f = f.add_directive(dir);
            }
        }
        f
    }

    let env_filter =
        quiet_noise(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);

    // One file, capped at 5 MiB, auto-overwriting when full.
    let file_layer = errlog::path()
        .and_then(|p| errlog::CappedFile::open(p, 5 * 1024 * 1024).ok())
        .map(|cf| {
            fmt::layer()
                .with_ansi(false)
                .with_writer(errlog::CappedWriter::new(cf))
                .with_filter(quiet_noise(EnvFilter::new("warn")))
        });

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();
}
