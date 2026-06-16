//! Miscellaneous commands: quit, insert/replace/command mode entry, major mode.

use std::cmp::Reverse;

use crate::kernel::command::args::{ArgSpec, ArgType};
use crate::kernel::state::Editor;
use crate::kernel::state::mode::{EditorMode, Minibuffer};
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

use super::colon::execute_colon_command;
use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("quit", "Exit the editor",
        vec![],
        |editor, _args| {
            editor.events.emit_typed(keys::events::BEFORE_QUIT, EmptyPayload);
            editor.running = false;
            Ok(())
        },
    );

    cmds.register_fn("force-quit", "Quit without saving (ZQ)",
        vec![],
        |editor, _args| { editor.running = false; Ok(()) },
    );

    cmds.register_fn("save-and-quit", "Save buffer then quit (ZZ)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let arc = editor.buffers.get_mut(buf_id)
                .ok_or_else(|| "No buffer".to_string())?;
            let path = arc.lock().unwrap().path.clone();
            if let Some(ref path) = path {
                let content = { let b = arc.lock().unwrap(); b.slice(0, b.len()) };
                editor.fs.write(path, &content).map_err(|e| e.to_string())?;
                arc.lock().unwrap().mark_saved();
            }
            editor.running = false;
            Ok(())
        },
    );

    cmds.register_fn("enter-insert-mode", "Enter insert mode (terminal mode for terminal buffers)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if crate::kernel::terminal::is_terminal(editor, buf_id) {
                editor.io.terminal_mode_buf = Some(buf_id);
                editor.editor_mode = EditorMode::new("terminal", false);
                editor.keymaps.pop_layer("vim");
                editor.keymaps.pop_layer("insert");
                return Ok(());
            }
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            if let Some(view) = editor.views.get_mut(&buf_id) {
                view.open_undo_session();
            }
            Ok(())
        },
    );

    cmds.register_fn("exit-insert-mode", "Return to normal mode",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            {
                let last_insert_pos = editor.views.get(&buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
                editor.last_insert_pos = last_insert_pos;
                if let Some(view) = editor.views.get_mut(&buf_id) {
                    view.close_undo_session();
                }
            }

            if editor.block_visual.active {
                let lines = editor.block_visual.lines.clone();
                let col = editor.block_visual.col;
                let pre_pos = editor.block_visual.pre_pos;
                editor.block_visual.active = false;

                let post_pos = editor.views.get(&buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
                if post_pos > pre_pos {
                    let inserted = {
                        if let Some(view) = editor.views.get(&buf_id) {
                            view.buffer.lock().unwrap().slice(pre_pos, post_pos).to_string()
                        } else {
                            String::new()
                        }
                    };
                    let mut insert_positions: Vec<(usize, usize)> = Vec::new();
                    for (i, &line_start) in lines.iter().enumerate().skip(1) {
                        let line_end = {
                            if let Some(view) = editor.views.get(&buf_id) {
                                let buf = view.buffer.lock().unwrap();
                                let text = buf.slice(0, buf.len());
                                text[line_start..].find('\n')
                                    .map(|e| line_start + e)
                                    .unwrap_or(buf.len())
                            } else {
                                line_start
                            }
                        };
                        let actual_line_len = line_end - line_start;
                        let insert_at = line_start + col.min(actual_line_len);
                        insert_positions.push((insert_at, i));
                    }
                    insert_positions.sort_by_key(|b| Reverse(b.0));
                    for (insert_at, _) in &insert_positions {
                        if let Some(view) = editor.views.get_mut(&buf_id) {
                            view.insert(*insert_at, &inserted);
                        }
                    }
                }

                let first_line = lines[0];
                let (line_end, buf_len) = {
                    if let Some(view) = editor.views.get(&buf_id) {
                        let buf = view.buffer.lock().unwrap();
                        let text = buf.slice(0, buf.len());
                        let line_end = text[first_line..].find('\n')
                            .map(|e| first_line + e)
                            .unwrap_or(buf.len());
                        (line_end, buf.len())
                    } else {
                        (first_line, 0)
                    }
                };
                let line_text_len = line_end - first_line;
                let _ = buf_len;
                if let Some(view) = editor.views.get_mut(&buf_id) {
                    view.set_cursor(first_line + col.min(line_text_len));
                }

                editor.editor_mode = EditorMode::new("normal", false);
                editor.keymaps.pop_layer("insert");
                editor.keymaps.push_layer("vim");
                emit_cursor_moved(editor);
                return Ok(());
            }

            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.pop_layer("insert");
            editor.keymaps.push_layer("vim");
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor_offset();
                let (is_empty, last_ch) = {
                    let buf = view.buffer.lock().unwrap();
                    (buf.is_empty(), if pos > 0 { buf.slice(0, pos).chars().last() } else { None })
                };
                if pos > 0 && !is_empty {
                    if let Some(ch) = last_ch {
                        view.set_cursor(pos - ch.len_utf8());
                    }
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("append", "Move one char right then enter insert mode",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor_offset();
                let next_ch_len = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(pos, buf.len());
                    text.chars().next().map(|c| c.len_utf8())
                };
                if let Some(len) = next_ch_len {
                    view.set_cursor(pos + len);
                }
            }
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("open-line-below", "Insert new line below cursor and enter insert mode",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let eol = {
                    let buf = view.buffer.lock().unwrap();
                    let start = buf.line_start_offset(line).unwrap_or(0);
                    let text = buf.line(line).unwrap_or_default();
                    start + text.len()
                };
                view.set_cursor(eol);
                view.insert(eol, "\n");
            }
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("insert-bol", "Go to first non-whitespace and enter insert mode (I)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = {
                    let buf = view.buffer.lock().unwrap();
                    buf.line_start_offset(line).map(|off| {
                        let line_text = buf.line(line).unwrap_or_default();
                        let indent = line_text.len() - line_text.trim_start().len();
                        off + indent
                    })
                };
                if let Some(pos) = target {
                    view.set_cursor(pos);
                }
            }
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("append-eol", "Go to end of line and enter insert mode (A)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = {
                    let buf = view.buffer.lock().unwrap();
                    buf.line_start_offset(line).map(|off| {
                        let line_text = buf.line(line).unwrap_or_default();
                        off + line_text.len()
                    })
                };
                if let Some(pos) = target {
                    view.set_cursor(pos);
                }
            }
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("enter-replace-mode", "Enter replace (overwrite) mode",
        vec![],
        |editor, _args| {
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("replace", true);
            editor.keymaps.push_layer("replace");
            Ok(())
        },
    );

    cmds.register_fn("exit-replace-mode", "Exit replace mode",
        vec![],
        |editor, _args| {
            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.pop_layer("replace");
            editor.keymaps.push_layer("vim");
            Ok(())
        },
    );

    cmds.register_fn("enter-command-mode", "Open the : command bar",
        vec![],
        |editor, _args| {
            editor.keymaps.pop_layer("visual");
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode {
                name: "command".into(),
                accepts_text: false,
                minibuffer: Some(Minibuffer { prompt: ":".into(), input: String::new() }),
            };
            editor.keymaps.push_layer("command");
            Ok(())
        },
    );

    cmds.register_fn("exit-command-mode", "Cancel command input (default impl)",
        vec![],
        |editor, _args| {
            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.pop_layer("command");
            editor.keymaps.push_layer("vim");
            Ok(())
        },
    );

    cmds.register_fn("command-backspace", "Delete last char of command input (default impl)",
        vec![],
        |editor, _args| {
            if editor.completion.visible {
                editor.completion.visible = false;
                editor.completion.items.clear();
            }
            let should_exit = if let Some(ref mut mb) = editor.editor_mode.minibuffer {
                let last_len = mb.input.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                let new_len = mb.input.len().saturating_sub(last_len);
                mb.input.truncate(new_len);
                mb.input.is_empty()
            } else {
                false
            };
            if should_exit {
                editor.editor_mode = EditorMode::new("normal", false);
                editor.keymaps.pop_layer("command");
                editor.keymaps.push_layer("vim");
            }
            Ok(())
        },
    );

    cmds.register_fn("command-execute", "Execute the buffered : command (default impl)",
        vec![],
        |editor, _args| {
            let input = if let Some(ref mb) = editor.editor_mode.minibuffer {
                mb.input.trim().to_string()
            } else {
                return Ok(());
            };
            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.pop_layer("command");
            editor.keymaps.push_layer("vim");
            execute_colon_command(editor, &input)
        },
    );

    cmds.register_fn("set-major-mode",
        "Set the major mode of the focused buffer",
        vec![ArgSpec::new("mode", ArgType::String)],
        |editor, args| {
            let name = args.get("mode")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "mode name required".to_string())?;
            let new_mode = crate::kernel::text_engine::MajorMode::from_name(name);
            let buf_id = get_current_buffer_id(editor);
            if let Some(arc) = editor.buffers.get_mut(buf_id) {
                arc.lock().unwrap().major_mode = new_mode;
            }
            editor.events.emit_typed(keys::events::MAJOR_MODE_CHANGED, MajorModeChangedPayload {
                mode: name.to_string(),
                buffer_id: buf_id.to_string(),
            });
            Ok(())
        },
    );

    cmds.register_fn("major-mode",
        "Return the current buffer's major mode name",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(arc) = editor.buffers.get(buf_id) {
                println!("{}", arc.lock().unwrap().major_mode.name());
            }
            Ok(())
        },
    );

    cmds.register_fn("execute-normal", "Execute one normal mode command from insert (ctrl-o)",
        vec![],
        |editor, _args| {
            editor.keymaps.pop_layer("insert");
            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.push_layer("vim");
            Ok(())
        },
    );
}
