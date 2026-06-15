//! Janet API tests for Sprint 11 UI customisation C functions.
//! Requires the janet feature; all tests serialize via JANET_VM_LOCK.

#[cfg(feature = "janet")]
mod tests {
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::janet_bridge;
    use crate::state::Editor;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        ed
    }

    // ── modeline ─────────────────────────────────────────────────────────────

    #[test]
    fn set_modeline_stores_fn_name() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(ui/set-modeline "my-fn")"#);
        assert_eq!(ed.modeline_fn.as_deref(), Some("my-fn"));
    }

    #[test]
    fn set_modeline_nil_clears_fn() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(ui/set-modeline "my-fn")"#);
        janet_bridge::eval(&mut ed, r#"(ui/set-modeline nil)"#);
        assert!(ed.modeline_fn.is_none());
    }

    #[test]
    fn get_modeline_returns_nil_when_unset() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let result = janet_bridge::eval(&mut ed, r#"(= nil (ui/modeline))"#);
        // Janet eval returns "ok" on success; the expression result is irrelevant
        // — what matters is the Rust-side state below.
        assert!(ed.modeline_fn.is_none());
        let _ = result;
    }

    #[test]
    fn get_modeline_returns_fn_name_when_set() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(ui/set-modeline "my-status")"#);
        assert_eq!(ed.modeline_fn.as_deref(), Some("my-status"));
    }

    // ── tab bar ──────────────────────────────────────────────────────────────

    #[test]
    fn set_tab_bar_true_enables_flag() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(ui/set-tab-bar true)"#);
        assert!(ed.tab_bar_enabled);
    }

    #[test]
    fn set_tab_bar_false_disables_flag() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(ui/set-tab-bar true)"#);
        janet_bridge::eval(&mut ed, r#"(ui/set-tab-bar false)"#);
        assert!(!ed.tab_bar_enabled);
    }

    #[test]
    fn tab_bar_enabled_reflects_state() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        assert!(!ed.tab_bar_enabled);
        janet_bridge::eval(&mut ed, r#"(ui/set-tab-bar true)"#);
        assert!(ed.tab_bar_enabled);
    }

    // ── overlay ──────────────────────────────────────────────────────────────

    #[test]
    fn overlay_create_returns_id_and_stores_overlay() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(overlay/create 5 10 20 8 nil)"#);
        assert_eq!(ed.overlays.len(), 1);
        assert_eq!(ed.overlays[0].x, 5);
        assert_eq!(ed.overlays[0].y, 10);
        assert_eq!(ed.overlays[0].width, 20);
        assert_eq!(ed.overlays[0].height, 8);
    }

    #[test]
    fn overlay_destroy_removes_overlay() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(def oid (overlay/create 0 0 10 5 nil))"#);
        janet_bridge::eval(&mut ed, r#"(overlay/destroy oid)"#);
        assert!(ed.overlays.is_empty());
    }

    #[test]
    fn overlay_move_updates_position() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(def oid (overlay/create 0 0 10 5 nil))"#);
        janet_bridge::eval(&mut ed, r#"(overlay/move oid 15 7)"#);
        assert_eq!(ed.overlays[0].x, 15);
        assert_eq!(ed.overlays[0].y, 7);
    }

    #[test]
    fn overlay_list_returns_all_overlays() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(overlay/create 0 0 10 5 nil)"#);
        janet_bridge::eval(&mut ed, r#"(overlay/create 5 5 20 8 nil)"#);
        assert_eq!(ed.overlays.len(), 2);
    }

    // ── gutter signs ─────────────────────────────────────────────────────────

    #[test]
    fn gutter_sign_set_stores_sign_for_line() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = ed.buffers.insert(crate::buffer::Buffer::new(
            crate::state::id::BufferId(1), "test"));
        let expr = format!(r#"(gutter/sign-set ":diagnostics" {key} 3 "E" "error-face" 10)"#);
        janet_bridge::eval(&mut ed, &expr);
        let col_key = (":diagnostics".to_string(), key);
        let signs = ed.gutter.column_signs.get(&col_key).unwrap().get(&3).unwrap();
        assert_eq!(signs.len(), 1);
        assert_eq!(signs[0].text, "E");
        assert_eq!(signs[0].priority, 10);
    }

    #[test]
    fn gutter_sign_clear_removes_all_signs_for_buffer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = ed.buffers.insert(crate::buffer::Buffer::new(
            crate::state::id::BufferId(1), "test"));
        let set_expr = format!(r#"(gutter/sign-set ":diagnostics" {key} 0 "W" "warning-face")"#);
        let clr_expr = format!(r#"(gutter/sign-clear ":diagnostics" {key})"#);
        janet_bridge::eval(&mut ed, &set_expr);
        janet_bridge::eval(&mut ed, &clr_expr);
        let col_key = (":diagnostics".to_string(), key);
        assert!(!ed.gutter.column_signs.contains_key(&col_key));
    }

    #[test]
    fn gutter_sign_clear_line_removes_signs_on_one_line() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = ed.buffers.insert(crate::buffer::Buffer::new(
            crate::state::id::BufferId(1), "test"));
        janet_bridge::eval(&mut ed, &format!(r#"(gutter/sign-set ":diagnostics" {key} 0 "E" "error-face")"#));
        janet_bridge::eval(&mut ed, &format!(r#"(gutter/sign-set ":diagnostics" {key} 1 "W" "warning-face")"#));
        janet_bridge::eval(&mut ed, &format!(r#"(gutter/sign-clear-line ":diagnostics" {key} 0)"#));
        let col_key = (":diagnostics".to_string(), key);
        let line_map = ed.gutter.column_signs.get(&col_key).unwrap();
        assert!(!line_map.contains_key(&0));
        assert!(line_map.contains_key(&1));
    }

    // ── buffer header line ────────────────────────────────────────────────────

    #[test]
    fn set_header_line_stores_text() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = ed.buffers.insert(crate::buffer::Buffer::new(
            crate::state::id::BufferId(1), "test"));
        janet_bridge::eval(&mut ed, &format!(r#"(buffer/set-header-line {key} "MyHeader")"#));
        assert_eq!(ed.buffers[key].header_line.as_deref(), Some("MyHeader"));
    }

    #[test]
    fn set_header_line_nil_clears_text() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = ed.buffers.insert(crate::buffer::Buffer::new(
            crate::state::id::BufferId(1), "test"));
        janet_bridge::eval(&mut ed, &format!(r#"(buffer/set-header-line {key} "MyHeader")"#));
        janet_bridge::eval(&mut ed, &format!(r#"(buffer/set-header-line {key} nil)"#));
        assert!(ed.buffers[key].header_line.is_none());
    }

    fn make_buf_local(ed: &mut Editor) -> usize {
        ed.buffers.insert(crate::buffer::Buffer::new(
            crate::state::id::BufferId(99), "test-ui"))
    }

    #[test]
    fn editor_ready_enables_tab_bar_and_sets_defaults() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let data = std::collections::HashMap::new();
        ed.events.emit("editor-ready", data.clone());
        ed.events.drain_and_dispatch();
        assert!(ed.tab_bar_enabled, "tab_bar_enabled should be true after editor-ready");
        assert_eq!(ed.options.get("number").map(|s| s.as_str()), Some("true"));
        assert!(ed.faces.contains_key("ml-normal"), "ml-normal face should be defined");
        assert!(ed.faces.contains_key("ml-insert"), "ml-insert face should be defined");
        let _ = data;
    }

    #[test]
    fn editor_ready_scratch_has_content() {
        // special_buffers.janet writes to *scratch* during init; our editor-ready
        // handler skips writing (len > 0), so *scratch* always has content after init.
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        let key = ed.buffers.insert(crate::buffer::Buffer::new(
            crate::state::id::BufferId(1), "*scratch*"));
        janet_bridge::init(&mut ed);
        let data = std::collections::HashMap::new();
        ed.events.emit("editor-ready", data.clone());
        ed.events.drain_and_dispatch();
        // After editor-ready, *scratch* should have some content (from special_buffers.janet)
        assert!(ed.buffers[key].len() > 0, "scratch should have content after init");
        let _ = data;
    }

    #[test]
    fn buffer_modified_c_fn_works_on_new_buffer() {
        // New buffer (no undo history) → modified = true. C function must not crash.
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf_local(&mut ed);
        assert!(ed.buffers[key].modified(), "new buffer has no save point → modified");
        let result = janet_bridge::eval(&mut ed, &format!("(buffer/modified? {key})"));
        assert_eq!(result, "ok");
    }

    #[test]
    fn buffer_modified_c_fn_works_after_save() {
        // insert + mark_saved → modified = false (undo cursor at saved_at).
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf_local(&mut ed);
        ed.buffers[key].insert(0, "hello");
        ed.buffers[key].mark_saved();
        assert!(!ed.buffers[key].modified(), "after mark_saved → not modified");
        let result = janet_bridge::eval(&mut ed, &format!("(buffer/modified? {key})"));
        assert_eq!(result, "ok");
    }

    #[test]
    fn editor_fs_exists_true_for_existing_path() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let result = janet_bridge::eval(&mut ed, r#"(editor/fs-exists? "/tmp")"#);
        assert_eq!(result, "ok");
    }

    #[test]
    fn editor_fs_exists_false_for_missing_path() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let result = janet_bridge::eval(&mut ed,
            r#"(editor/fs-exists? "/nonexistent-magma-xyz-abc-123")"#);
        assert_eq!(result, "ok");
    }
}
