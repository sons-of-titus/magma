//! Session commands — quit and major-mode reporting.
//!
//! Magma ships non-modal: buffers always accept text and editing happens
//! through commands bound to chords.  Modal editing (vim-style normal mode)
//! is community-extension territory — the mode struct and keymap layers
//! remain as extension substrate, but nothing in the core enters a mode.

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::event::keys;
use crate::event::payload::*;
use crate::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "quit",
        "Exit the editor (refuses when buffers have unsaved changes)",
        vec![],
        |editor, _args| {
            let dirty = editor.dirty_buffer_names();
            if !dirty.is_empty() {
                editor.quit_refused = true;
                return Err(format!(
                    "Unsaved changes in {} — save them or choose “Quit without Saving”.",
                    dirty.join(", ")
                ));
            }
            editor.quit_refused = false;
            editor
                .events
                .emit_typed(keys::events::BEFORE_QUIT, EmptyPayload);
            editor.running = false;
            Ok(())
        },
    );

    cmds.register_fn(
        "force-quit",
        "Quit without saving",
        vec![],
        |editor, _args| {
            editor.running = false;
            Ok(())
        },
    );

    // Editor zoom: the editor font size is the zoom level.  Persistence
    // rides the same `font.*` options the Settings page writes.
    for (name, description, delta) in [
        ("zoom-in", "Increase the editor font size", 1.0f32),
        ("zoom-out", "Decrease the editor font size", -1.0),
        ("zoom-reset", "Reset the editor font size to 15px", 0.0),
    ] {
        cmds.register_fn(name, description, vec![], move |editor, _args| {
            let base = if delta == 0.0 { 15.0 } else { editor.font_config.size + delta };
            editor.font_config.size = base.clamp(6.0, 72.0);
            editor
                .options
                .insert("font.size".into(), editor.font_config.size.to_string());
            crate::state::persist::save_settings(editor);
            editor.font_changed = true;
            editor.frame_dirty = true;
            Ok(())
        });
    }

    cmds.register_fn(
        "editor-escape",
        "Clear the active selection",
        vec![],
        |editor, _args| {
            editor.selection = None;
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "save-and-quit",
        "Save buffer then quit",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let arc = editor
                .buffers
                .get_mut(buf_id)
                .ok_or_else(|| "No buffer".to_string())?;
            let path = arc.lock().unwrap_or_else(|e| e.into_inner()).path.clone();
            if let Some(ref path) = path {
                let content = {
                    let b = arc.lock().unwrap_or_else(|e| e.into_inner());
                    b.slice(0, b.len())
                };
                editor.fs.write(path, &content).map_err(|e| e.to_string())?;
                arc.lock().unwrap_or_else(|e| e.into_inner()).mark_saved();
            }
            editor.running = false;
            Ok(())
        },
    );

    cmds.register_fn(
        "set-major-mode",
        "Set the major mode of the focused buffer, or of the buffer named",
        vec![
            ArgSpec::new("mode", ArgType::String),
            // Absent = the focused buffer; no default value on purpose —
            // defaulting to slot 0 would silently target the wrong buffer.
            ArgSpec {
                name: "buffer".to_string(),
                arg_type: ArgType::Buffer,
                required: false,
                default: None,
                prompt: None,
            },
        ],
        |editor, args| {
            let name = args
                .get("mode")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "mode name required".to_string())?;
            let new_mode = crate::text_engine::MajorMode::from_name(name);
            // buffer-created events queue and drain in batches (project
            // open, session restore): the mode must land on the buffer the
            // event describes, not whichever one is focused at drain time.
            let buf_id = args
                .get("buffer")
                .and_then(|a| a.as_buffer())
                .map(|b| b as usize)
                .unwrap_or_else(|| get_current_buffer_id(editor));
            let Some(arc) = editor.buffers.get(buf_id) else {
                return Err(format!("no such buffer: {buf_id}"));
            };
            arc.lock().unwrap_or_else(|e| e.into_inner()).major_mode = new_mode;
            editor.events.emit_typed(
                keys::events::MAJOR_MODE_CHANGED,
                MajorModeChangedPayload {
                    mode: name.to_string(),
                    buffer_id: buf_id.to_string(),
                },
            );
            Ok(())
        },
    );

    cmds.register_fn(
        "major-mode",
        "Return the current buffer's major mode name",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(arc) = editor.buffers.get(buf_id) {
                info!(
                    "{}",
                    arc.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .major_mode
                        .name()
                );
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "syntax-highlight",
        "Re-run fallback syntax highlighting on a buffer",
        vec![ArgSpec::optional(
            "buffer",
            ArgType::Integer,
            ArgValue::Integer(-1),
        )],
        |editor, args| {
            let key = args
                .get("buffer")
                .and_then(|a| a.as_integer())
                .filter(|&k| k >= 0)
                .map(|k| k as usize)
                .unwrap_or_else(|| get_current_buffer_id(editor));
            crate::semantic::fallback_syntax::force_highlight(editor, key);
            Ok(())
        },
    );
}
