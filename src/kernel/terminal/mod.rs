use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::state::Editor;

/// A running terminal session bound to a buffer.
pub struct TerminalSession {
    /// PTY writer handle, wrapped for thread-safe access.
    pub stdin: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
}

/// Spawn a shell in a new buffer and start streaming output.
/// Returns the slab key of the new buffer.
pub fn start_terminal(
    editor: &mut Editor,
    cmd: &str,
    args: &[String],
) -> Result<usize, String> {
    let sender = {
        let bg = editor.background.as_ref()
            .ok_or_else(|| "No background handle available".to_string())?;
        bg.sender.clone()
    };

    let size = PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 };
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(size)
        .map_err(|e| format!("Failed to create PTY: {}", e))?;

    let mut cmd_builder = CommandBuilder::new(cmd);
    for a in args {
        cmd_builder.arg(a);
    }
    cmd_builder.env("TERM", "xterm-256color");
    // Start the shell in the editor's working directory, not the user's home.
    if let Ok(cwd) = std::env::current_dir() {
        cmd_builder.cwd(cwd);
    }

    let _child = pair.slave.spawn_command(cmd_builder)
        .map_err(|e| format!("Failed to spawn command in PTY: {}", e))?;

    let reader = pair.master.try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;
    let writer = pair.master.take_writer()
        .map_err(|e| format!("Failed to get PTY writer: {}", e))?;

    // Create buffer
    let name = format!("*terminal: {}*", cmd);
    let id = editor.allocate_buffer_id();
    let buf = crate::kernel::text_engine::Buffer::from_string(
        crate::kernel::state::id::BufferId(id), &name, "",
    );
    let entry = editor.buffers.vacant_entry();
    let buf_key = entry.key();
    entry.insert(buf);

    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(writer)),
    };

    // Attach to focused window
    if let Some(win) = editor.windows.focused_window_mut() {
        win.buffer_id = Some(buf_key);
    }

    // Store session before spawning reader
    editor.io.terminals.insert(buf_key, session);

    // Spawn PTY reader in background
    std::thread::spawn(move || {
        read_pipe(reader, buf_key, sender);
    });

    Ok(buf_key)
}

/// Send text to a terminal's stdin (PTY master).
pub fn send_input(editor: &Editor, buf_id: usize, input: &str) -> Result<(), String> {
    let session = editor.io.terminals.get(&buf_id)
        .ok_or_else(|| "Buffer is not a terminal".to_string())?;
    let mut writer = session.stdin.lock().map_err(|e| e.to_string())?;
    use std::io::Write;
    writer.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Check if a buffer has an active terminal session.
pub fn is_terminal(editor: &Editor, buf_id: usize) -> bool {
    editor.io.terminals.contains_key(&buf_id)
}

/// Strip ANSI escape sequences from terminal output so only visible text remains.
///
/// Ghost-suggestion text (e.g. zsh-autosuggestions) works by writing extra
/// characters after the cursor and then moving the cursor back with either
/// `\x08` (backspace) or `\x1b[nD` (cursor-back n).  These two sources of
/// "undo" can arrive in different `read()` chunks, so we cannot cancel them
/// inside a single `strip_ansi` call.  Instead:
///
/// * Faint/dim text (`\x1b[2m`…`\x1b[m`) is always suppressed — that covers
///   the default zsh-autosuggestions style.
/// * For non-dim ghost text, cursor-back sequences are converted to `\x08`
///   byte markers that `apply_terminal_output` will act on against the
///   *buffer* rather than the local string.  This makes the cancellation
///   work even across separate read() calls.
/// * A `faint_suppressed` counter prevents those `\x08` markers from eating
///   real text when the ghost was already suppressed by the faint rule.
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    let mut faint = false;           // true while \x1b[2m (dim/faint) is active
    let mut faint_suppressed: usize = 0; // chars suppressed in current faint run

    while let Some(c) = chars.next() {
        match c {
            '\x1b' => {
                match chars.next() {
                    Some('[') => {
                        // CSI: collect parameters until alphabetic or '~'.
                        let mut params = String::new();
                        let mut cmd = '\0';
                        for next in &mut chars {
                            if next.is_ascii_alphabetic() || next == '~' {
                                cmd = next;
                                break;
                            }
                            params.push(next);
                        }
                        match cmd {
                            // SGR: update text attributes.
                            'm' => {
                                if params.is_empty() || params == "0" {
                                    faint = false;
                                } else {
                                    for code in params.split(';') {
                                        match code.trim() {
                                            "2" | "02" => faint = true,
                                            "0" | ""   => faint = false,
                                            "22"        => faint = false,
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            // Cursor Backward (D): emit \x08 markers so
                            // apply_terminal_output can remove the ghost chars
                            // from the buffer even if they arrived in a prior chunk.
                            'D' => {
                                let n = params.parse::<usize>().unwrap_or(1);
                                // Chars suppressed by faint don't appear in `out`
                                // or in the buffer, so don't need a \x08 for them.
                                let effective = n.saturating_sub(faint_suppressed);
                                faint_suppressed = faint_suppressed.saturating_sub(n);
                                for _ in 0..effective {
                                    out.push('\x08');
                                }
                            }
                            _ => {} // all other CSI sequences stripped
                        }
                    }
                    Some(']') => {
                        loop {
                            match chars.next() {
                                None | Some('\x07') => break,
                                Some('\x1b') => { chars.next(); break; }
                                _ => {}
                            }
                        }
                    }
                    Some('(') | Some(')') => { chars.next(); }
                    Some('O') | Some('N') => { chars.next(); }
                    _ => {}
                }
            }
            // Backspace (^H): same logic as \x1b[1D — single-column cursor-back.
            '\x08' => {
                if faint_suppressed > 0 {
                    faint_suppressed -= 1;
                } else {
                    out.push('\x08');
                }
            }
            '\r' => {
                faint_suppressed = 0; // line is being redrawn from column 0
                if chars.as_str().starts_with('\n') {
                    chars.next();
                    out.push('\n');
                } else {
                    out.push('\r');
                }
            }
            c if c.is_control() && c != '\n' && c != '\t' => {}
            _ if faint => { faint_suppressed += 1; } // ghost char — suppress
            c => {
                faint_suppressed = 0; // real char pushed; prior suppressed region is past
                out.push(c);
            }
        }
    }
    out
}

fn read_pipe(
    mut reader: Box<dyn std::io::Read + Send>,
    buf_id: usize,
    sender: tokio::sync::mpsc::UnboundedSender<BackgroundEvent>,
) {
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                let data = strip_ansi(&raw);
                if sender.send(BackgroundEvent::TerminalOutput { buf_id, data }).is_err() {
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
    // Shell process exited — notify the main thread so it can kill the buffer.
    let _ = sender.send(BackgroundEvent::TerminalExited { buf_id, exit_code: 0 });
}


