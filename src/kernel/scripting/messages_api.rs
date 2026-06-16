//! Janet API functions for the message log, help buffer, and warnings (Sprint 2).
//! Registered as `extern "C-unwind"` functions via evil-janet.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

/// Find or create a special buffer by name, returning its slab key.
///
/// The buffer is marked ephemeral on first creation.  The caller is
/// responsible for setting read_only before and after writing.
fn get_or_create_special(
    ed: &mut crate::kernel::state::Editor,
    name: &str,
) -> usize {
    for (key, buf) in &ed.buffers {
        if buf.name == name {
            return key;
        }
    }
    let buf_id = ed.allocate_buffer_id();
    let mut buf = crate::kernel::text_engine::Buffer::new(
        crate::kernel::state::id::BufferId(buf_id),
        name,
    );
    buf.ephemeral = true;
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    key
}

/// (editor/log-message text)
///
/// Append `text` followed by a newline to the `*Messages*` buffer, creating
/// it on first use.  The buffer is read-only between calls; this function
/// bypasses the flag to append.  Emits `buffer-message-appended`.
unsafe extern "C-unwind" fn c_editor_log_message(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let text = conv::get_str(argc, argv, 0).unwrap_or_default();
        let key = get_or_create_special(ed, "*Messages*");
        // Bypass read-only: write directly via the rope
        if let Some(buf) = ed.buffers.get_mut(key) {
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

/// (editor/warn text)
///
/// Append `text` followed by a newline to the `*Warnings*` buffer, creating
/// it on first use.  Emits `warning-emitted`.
unsafe extern "C-unwind" fn c_editor_warn(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let text = conv::get_str(argc, argv, 0).unwrap_or_default();
        let key = get_or_create_special(ed, "*Warnings*");
        if let Some(buf) = ed.buffers.get_mut(key) {
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
///
/// Write `text` into the `*Help*` buffer (replacing any previous content),
/// mark it read-only and ephemeral, and focus it in the current window.
/// Emits `help-shown`.
unsafe extern "C-unwind" fn c_editor_show_help(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let text = conv::get_str(argc, argv, 0).unwrap_or_default();
        let key = get_or_create_special(ed, "*Help*");
        if let Some(buf) = ed.buffers.get_mut(key) {
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
        if let Some(win) = ed.windows.focused_window_mut() {
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
