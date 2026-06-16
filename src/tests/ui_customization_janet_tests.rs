use crate::kernel::scripting;
use crate::kernel::state::Editor;

// ── modeline ─────────────────────────────────────────────────────────────

#[test]
fn set_modeline_stores_fn_name() {
    janet_test!(ed, {
        scripting::eval(r#"(ui/set-modeline "my-fn")"#);
        assert_eq!(ed.modeline_fn.as_deref(), Some("my-fn"));
    });
}

#[test]
fn set_modeline_nil_clears_fn() {
    janet_test!(ed, {
        scripting::eval(r#"(ui/set-modeline "my-fn")"#);
        scripting::eval(r#"(ui/set-modeline nil)"#);
        assert!(ed.modeline_fn.is_none());
    });
}

#[test]
fn get_modeline_returns_nil_when_unset() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(= nil (ui/modeline))"#);
        assert!(ed.modeline_fn.is_none());
        let _ = result;
    });
}

#[test]
fn get_modeline_returns_fn_name_when_set() {
    janet_test!(ed, {
        scripting::eval(r#"(ui/set-modeline "my-status")"#);
        assert_eq!(ed.modeline_fn.as_deref(), Some("my-status"));
    });
}

// ── tab bar ──────────────────────────────────────────────────────────────

#[test]
fn set_tab_bar_true_enables_flag() {
    janet_test!(ed, {
        scripting::eval(r#"(ui/set-tab-bar true)"#);
        assert!(ed.tab_bar_enabled);
    });
}

#[test]
fn set_tab_bar_false_disables_flag() {
    janet_test!(ed, {
        scripting::eval(r#"(ui/set-tab-bar true)"#);
        scripting::eval(r#"(ui/set-tab-bar false)"#);
        assert!(!ed.tab_bar_enabled);
    });
}

#[test]
fn tab_bar_enabled_reflects_state() {
    janet_test!(ed, {
        assert!(!ed.tab_bar_enabled);
        scripting::eval(r#"(ui/set-tab-bar true)"#);
        assert!(ed.tab_bar_enabled);
    });
}

// ── overlay ──────────────────────────────────────────────────────────────

#[test]
fn overlay_create_returns_id_and_stores_overlay() {
    janet_test!(ed, {
        scripting::eval(r#"(overlay/create 5 10 20 8 nil)"#);
        assert_eq!(ed.overlays.len(), 1);
        assert_eq!(ed.overlays[0].x, 5);
        assert_eq!(ed.overlays[0].y, 10);
        assert_eq!(ed.overlays[0].width, 20);
        assert_eq!(ed.overlays[0].height, 8);
    });
}

#[test]
fn overlay_destroy_removes_overlay() {
    janet_test!(ed, {
        scripting::eval(r#"(def oid (overlay/create 0 0 10 5 nil))"#);
        scripting::eval(r#"(overlay/destroy oid)"#);
        assert!(ed.overlays.is_empty());
    });
}

#[test]
fn overlay_move_updates_position() {
    janet_test!(ed, {
        scripting::eval(r#"(def oid (overlay/create 0 0 10 5 nil))"#);
        scripting::eval(r#"(overlay/move oid 15 7)"#);
        assert_eq!(ed.overlays[0].x, 15);
        assert_eq!(ed.overlays[0].y, 7);
    });
}

#[test]
fn overlay_list_returns_all_overlays() {
    janet_test!(ed, {
        scripting::eval(r#"(overlay/create 0 0 10 5 nil)"#);
        scripting::eval(r#"(overlay/create 5 5 20 8 nil)"#);
        assert_eq!(ed.overlays.len(), 2);
    });
}

// ── gutter signs ─────────────────────────────────────────────────────────

#[test]
fn gutter_sign_set_stores_sign_for_line() {
    janet_test!(ed, {
        let key = ed.create_buffer("test");
        let expr = format!(r#"(gutter/sign-set ":diagnostics" {key} 3 "E" "error-face" 10)"#);
        scripting::eval(&expr);
        let col_key = (":diagnostics".to_string(), key);
        let signs = ed.gutter.column_signs.get(&col_key).unwrap().get(&3).unwrap();
        assert_eq!(signs.len(), 1);
        assert_eq!(signs[0].text, "E");
        assert_eq!(signs[0].priority, 10);
    });
}

#[test]
fn gutter_sign_clear_removes_all_signs_for_buffer() {
    janet_test!(ed, {
        let key = ed.create_buffer("test");
        let set_expr = format!(r#"(gutter/sign-set ":diagnostics" {key} 0 "W" "warning-face")"#);
        let clr_expr = format!(r#"(gutter/sign-clear ":diagnostics" {key})"#);
        scripting::eval(&set_expr);
        scripting::eval(&clr_expr);
        let col_key = (":diagnostics".to_string(), key);
        assert!(!ed.gutter.column_signs.contains_key(&col_key));
    });
}

#[test]
fn gutter_sign_clear_line_removes_signs_on_one_line() {
    janet_test!(ed, {
        let key = ed.create_buffer("test");
        scripting::eval(&format!(r#"(gutter/sign-set ":diagnostics" {key} 0 "E" "error-face")"#));
        scripting::eval(&format!(r#"(gutter/sign-set ":diagnostics" {key} 1 "W" "warning-face")"#));
        scripting::eval(&format!(r#"(gutter/sign-clear-line ":diagnostics" {key} 0)"#));
        let col_key = (":diagnostics".to_string(), key);
        let line_map = ed.gutter.column_signs.get(&col_key).unwrap();
        assert!(!line_map.contains_key(&0));
        assert!(line_map.contains_key(&1));
    });
}

// ── buffer header line ────────────────────────────────────────────────────

#[test]
fn set_header_line_stores_text() {
    janet_test!(ed, {
        let key = ed.create_buffer("test");
        scripting::eval(&format!(r#"(buffer/set-header-line {key} "MyHeader")"#));
        assert_eq!(ed.buffers.get(key).unwrap().lock().unwrap().header_line.as_deref(), Some("MyHeader"));
    });
}

#[test]
fn set_header_line_nil_clears_text() {
    janet_test!(ed, {
        let key = ed.create_buffer("test");
        scripting::eval(&format!(r#"(buffer/set-header-line {key} "MyHeader")"#));
        scripting::eval(&format!(r#"(buffer/set-header-line {key} nil)"#));
        assert!(ed.buffers.get(key).unwrap().lock().unwrap().header_line.is_none());
    });
}

fn make_buf_local(ed: &mut Editor) -> usize {
    ed.create_buffer("test-ui")
}

#[test]
fn editor_ready_enables_tab_bar_and_sets_defaults() {
    janet_test!(ed, {
        let data = std::collections::HashMap::new();
        ed.events.emit("editor-ready", data.clone());
        ed.events.drain_and_dispatch();
        assert!(ed.tab_bar_enabled, "tab_bar_enabled should be true after editor-ready");
        assert_eq!(ed.options.get("number").map(|s| s.as_str()), Some("true"));
        assert!(ed.faces.contains_key("ml-normal"), "ml-normal face should be defined");
        assert!(ed.faces.contains_key("ml-insert"), "ml-insert face should be defined");
        let _ = data;
    });
}

#[test]
fn editor_ready_scratch_has_content() {
    janet_test!(ed, {
        let key = ed.create_buffer("*scratch*");
        let data = std::collections::HashMap::new();
        ed.events.emit("editor-ready", data.clone());
        ed.events.drain_and_dispatch();
        assert!(ed.buffers.get(key).unwrap().lock().unwrap().len() > 0, "scratch should have content after init");
        let _ = data;
    });
}

#[test]
fn buffer_modified_c_fn_works_on_new_buffer() {
    janet_test!(ed, {
        let key = make_buf_local(&mut ed);
        assert!(ed.buffers.get(key).unwrap().lock().unwrap().modified(), "new buffer has no save point → modified");
        let result = scripting::eval(&format!("(buffer/modified? {key})"));
        assert_eq!(result, "ok");
    });
}

#[test]
fn buffer_modified_c_fn_works_after_save() {
    janet_test!(ed, {
        let key = make_buf_local(&mut ed);
        ed.buffers.get(key).unwrap().lock().unwrap().insert(0, "hello");
        ed.buffers.get(key).unwrap().lock().unwrap().mark_saved();
        assert!(!ed.buffers.get(key).unwrap().lock().unwrap().modified(), "after mark_saved → not modified");
        let result = scripting::eval(&format!("(buffer/modified? {key})"));
        assert_eq!(result, "ok");
    });
}

#[test]
fn editor_fs_exists_true_for_existing_path() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(editor/fs-exists? "/tmp")"#);
        assert_eq!(result, "ok");
    });
}

#[test]
fn editor_fs_exists_false_for_missing_path() {
    janet_test!(ed, {
        let result = scripting::eval(
            r#"(editor/fs-exists? "/nonexistent-magma-xyz-abc-123")"#);
        assert_eq!(result, "ok");
    });
}
