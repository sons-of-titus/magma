use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use portable_pty::{Child, MasterPty};

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
    pub(crate) child: Box<dyn Child + Send + Sync>,
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
        let _ = self.child.kill();
    }
}
