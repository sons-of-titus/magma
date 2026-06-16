//! Key-dispatch state machine and mode-aware text-input handler.
//!
//! Submodules provide the shared input-event types and key-normalisation
//! helpers that both the TUI and GUI adapters must use.

pub mod event;
pub mod keys;
pub mod mouse;

/// Forward a key to the shell process running in a terminal buffer.
///
/// This completely bypasses the editor keymap so that keys like ctrl-c,
/// ctrl-s, backspace, and return reach the shell instead of triggering
/// editor commands.
fn forward_to_terminal(ed: &mut crate::state::Editor, buf_id: usize, key: &str) {
    if key == "esc" {
        let _ = crate::command::execute_command(
            ed, "exit-terminal-mode", &std::collections::HashMap::new(),
        );
        return;
    }

    if let Some(rest) = key.strip_prefix("meta-") {
        let s = format!("\x1b{rest}");
        let _ = crate::terminal::send_input(ed, buf_id, &s);
        return;
    }

    if let Some(rest) = key.strip_prefix("ctrl-") {
        let byte: u8 = match rest.chars().next().unwrap_or('\0') {
            c @ 'a'..='z' => c as u8 - b'a' + 1,
            c @ 'A'..='Z' => c as u8 - b'A' + 1,
            '[' => 0x1b,
            '\\' => 0x1c,
            ']' => 0x1d,
            '^' => 0x1e,
            '_' => 0x1f,
            ' ' => 0x00,
            _ => 0,
        };
        if byte > 0 || rest.starts_with(' ') {
            let ctrl_bytes = [byte];
            let _ = crate::terminal::send_input(
                ed, buf_id, std::str::from_utf8(&ctrl_bytes).unwrap_or(""),
            );
        }
        return;
    }

    let seq: &str = match key {
        "return"    => "\r",
        "tab"       => "\t",
        "backspace" => "\x7f",
        "delete"    => "\x1b[3~",
        "up"        => "\x1b[A",
        "down"      => "\x1b[B",
        "right"     => "\x1b[C",
        "left"      => "\x1b[D",
        "home"      => "\x1b[H",
        "end"       => "\x1b[F",
        "page-up"   => "\x1b[5~",
        "page-down" => "\x1b[6~",
        "f1"        => "\x1bOP",
        "f2"        => "\x1bOQ",
        "f3"        => "\x1bOR",
        "f4"        => "\x1bOS",
        "f5"        => "\x1b[15~",
        "f6"        => "\x1b[17~",
        "f7"        => "\x1b[18~",
        "f8"        => "\x1b[19~",
        "f9"        => "\x1b[20~",
        "f10"       => "\x1b[21~",
        "f11"       => "\x1b[23~",
        "f12"       => "\x1b[24~",
        "unknown"   => return,
        k           => k,
    };
    let _ = crate::terminal::send_input(ed, buf_id, seq);
}

/// Dispatch a single key through the generic editor pipeline.
///
/// Order of dispatch:
/// 1. Terminal forwarding — raw PTY bypass.
/// 2. Raw input interceptor (Janet `editor/on-input`).
/// 3. Keymap lookup — if a command is bound, execute it.
/// 4. Text input — only when `editor_mode.accepts_text` is true.
pub fn dispatch_key(ed: &mut crate::state::Editor, key: &str) {
    // ── Terminal forwarding ─────────────────────────────────────────
    if let Some(buf_id) = ed.io.terminal_mode_buf {
        forward_to_terminal(ed, buf_id, key);
        ed.events.drain_and_dispatch();
        return;
    }

    // ── Raw input interceptor (editor/on-input from Janet) ──────────
    if let Some(ref fn_name) = ed.on_input_fn.clone() {
        ed.input_consumed = false;
        let expr = format!("({} {:?})", fn_name, key);
        if let Some(ref mut rt) = ed.runtime {
            rt.eval(&expr);
        }
        if ed.input_consumed {
            ed.events.drain_and_dispatch();
            return;
        }
    }

    // ── Minibuffer dispatch ─────────────────────────────────────────
    // Insertable keys go directly into the minibuffer.
    // Non-insertable keys (return, tab, esc, etc.) fall through to keymap lookup.
    if ed.editor_mode.minibuffer.is_some() {
        if is_insertable(key) {
            handle_text_input(ed, key);
            ed.events.drain_and_dispatch();
            return;
        }
        // Fall through to keymap lookup for non-insertable keys.
    }

    // ── Keymap lookup → command, else text if accepts_text ─────────
    let buf_id = focused_buffer_id(ed);
    if let Some(cmd) = ed.keymaps.resolve_for_buffer(key, Some(buf_id as u64)) {
        let _ = crate::command::execute_command(ed, &cmd, &std::collections::HashMap::new());
    } else if ed.editor_mode.accepts_text {
        handle_text_input(ed, key);
    }
    ed.events.drain_and_dispatch();
}

/// Get the focused buffer's slab key, or 0.
pub(crate) fn focused_buffer_id(ed: &crate::state::Editor) -> usize {
    ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap_or(0)
}

/// Named key strings that must never be inserted verbatim into the buffer.
pub const NON_INSERTABLE: &[&str] = &[
    "esc", "backspace", "delete", "tab", "return",
    "left", "right", "up", "down",
    "home", "end", "page-up", "page-down",
    "f1", "f2", "f3", "f4", "f5", "f6",
    "f7", "f8", "f9", "f10", "f11", "f12",
];

/// Returns `true` if `s` is a printable string that should be inserted into
/// the buffer verbatim.
pub fn is_insertable(s: &str) -> bool {
    if s.starts_with("ctrl-") || s.starts_with("meta-") { return false; }
    if s.chars().any(|c| c.is_control()) { return false; }
    !NON_INSERTABLE.contains(&s)
}

/// Insert typed text into the focused buffer or minibuffer.
///
/// This is called when no keymap binding matched and `accepts_text` is true.
pub fn handle_text_input(ed: &mut crate::state::Editor, key: &str) {
    if !is_insertable(key) {
        return;
    }

    // Minibuffer: append to input string.
    if let Some(ref mut mb) = ed.editor_mode.minibuffer {
        if ed.completion.visible {
            ed.completion.visible = false;
            ed.completion.items.clear();
        }
        mb.input.push_str(key);
        return;
    }

    // Buffer text insertion.
    if let Some(slab) = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
    {
        if ed.multi_cursor.active && !ed.multi_cursor.extra_cursors.is_empty() {
            apply_to_all_cursors(ed, slab, key);
            return;
        }

        if let Some(buf) = ed.buffers.get_mut(slab) {
            let cur = buf.cursor();
            buf.insert(cur, key);
        }
    }
}

/// Apply a text input operation to all cursors (primary + extras).
fn apply_to_all_cursors(ed: &mut crate::state::Editor, slab: usize, key: &str) {
    let mut cursors: Vec<usize> = Vec::new();
    cursors.push({
        if let Some(buf) = ed.buffers.get(slab) {
            buf.cursor()
        } else {
            return;
        }
    });
    for ec in &ed.multi_cursor.extra_cursors {
        cursors.push(ec.pos);
    }
    cursors.sort();
    cursors.dedup();

    for &pos in cursors.iter().rev() {
        if let Some(buf) = ed.buffers.get_mut(slab) {
            buf.set_cursor(pos);
            buf.insert(pos, key);
        }
    }

    if let Some(&first) = cursors.first()
        && let Some(buf) = ed.buffers.get_mut(slab) {
            let adjustment = key.len();
            buf.set_cursor(first + adjustment);
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_insertable() {
        for c in 'a'..='z' { assert!(is_insertable(&c.to_string())); }
        for c in 'A'..='Z' { assert!(is_insertable(&c.to_string())); }
    }

    #[test]
    fn digits_insertable() {
        for c in '0'..='9' { assert!(is_insertable(&c.to_string())); }
    }

    #[test]
    fn symbols_insertable() {
        for s in ["!", "@", "#", "$", " ", ".", ",", ":", ";", "'", "\""] {
            assert!(is_insertable(s), "{s:?} should be insertable");
        }
    }

    #[test]
    fn f_letter_insertable() {
        assert!(is_insertable("f"));
    }

    #[test]
    fn function_keys_not_insertable() {
        for k in NON_INSERTABLE { assert!(!is_insertable(k), "{k} must not be insertable"); }
    }

    #[test]
    fn ctrl_meta_not_insertable() {
        assert!(!is_insertable("ctrl-s"));
        assert!(!is_insertable("ctrl-q"));
        assert!(!is_insertable("meta-x"));
    }

    #[test]
    fn nav_key_names_not_insertable() {
        for k in ["left", "right", "up", "down", "home", "end", "page-up", "page-down"] {
            assert!(!is_insertable(k), "{k} must not be insertable");
        }
    }
}
