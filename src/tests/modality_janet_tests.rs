//! Janet API tests for the modality abstraction (Sprint 11f).

#[cfg(feature = "janet")]
mod tests {
    use crate::state::Editor;
    use crate::state::id::BufferId;
    use crate::buffer::Buffer;
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::janet_bridge;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let id = ed.allocate_buffer_id();
        let buf = Buffer::new(BufferId(id), "test");
        let entry = ed.buffers.vacant_entry();
        let key = entry.key();
        entry.insert(buf);
        if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
        ed
    }

    // ── editor/set-mode ───────────────────────────────────────────────────

    #[test]
    fn set_mode_updates_editor_mode_name() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed,
            r#"(editor/set-mode "insert" {:accepts-text true})"#);
        assert_eq!(ed.editor_mode.name, "insert");
        assert!(ed.editor_mode.accepts_text);
    }

    #[test]
    fn set_mode_emits_mode_changed_event() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // Register a handler that sets plugin state on mode-changed
        janet_bridge::eval(&mut ed, r#"
            (event/on "mode-changed"
              (fn [d] (plugin-state/set "last-mode-to" (get d :to ""))))"#);
        janet_bridge::eval(&mut ed, r#"(editor/set-mode "visual" {:accepts-text false})"#);
        ed.events.drain_and_dispatch();
        assert_eq!(ed.plugin_state.get("last-mode-to").map(|s| s.as_str()), Some("visual"));
    }

    #[test]
    fn mode_name_getter() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(editor/set-mode "replace" {:accepts-text true})"#);
        assert_eq!(ed.editor_mode.name, "replace");
    }

    #[test]
    fn mode_accepts_text_getter() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(editor/set-mode "insert" {:accepts-text true})"#);
        assert!(ed.editor_mode.accepts_text);
        janet_bridge::eval(&mut ed, r#"(editor/set-mode "normal" {:accepts-text false})"#);
        assert!(!ed.editor_mode.accepts_text);
    }

    // ── editor/minibuffer-* ───────────────────────────────────────────────

    #[test]
    fn minibuffer_open_and_close() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(minibuffer/open ":" "command")"#);
        assert!(ed.editor_mode.minibuffer.is_some());
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().prompt, ":");
        janet_bridge::eval(&mut ed, "(minibuffer/close)");
        assert!(ed.editor_mode.minibuffer.is_none());
    }

    #[test]
    fn minibuffer_set_input() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(minibuffer/open "/")"#);
        janet_bridge::eval(&mut ed, r#"(minibuffer/set-input "hello")"#);
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().input, "hello");
    }

    #[test]
    fn minibuffer_kind_stored_in_plugin_state() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(minibuffer/open "find: " "fuzzy-file")"#);
        assert_eq!(
            ed.plugin_state.get("minibuffer.kind").map(|s| s.as_str()),
            Some("fuzzy-file"),
        );
    }

    // ── editor/selection-* ────────────────────────────────────────────────

    #[test]
    fn selection_set_and_clear() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, "(selection/set 10 \"char\")");
        assert!(ed.selection.is_some());
        assert_eq!(ed.selection.as_ref().unwrap().anchor, 10);
        janet_bridge::eval(&mut ed, "(selection/clear)");
        assert!(ed.selection.is_none());
    }

    #[test]
    fn selection_line_kind() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, "(selection/set 5 \"line\")");
        let sel = ed.selection.as_ref().unwrap();
        assert_eq!(sel.kind, "line");
        assert!(sel.is_line());
    }

    // ── editor/plugin-state-* ─────────────────────────────────────────────

    #[test]
    fn plugin_state_get_set_del() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(plugin-state/set "vim.count" "5")"#);
        assert_eq!(ed.plugin_state.get("vim.count").map(|s| s.as_str()), Some("5"));
        janet_bridge::eval(&mut ed, r#"(plugin-state/del "vim.count")"#);
        assert!(ed.plugin_state.get("vim.count").is_none());
    }

    #[test]
    fn plugin_state_namespacing() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(plugin-state/set "vim.op" "d")"#);
        janet_bridge::eval(&mut ed, r#"(plugin-state/set "helix.op" "c")"#);
        assert_eq!(ed.plugin_state.get("vim.op").map(|s| s.as_str()), Some("d"));
        assert_eq!(ed.plugin_state.get("helix.op").map(|s| s.as_str()), Some("c"));
    }

    // ── vim.janet mode sync ───────────────────────────────────────────────

    #[test]
    fn vim_janet_sets_editor_mode_on_init() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // editor-ready event fires on init; vim.janet sets mode to "normal"
        ed.events.drain_and_dispatch();
        assert_eq!(ed.editor_mode.name, "normal");
    }
}
