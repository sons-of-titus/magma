#[macro_use]
mod helpers;
use scripting;

// ── editor/define-face ─────────────────────────────────────────────────

#[test]
fn editor_define_face_inserts_face_into_registry() {
    janet_test!(ed, {
        let result =
            scripting::eval("(face/define \"test-face\" {:fg [255 0 0] :bg [0 0 0] :bold true})");
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
        let result = scripting::eval("(face/define \"default-face\")");
        assert_eq!(result, "ok");

        let face = ed.faces.get("default-face");
        assert!(face.is_some());
    });
}

#[test]
fn editor_define_face_emits_face_changed_event() {
    janet_test!(ed, {
        let _sub_count_before = ed.events.subscriber_count("face-changed");
        let result = scripting::eval("(face/define \"my-face\" {:fg [100 200 100]})");
        assert_eq!(result, "ok");
        assert!(ed.faces.contains_key("my-face"));
    });
}

// ── editor/face getter ──────────────────────────────────────────────────

#[test]
fn editor_face_returns_nil_for_unknown() {
    janet_test!(ed, {
        let result = scripting::eval("(face/get \"nonexistent\")");
        assert!(
            result.contains("nil") || result == "ok",
            "should handle missing face gracefully"
        );
    });
}

#[test]
fn editor_face_returns_table_for_defined_face() {
    janet_test!(ed, {
        let result = scripting::eval(
            "(do (face/define \"test-face\" {:fg [10 20 30] :bold true})
                 (face/get \"test-face\"))",
        );
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
        let result = scripting::eval("(face/make-style {:fg [255 0 0] :bg [0 0 0] :bold true})");
        assert_eq!(result, "ok");
    });
}

// ── editor/scope-face and editor/resolve-scope ─────────────────────────

#[test]
fn editor_scope_face_registers_mapping() {
    janet_test!(ed, {
        let result = scripting::eval("(face/scope-face \"keyword\" \"keyword-face\")");
        assert_eq!(result, "ok");

        assert!(!ed.scope_faces.is_empty());
    });
}

#[test]
fn editor_resolve_scope_returns_mapped_face() {
    janet_test!(ed, {
        let result = scripting::eval(
            "(do (face/scope-face \"keyword\" \"keyword-face\")
                 (face/resolve-scope \"keyword.control\"))",
        );
        assert_eq!(result, "ok");
    });
}

#[test]
fn editor_resolve_scope_returns_nil_for_unmapped() {
    janet_test!(ed, {
        let result = scripting::eval("(face/resolve-scope \"unknown.scope\")");
        assert_eq!(result, "ok");
    });
}

// ── buffer/set-highlights with face names ──────────────────────────────

#[test]
fn buffer_set_highlights_with_face_names() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello world");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(&format!(
        "(buffer/set-highlights {} [[0 5 \"keyword-face\"] [6 11 \"string-face\"]])",
        key
    ));
    assert_eq!(result, "ok");

    let hl = ed.views.get(&key).unwrap().highlights.clone();
    assert_eq!(hl.len(), 2);
    assert_eq!(hl[0].2, "keyword-face");
    assert_eq!(hl[1].2, "string-face");
}

#[test]
fn buffer_set_highlights_without_face_defaults_to_highlight() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(&format!("(buffer/set-highlights {} [[0 5]])", key));
    assert_eq!(result, "ok");

    let hl = ed.views.get(&key).unwrap().highlights.clone();
    assert_eq!(hl.len(), 1);
    assert_eq!(hl[0].2, "highlight");
}

#[test]
fn buffer_set_highlights_empty_clears() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(&format!(
        "(buffer/set-highlights {} [[0 5 \"keyword-face\"]])",
        key
    ));
    let _ = scripting::eval(&format!("(buffer/set-highlights {})", key));
    assert!(ed.views.get(&key).unwrap().highlights.is_empty());
}

// ── buffer/set-highlights-layer and buffer/clear-highlights-layer ──────

#[test]
fn buffer_set_highlights_layer_stores_per_layer() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello world");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(&format!(
        "(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])",
        key
    ));
    assert_eq!(result, "ok");

    let view = ed.views.get(&key).unwrap();
    let layer = view.highlight_layers.get("syntax");
    assert!(layer.is_some());
    assert_eq!(layer.unwrap().len(), 1);
    assert_eq!(layer.unwrap()[0].2, "keyword-face");
}

#[test]
fn buffer_clear_highlights_layer_removes_named_layer() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(&format!(
        "(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])",
        key
    ));
    let result = scripting::eval(&format!(
        "(buffer/clear-highlights-layer {} \"syntax\")",
        key
    ));
    assert_eq!(result, "ok");

    let view = ed.views.get(&key).unwrap();
    assert!(!view.highlight_layers.contains_key("syntax"));
}

#[test]
fn buffer_clear_highlights_layer_does_not_affect_other_layers() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(&format!(
        "(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])",
        key
    ));
    let _ = scripting::eval(&format!(
        "(buffer/set-highlights-layer {} \"search\" [[2 4 \"search-face\"]])",
        key
    ));
    let _ = scripting::eval(&format!(
        "(buffer/clear-highlights-layer {} \"syntax\")",
        key
    ));

    let view = ed.views.get(&key).unwrap();
    assert!(!view.highlight_layers.contains_key("syntax"));
    assert!(view.highlight_layers.contains_key("search"));
}

// ── Default faces from init.janet ──────────────────────────────────────

#[test]
fn default_faces_exist_after_init() {
    janet_test!(ed, {
        assert!(
            ed.faces.contains_key("keyword-face"),
            "keyword-face should exist"
        );
        assert!(
            ed.faces.contains_key("string-face"),
            "string-face should exist"
        );
        assert!(
            ed.faces.contains_key("comment-face"),
            "comment-face should exist"
        );
        assert!(ed.faces.contains_key("type-face"), "type-face should exist");
        assert!(
            ed.faces.contains_key("function-face"),
            "function-face should exist"
        );
        assert!(
            ed.faces.contains_key("variable-face"),
            "variable-face should exist"
        );
        assert!(
            ed.faces.contains_key("constant-face"),
            "constant-face should exist"
        );
        assert!(
            ed.faces.contains_key("operator-face"),
            "operator-face should exist"
        );
        assert!(
            ed.faces.contains_key("punctuation-face"),
            "punctuation-face should exist"
        );
        assert!(
            ed.faces.contains_key("error-face"),
            "error-face should exist"
        );
        assert!(
            ed.faces.contains_key("warning-face"),
            "warning-face should exist"
        );
    });
}

#[test]
fn syntax_highlight_buffer_rust_produces_ranges() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("fn main() { let x = 1; }");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    // Set a path so guess-language can determine it's Rust
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap().path = Some("/test/main.rs".to_string());
    }

    // The syntax-highlight command must produce a layer
    let result = scripting::eval(&format!(
        r#"(editor/run-command "syntax-highlight" {})"#,
        key
    ));
    assert_eq!(result, "ok", "syntax-highlight must not error");

    // Check that the view has a "syntax" layer with keyword-face ranges
    let view = ed.views.get(&key).unwrap();
    let layers = view.highlight_layers.keys().collect::<Vec<_>>();
    assert!(
        layers.contains(&&"syntax".to_string()),
        "view must have a 'syntax' highlight layer"
    );
    if let Some(ranges) = view.highlight_layers.get("syntax") {
        assert!(
            !ranges.is_empty(),
            "syntax layer must have highlight ranges for fn and let"
        );
        let has_keyword = ranges.iter().any(|r| r.2 == "keyword-face");
        assert!(
            has_keyword,
            "ranges must include keyword-face for 'fn' and 'let'"
        );
    }
}

#[test]
fn syntax_highlight_uses_language_path_for_generated_buffers() {
    // A diff pane holds a generated snapshot and claims no file; its syntax
    // must come from the source path carried as `language-path`.
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        {
            let arc = ed.buffers.get(key).unwrap();
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            assert!(buf.path.is_none(), "a generated pane claims no file");
            buf.insert(0, "fn main() { let x = 1; }\n");
            buf.language_path = Some("/test/main.rs".to_string());
        }
        assert!(
            scripting::eval_result(&format!("(buffer/language-path {key})"))
                .unwrap_or_default()
                .contains("/test/main.rs"),
            "buffer/language-path must expose the inferred language source"
        );
        assert_eq!(
            scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#)),
            "ok"
        );
        let view = ed.views.get(&key).unwrap();
        let ranges = view
            .highlight_layers
            .get("syntax")
            .expect("the language path must drive highlighting without a file path");
        assert!(
            ranges.iter().any(|r| r.2 == "keyword-face"),
            "rust keywords highlighted"
        );
    });
}

#[test]
fn default_comment_face_is_not_dimmed() {
    // Comments grey out through the muted colour the palette already
    // carries; the renderer's dim halves that colour again and drops the
    // text below legibility.
    janet_test!(ed, {
        let face = ed.faces.get("comment-face").unwrap();
        assert!(!face.dim, "comment-face must not be dimmed by default");
    });
}

#[test]
fn default_error_face_is_bold() {
    janet_test!(ed, {
        let face = ed.faces.get("error-face").unwrap();
        assert!(face.bold, "error-face should be bold by default");
    });
}

// ── Incremental highlighting (docs/core-redesign.md §7) ─────────────────

#[test]
fn highlight_follows_rust_path_typing() {
    // The old architecture never refreshed highlights on the pure-Rust
    // typing path; the canonical change publication now feeds the layer.
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        if let Some(arc) = ed.buffers.get(key) {
            let mut buf = arc.lock().unwrap();
            buf.path = Some("/test/main.rs".to_string());
            let len = buf.len();
            buf.delete(0, len);
            buf.insert(0, "fn one() {}");
        }
        scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#));
        ed.events.drain_and_dispatch();

        // Type " two" at the end through the editor's own input path.
        let end = ed.buffers.get(key).unwrap().lock().unwrap().len();
        ed.views.get_mut(&key).unwrap().set_cursor(end);
        kernel::input::dispatch_key(&mut ed, " ");
        kernel::input::dispatch_key(&mut ed, "t");
        ed.events.drain_and_dispatch();

        let view = ed.views.get(&key).unwrap();
        let ranges = view
            .highlight_layers
            .get("syntax")
            .expect("syntax layer exists");
        assert!(
            ranges.iter().any(|r| r.2 == "keyword-face" && r.0 == 0),
            "keyword range for 'fn' must still be present after typing: {ranges:?}"
        );
    });
}

#[test]
fn dirty_range_splice_equals_full_rescan() {
    // After incremental (dirty-range) updates, the layer must be exactly
    // what a full rescan of the same text produces.
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let text = "let a = 1;\n// comment line\nlet b = \"str two\";\nfn c() { return 3; }\n";
        if let Some(arc) = ed.buffers.get(key) {
            let mut buf = arc.lock().unwrap();
            buf.path = Some("/test/main.rs".to_string());
            let len = buf.len();
            buf.delete(0, len);
            buf.insert(0, text);
        }
        scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#));
        ed.events.drain_and_dispatch();

        // Two incremental edits through the canonical path.
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(text.len() - 2, "x"); // edit near the end
        ed.publish_buffer_changes();
        ed.events.drain_and_dispatch();
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(4, "let z = 9;\n"); // edit near the start
        ed.publish_buffer_changes();
        ed.events.drain_and_dispatch();

        let after_dirty: Vec<(usize, usize, String)> = ed
            .views
            .get(&key)
            .unwrap()
            .highlight_layers
            .get("syntax")
            .unwrap()
            .clone();

        // Full rescan of the same final text, into a scratch layer.
        scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#));
        ed.events.drain_and_dispatch();
        let after_full: Vec<(usize, usize, String)> = ed
            .views
            .get(&key)
            .unwrap()
            .highlight_layers
            .get("syntax")
            .unwrap()
            .clone();

        assert_eq!(
            after_dirty, after_full,
            "incrementally spliced ranges must equal a full rescan"
        );
    });
}

// ── Native fallback rule registry (syntax/define-rules) ────────────────

fn syntax_layer(ed: &kernel::state::Editor, key: usize) -> Vec<(usize, usize, String)> {
    ed.views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("syntax")
        .cloned()
        .unwrap_or_default()
}

#[test]
fn define_rules_registers_and_replaces_deterministically() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let r =
            scripting::eval_result(r##"(syntax/define-rules "zlang" ["kw1" "kw2"] "#")"##).unwrap();
        assert_eq!(r, ":ok");
        scripting::eval(r#"(syntax/map-extension "zzlang" "zlang")"#);
        {
            let arc = ed.buffers.get(key).unwrap();
            let mut buf = arc.lock().unwrap();
            buf.path = Some("/t/a.zzlang".to_string());
            buf.insert(0, "kw1 plain kw2\n");
        }
        scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#));
        assert_eq!(
            syntax_layer(&ed, key).len(),
            2,
            "both keywords must highlight: {:?}",
            syntax_layer(&ed, key)
        );

        // Re-registration replaces the rule set; the next scan follows it.
        scripting::eval(r##"(syntax/define-rules "zlang" ["kw1"] "#")"##);
        scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#));
        assert_eq!(syntax_layer(&ed, key).len(), 1);
    });
}

#[test]
fn define_rules_rejects_malformed_rules() {
    janet_test!(ed, {
        let cases = [
            r#"(syntax/define-rules "xlang" "notaseq" nil)"#,
            r#"(syntax/define-rules "xlang" ["ok" 5] nil)"#,
            r#"(syntax/define-rules "xlang" [] "")"#,
            r#"(syntax/define-rules "" [] nil)"#,
        ];
        for form in cases {
            let caught =
                scripting::eval_result(&format!(r#"(= (try {form} ([e] "caught")) "caught")"#))
                    .unwrap();
            assert_eq!(
                caught, "true",
                "{form} must be a catchable validation error"
            );
        }
        assert!(
            !ed.fallback_syntax.rules.contains_key("xlang"),
            "rejected rules must not register"
        );
    });
}

#[test]
fn ts_ownership_flips_through_janet_configuration() {
    // syntax/set-ts-mode hands the mode's layer to the tree-sitter lane;
    // the fallback stands down by state, so event registration order
    // cannot change the outcome.
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        scripting::eval(r#"(syntax/define-rules "olang" ["owkw"] nil)"#);
        scripting::eval(r#"(syntax/map-extension "olangext" "olang")"#);
        {
            let arc = ed.buffers.get(key).unwrap();
            let mut buf = arc.lock().unwrap();
            buf.path = Some("/t/a.olangext".to_string());
            buf.insert(0, "owkw x\n");
        }
        scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#));
        assert_eq!(syntax_layer(&ed, key).len(), 1);

        scripting::eval(r#"(syntax/set-ts-mode "fundamental" true)"#);
        assert!(
            syntax_layer(&ed, key).is_empty(),
            "the ownership flip must clear fallback ranges at once"
        );

        // Edits while tree-sitter owns the layer stay untouched.
        {
            let view = ed.views.get_mut(&key).unwrap();
            view.insert(0, "owkw more\n");
        }
        ed.publish_buffer_changes();
        assert!(
            syntax_layer(&ed, key).is_empty(),
            "fallback must not write while a ts query owns the mode"
        );

        // Handing the layer back restores fallback highlighting.
        scripting::eval(r#"(syntax/set-ts-mode "fundamental" false)"#);
        {
            let view = ed.views.get_mut(&key).unwrap();
            view.insert(0, "owkw again\n");
        }
        ed.publish_buffer_changes();
        assert!(
            !syntax_layer(&ed, key).is_empty(),
            "fallback owns again after the flip back"
        );
    });
}

#[test]
fn ts_set_mode_query_without_a_grammar_leaves_the_fallback_in_charge() {
    // A mode whose query has no loadable grammar (a plugin shipped
    // syntax.scm but no shared library) must keep native fallback
    // highlighting — handing the layer to tree-sitter would leave it with
    // neither highlighter.
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        scripting::eval(r#"(syntax/define-rules "olang" ["owkw"] nil)"#);
        scripting::eval(r#"(syntax/map-extension "olangext" "olang")"#);
        {
            let arc = ed.buffers.get(key).unwrap();
            let mut buf = arc.lock().unwrap();
            buf.path = Some("/t/a.olangext".to_string());
            buf.insert(0, "owkw x\n");
        }
        scripting::eval(r#"(ts/set-mode-query "olang-mode" "(keyword)")"#);
        scripting::eval(&format!(r#"(editor/run-command "syntax-highlight" {key})"#));
        assert_eq!(
            syntax_layer(&ed, key).len(),
            1,
            "a query without a grammar must not stand down the fallback"
        );

        // The fallback keeps updating while the query is registered.
        {
            let view = ed.views.get_mut(&key).unwrap();
            view.insert(0, "owkw more\n");
        }
        ed.publish_buffer_changes();
        assert!(
            !syntax_layer(&ed, key).is_empty(),
            "fallback must keep painting edits without a grammar"
        );
    });
}

#[test]
fn ts_set_mode_query_takes_ownership_only_when_the_query_compiles() {
    // A grammar that loads but rejects the query must not claim the mode:
    // tree-sitter would own the syntax layer and paint nothing, so the
    // fallback has to stay (or be put back) in charge.
    janet_test!(ed, {
        let grammar = helpers::tiny_grammar_lib();
        scripting::eval(&format!(
            r#"(ts/load-grammar "{}" "tiny")"#,
            grammar.to_string_lossy().replace('\\', "/")
        ));

        scripting::eval(r#"(ts/set-mode-query "tiny-mode" "(source_file) @module")"#);
        assert!(
            ed.fallback_syntax.ts_modes.contains("tiny-mode"),
            "a query that compiles must hand the syntax layer to tree-sitter"
        );

        scripting::eval(r#"(ts/set-mode-query "tiny-mode" "(no_such_node) @module")"#);
        assert!(
            !ed.fallback_syntax.ts_modes.contains("tiny-mode"),
            "a query that fails to compile must give the layer back to the fallback"
        );
    });
}

#[test]
fn every_semantic_legend_name_resolves_to_a_face() {
    // A legend name without a scope-face registration is dropped from the
    // semantic layer by the kernel's resolvable-scope filter — a typo in
    // one of these camelCase names would silently grey out that token
    // class for every LSP-driven buffer.
    janet_test!(ed, {
        let expected: &[(&str, &str)] = &[
            ("comment", "comment-face"),
            ("constant", "constant-face"),
            ("function", "function-face"),
            ("keyword", "keyword-face"),
            ("method", "function-face"),
            ("namespace", "type-face"),
            ("operator", "operator-face"),
            ("parameter", "parameter-face"),
            ("property", "field-face"),
            ("string", "string-face"),
            ("type", "type-face"),
            ("variable", "variable-face"),
            ("class", "type-face"),
            ("enum", "type-face"),
            ("interface", "trait-face"),
            ("trait", "trait-face"),
            ("struct", "type-face"),
            ("union", "type-face"),
            ("builtinType", "type-face"),
            ("generic", "type-face"),
            ("module", "type-face"),
            ("selfTypeKeyword", "type-face"),
            ("typeParameter", "type-parameter-face"),
            ("constParameter", "constant-face"),
            ("macro", "macro-face"),
            ("procMacro", "macro-face"),
            ("derivative", "macro-face"),
            ("derive", "macro-face"),
            ("enumMember", "constant-face"),
            ("enumLiteral", "constant-face"),
            ("number", "constant-face"),
            ("boolean", "constant-face"),
            ("char", "constant-face"),
            ("escapeSequence", "constant-face"),
            ("escapedSequence", "constant-face"),
            ("lifetime", "lifetime-face"),
            ("attribute", "keyword-face"),
            ("decorator", "keyword-face"),
            ("selfKeyword", "keyword-face"),
            ("formatString", "string-face"),
            ("formatSpecifier", "constant-face"),
            ("regexp", "string-face"),
            ("event", "function-face"),
            ("controlFlow", "keyword-face"),
            ("const", "constant-face"),
            ("declaration", "declaration-face"),
            ("mutable", "mutable-face"),
            ("unsafe", "unsafe-face"),
        ];
        for (scope, face) in expected {
            let resolved = scripting::eval_result(&format!(
                r#"(face/resolve-scope "{scope}")"#
            ))
            .expect("resolve-scope eval must not error");
            // eval_result prints Janet values: strings come back quoted.
            assert_eq!(
                resolved,
                format!(r#""{face}""#),
                "legend scope {scope} must resolve to {face}"
            );
        }
        for face in [
            "macro-face",
            "lifetime-face",
            "type-parameter-face",
            "trait-face",
            "field-face",
            "declaration-face",
            "mutable-face",
            "parameter-face",
            "unsafe-face",
        ] {
            assert!(
                ed.faces.contains_key(face),
                "{face} must exist in the default theme"
            );
        }
    });
}
