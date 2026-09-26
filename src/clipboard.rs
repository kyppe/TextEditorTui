//! The yank register, bridged to the system clipboard.
//!
//! Yank/delete fills both this register and the system clipboard, and paste
//! reads the system clipboard back — so copying in a browser and pasting
//! here (or the reverse) just works, the way Vim behaves with
//! `clipboard=unnamedplus`.
//!
//! The bridge shells out to whichever clipboard tool is installed rather
//! than linking a clipboard crate: no new dependency, no X11/Wayland build
//! complications, and on a Wayland desktop `wl-copy` is already there. If
//! none of the tools exist, everything still works — yank and paste just
//! stay internal to the app.

use std::io::Write;
use std::process::{Command, Stdio};

/// Yanked text plus whether it was taken as whole lines (`yy`, `dd`) or as
/// a character range (`y` over a selection, `x`). Vim's linewise flag: it
/// decides whether `p` opens a new line or inserts inline.
#[derive(Debug, Clone, Default)]
pub struct Register {
    pub text: String,
    pub linewise: bool,
}

/// Copy tools, tried in order: `(program, args)` — each reads stdin.
const COPY: &[(&str, &[&str])] = &[
    ("wl-copy", &[]),
    ("xclip", &["-selection", "clipboard"]),
    ("xsel", &["--clipboard", "--input"]),
    ("pbcopy", &[]),
];

/// Paste tools, tried in order — each writes the clipboard to stdout.
const PASTE: &[(&str, &[&str])] = &[
    ("wl-paste", &["--no-newline"]),
    ("xclip", &["-selection", "clipboard", "-o"]),
    ("xsel", &["--clipboard", "--output"]),
    ("pbpaste", &[]),
];

/// Pushes `text` to the system clipboard. Best-effort: a missing tool or a
/// failing one is not an error the user needs to hear about, since the
/// in-app register still holds the text.
pub fn set_system(text: &str) {
    for (program, args) in COPY {
        let child = Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { continue };
        if let Some(stdin) = child.stdin.as_mut() {
            if stdin.write_all(text.as_bytes()).is_err() {
                continue;
            }
        }
        // Drop stdin so the tool sees EOF, then reap it.
        drop(child.stdin.take());
        let _ = child.wait();
        return;
    }
}

/// Reads the system clipboard, or `None` if no tool is available.
pub fn get_system() -> Option<String> {
    for (program, args) in PASTE {
        let out = Command::new(program)
            .args(*args)
            .stderr(Stdio::null())
            .output();
        if let Ok(out) = out {
            if out.status.success() {
                return String::from_utf8(out.stdout).ok();
            }
        }
    }
    None
}

/// The register to paste from: the system clipboard when it holds something
/// other than what we last yanked, otherwise our own register (which knows
/// whether the yank was linewise). External text ending in a newline is
/// treated as linewise, which is how other editors copy whole lines.
pub fn resolve_for_paste(own: &Register) -> Register {
    match get_system() {
        Some(sys) if !sys.is_empty() && sys.trim_end_matches('\n') != own.text => {
            let linewise = sys.ends_with('\n');
            Register {
                text: sys.trim_end_matches('\n').to_string(),
                linewise,
            }
        }
        _ => own.clone(),
    }
}

/// Records a yank in `own` and mirrors it to the system clipboard.
pub fn yank(own: &mut Register, text: String, linewise: bool) {
    // Linewise yanks carry a trailing newline out to other apps, so pasting
    // into an editor lands as a whole line rather than joining onto one.
    let external = if linewise {
        format!("{text}\n")
    } else {
        text.clone()
    };
    *own = Register { text, linewise };
    set_system(&external);
}
