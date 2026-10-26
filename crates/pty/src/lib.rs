//! One managed PTY session — the child process, its master, the kill handle,
//! and the screen the reader thread fills.
//!
//! Sessions never live in the buffer slab; the terminal dock renders `screen`
//! directly and the manager owns the vector of sessions.  Tab naming lives
//! here too: a default name is `"{label} {index}"` and is regenerated
//! gaplessly whenever sessions come and go, while an explicit rename makes a
//! tab's name permanently user-owned.

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use portable_pty::{ChildKiller, MasterPty};

use vt::screen::TerminalScreen;

/// One managed PTY session.
pub struct TerminalSession {
    pub id: usize,
    pub title: String,
    /// The generated part of the tab name ("zsh", "sh -c read _"), kept so
    /// renumbering can regenerate `title`; `None` once the user renames the
    /// tab, which makes the name permanently user-owned.
    auto_label: Option<String>,
    pub cwd: PathBuf,
    pub screen: TerminalScreen,
    pub(crate) writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    /// Mutex, not bare: `dyn MasterPty` is Send but not Sync, and `Editor`
    /// must stay Sync (it is shared across threads via `Arc<RwLock<Editor>>`).
    pub(crate) master: Mutex<Box<dyn MasterPty + Send>>,
    /// PTY child pid, for whole-group kills.  The PTY shell is a session
    /// leader, so signaling -pid covers its entire subtree.
    pub(crate) pid: Option<i32>,
    /// Kill handle only: the child itself moved to the waiter thread, which
    /// reaps the real exit code when it exits.
    pub(crate) child_killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl TerminalSession {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: usize,
        title: String,
        auto_label: Option<String>,
        cwd: PathBuf,
        screen: TerminalScreen,
        writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
        master: Mutex<Box<dyn MasterPty + Send>>,
        pid: Option<i32>,
        child_killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    ) -> Self {
        TerminalSession {
            id,
            title,
            auto_label,
            cwd,
            screen,
            writer,
            master,
            pid,
            child_killer,
        }
    }

    pub fn send_input(&self, input: &str) -> Result<(), String> {
        use std::io::Write;
        let mut writer = self.writer.lock().map_err(|e| e.to_string())?;
        writer.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        self.master
            .lock()
            .map_err(|e| e.to_string())?
            .resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    pub fn kill(&mut self) {
        // The PTY shell is a session leader; the group signal takes its
        // whole subtree (vim, make, ...) down with it.
        if let Some(pid) = self.pid {
            util::proctree::kill_tree_hard(pid);
        }
        if let Ok(mut killer) = self.child_killer.lock() {
            let _ = killer.kill();
        }
    }

    /// Rename the tab.  This makes the name user-owned, so renumbering leaves
    /// it alone forever.  Returns `true` when the name actually changed —
    /// committing an unchanged name is not a rename.
    pub fn rename(&mut self, name: &str) -> bool {
        if self.title == name {
            return false;
        }
        self.title = name.to_string();
        self.auto_label = None;
        true
    }

    /// Regenerate default tab names as "{label} {index}" with gapless,
    /// per-label numbering; user-renamed tabs are skipped untouched.
    pub fn renumber_all(sessions: &mut [TerminalSession]) {
        let mut counters: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for s in sessions {
            let Some(label) = s.auto_label.clone() else { continue };
            let n = counters.entry(label.clone()).or_insert(0);
            *n += 1;
            s.title = format!("{label} {n}");
        }
    }
}
