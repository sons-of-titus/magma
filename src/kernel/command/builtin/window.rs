//! Window, tab, and terminal mode commands.

use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::state::mode::EditorMode;
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

    cmds.register_fn("terminal-start", "Start a terminal emulator in a new buffer",
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
            let buf_key = crate::kernel::terminal::start_terminal(editor, &cmd, &cmd_args)?;
            editor.io.terminal_mode_buf = Some(buf_key);
            editor.editor_mode = EditorMode::new("terminal", false);
            editor.keymaps.pop_layer("vim");
            editor.keymaps.pop_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("terminal-send-input", "Send text to a terminal process",
        vec![
            ArgSpec::new("buffer", ArgType::Integer),
            ArgSpec::new("text", ArgType::String),
        ],
        |editor, args| {
            let buf_id = args.get("buffer").and_then(|a| a.as_integer())
                .ok_or_else(|| "Missing buffer id".to_string())? as usize;
            let text = args.get("text").and_then(|a| a.as_string())
                .ok_or_else(|| "Missing text".to_string())?;
            crate::kernel::terminal::send_input(editor, buf_id, text)?;
            Ok(())
        },
    );

    cmds.register_fn("enter-terminal-mode", "Enter terminal insert mode for the current buffer",
        vec![],
        |editor, _args| {
            let buf_id = super::helpers::get_current_buffer_id(editor);
            if crate::kernel::terminal::is_terminal(editor, buf_id) {
                editor.io.terminal_mode_buf = Some(buf_id);
                editor.editor_mode = EditorMode::new("terminal", false);
                editor.keymaps.pop_layer("vim");
                editor.keymaps.pop_layer("insert");
            }
            Ok(())
        },
    );

    cmds.register_fn("exit-terminal-mode", "Exit terminal insert mode, return to normal",
        vec![],
        |editor, _args| {
            editor.editor_mode = EditorMode::new("normal", false);
            editor.io.terminal_mode_buf = None;
            editor.keymaps.push_layer("vim");
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
