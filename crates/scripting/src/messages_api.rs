//! Janet API functions for the message log, help buffer, and warnings (Sprint 2).
//! Registered as `extern "C-unwind"` functions via evil-janet.

use evil_janet::*;
use super::conv;
use super::with_editor;
use magma::kernel::event::payload::*;
use magma::kernel::event::keys;

/// Find or create a special buffer by name, returning its slab key.
///
/// The buffer is marked ephemeral on first creation.  The caller is
/// responsible for setting read_only before and after writing.
fn get_or_create_special(
    ed: &mut magma::kernel::state::Editor,
    name: &str,
) -> usize {
    for (key, arc) in &ed.buffers {
        if arc.lock().unwrap_or_else(|e| e.into_inner()).name == name {
            return key;
        }
    }
    let key = ed.create_buffer(name);
    if let Some(arc) = ed.buffers.get_mut(key) {
        arc.lock().unwrap_or_else(|e| e.into_inner()).ephemeral = true;
    }
    key
}

/// (editor/log-message text)
unsafe extern "C-unwind" fn c_editor_log_message(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let text = conv::get_str(argc, argv, 0).unwrap_or_default();
        let key = get_or_create_special(ed, "*Messages*");
        // Bypass read-only: write directly via the buffer
        if let Some(arc) = ed.buffers.get_mut(key) {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.read_only = false;
            let end = buf.len();
            let line = format!("{}\n", text);
            buf.insert(end, &line);
            buf.read_only = true;
            buf.ephemeral = true;
        }
        ed.events.emit_typed(keys::events::BUFFER_MESSAGE_APPENDED, BufferMessageAppendedPayload {
            buffer_id: key.to_string(),
            text,
        });
        conv::nil()
    })
}

/// (editor/echo text) or (editor/echo text kind)
///
/// kind is "info" (default) or "error".  Shows a transient status-bar
/// message and appends the line to the *Messages* log.
unsafe extern "C-unwind" fn c_editor_echo(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let text = conv::get_str(argc, argv, 0).unwrap_or_default().to_string();
        let kind = if conv::get_str(argc, argv, 1).as_deref() == Some("error") {
            magma::kernel::state::MessageKind::Error
        } else {
            magma::kernel::state::MessageKind::Info
        };
        ed.show_message(kind, text.clone());
        let key = get_or_create_special(ed, "*Messages*");
        if let Some(arc) = ed.buffers.get_mut(key) {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.read_only = false;
            let end = buf.len();
            buf.insert(end, &format!("{text}\n"));
            buf.read_only = true;
            buf.ephemeral = true;
        }
        conv::nil()
    })
}

/// (editor/warn text)
unsafe extern "C-unwind" fn c_editor_warn(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let text = conv::get_str(argc, argv, 0).unwrap_or_default();
        let key = get_or_create_special(ed, "*Warnings*");
        if let Some(arc) = ed.buffers.get_mut(key) {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.read_only = false;
            let end = buf.len();
            let line = format!("{}\n", text);
            buf.insert(end, &line);
            buf.read_only = true;
            buf.ephemeral = true;
        }
        ed.events.emit_typed(keys::events::WARNING_EMITTED, WarningEmittedPayload {
            buffer_id: key.to_string(),
            text,
        });
        conv::nil()
    })
}

/// (editor/show-help text)
unsafe extern "C-unwind" fn c_editor_show_help(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let text = conv::get_str(argc, argv, 0).unwrap_or_default();
        let key = get_or_create_special(ed, "*Help*");
        if let Some(arc) = ed.buffers.get_mut(key) {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.read_only = false;
            // Replace entire content
            let old_len = buf.len();
            if old_len > 0 {
                buf.delete(0, old_len);
            }
            buf.insert(0, &text);
            buf.read_only = true;
            buf.ephemeral = true;
        }
        // Focus the *Help* buffer in the current window
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        ed.events.emit_typed(keys::events::HELP_SHOWN, HelpShownPayload {
            buffer_id: key.to_string(),
        });
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/log-message".as_ptr() as *const _,
            cfun: Some(c_editor_log_message as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Append a line to the *Messages* buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/echo".as_ptr() as *const _,
            cfun: Some(c_editor_echo as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Show a transient status message (optional second argument: \"error\") and log it to *Messages*".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/warn".as_ptr() as *const _,
            cfun: Some(c_editor_warn as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Append a warning line to the *Warnings* buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/show-help".as_ptr() as *const _,
            cfun: Some(c_editor_show_help as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Write text to *Help*, mark it read-only, and focus it".as_ptr() as *const _,
        },
    ]
}
