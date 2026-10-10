//! Window, tab, and terminal mode commands.

use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::event::payload::*;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("tabnext", "Switch to the next buffer",
        vec![],
        |editor, _args| {
            let keys: Vec<usize> = editor.buffers.iter().map(|(k, _)| k).collect();
            if keys.len() <= 1 { return Ok(()); }
            if let Some(win) = editor.view_tree.focused_window_mut()
                && let Some(current) = win.buffer_id
                    && let Some(pos) = keys.iter().position(|k| *k == current) {
                        let next = (pos + 1) % keys.len();
                        win.buffer_id = Some(keys[next]);
                        editor.events.emit_typed(crate::kernel::event::keys::events::BUFFER_CHANGED, BufferChangedPayload {
                            buffer_id: keys[next].to_string(),
                            revision_before: "0".to_string(),
                            revision_after: "0".to_string(),
                            edits: String::new(),
                        });
                    }
            Ok(())
        },
    );

    cmds.register_fn("tabprev", "Switch to the previous buffer",
        vec![],
        |editor, _args| {
            let keys: Vec<usize> = editor.buffers.iter().map(|(k, _)| k).collect();
            if keys.len() <= 1 { return Ok(()); }
            if let Some(win) = editor.view_tree.focused_window_mut()
                && let Some(current) = win.buffer_id
                    && let Some(pos) = keys.iter().position(|k| *k == current) {
                        let prev = if pos == 0 { keys.len() - 1 } else { pos - 1 };
                        win.buffer_id = Some(keys[prev]);
                        editor.events.emit_typed(crate::kernel::event::keys::events::BUFFER_CHANGED, BufferChangedPayload {
                            buffer_id: keys[prev].to_string(),
                            revision_before: "0".to_string(),
                            revision_after: "0".to_string(),
                            edits: String::new(),
                        });
                    }
            Ok(())
        },
    );

    cmds.register_fn("window-split", "Split window horizontally (ctrl-w s)",
        vec![],
        |editor, _args| {
            if let Some(win) = editor.view_tree.focused_window() {
                let existing_buf = editor.view_tree.buffer(win);
                if let Some(new_id) = editor.view_tree.split_horizontal(win)
                    && let Some(new_win) = editor.view_tree.window_mut(new_id) {
                        new_win.buffer_id = existing_buf;
                        // A split showing the same buffer gets its own view
                        // fork: independent cursor/scroll (docs/core-redesign.md §3).
                        if let Some(buf) = existing_buf {
                            editor.fork_view_for_window(new_id, buf);
                        }
                        editor.view_tree.focus(new_id);
                    }
            }
            Ok(())
        },
    );

    cmds.register_fn("window-vsplit", "Split window vertically (ctrl-w v)",
        vec![],
        |editor, _args| {
            if let Some(win) = editor.view_tree.focused_window() {
                let existing_buf = editor.view_tree.buffer(win);
                if let Some(new_id) = editor.view_tree.split_vertical(win)
                    && let Some(new_win) = editor.view_tree.window_mut(new_id) {
                        new_win.buffer_id = existing_buf;
                        // A split showing the same buffer gets its own view
                        // fork: independent cursor/scroll (docs/core-redesign.md §3).
                        if let Some(buf) = existing_buf {
                            editor.fork_view_for_window(new_id, buf);
                        }
                        editor.view_tree.focus(new_id);
                    }
            }
            Ok(())
        },
    );

    cmds.register_fn("window-close", "Close current window (ctrl-w q)",
        vec![],
        |editor, _args| {
            if let Some(win) = editor.view_tree.focused_window() {
                editor.view_tree.close_window(win);
            }
            Ok(())
        },
    );

    cmds.register_fn("window-wider", "Grow the focused pane's share of the split",
        vec![],
        |editor, _args| {
            if let Some(win) = editor.view_tree.focused_window()
                && let Some(w) = editor.view_tree.pane(win).map(|p| p.weight) {
                    editor.view_tree.resize_weighted(win, w + 0.1);
                }
            Ok(())
        },
    );

    cmds.register_fn("window-narrower", "Shrink the focused pane's share of the split",
        vec![],
        |editor, _args| {
            if let Some(win) = editor.view_tree.focused_window()
                && let Some(w) = editor.view_tree.pane(win).map(|p| p.weight) {
                    editor.view_tree.resize_weighted(win, w - 0.1);
                }
            Ok(())
        },
    );

    cmds.register_fn("terminal-start", "Start an interactive terminal in the terminal dock",
        vec![ArgSpec::optional("shell", ArgType::String, ArgValue::String("".to_string()))],
        |editor, args| {
            let custom_shell = args.get("shell")
                .and_then(|a| a.as_string())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let (cmd, cmd_args) = if let Some(shell) = custom_shell {
                (shell, vec![])
            } else {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
                (shell, vec![])
            };
            let cwd = editor.project_manager.project.root.clone()
                .or_else(|| std::env::current_dir().ok())
                .ok_or_else(|| "No working directory available".to_string())?;
            let sender = editor.background.as_ref()
                .ok_or_else(|| "No background handle available".to_string())?
                .sender.clone();
            let id = editor.io.terminals.spawn(&cmd, &cmd_args, &cwd, sender)?;
            crate::kernel::terminal::set_focused(editor, Some(id));
            Ok(())
        },
    );

    cmds.register_fn("terminal-send-input", "Send text to a terminal process",
        vec![
            ArgSpec::new("terminal", ArgType::Integer),
            ArgSpec::new("text", ArgType::String),
        ],
        |editor, args| {
            let term_id = args.get("terminal").and_then(|a| a.as_integer())
                .ok_or_else(|| "Missing terminal id".to_string())? as usize;
            let text = args.get("text").and_then(|a| a.as_string())
                .ok_or_else(|| "Missing text".to_string())?;
            crate::kernel::terminal::send_input(editor, term_id, text)?;
            Ok(())
        },
    );

    cmds.register_fn("terminal-focus", "Focus the active terminal (keyboard goes to the PTY)",
        vec![],
        |editor, _args| {
            let id = editor.io.terminals.active_id()
                .ok_or_else(|| "No terminal running".to_string())?;
            crate::kernel::terminal::set_focused(editor, Some(id));
            Ok(())
        },
    );

    cmds.register_fn("exit-terminal-mode", "Release the keyboard from the terminal, back to normal",
        vec![],
        |editor, _args| {
            crate::kernel::terminal::set_focused(editor, None);
            Ok(())
        },
    );

    cmds.register_fn("terminal-close", "Kill and remove the focused terminal session",
        vec![ArgSpec::optional("terminal", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let id = args.get("terminal").and_then(|a| a.as_integer())
                .filter(|&v| v > 0)
                .map(|v| v as usize)
                .or(editor.io.terminals.focused)
                .or_else(|| editor.io.terminals.sessions.last().map(|s| s.id))
                .ok_or_else(|| "No terminal running".to_string())?;
            editor.io.terminals.close(id);
            if editor.io.terminals.focused.is_none() {
                crate::kernel::terminal::set_focused(editor, None);
            }
            Ok(())
        },
    );

    cmds.register_fn("buffer-next", "Switch to next buffer (gt)",
        vec![],
        |editor, _args| {
            let current = get_current_buffer_id(editor);
            let keys: Vec<usize> = editor.buffers.iter().map(|(k, _)| k).collect();
            if let Some(idx) = keys.iter().position(|k| *k == current) {
                let next = keys[(idx + 1) % keys.len()];
                if let Some(win) = editor.view_tree.focused_window_mut() {
                    win.buffer_id = Some(next);
                }
                editor.events.emit_typed(crate::kernel::event::keys::events::BUFFER_FOCUSED, BufferFocusedPayload {
                    buffer_id: next.to_string(),
                });
                emit_cursor_moved(editor);
            }
            Ok(())
        },
    );

    cmds.register_fn("buffer-prev", "Switch to previous buffer (gT)",
        vec![],
        |editor, _args| {
            let current = get_current_buffer_id(editor);
            let keys: Vec<usize> = editor.buffers.iter().map(|(k, _)| k).collect();
            if let Some(idx) = keys.iter().position(|k| *k == current) {
                let prev = keys[(idx + keys.len() - 1) % keys.len()];
                if let Some(win) = editor.view_tree.focused_window_mut() {
                    win.buffer_id = Some(prev);
                }
                editor.events.emit_typed(crate::kernel::event::keys::events::BUFFER_FOCUSED, BufferFocusedPayload {
                    buffer_id: prev.to_string(),
                });
                emit_cursor_moved(editor);
            }
            Ok(())
        },
    );
}
