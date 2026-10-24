use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use portable_pty::{ChildKiller, MasterPty};

use super::screen::TerminalScreen;

/// One managed PTY session — the child process, its master, and the screen
/// the reader thread fills.  Sessions never live in the buffer slab; the
/// terminal dock renders `screen` directly and the manager owns the lifecycle.
pub struct TerminalSession {
    pub id: usize,
    pub title: String,
    /// The generated part of the tab name ("zsh", "sh -c read _"), kept so
    /// renumbering can regenerate `title`; `None` once the user renames the
    /// tab, which makes the name permanently user-owned.
    pub(crate) auto_label: Option<String>,
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
            crate::kernel::util::proctree::kill_tree_hard(pid);
        }
        if let Ok(mut killer) = self.child_killer.lock() {
            let _ = killer.kill();
        }
    }
}
