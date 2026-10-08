//! Session commands — quit and major-mode reporting.
//!
//! Magma ships non-modal: buffers always accept text and editing happens
//! through commands bound to chords.  Modal editing (vim-style normal mode)
//! is community-extension territory — the mode struct and keymap layers
//! remain as extension substrate, but nothing in the core enters a mode.

use crate::kernel::command::args::{ArgSpec, ArgType};
use crate::kernel::state::Editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("quit", "Exit the editor (refuses when buffers have unsaved changes)",
        vec![],
        |editor, _args| {
            let dirty = editor.dirty_buffer_names();
            if !dirty.is_empty() {
                return Err(format!(
                    "No write since last change: {} (use force-quit to override)",
                    dirty.join(", ")
                ));
            }
            editor.events.emit_typed(keys::events::BEFORE_QUIT, EmptyPayload);
            editor.running = false;
            Ok(())
        },
    );

    cmds.register_fn("force-quit", "Quit without saving",
        vec![],
        |editor, _args| { editor.running = false; Ok(()) },
    );

    cmds.register_fn("editor-escape", "Clear the active selection",
        vec![],
        |editor, _args| {
            editor.selection = None;
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("save-and-quit", "Save buffer then quit",
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
}
