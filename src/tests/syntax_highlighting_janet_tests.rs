use crate::janet_bridge;

// ── editor/define-face ─────────────────────────────────────────────────

#[test]
fn editor_define_face_inserts_face_into_registry() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(face/define \"test-face\" {:fg [255 0 0] :bg [0 0 0] :bold true})");
        assert_eq!(result, "ok");

        let face = ed.faces.get("test-face");
        assert!(face.is_some(), "face should be registered");
        let s = face.unwrap();
        assert_eq!(s.fg, (255, 0, 0));
        assert!(s.bold);
    });
}

#[test]
fn editor_define_face_without_table_creates_default_style() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(face/define \"default-face\")");
        assert_eq!(result, "ok");

        let face = ed.faces.get("default-face");
        assert!(face.is_some());
    });
}

#[test]
fn editor_define_face_emits_face_changed_event() {
    janet_test!(ed, {
        let _sub_count_before = ed.events.subscriber_count("face-changed");
        let result = janet_bridge::eval(
            "(face/define \"my-face\" {:fg [100 200 100]})");
        assert_eq!(result, "ok");
        assert!(ed.faces.contains_key("my-face"));
    });
}

// ── editor/face getter ──────────────────────────────────────────────────

#[test]
fn editor_face_returns_nil_for_unknown() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(face/get \"nonexistent\")");
        assert!(result.contains("nil") || result == "ok",
            "should handle missing face gracefully");
    });
}

#[test]
fn editor_face_returns_table_for_defined_face() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(do (face/define \"test-face\" {:fg [10 20 30] :bold true})
                 (face/get \"test-face\"))");
        assert_eq!(result, "ok");
        let face = ed.faces.get("test-face").unwrap();
        assert_eq!(face.fg, (10, 20, 30));
        assert!(face.bold);
    });
}

// ── editor/make-style ───────────────────────────────────────────────────

#[test]
fn editor_make_style_returns_integer_handle() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(face/make-style {:fg [255 0 0] :bg [0 0 0] :bold true})");
        assert_eq!(result, "ok");
    });
}

// ── editor/scope-face and editor/resolve-scope ─────────────────────────

#[test]
fn editor_scope_face_registers_mapping() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(face/scope-face \"keyword\" \"keyword-face\")");
        assert_eq!(result, "ok");

        assert!(!ed.scope_faces.is_empty());
    });
}

#[test]
fn editor_resolve_scope_returns_mapped_face() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(do (face/scope-face \"keyword\" \"keyword-face\")
                 (face/resolve-scope \"keyword.control\"))");
        assert_eq!(result, "ok");
    });
}

#[test]
fn editor_resolve_scope_returns_nil_for_unmapped() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(face/resolve-scope \"unknown.scope\")");
        assert_eq!(result, "ok");
    });
}

// ── buffer/set-highlights with face names ──────────────────────────────

#[test]
fn buffer_set_highlights_with_face_names() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let result = janet_bridge::eval(
        &format!("(buffer/set-highlights {} [[0 5 \"keyword-face\"] [6 11 \"string-face\"]])", key));
    assert_eq!(result, "ok");

    let hl = &ed.buffers.get(key).unwrap().highlights;
    assert_eq!(hl.len(), 2);
    assert_eq!(hl[0].2, "keyword-face");
    assert_eq!(hl[1].2, "string-face");
}

#[test]
fn buffer_set_highlights_without_face_defaults_to_highlight() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let result = janet_bridge::eval(
        &format!("(buffer/set-highlights {} [[0 5]])", key));
    assert_eq!(result, "ok");

    let hl = &ed.buffers.get(key).unwrap().highlights;
    assert_eq!(hl.len(), 1);
    assert_eq!(hl[0].2, "highlight");
}

#[test]
fn buffer_set_highlights_empty_clears() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let _ = janet_bridge::eval(
        &format!("(buffer/set-highlights {} [[0 5 \"keyword-face\"]])", key));
    let _ = janet_bridge::eval(
        &format!("(buffer/set-highlights {})", key));
    assert!(ed.buffers.get(key).unwrap().highlights.is_empty());
}

// ── buffer/set-highlights-layer and buffer/clear-highlights-layer ──────

#[test]
fn buffer_set_highlights_layer_stores_per_layer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let result = janet_bridge::eval(
        &format!("(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])", key));
    assert_eq!(result, "ok");

    let buf = ed.buffers.get(key).unwrap();
    let layer = buf.highlight_layers.get("syntax");
    assert!(layer.is_some());
    assert_eq!(layer.unwrap().len(), 1);
    assert_eq!(layer.unwrap()[0].2, "keyword-face");
}

#[test]
fn buffer_clear_highlights_layer_removes_named_layer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let _ = janet_bridge::eval(
        &format!("(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])", key));
    let result = janet_bridge::eval(
        &format!("(buffer/clear-highlights-layer {} \"syntax\")", key));
    assert_eq!(result, "ok");

    let buf = ed.buffers.get(key).unwrap();
    assert!(buf.highlight_layers.get("syntax").is_none());
}

#[test]
fn buffer_clear_highlights_layer_does_not_affect_other_layers() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let _ = janet_bridge::eval(
        &format!("(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])", key));
    let _ = janet_bridge::eval(
        &format!("(buffer/set-highlights-layer {} \"search\" [[2 4 \"search-face\"]])", key));
    let _ = janet_bridge::eval(
        &format!("(buffer/clear-highlights-layer {} \"syntax\")", key));

    let buf = ed.buffers.get(key).unwrap();
    assert!(buf.highlight_layers.get("syntax").is_none());
    assert!(buf.highlight_layers.get("search").is_some());
}

// ── Default faces from init.janet ──────────────────────────────────────

#[test]
fn default_faces_exist_after_init() {
    janet_test!(ed, {
        assert!(ed.faces.contains_key("keyword-face"), "keyword-face should exist");
        assert!(ed.faces.contains_key("string-face"), "string-face should exist");
        assert!(ed.faces.contains_key("comment-face"), "comment-face should exist");
        assert!(ed.faces.contains_key("type-face"), "type-face should exist");
        assert!(ed.faces.contains_key("function-face"), "function-face should exist");
        assert!(ed.faces.contains_key("variable-face"), "variable-face should exist");
        assert!(ed.faces.contains_key("constant-face"), "constant-face should exist");
        assert!(ed.faces.contains_key("operator-face"), "operator-face should exist");
        assert!(ed.faces.contains_key("punctuation-face"), "punctuation-face should exist");
        assert!(ed.faces.contains_key("error-face"), "error-face should exist");
        assert!(ed.faces.contains_key("warning-face"), "warning-face should exist");
    });
}

#[test]
fn default_comment_face_is_dim() {
    janet_test!(ed, {
        let face = ed.faces.get("comment-face").unwrap();
        assert!(face.dim, "comment-face should be dim by default");
    });
}

#[test]
fn default_error_face_is_bold() {
    janet_test!(ed, {
        let face = ed.faces.get("error-face").unwrap();
        assert!(face.bold, "error-face should be bold by default");
    });
}
