//! Key-dispatch state machine and mode-aware text-input handler.

pub mod event;
pub mod mouse;

/// Forward a key to the shell process of the focused terminal session.
///
/// This completely bypasses the editor keymap so that keys like ctrl-c,
/// ctrl-s, backspace, and return reach the shell instead of triggering
/// editor commands.
fn forward_to_terminal(ed: &mut crate::kernel::state::Editor, buf_id: usize, key: &str) {
    if key == "esc" {
        let _ = crate::kernel::command::execute_command(
            ed, "exit-terminal-mode", &std::collections::HashMap::new(),
        );
        return;
    }

    if let Some(rest) = key.strip_prefix("meta-") {
        let s = format!("\x1b{rest}");
        let _ = crate::kernel::terminal::send_input(ed, buf_id, &s);
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
            let _ = crate::kernel::terminal::send_input(
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
    let _ = crate::kernel::terminal::send_input(ed, buf_id, seq);
}

/// Dispatch a single key through the generic editor pipeline.
///
/// Order of dispatch:
/// 1. Terminal forwarding — when a terminal session holds the keyboard,
///    keys go straight to its PTY.
/// 2. Completion popup — while visible it owns navigation/accept keys and
///    word typing refilters it in place.
/// 3. Raw input interceptor (Janet `editor/on-input`).
/// 4. Keymap lookup — if a command is bound, execute it.
/// 5. Text input — only when `editor_mode.accepts_text` is true.
pub fn dispatch_key(ed: &mut crate::kernel::state::Editor, key: &str) {
    // ── Terminal forwarding ─────────────────────────────────────────
    if let Some(term_id) = ed.io.terminals.focused {
        forward_to_terminal(ed, term_id, key);
        ed.dispatch_events_scoped();
        return;
    }

    // ── Quick-fix picker keys ───────────────────────────────────────
    if ed.quickfix_menu.is_some() {
        // A menu left over from a buffer switch closes on any key.
        let stale = ed.quickfix_menu.as_ref().map(|m| m.buf_key)
            != Some(focused_buffer_id(ed));
        let handled = match key {
            "down" if !stale => {
                crate::kernel::state::overlay::quickfix_menu_next(ed); true
            }
            "up" if !stale => {
                crate::kernel::state::overlay::quickfix_menu_prev(ed); true
            }
            "return" | "tab" if !stale => {
                crate::kernel::state::overlay::quickfix_menu_accept(ed); true
            }
            _ => {
                // Escape, or any typing key: the menu closes and typing
                // falls through to the normal pipeline.
                crate::kernel::state::overlay::quickfix_menu_close(ed);
                key == "esc"
            }
        };
        ed.frame_dirty = true;
        if handled {
            ed.publish_buffer_changes();
            ed.dispatch_events_scoped();
            return;
        }
    }

    // ── Completion popup keys ───────────────────────────────────────
    if ed.completion.visible {
        let mut consumed = true;
        match key {
            "up"    => crate::kernel::completion::prev(ed),
            "down"  => crate::kernel::completion::next(ed),
            "tab" | "return" => crate::kernel::completion::accept(ed),
            "esc"   => crate::kernel::completion::dismiss(ed),
            k if is_word_input(k) => {
                handle_text_input(ed, k);
                crate::kernel::completion::trigger(ed);
            }
            _ => {
                // Chords and navigation leave the popup behind and fall
                // through to the regular pipeline.
                crate::kernel::completion::dismiss(ed);
                consumed = false;
            }
        }
        if consumed {
            ed.publish_buffer_changes();
            ed.dispatch_events_scoped();
            return;
        }
    }

    // ── Raw input interceptor (editor/on-input from Janet) ──────────
    if let Some(ref fn_name) = ed.on_input_fn.clone() {
        ed.input_consumed = false;
        let expr = format!("({} {:?})", fn_name, key);
        let mut rt = ed.runtime.take();
        if let Some(rt) = &mut rt {
            rt.eval(ed, &expr);
        }
        ed.runtime = rt;
        if ed.input_consumed {
            ed.publish_buffer_changes();
            ed.dispatch_events_scoped();
            return;
        }
    }

    // ── Keymap lookup → command, else text if accepts_text ─────────
    let buf_id = focused_buffer_id(ed);
    if let Some(cmd) = ed.keymaps.resolve_for_buffer(key, Some(buf_id as u64)) {
        if let Err(e) = crate::kernel::command::execute_command(ed, &cmd, &std::collections::HashMap::new()) {
            ed.show_message(crate::kernel::state::MessageKind::Error, e);
        }
    } else if ed.editor_mode.accepts_text {
        handle_text_input(ed, key);
    }
    ed.frame_dirty = true;
    ed.publish_buffer_changes();
    ed.dispatch_events_scoped();
}

/// Get the focused buffer's slab key, or 0.
pub fn focused_buffer_id(ed: &crate::kernel::state::Editor) -> usize {
    ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
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

/// Insert typed text into the focused buffer.
///
/// This is called when no keymap binding matched and `accepts_text` is true.
pub fn handle_text_input(ed: &mut crate::kernel::state::Editor, key: &str) {
    if !is_insertable(key) {
        return;
    }

    // Buffer text insertion.
    if let Some(slab) = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
    {
        if ed.multi_cursor.active && !ed.multi_cursor.extra_cursors.is_empty() {
            apply_to_all_cursors(ed, slab, key);
            return;
        }

        if let Some(view) = ed.view_of_buffer_mut(slab) {
            let cur = view.cursor_offset();
            view.insert(cur, key);
        }
        // Typing a word extends or starts the completion popup.
        crate::kernel::completion::auto_trigger_if_word(ed, key);
    }
}

/// Single alphanumeric/underscore character — the input the completion
/// popup filters on.
fn is_word_input(key: &str) -> bool {
    key.chars().count() == 1
        && key.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Apply a text input operation to all cursors (primary + extras).
fn apply_to_all_cursors(ed: &mut crate::kernel::state::Editor, slab: usize, key: &str) {
    let mut cursors: Vec<usize> = Vec::new();
    cursors.push({
        if let Some(view) = ed.view_of_buffer(slab) {
            view.cursor_offset()
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
        if let Some(view) = ed.view_of_buffer_mut(slab) {
            view.set_cursor(pos);
            view.insert(pos, key);
        }
    }

    if let Some(&first) = cursors.first()
        && let Some(view) = ed.view_of_buffer_mut(slab) {
            let adjustment = key.len();
            view.set_cursor(first + adjustment);
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
