//! Key-dispatch state machine and mode-aware text-input handler.

pub mod event;
pub mod mouse;

/// Escape sequence for an unmodified named key, or `None` for text keys.
fn named_seq(name: &str) -> Option<&'static str> {
    let seq: &str = match name {
        "return" => "\r",
        "tab" => "\t",
        "backspace" => "\x7f",
        "delete" => "\x1b[3~",
        "up" => "\x1b[A",
        "down" => "\x1b[B",
        "right" => "\x1b[C",
        "left" => "\x1b[D",
        "home" => "\x1b[H",
        "end" => "\x1b[F",
        "page-up" => "\x1b[5~",
        "page-down" => "\x1b[6~",
        "f1" => "\x1bOP",
        "f2" => "\x1bOQ",
        "f3" => "\x1bOR",
        "f4" => "\x1bOS",
        "f5" => "\x1b[15~",
        "f6" => "\x1b[17~",
        "f7" => "\x1b[18~",
        "f8" => "\x1b[19~",
        "f9" => "\x1b[20~",
        "f10" => "\x1b[21~",
        "f11" => "\x1b[23~",
        "f12" => "\x1b[24~",
        _ => return None,
    };
    Some(seq)
}

/// xterm CSI modifier parameter for a named key: 1=shift, 2=alt, 4=ctrl.
fn modified_named_seq(name: &str, shift: bool, alt: bool, ctrl: bool) -> Option<String> {
    let (first, final_byte) = match name {
        "up" => ("1", 'A'),
        "down" => ("1", 'B'),
        "right" => ("1", 'C'),
        "left" => ("1", 'D'),
        "home" => ("1", 'H'),
        "end" => ("1", 'F'),
        "insert" => ("2", '~'),
        "delete" => ("3", '~'),
        "page-up" => ("5", '~'),
        "page-down" => ("6", '~'),
        "f1" => ("1", 'P'),
        "f2" => ("1", 'Q'),
        "f3" => ("1", 'R'),
        "f4" => ("1", 'S'),
        "f5" => ("15", '~'),
        "f6" => ("17", '~'),
        "f7" => ("18", '~'),
        "f8" => ("19", '~'),
        "f9" => ("20", '~'),
        "f10" => ("21", '~'),
        "f11" => ("23", '~'),
        "f12" => ("24", '~'),
        _ => return None,
    };
    let m = 1 + (shift as i32) + (2 * alt as i32) + (4 * ctrl as i32);
    Some(format!("\x1b[{first};{m}{final_byte}"))
}

/// Split leading modifier prefixes; the remainder is the base key, which
/// may itself contain dashes ("ctrl-shift-page-up" → ctrl+shift + "page-up").
fn split_modifiers(key: &str) -> (bool, bool, bool, &str) {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut rest = key;
    loop {
        if let Some(r) = rest.strip_prefix("ctrl-") {
            ctrl = true;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("meta-") {
            alt = true;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("shift-") {
            shift = true;
            rest = r;
        } else {
            break;
        }
    }
    (ctrl, alt, shift, rest)
}

/// Forward a key to the shell process of the focused terminal session.
///
/// This completely bypasses the editor keymap so that keys like ctrl-c,
/// ctrl-s, backspace, and return reach the shell instead of triggering
/// editor commands.
fn forward_to_terminal(ed: &mut crate::state::Editor, buf_id: usize, key: &str) {
    if key == "esc" {
        let _ = crate::command::execute_command(
            ed,
            "exit-terminal-mode",
            &std::collections::HashMap::new(),
        );
        return;
    }

    let (ctrl, alt, shift, base) = split_modifiers(key);

    if ctrl || alt || shift {
        if let Some(seq) = modified_named_seq(base, shift, alt, ctrl) {
            logging::LogErr::log_err(crate::terminal::send_input(ed, buf_id, &seq));
            return;
        }
        // Text keys keep their unmodified byte under shift/ctrl (xterm
        // default); alt adds an ESC prefix.
        let body: String = match base {
            "return" => "\r".into(),
            "tab" => "\t".into(),
            "space" if ctrl => "\0".into(),
            "space" => " ".into(),
            "backspace" if ctrl => "\x08".into(),
            "backspace" => "\x7f".into(),
            "esc" => "\x1b".into(),
            other if alt => {
                logging::LogErr::log_err(crate::terminal::send_input(ed, buf_id, &format!("\x1b{other}")));
                return;
            }
            other if ctrl => {
                let byte: u8 = match other.chars().next().unwrap_or('\0') {
                    c @ 'a'..='z' => c as u8 - b'a' + 1,
                    c @ 'A'..='Z' => c as u8 - b'A' + 1,
                    '[' => 0x1b,
                    '\\' => 0x1c,
                    ']' => 0x1d,
                    '^' => 0x1e,
                    '_' => 0x1f,
                    _ => 0,
                };
                if byte > 0 {
                    let ctrl_bytes = [byte];
                    logging::LogErr::log_err(crate::terminal::send_input(
                        ed,
                        buf_id,
                        std::str::from_utf8(&ctrl_bytes).unwrap_or(""),
                    ));
                }
                return;
            }
            other => other.to_string(),
        };
        logging::LogErr::log_err(if alt {
            crate::terminal::send_input(ed, buf_id, &format!("\x1b{body}"))
        } else {
            crate::terminal::send_input(ed, buf_id, &body)
        });
        return;
    }

    let seq: &str = match named_seq(base) {
        Some(s) => s,
        None if base == "unknown" => return,
        None => base,
    };
    logging::LogErr::log_err(crate::terminal::send_input(ed, buf_id, seq));
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
pub fn dispatch_key(ed: &mut crate::state::Editor, key: &str) {
    // ── Terminal forwarding ─────────────────────────────────────────
    if let Some(term_id) = ed.io.terminals.focused {
        forward_to_terminal(ed, term_id, key);
        ed.dispatch_events_scoped();
        return;
    }

    // ── Quick-fix picker keys ───────────────────────────────────────
    if ed.quickfix_menu.is_some() {
        // A menu left over from a buffer switch closes on any key.
        let stale = ed.quickfix_menu.as_ref().map(|m| m.buf_key) != Some(focused_buffer_id(ed));
        let handled = match key {
            "down" if !stale => {
                crate::state::overlay::quickfix_menu_next(ed);
                true
            }
            "up" if !stale => {
                crate::state::overlay::quickfix_menu_prev(ed);
                true
            }
            "return" | "tab" if !stale => {
                crate::state::overlay::quickfix_menu_accept(ed);
                true
            }
            _ => {
                // Escape, or any typing key: the menu closes and typing
                // falls through to the normal pipeline.
                crate::state::overlay::quickfix_menu_close(ed);
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
            "up" => crate::completion::prev(ed),
            "down" => crate::completion::next(ed),
            "tab" | "return" => crate::completion::accept(ed),
            "esc" => crate::completion::dismiss(ed),
            k if is_word_input(k) => {
                handle_text_input(ed, k);
                crate::completion::trigger(ed);
            }
            _ => {
                // Chords and navigation leave the popup behind and fall
                // through to the regular pipeline.
                crate::completion::dismiss(ed);
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
        if let Err(e) = crate::command::execute_command(ed, &cmd, &std::collections::HashMap::new())
        {
            ed.show_message(crate::state::MessageKind::Error, e);
        }
    } else if ed.editor_mode.accepts_text {
        handle_text_input(ed, key);
    }
    ed.frame_dirty = true;
    ed.publish_buffer_changes();
    ed.dispatch_events_scoped();
}

/// Get the focused buffer's slab key, or 0.
pub fn focused_buffer_id(ed: &crate::state::Editor) -> usize {
    ed.view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap_or(0)
}

/// Named key strings that must never be inserted verbatim into the buffer.
pub const NON_INSERTABLE: &[&str] = &[
    "esc",
    "backspace",
    "delete",
    "tab",
    "return",
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "page-up",
    "page-down",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

/// Returns `true` if `s` is a printable string that should be inserted into
/// the buffer verbatim.
pub fn is_insertable(s: &str) -> bool {
    if s.starts_with("ctrl-") || s.starts_with("meta-") {
        return false;
    }
    if s.chars().any(|c| c.is_control()) {
        return false;
    }
    !NON_INSERTABLE.contains(&s)
}

/// Insert typed text into the focused buffer.
///
/// This is called when no keymap binding matched and `accepts_text` is true.
pub fn handle_text_input(ed: &mut crate::state::Editor, key: &str) {
    if !is_insertable(key) {
        return;
    }

    // Buffer text insertion.
    if let Some(slab) = ed
        .view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
    {
        if ed.multi_cursor.active && !ed.multi_cursor.extra_cursors.is_empty() {
            apply_to_all_cursors(ed, slab, key);
            return;
        }

        // A typed run is one undo step: open the insert session on the
        // first keystroke and let execute_command close it when any
        // explicit command interrupts the typing.
        let session_open = ed
            .buffers
            .get(slab)
            .is_some_and(|a| a.lock().unwrap_or_else(|e| e.into_inner()).has_open_undo_session());
        if let Some(view) = ed.view_of_buffer_mut(slab) {
            if !session_open {
                view.open_undo_session();
            }
            let cur = view.cursor_offset();
            view.insert(cur, key);
        }
        // Typing a word extends or starts the completion popup.
        crate::completion::auto_trigger_if_word(ed, key);
        // Call punctuation drives the signature popup: request on `(`/`,`,
        // drop on `)`.
        crate::signature::auto_trigger(ed, key);
    }
}

/// Single alphanumeric/underscore character — the input the completion
/// popup filters on.
fn is_word_input(key: &str) -> bool {
    key.chars().count() == 1
        && key
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Apply a text input operation to all cursors (primary + extras): one
/// transaction (one undo step), and every cursor lands after its own
/// insertion so the next keystroke cannot insert into stale offsets.
fn apply_to_all_cursors(ed: &mut crate::state::Editor, slab: usize, key: &str) {
    let mut cursors: Vec<(usize, Option<usize>)> = Vec::new();
    let primary_old;
    if let Some(view) = ed.view_of_buffer(slab) {
        primary_old = view.cursor_offset();
        cursors.push((primary_old, None));
    } else {
        return;
    }
    for (i, ec) in ed.multi_cursor.extra_cursors.iter().enumerate() {
        cursors.push((ec.pos, Some(i)));
    }
    cursors.sort();
    cursors.dedup_by(|a, b| a.0 == b.0);

    let edits: Vec<(usize, &str)> = cursors.iter().map(|&(p, _)| (p, key)).collect();
    let Some(view) = ed.view_of_buffer_mut(slab) else {
        return;
    };
    let Some(txn) = view.insert_many(&edits) else {
        return;
    };
    // Every cursor rides the same right-sticky anchor transform the
    // primary cursor uses, so each lands just after its own insertion
    // regardless of sort order or boundary snaps.
    view.set_cursor(txn.transform_anchor(crate::text_engine::anchor::Anchor::after(primary_old)).offset);
    for ec in ed.multi_cursor.extra_cursors.iter_mut() {
        ec.pos = txn.transform_anchor(crate::text_engine::anchor::Anchor::after(ec.pos)).offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_split_peels_prefixes_but_keeps_dashed_bases() {
        assert_eq!(split_modifiers("page-up"), (false, false, false, "page-up"));
        assert_eq!(split_modifiers("ctrl-up"), (true, false, false, "up"));
        assert_eq!(split_modifiers("meta-f8"), (false, true, false, "f8"));
        assert_eq!(
            split_modifiers("ctrl-shift-page-up"),
            (true, false, true, "page-up")
        );
        assert_eq!(split_modifiers("shift-return"), (false, false, true, "return"));
        assert_eq!(split_modifiers("meta-k"), (false, true, false, "k"));
    }

    #[test]
    fn modified_named_keys_send_xterm_csi_sequences() {
        assert_eq!(modified_named_seq("up", false, false, true).as_deref(), Some("\x1b[1;5A"));
        assert_eq!(modified_named_seq("left", false, true, false).as_deref(), Some("\x1b[1;3D"));
        assert_eq!(modified_named_seq("page-up", true, false, true).as_deref(), Some("\x1b[5;6~"));
        assert_eq!(modified_named_seq("f8", false, true, false).as_deref(), Some("\x1b[19;3~"));
        assert_eq!(modified_named_seq("return", false, false, true), None);
    }

    #[test]
    fn letters_insertable() {
        for c in 'a'..='z' {
            assert!(is_insertable(&c.to_string()));
        }
        for c in 'A'..='Z' {
            assert!(is_insertable(&c.to_string()));
        }
    }

    #[test]
    fn digits_insertable() {
        for c in '0'..='9' {
            assert!(is_insertable(&c.to_string()));
        }
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
        for k in NON_INSERTABLE {
            assert!(!is_insertable(k), "{k} must not be insertable");
        }
    }

    #[test]
    fn ctrl_meta_not_insertable() {
        assert!(!is_insertable("ctrl-s"));
        assert!(!is_insertable("ctrl-q"));
        assert!(!is_insertable("meta-x"));
    }

    #[test]
    fn nav_key_names_not_insertable() {
        for k in [
            "left",
            "right",
            "up",
            "down",
            "home",
            "end",
            "page-up",
            "page-down",
        ] {
            assert!(!is_insertable(k), "{k} must not be insertable");
        }
    }
}
