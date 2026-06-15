//! Janet API tests for the modal input policy (Sprint 11e).
//! Tests input/register-operator, input/register-prefix, etc.

#[cfg(feature = "janet")]
mod tests {
    use crate::state::Editor;
    use crate::state::id::BufferId;
    use crate::buffer::Buffer;
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::input::dispatch_key;
    use crate::janet_bridge;

    fn make_editor(content: &str) -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let id = ed.allocate_buffer_id();
        let buf = Buffer::from_string(BufferId(id), "test", content);
        let entry = ed.buffers.vacant_entry();
        let key = entry.key();
        entry.insert(buf);
        if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
        ed
    }

    fn text(ed: &Editor) -> String {
        let slab = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid)).unwrap();
        ed.buffers.get(slab).unwrap()
            .slice(0, ed.buffers.get(slab).unwrap().len())
    }

    // ── input/register-operator ───────────────────────────────────────────

    #[test]
    fn register_operator_adds_to_registry() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(ed.vim_operators.contains_key("d"));
        assert!(ed.vim_operators.contains_key("c"));
        assert!(ed.vim_operators.contains_key("y"));
    }

    #[test]
    fn register_operator_line_cmd_correct() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert_eq!(ed.vim_operators.get("d").map(|s| s.as_str()), Some("delete-line"));
        assert_eq!(ed.vim_operators.get("y").map(|s| s.as_str()), Some("yank-line"));
    }

    #[test]
    fn clear_operators_removes_all() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(!ed.vim_operators.is_empty());
        janet_bridge::eval(&mut ed, "(input/clear-operators)");
        assert!(ed.vim_operators.is_empty());
    }

    #[test]
    fn get_operator_returns_line_cmd() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(input/register-operator "X" "delete-line")"#);
        assert_eq!(ed.vim_operators.get("X").map(|s| s.as_str()), Some("delete-line"));
    }

    // ── input/register-prefix ─────────────────────────────────────────────

    #[test]
    fn register_prefix_populates_registry() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(ed.vim_prefixes.contains("g"));
        assert!(ed.vim_prefixes.contains("z"));
        assert!(ed.vim_prefixes.contains("ctrl-w"));
    }

    #[test]
    fn unregister_prefix_removes_key() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(ed.vim_prefixes.contains("g"));
        janet_bridge::eval(&mut ed, r#"(input/unregister-prefix "g")"#);
        assert!(!ed.vim_prefixes.contains("g"));
    }

    #[test]
    fn registered_prefix_p_query() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(ed.vim_prefixes.contains("g"));
        assert!(!ed.vim_prefixes.contains("x"));
    }

    // ── input/register-motion ─────────────────────────────────────────────

    #[test]
    fn register_motion_populates_registry() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(ed.vim_motions.contains_key("w"));
        assert!(ed.vim_motions.contains_key("h"));
        assert!(ed.vim_motions.contains_key("G"));
    }

    #[test]
    fn motion_command_correct() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert_eq!(ed.vim_motions.get("w").map(|s| s.as_str()), Some("move-word-forward"));
        assert_eq!(ed.vim_motions.get("h").map(|s| s.as_str()), Some("cursor-left"));
    }

    #[test]
    fn clear_motions_removes_all() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(!ed.vim_motions.is_empty());
        janet_bridge::eval(&mut ed, "(input/clear-motions)");
        assert!(ed.vim_motions.is_empty());
    }

    // ── input/register-char-capture ───────────────────────────────────────

    #[test]
    fn char_capture_registry_populated() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert!(ed.vim_char_captures.contains_key("f"));
        assert!(ed.vim_char_captures.contains_key("r"));
        assert!(ed.vim_char_captures.contains_key("m"));
    }

    #[test]
    fn char_capture_find_forward_dispatch() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("hello");
        janet_bridge::init(&mut ed);
        let slab = ed.windows.focused_window().and_then(|wid| ed.windows.buffer(wid)).unwrap();
        ed.buffers.get_mut(slab).unwrap().set_cursor(0);
        dispatch_key(&mut ed, "f");  // enter char-capture for "f"
        dispatch_key(&mut ed, "l");  // find 'l' forward
        // cursor should have moved to position of 'l' (index 2 in "hello")
        let c = ed.buffers.get(slab).unwrap().cursor();
        assert_eq!(c, 2, "f+l should move cursor to 'l' at index 2");
    }

    // ── Two-key prefix keymap entries (replace handle_prefix) ─────────────

    #[test]
    fn gg_goes_to_buffer_start() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("hello\nworld");
        janet_bridge::init(&mut ed);
        let slab = ed.windows.focused_window().and_then(|wid| ed.windows.buffer(wid)).unwrap();
        ed.buffers.get_mut(slab).unwrap().set_cursor(6);
        dispatch_key(&mut ed, "g");
        dispatch_key(&mut ed, "g");
        assert_eq!(ed.buffers.get(slab).unwrap().cursor(), 0);
    }

    #[test]
    fn zt_scrolls_to_top() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("a\nb\nc\nd");
        janet_bridge::init(&mut ed);
        // Just verify it doesn't crash; the scroll command changes window scroll state
        dispatch_key(&mut ed, "z");
        dispatch_key(&mut ed, "t");
    }
}
