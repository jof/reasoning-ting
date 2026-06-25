//! Focus guard — only inject when the right window/app is frontmost, so the
//! daemon can run continuously without firing into the wrong place.
//!
//! - Linux/X11: native via x11rb — read _NET_ACTIVE_WINDOW + _NET_WM_PID, then
//!   check the focused window's process subtree for the target process (claude).
//! - macOS: the frontmost app's name vs an allowlist (terminal / Claude app),
//!   via the always-present `osascript` (a terminal can't cheaply expose its
//!   child pid, so we match the app instead).
//! - Other / fallback: allow.

pub trait FocusGuard {
    fn allowed(&self) -> bool;
}

/// Always allow (for --no-focus-guard).
pub struct NoGuard;
impl FocusGuard for NoGuard {
    fn allowed(&self) -> bool {
        true
    }
}

pub fn make(no_guard: bool, target_proc: String) -> Box<dyn FocusGuard> {
    if no_guard {
        return Box::new(NoGuard);
    }
    #[cfg(target_os = "linux")]
    {
        match linux::X11Guard::new(target_proc.clone()) {
            Ok(g) => return Box::new(g),
            Err(e) => {
                eprintln!("focus guard unavailable ({e}); allowing all");
                return Box::new(NoGuard);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        return Box::new(macos::AppGuard::new(macos::default_allowlist()));
    }
    #[allow(unreachable_code)]
    {
        let _ = target_proc;
        Box::new(NoGuard)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::FocusGuard;
    use anyhow::{anyhow, Result};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};

    pub struct X11Guard {
        conn: x11rb::rust_connection::RustConnection,
        root: Window,
        net_active: u32,
        net_pid: u32,
        target: String,
    }

    impl X11Guard {
        pub fn new(target: String) -> Result<Self> {
            let (conn, screen_num) = x11rb::connect(None)?;
            let root = conn.setup().roots[screen_num].root;
            let net_active = intern(&conn, b"_NET_ACTIVE_WINDOW")?;
            let net_pid = intern(&conn, b"_NET_WM_PID")?;
            Ok(Self {
                conn,
                root,
                net_active,
                net_pid,
                target,
            })
        }
        fn active_window(&self) -> Result<Window> {
            let r = self
                .conn
                .get_property(false, self.root, self.net_active, AtomEnum::WINDOW, 0, 1)?
                .reply()?;
            let w = r
                .value32()
                .and_then(|mut v| v.next())
                .ok_or_else(|| anyhow!("no active window"))?;
            Ok(w)
        }
        fn window_pid(&self, w: Window) -> Result<u32> {
            let r = self
                .conn
                .get_property(false, w, self.net_pid, AtomEnum::CARDINAL, 0, 1)?
                .reply()?;
            r.value32()
                .and_then(|mut v| v.next())
                .ok_or_else(|| anyhow!("no _NET_WM_PID"))
        }
    }

    impl FocusGuard for X11Guard {
        fn allowed(&self) -> bool {
            (|| -> Result<bool> {
                let w = self.active_window()?;
                let pid = self.window_pid(w)?;
                Ok(proc_subtree_contains(pid, &self.target))
            })()
            .unwrap_or(false)
        }
    }

    fn intern(conn: &impl Connection, name: &[u8]) -> Result<u32> {
        Ok(conn.intern_atom(false, name)?.reply()?.atom)
    }

    /// True if `needle` appears in the cmdline of `pid` or any descendant.
    fn proc_subtree_contains(pid: u32, needle: &str) -> bool {
        let mut stack = vec![pid];
        let mut seen = std::collections::HashSet::new();
        while let Some(p) = stack.pop() {
            if !seen.insert(p) {
                continue;
            }
            if let Ok(cmd) = std::fs::read(format!("/proc/{p}/cmdline")) {
                if String::from_utf8_lossy(&cmd).contains(needle) {
                    return true;
                }
            }
            for c in children(p) {
                stack.push(c);
            }
        }
        false
    }

    fn children(pid: u32) -> Vec<u32> {
        // /proc/<pid>/task/<tid>/children is space-separated child pids
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(format!("/proc/{pid}/task")) {
            for tid in rd.flatten() {
                let p = tid.path().join("children");
                if let Ok(s) = std::fs::read_to_string(&p) {
                    out.extend(s.split_whitespace().filter_map(|x| x.parse::<u32>().ok()));
                }
            }
        }
        out
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::FocusGuard;

    pub fn default_allowlist() -> Vec<String> {
        ["Terminal", "iTerm", "Ghostty", "Alacritty", "kitty", "WezTerm", "Claude"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    pub struct AppGuard {
        allow: Vec<String>,
    }
    impl AppGuard {
        pub fn new(allow: Vec<String>) -> Self {
            Self { allow }
        }
    }
    impl FocusGuard for AppGuard {
        fn allowed(&self) -> bool {
            // frontmost app name via the always-present osascript
            let out = std::process::Command::new("osascript")
                .args([
                    "-e",
                    "tell application \"System Events\" to get name of first application process whose frontmost is true",
                ])
                .output();
            match out {
                Ok(o) => {
                    let name = String::from_utf8_lossy(&o.stdout);
                    self.allow.iter().any(|a| name.contains(a))
                }
                Err(_) => false,
            }
        }
    }
}
