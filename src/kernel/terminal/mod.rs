//! Terminal subsystem — PTY sessions owned by `TerminalManager`, not buffers.
//!
//! Every terminal lives in the manager for its whole lifecycle (spawn →
//! output → exit → close).  The GUI's terminal dock is the only surface that
//! renders sessions; the central pane and the tab bar never see them.

pub mod ansi;
pub mod screen;
pub mod session;

pub use session::TerminalSession;

use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::state::Editor;
use crate::kernel::state::mode::EditorMode;

/// Owns every terminal session and which one holds the keyboard.
pub struct TerminalManager {
    pub sessions: Vec<TerminalSession>,
    next_id: usize,
    /// The session keys are routed to; `None` means editor keys.
    pub focused: Option<usize>,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self { sessions: Vec::new(), next_id: 1, focused: None }
    }
}

impl TerminalManager {
    /// Spawn `cmd` in a fresh PTY rooted at `cwd` and start streaming its
    /// output through the background event channel.  Returns the session id.
    pub fn spawn(
        &mut self,
        cmd: &str,
        args: &[String],
        cwd: &std::path::Path,
        sender: tokio::sync::mpsc::UnboundedSender<BackgroundEvent>,
    ) -> Result<usize, String> {
        let size = PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 };
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(size)
            .map_err(|e| format!("Failed to create PTY: {}", e))?;

        let mut cmd_builder = CommandBuilder::new(cmd);
        for a in args {
            cmd_builder.arg(a);
        }
        cmd_builder.env("TERM", "xterm-256color");
        cmd_builder.cwd(cwd.to_path_buf());

        let child = pair.slave.spawn_command(cmd_builder)
            .map_err(|e| format!("Failed to spawn command in PTY: {}", e))?;
        let reader = pair.master.try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;
        let writer = pair.master.take_writer()
            .map_err(|e| format!("Failed to get PTY writer: {}", e))?;

        let id = self.next_id;
        self.next_id += 1;
        let title = if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{cmd} {}", args.join(" "))
        };
        self.sessions.push(TerminalSession {
            id,
            title,
            cwd: cwd.to_path_buf(),
            screen: screen::TerminalScreen::default(),
            writer: Arc::new(Mutex::new(writer)),
            master: Mutex::new(pair.master),
            child,
        });

        std::thread::spawn(move || read_pipe(reader, id, sender));
        Ok(id)
    }

    pub fn get(&self, id: usize) -> Option<&TerminalSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut TerminalSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// The session holding the keyboard, or the newest one.
    pub fn active_id(&self) -> Option<usize> {
        self.focused.or_else(|| self.sessions.last().map(|s| s.id))
    }

    pub fn send_input(&self, id: usize, input: &str) -> Result<(), String> {
        self.get(id)
            .ok_or_else(|| "Not a terminal".to_string())?
            .send_input(input)
    }

    pub fn resize(&mut self, id: usize, cols: u16, rows: u16) -> Result<(), String> {
        let session = self.get_mut(id)
            .ok_or_else(|| "Not a terminal".to_string())?;
        let result = session.resize(cols, rows);
        session.screen.resize(cols, rows);
        result
    }

    pub fn apply_output(&mut self, id: usize, data: &str) {
        if let Some(s) = self.get_mut(id) {
            s.screen.apply(data);
        }
    }

    /// The child exited: close the session entirely — the tab disappears and
    /// the keyboard is released, the way a terminal emulator closes its tab
    /// when the shell inside it ends.
    pub fn mark_exited(&mut self, id: usize) {
        if let Some(s) = self.get_mut(id) {
            s.kill();
        }
        self.sessions.retain(|s| s.id != id);
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    /// Kill and remove a session for good.
    pub fn close(&mut self, id: usize) {
        if let Some(s) = self.get_mut(id) {
            s.kill();
        }
        self.sessions.retain(|s| s.id != id);
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    /// Kill and remove every session — the editor session is ending.
    pub fn close_all(&mut self) {
        for s in &mut self.sessions {
            s.kill();
        }
        self.sessions.clear();
        self.focused = None;
    }
}

fn read_pipe(
    mut reader: Box<dyn std::io::Read + Send>,
    terminal_id: usize,
    sender: tokio::sync::mpsc::UnboundedSender<BackgroundEvent>,
) {
    use std::io::Read;
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                // Raw bytes: the emulator on the foreground parses ANSI.
                let data = String::from_utf8_lossy(&buf[..n]).to_string();
                if sender.send(BackgroundEvent::TerminalOutput { terminal_id, data }).is_err() {
                    break;
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    break;
                }
            }
        }
    }
    // PTY EOF — the main thread reaps the real exit code from the child.
    let _ = sender.send(BackgroundEvent::TerminalExited { terminal_id, exit_code: 0 });
}

// ── Editor-level surface ─────────────────────────────────────────────────────

/// Send text to a terminal's stdin (PTY master).
pub fn send_input(editor: &Editor, id: usize, input: &str) -> Result<(), String> {
    editor.io.terminals.send_input(id, input)
}

/// Bind or release the keyboard to a terminal session.  Focusing enters
/// terminal mode (editor keymap layers off); releasing restores normal mode.
pub fn set_focused(editor: &mut Editor, id: Option<usize>) {
    editor.io.terminals.focused = id;
    if id.is_some() {
        editor.editor_mode = EditorMode::new("terminal", false);
        editor.keymaps.pop_layer("editor");
        editor.keymaps.pop_layer("insert");
    } else {
        editor.editor_mode = EditorMode::new("normal", false);
        editor.keymaps.push_layer("editor");
    }
}
