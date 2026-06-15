//! Janet API tests for Sprint 19 namespace refactor.
//! Verifies that all new namespaces are registered and reachable.

#[cfg(feature = "janet")]
mod tests {
    use crate::buffer::Buffer;
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::janet_bridge;
    use crate::state::id::BufferId;
    use crate::state::Editor;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let id = ed.allocate_buffer_id();
        let buf = Buffer::new(BufferId(id), "test");
        let entry = ed.buffers.vacant_entry();
        let key = entry.key();
        entry.insert(buf);
        if let Some(win) = ed.windows.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        ed
    }

    // ── minibuffer/ ───────────────────────────────────────────────────────────

    #[test]
    fn minibuffer_open_and_close() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(minibuffer/open ">" "test")"#);
        assert!(ed.editor_mode.minibuffer.is_some());
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().prompt, ">");
        janet_bridge::eval(&mut ed, "(minibuffer/close)");
        assert!(ed.editor_mode.minibuffer.is_none());
    }

    #[test]
    fn minibuffer_set_input_updates_state() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(minibuffer/open "/")"#);
        janet_bridge::eval(&mut ed, r#"(minibuffer/set-input "pattern")"#);
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().input, "pattern");
    }

    // ── selection/ ───────────────────────────────────────────────────────────

    #[test]
    fn selection_set_and_clear() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(selection/set 5 "char")"#);
        assert!(ed.selection.is_some());
        assert_eq!(ed.selection.as_ref().unwrap().anchor, 5);
        janet_bridge::eval(&mut ed, "(selection/clear)");
        assert!(ed.selection.is_none());
    }

    #[test]
    fn selection_get_returns_nil_when_no_selection() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval(&mut ed, "(nil? (selection/get))");
        assert_eq!(r, "ok");
    }

    // ── register/ ────────────────────────────────────────────────────────────

    #[test]
    fn register_set_and_get() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(register/set "\"" "hello")"#);
        assert_eq!(ed.registers.get("\"").map(|s| s.as_str()), Some("hello"));
        let r = janet_bridge::eval(&mut ed, r#"(= "hello" (register/get "\""))"#);
        assert_eq!(r, "ok");
    }

    // ── option/ ──────────────────────────────────────────────────────────────

    #[test]
    fn option_set_and_get() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(option/set "number" "true")"#);
        assert_eq!(ed.options.get("number").map(|s| s.as_str()), Some("true"));
        let r = janet_bridge::eval(&mut ed, r#"(= "true" (option/get "number"))"#);
        assert_eq!(r, "ok");
    }

    #[test]
    fn option_list_returns_table() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval(&mut ed, "(table? (option/list))");
        assert_eq!(r, "ok");
    }

    // ── plugin-state/ ────────────────────────────────────────────────────────

    #[test]
    fn plugin_state_set_get_del() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(plugin-state/set "k" "v")"#);
        assert_eq!(ed.plugin_state.get("k").map(|s| s.as_str()), Some("v"));
        janet_bridge::eval(&mut ed, r#"(plugin-state/del "k")"#);
        assert!(ed.plugin_state.get("k").is_none());
    }

    // ── clipboard/ ───────────────────────────────────────────────────────────

    #[test]
    fn clipboard_set_and_get() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(clipboard/set "board-text")"#);
        let got = ed.clipboard.get_text();
        assert_eq!(got.as_deref(), Some("board-text"));
    }

    // ── search/ ──────────────────────────────────────────────────────────────

    #[test]
    fn search_set_and_get_pattern() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(search/set-pattern "foo")"#);
        assert_eq!(ed.search_pattern.as_deref(), Some("foo"));
        let r = janet_bridge::eval(&mut ed, r#"(= "foo" (search/pattern))"#);
        assert_eq!(r, "ok");
    }

    // ── face/ ────────────────────────────────────────────────────────────────

    #[test]
    fn face_define_and_get() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(face/define "sprint19-test" {:fg [1 2 3]})"#);
        assert!(ed.faces.contains_key("sprint19-test"));
        let r = janet_bridge::eval(&mut ed, r#"(table? (face/get "sprint19-test"))"#);
        assert_eq!(r, "ok");
    }

    // ── font/ ────────────────────────────────────────────────────────────────

    #[test]
    fn font_set_size_and_size() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, "(font/set-size 18)");
        assert_eq!(ed.font_config.size, 18.0);
        let r = janet_bridge::eval(&mut ed, "(= 18 (font/size))");
        assert_eq!(r, "ok");
    }

    #[test]
    fn font_invalidate_marks_atlas_dirty() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        ed.font_changed = false;
        janet_bridge::eval(&mut ed, "(font/invalidate)");
        assert!(ed.font_changed);
    }

    // ── overlay/ ─────────────────────────────────────────────────────────────

    #[test]
    fn overlay_create_and_destroy() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(def id (overlay/create 0 0 10 5 nil))"#);
        assert_eq!(ed.overlays.len(), 1);
        janet_bridge::eval(&mut ed, r#"(overlay/destroy id)"#);
        assert!(ed.overlays.is_empty());
    }

    // ── mark-ring/ ───────────────────────────────────────────────────────────

    #[test]
    fn mark_ring_push_and_pop() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(mark-ring/push "a.rs" 10)"#);
        assert_eq!(ed.mark_ring.len(), 1);
        let r = janet_bridge::eval(&mut ed, r#"(= "a.rs" ((mark-ring/pop) :path))"#);
        assert_eq!(r, "ok");
        assert!(ed.mark_ring.is_empty());
    }

    // ── module/ ──────────────────────────────────────────────────────────────

    #[test]
    fn module_path_add_and_list() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(module/path-add "/tmp/test-plugins")"#);
        assert!(ed.module_paths.contains(&"/tmp/test-plugins".to_string()));
        let r = janet_bridge::eval(&mut ed, "(array? (module/path))");
        assert_eq!(r, "ok");
    }

    // ── ui/ ──────────────────────────────────────────────────────────────────

    #[test]
    fn ui_set_tab_bar_and_read() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, "(ui/set-tab-bar true)");
        assert!(ed.tab_bar_enabled);
    }

    #[test]
    fn ui_set_modeline_stores_fn_name() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(ui/set-modeline "my-fn")"#);
        assert_eq!(ed.modeline_fn.as_deref(), Some("my-fn"));
    }

    // ── gutter/set-fold-icons ────────────────────────────────────────────────

    #[test]
    fn gutter_set_fold_icons_stores_values() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/set-fold-icons "v" ">" "fold-face")"#);
        assert_eq!(ed.gutter.fold_icons.open, "v");
        assert_eq!(ed.gutter.fold_icons.closed, ">");
        assert_eq!(ed.gutter.fold_icons.face, "fold-face");
    }
}
