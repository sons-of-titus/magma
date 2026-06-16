use crate::kernel::scripting;

// ── editor/set-mode ───────────────────────────────────────────────────

#[test]
fn set_mode_updates_editor_mode_name() {
    janet_test!(ed, {
        scripting::eval(
            r#"(editor/set-mode "insert" {:accepts-text true})"#);
        assert_eq!(ed.editor_mode.name, "insert");
        assert!(ed.editor_mode.accepts_text);
    });
}

#[test]
fn set_mode_emits_mode_changed_event() {
    janet_test!(ed, {
        scripting::eval(r#"
            (event/on "mode-changed"
              (fn [d] (plugin-state/set "last-mode-to" (get d :to ""))))"#);
        scripting::eval(r#"(editor/set-mode "visual" {:accepts-text false})"#);
        ed.events.drain_and_dispatch();
        assert_eq!(ed.plugin_state.get("last-mode-to").map(|s| s.as_str()), Some("visual"));
    });
}

#[test]
fn mode_name_getter() {
    janet_test!(ed, {
        scripting::eval(r#"(editor/set-mode "replace" {:accepts-text true})"#);
        assert_eq!(ed.editor_mode.name, "replace");
    });
}

#[test]
fn mode_accepts_text_getter() {
    janet_test!(ed, {
        scripting::eval(r#"(editor/set-mode "insert" {:accepts-text true})"#);
        assert!(ed.editor_mode.accepts_text);
        scripting::eval(r#"(editor/set-mode "normal" {:accepts-text false})"#);
        assert!(!ed.editor_mode.accepts_text);
    });
}

// ── editor/minibuffer-* ───────────────────────────────────────────────

#[test]
fn minibuffer_open_and_close() {
    janet_test!(ed, {
        scripting::eval(r#"(minibuffer/open ":" "command")"#);
        assert!(ed.editor_mode.minibuffer.is_some());
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().prompt, ":");
        scripting::eval("(minibuffer/close)");
        assert!(ed.editor_mode.minibuffer.is_none());
    });
}

#[test]
fn minibuffer_set_input() {
    janet_test!(ed, {
        scripting::eval(r#"(minibuffer/open "/")"#);
        scripting::eval(r#"(minibuffer/set-input "hello")"#);
        assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().input, "hello");
    });
}

#[test]
fn minibuffer_kind_stored_in_plugin_state() {
    janet_test!(ed, {
        scripting::eval(r#"(minibuffer/open "find: " "fuzzy-file")"#);
        assert_eq!(
            ed.plugin_state.get("minibuffer.kind").map(|s| s.as_str()),
            Some("fuzzy-file"),
        );
    });
}

// ── editor/selection-* ────────────────────────────────────────────────

#[test]
fn selection_set_and_clear() {
    janet_test!(ed, {
        scripting::eval("(selection/set 10 \"char\")");
        assert!(ed.selection.is_some());
        assert_eq!(ed.selection.as_ref().unwrap().anchor, 10);
        scripting::eval("(selection/clear)");
        assert!(ed.selection.is_none());
    });
}

#[test]
fn selection_line_kind() {
    janet_test!(ed, {
        scripting::eval("(selection/set 5 \"line\")");
        let sel = ed.selection.as_ref().unwrap();
        assert_eq!(sel.kind, "line");
        assert!(sel.is_line());
    });
}

// ── editor/plugin-state-* ─────────────────────────────────────────────

#[test]
fn plugin_state_get_set_del() {
    janet_test!(ed, {
        scripting::eval(r#"(plugin-state/set "vim.count" "5")"#);
        assert_eq!(ed.plugin_state.get("vim.count").map(|s| s.as_str()), Some("5"));
        scripting::eval(r#"(plugin-state/del "vim.count")"#);
        assert!(ed.plugin_state.get("vim.count").is_none());
    });
}

#[test]
fn plugin_state_namespacing() {
    janet_test!(ed, {
        scripting::eval(r#"(plugin-state/set "vim.op" "d")"#);
        scripting::eval(r#"(plugin-state/set "helix.op" "c")"#);
        assert_eq!(ed.plugin_state.get("vim.op").map(|s| s.as_str()), Some("d"));
        assert_eq!(ed.plugin_state.get("helix.op").map(|s| s.as_str()), Some("c"));
    });
}

// ── vim.janet mode sync ───────────────────────────────────────────────

#[test]
fn vim_janet_sets_editor_mode_on_init() {
    janet_test!(ed, {
        ed.events.drain_and_dispatch();
        assert_eq!(ed.editor_mode.name, "normal");
    });
}
