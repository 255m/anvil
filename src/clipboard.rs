//! System clipboard access, so `p` pastes what you copied in other programs
//! and `y`/`d` put text where other programs can paste it (like Neovim with
//! `clipboard=unnamedplus`).
//!
//! A terminal program can't talk to the clipboard directly, so this runs the
//! platform's clipboard tool: `wl-copy`/`wl-paste` (Wayland), `xclip` or `xsel`
//! (X11), `pbcopy`/`pbpaste` (macOS), `termux-clipboard-*` (Termux) and
//! PowerShell/`clip.exe` (WSL). If none is installed, callers fall back to
//! Anvil's own register (and OSC 52 for copying).

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

/// How long to wait for a clipboard tool before giving up (a stuck tool must
/// never freeze the editor).
const TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backend {
    pub name: &'static str,
    read: &'static [&'static str],
    write: &'static [&'static str],
    /// PowerShell ends its output with a line break that isn't part of the text.
    strip_trailing_newline: bool,
}

const WL: Backend = Backend {
    name: "wl-clipboard",
    read: &["wl-paste", "--no-newline"],
    write: &["wl-copy"],
    strip_trailing_newline: false,
};
const XCLIP: Backend = Backend {
    name: "xclip",
    read: &["xclip", "-selection", "clipboard", "-o"],
    write: &["xclip", "-selection", "clipboard", "-i"],
    strip_trailing_newline: false,
};
const XSEL: Backend = Backend {
    name: "xsel",
    read: &["xsel", "--clipboard", "--output"],
    write: &["xsel", "--clipboard", "--input"],
    strip_trailing_newline: false,
};
const MACOS: Backend = Backend {
    name: "pbcopy",
    read: &["pbpaste"],
    write: &["pbcopy"],
    strip_trailing_newline: false,
};
const TERMUX: Backend = Backend {
    name: "termux",
    read: &["termux-clipboard-get"],
    write: &["termux-clipboard-set"],
    strip_trailing_newline: false,
};
const WSL: Backend = Backend {
    name: "powershell",
    read: &["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "Get-Clipboard"],
    write: &["clip.exe"],
    strip_trailing_newline: true,
};

/// What the machine looks like, for choosing a tool.
pub struct Env<'a> {
    pub wayland: bool,
    pub x11: bool,
    pub macos: bool,
    pub wsl: bool,
    pub termux: bool,
    /// Is this program on PATH?
    pub have: &'a dyn Fn(&str) -> bool,
}

/// Pick the first clipboard tool that fits this session and is installed.
pub fn pick(env: &Env) -> Option<Backend> {
    let usable = |b: &Backend| (env.have)(b.read[0]) && (env.have)(b.write[0]);
    let mut candidates: Vec<Backend> = Vec::new();
    if env.termux { candidates.push(TERMUX); }
    if env.macos { candidates.push(MACOS); }
    if env.wayland { candidates.push(WL); }
    if env.x11 || env.wayland { candidates.push(XCLIP); candidates.push(XSEL); }
    if env.wsl { candidates.push(WSL); }
    candidates.into_iter().find(|b| usable(b))
}

fn is_exec(p: &Path) -> bool {
    match std::fs::metadata(p) {
        Ok(m) if m.is_file() => {
            #[cfg(unix)]
            { use std::os::unix::fs::PermissionsExt; m.permissions().mode() & 0o111 != 0 }
            #[cfg(not(unix))]
            { true }
        }
        _ => false,
    }
}

fn on_path(prog: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| is_exec(&d.join(prog))))
        .unwrap_or(false)
}

fn is_wsl() -> bool {
    std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::fs::read_to_string("/proc/version")
            .map(|v| v.to_lowercase().contains("microsoft"))
            .unwrap_or(false)
}

fn backend() -> Option<&'static Backend> {
    static B: OnceLock<Option<Backend>> = OnceLock::new();
    B.get_or_init(|| {
        let env = Env {
            wayland: std::env::var_os("WAYLAND_DISPLAY").is_some(),
            x11: std::env::var_os("DISPLAY").is_some(),
            macos: cfg!(target_os = "macos"),
            wsl: is_wsl(),
            termux: std::env::var_os("TERMUX_VERSION").is_some(),
            have: &on_path,
        };
        pick(&env)
    }).as_ref()
}

/// Is there a system clipboard tool for this session?
pub fn available() -> bool { backend().is_some() }

/// Name of the tool in use (for messages).
pub fn tool_name() -> Option<&'static str> { backend().map(|b| b.name) }

/// Text on the system clipboard. `None` if there is no tool, the clipboard is
/// empty or holds something that isn't text, or the tool timed out.
pub fn read() -> Option<String> {
    let b = backend()?;
    run_read(b.read, b.strip_trailing_newline)
}

/// Put `text` on the system clipboard. Returns false if that didn't work.
pub fn write(text: &str) -> bool {
    match backend() {
        Some(b) => run_write(b.write, text),
        None => false,
    }
}

/// `\r\n` -> `\n`, and optionally drop one trailing line break.
fn normalize(raw: &[u8], strip_trailing_newline: bool) -> String {
    let mut s = String::from_utf8_lossy(raw).replace("\r\n", "\n");
    if strip_trailing_newline && s.ends_with('\n') { s.pop(); }
    s
}

fn run_read(cmd: &[&str], strip: bool) -> Option<String> {
    let mut child = Command::new(cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    match rx.recv_timeout(TIMEOUT) {
        Ok(buf) => {
            let ok = child.wait().map(|s| s.success()).unwrap_or(false);
            if ok { Some(normalize(&buf, strip)) } else { None }
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

fn run_write(cmd: &[&str], text: &str) -> bool {
    let mut child = match Command::new(cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() { let _ = child.kill(); let _ = child.wait(); return false; }
        // dropping stdin closes the pipe: the tool now has the whole text
    }
    // xclip, xsel and wl-copy exit once they own the selection, so waiting here
    // means a `p` right after a `y` reads the new text, not the old one
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(2)),
            Ok(None) => {
                // still serving the selection: that is fine; reap it in the background
                thread::spawn(move || { let _ = child.wait(); });
                return true;
            }
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn env(have: &[&'static str], wayland: bool, x11: bool, macos: bool, wsl: bool, termux: bool) -> Option<Backend> {
        let set: HashSet<&str> = have.iter().copied().collect();
        let h = move |p: &str| set.contains(p);
        pick(&Env { wayland, x11, macos, wsl, termux, have: &h })
    }

    #[test]
    fn picks_the_tool_for_the_session() {
        assert_eq!(env(&["wl-copy", "wl-paste"], true, false, false, false, false), Some(WL));
        assert_eq!(env(&["xclip"], false, true, false, false, false), Some(XCLIP));
        assert_eq!(env(&["xsel"], false, true, false, false, false), Some(XSEL));
        assert_eq!(env(&["pbcopy", "pbpaste"], false, false, true, false, false), Some(MACOS));
        assert_eq!(env(&["termux-clipboard-get", "termux-clipboard-set"], false, false, false, false, true), Some(TERMUX));
        assert_eq!(env(&["powershell.exe", "clip.exe"], false, false, false, true, false), Some(WSL));
    }

    #[test]
    fn prefers_xclip_over_xsel_and_wayland_over_x11() {
        assert_eq!(env(&["xclip", "xsel"], false, true, false, false, false), Some(XCLIP));
        assert_eq!(env(&["wl-copy", "wl-paste", "xclip"], true, true, false, false, false), Some(WL));
    }

    #[test]
    fn falls_back_when_the_preferred_tool_is_missing() {
        // Wayland session without wl-clipboard but with xclip (XWayland)
        assert_eq!(env(&["xclip"], true, true, false, false, false), Some(XCLIP));
        // WSLg: no Linux clipboard tools, so use Windows' own
        assert_eq!(env(&["powershell.exe", "clip.exe"], true, true, false, true, false), Some(WSL));
    }

    #[test]
    fn no_tool_means_none() {
        assert_eq!(env(&[], true, true, false, false, false), None);
        // a tool is useless without a display to talk to
        assert_eq!(env(&["xclip"], false, false, false, false, false), None);
        // both halves of a pair must exist
        assert_eq!(env(&["wl-paste"], true, false, false, false, false), None);
    }

    #[test]
    fn normalizes_line_endings() {
        assert_eq!(normalize(b"a\r\nb\r\n", false), "a\nb\n");
        assert_eq!(normalize(b"command\r\n", true), "command");
        assert_eq!(normalize(b"a\r\nb\r\n", true), "a\nb");
        assert_eq!(normalize(b"no newline", true), "no newline");
        assert_eq!(normalize("مرحبا\n".as_bytes(), false), "مرحبا\n");
    }

    #[test]
    fn reads_a_tools_output() {
        assert_eq!(run_read(&["printf", "%s", "hello world"], false).as_deref(), Some("hello world"));
        assert_eq!(run_read(&["printf", "%s\\n", "multi\nline"], false).as_deref(), Some("multi\nline\n"));
    }

    #[test]
    fn a_failing_tool_reads_as_nothing() {
        // e.g. wl-paste on an empty clipboard exits with an error
        assert_eq!(run_read(&["sh", "-c", "echo oops; exit 1"], false), None);
        assert_eq!(run_read(&["this-program-does-not-exist-anvil"], false), None);
    }

    #[test]
    fn a_stuck_tool_cannot_freeze_the_editor() {
        let t = Instant::now();
        assert_eq!(run_read(&["sleep", "10"], false), None);
        assert!(t.elapsed() < Duration::from_secs(3), "took {:?}", t.elapsed());
    }

    #[test]
    fn writes_text_to_a_tools_stdin() {
        let f = std::env::temp_dir().join(format!("anvil-clip-test-{}", std::process::id()));
        let script = format!("cat > '{}'", f.display());
        assert!(run_write(&["sh", "-c", &script], "line1\nline2 é"));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "line1\nline2 é");
        let _ = std::fs::remove_file(&f);
        assert!(!run_write(&["sh", "-c", "cat >/dev/null; exit 3"], "x"));
        assert!(!run_write(&["this-program-does-not-exist-anvil"], "x"));
    }

    #[test]
    fn a_tool_that_keeps_running_still_counts_as_written() {
        // wl-copy style: stays alive serving the selection
        let t = Instant::now();
        assert!(run_write(&["sh", "-c", "cat >/dev/null; sleep 2"], "x"));
        assert!(t.elapsed() < Duration::from_secs(2), "should not wait for the tool to exit");
    }
}
