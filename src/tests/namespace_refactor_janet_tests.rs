use crate::janet_bridge;

// ── minibuffer/ ───────────────────────────────────────────────────────────

#[test]
fn minibuffer_open_and_close() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(minibuffer/open ">" "test")"#);
        assert!(ed.editor_mode.minibuffer.is_some());
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().prompt, ">");
        janet_bridge::eval("(minibuffer/close)");
        assert!(ed.editor_mode.minibuffer.is_none());
    });
}

#[test]
fn minibuffer_set_input_updates_state() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(minibuffer/open "/")"#);
        janet_bridge::eval(r#"(minibuffer/set-input "pattern")"#);
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().input, "pattern");
    });
}

// ── selection/ ───────────────────────────────────────────────────────────

#[test]
fn selection_set_and_clear() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(selection/set 5 "char")"#);
        assert!(ed.selection.is_some());
        assert_eq!(ed.selection.as_ref().unwrap().anchor, 5);
        janet_bridge::eval("(selection/clear)");
        assert!(ed.selection.is_none());
    });
}

#[test]
fn selection_get_returns_nil_when_no_selection() {
    janet_test!(ed, {
        let r = janet_bridge::eval("(nil? (selection/get))");
        assert_eq!(r, "ok");
    });
}

// ── register/ ────────────────────────────────────────────────────────────

#[test]
fn register_set_and_get() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(register/set "\"" "hello")"#);
        assert_eq!(ed.registers.get("\"").map(|s| s.as_str()), Some("hello"));
        let r = janet_bridge::eval(r#"(= "hello" (register/get "\""))"#);
        assert_eq!(r, "ok");
    });
}

// ── option/ ──────────────────────────────────────────────────────────────

#[test]
fn option_set_and_get() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(option/set "number" "true")"#);
        assert_eq!(ed.options.get("number").map(|s| s.as_str()), Some("true"));
        let r = janet_bridge::eval(r#"(= "true" (option/get "number"))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn option_list_returns_table() {
    janet_test!(ed, {
        let r = janet_bridge::eval("(table? (option/list))");
        assert_eq!(r, "ok");
    });
}

// ── plugin-state/ ────────────────────────────────────────────────────────

#[test]
fn plugin_state_set_get_del() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(plugin-state/set "k" "v")"#);
        assert_eq!(ed.plugin_state.get("k").map(|s| s.as_str()), Some("v"));
        janet_bridge::eval(r#"(plugin-state/del "k")"#);
        assert!(ed.plugin_state.get("k").is_none());
    });
}

// ── clipboard/ ───────────────────────────────────────────────────────────

#[test]
fn clipboard_set_and_get() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(clipboard/set "board-text")"#);
        let got = ed.clipboard.get_text();
        assert_eq!(got.as_deref(), Some("board-text"));
    });
}

// ── search/ ──────────────────────────────────────────────────────────────

#[test]
fn search_set_and_get_pattern() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(search/set-pattern "foo")"#);
        assert_eq!(ed.search_pattern.as_deref(), Some("foo"));
        let r = janet_bridge::eval(r#"(= "foo" (search/pattern))"#);
        assert_eq!(r, "ok");
    });
}

// ── face/ ────────────────────────────────────────────────────────────────

#[test]
fn face_define_and_get() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(face/define "sprint19-test" {:fg [1 2 3]})"#);
        assert!(ed.faces.contains_key("sprint19-test"));
        let r = janet_bridge::eval(r#"(table? (face/get "sprint19-test"))"#);
        assert_eq!(r, "ok");
    });
}

// ── font/ ────────────────────────────────────────────────────────────────

#[test]
fn font_set_size_and_size() {
    janet_test!(ed, {
        janet_bridge::eval("(font/set-size 18)");
        assert_eq!(ed.font_config.size, 18.0);
        let r = janet_bridge::eval("(= 18 (font/size))");
        assert_eq!(r, "ok");
    });
}

#[test]
fn font_invalidate_marks_atlas_dirty() {
    janet_test!(ed, {
        ed.font_changed = false;
        janet_bridge::eval("(font/invalidate)");
        assert!(ed.font_changed);
    });
}

// ── overlay/ ─────────────────────────────────────────────────────────────

#[test]
fn overlay_create_and_destroy() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(def id (overlay/create 0 0 10 5 nil))"#);
        assert_eq!(ed.overlays.len(), 1);
        janet_bridge::eval(r#"(overlay/destroy id)"#);
        assert!(ed.overlays.is_empty());
    });
}

// ── mark-ring/ ───────────────────────────────────────────────────────────

#[test]
fn mark_ring_push_and_pop() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(mark-ring/push "a.rs" 10)"#);
        assert_eq!(ed.mark_ring.len(), 1);
        let r = janet_bridge::eval(r#"(= "a.rs" ((mark-ring/pop) :path))"#);
        assert_eq!(r, "ok");
        assert!(ed.mark_ring.is_empty());
    });
}

// ── module/ ──────────────────────────────────────────────────────────────

#[test]
fn module_path_add_and_list() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(module/path-add "/tmp/test-plugins")"#);
        assert!(ed.module_paths.contains(&"/tmp/test-plugins".to_string()));
        let r = janet_bridge::eval("(array? (module/path))");
        assert_eq!(r, "ok");
    });
}

// ── ui/ ──────────────────────────────────────────────────────────────────

#[test]
fn ui_set_tab_bar_and_read() {
    janet_test!(ed, {
        janet_bridge::eval("(ui/set-tab-bar true)");
        assert!(ed.tab_bar_enabled);
    });
}

#[test]
fn ui_set_modeline_stores_fn_name() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(ui/set-modeline "my-fn")"#);
        assert_eq!(ed.modeline_fn.as_deref(), Some("my-fn"));
    });
}

// ── gutter/set-fold-icons ────────────────────────────────────────────────

#[test]
fn gutter_set_fold_icons_stores_values() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(gutter/set-fold-icons "v" ">" "fold-face")"#);
        assert_eq!(ed.gutter.fold_icons.open, "v");
        assert_eq!(ed.gutter.fold_icons.closed, ">");
        assert_eq!(ed.gutter.fold_icons.face, "fold-face");
    });
}
