#[macro_use]
mod helpers;
use scripting;

// ── C function registration ─────────────────────────────────────────────

#[test]
fn lsp_request_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/request \"test-lang\" \"test/method\" \"{}\")");
        assert_eq!(result, "ok", "lsp/request should return ok");
    });
}

#[test]
fn lsp_hover_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/hover \"rust\")");
        assert_eq!(result, "ok", "lsp/hover should return ok");
    });
}

#[test]
fn lsp_code_actions_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/code-actions \"rust\")");
        assert_eq!(result, "ok", "lsp/code-actions should return ok");
    });
}

#[test]
fn lsp_completion_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/completion \"rust\")");
        assert_eq!(result, "ok", "lsp/completion should return ok");
    });
}

#[test]
fn lsp_rename_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/rename \"rust\" \"new_name\")");
        assert_eq!(result, "ok", "lsp/rename should return ok");
    });
}

#[test]
fn lsp_apply_edit_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/apply-edit \"{}\")");
        assert_eq!(result, "ok", "lsp/apply-edit should return ok");
    });
}

#[test]
fn lsp_request_with_params_registered() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(lsp/request "rust" "textDocument/hover" "{\"pos\":{}}")"#);
        assert_eq!(result, "ok", "lsp/request with params should return ok");
    });
}

#[test]
fn lsp_start_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/start \"test-lang\" \"echo\" \"arg1\")");
        assert_eq!(result, "ok", "lsp/start should return ok");
    });
}

#[test]
fn lsp_notify_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(lsp/notify \"test-lang\" \"test/notification\" \"{}\")");
        assert_eq!(result, "ok", "lsp/notify should return ok");
    });
}

// ── lsp-hover ────────────────────────────────────────────────────────────

#[test]
fn lsp_hover_command_stays_registered() {
    // Hover text is consumed by the kernel (dwell card / status message);
    // the manual trigger command remains for explicit invocation.
    janet_test!(ed, {
        let described = ed
            .commands
            .list()
            .into_iter()
            .any(|name| name == "lsp-hover");
        assert!(described, "lsp-hover command must stay registered");
    });
}

// ── lsp-definition event dispatch ────────────────────────────────────────

#[test]
fn lsp_definition_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-definition");
        assert!(
            count > 0,
            "lsp-definition should be subscribed in lsp.janet"
        );
    });
}

// ── lsp-code-actions event dispatch ──────────────────────────────────────

#[test]
fn lsp_code_actions_command_stays_registered() {
    // Actions open the kernel's quick-fix picker on arrival; the trigger
    // command (alt-return) remains.
    janet_test!(ed, {
        let described = ed
            .commands
            .list()
            .into_iter()
            .any(|name| name == "lsp-code-actions");
        assert!(described, "lsp-code-actions command must stay registered");
    });
}

// ── lsp-completion-items event dispatch ──────────────────────────────────

#[test]
fn lsp_completion_command_is_defined() {
    // Completion results are consumed by the kernel popup merge now; the
    // manual trigger command remains for explicit use.
    janet_test!(ed, {
        let described = ed
            .commands
            .list()
            .into_iter()
            .any(|name| name == "lsp-completion");
        assert!(described, "lsp-completion command must stay registered");
    });
}

// ── lsp-rename-result event dispatch ─────────────────────────────────────

#[test]
fn lsp_rename_result_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-rename-result");
        assert!(
            count > 0,
            "lsp-rename-result should be subscribed in lsp.janet"
        );
    });
}

// ── lsp-progress event dispatch ──────────────────────────────────────────

#[test]
fn lsp_progress_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-progress");
        assert!(count > 0, "lsp-progress should be subscribed in lsp.janet");
    });
}

// ── lsp-response event dispatch ──────────────────────────────────────────

#[test]
fn lsp_response_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-response");
        assert!(count > 0, "lsp-response should be subscribed in lsp.janet");
    });
}

// ── Canonical positions for LSP (docs/core-redesign.md §8) ──────────────

#[test]
fn lsp_position_counts_utf16_code_units() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        // "a😀b\nxé…": 😀 is 4 UTF-8 bytes but 2 UTF-16 units; é is 2 bytes/1 unit; … 3 bytes/1 unit.
        if let Some(arc) = ed.buffers.get(key) {
            let mut buf = arc.lock().unwrap();
            let len = buf.len();
            buf.delete(0, len);
            buf.insert(0, "a\u{1F600}b\nx\u{00E9}\u{2026}");
        }
        // Offsets: a=0, 😀=1..5, b=5, \n=6, x=7, é=8..10, …=10..13.
        // UTF-16 columns: after 'a'=1, after 😀=3, after 'ab'=4; é is 1 unit, … is 1 unit.
        let checks = [
            ("(= ((buffer/lsp-position K 0) 0) 0)", "line of BOF"),
            (
                "(= ((buffer/lsp-position K 5) 1) 3)",
                "col after emoji = 3 utf16 units",
            ),
            ("(= ((buffer/lsp-position K 7) 0) 1)", "line of x = 1"),
            ("(= ((buffer/lsp-position K 8) 1) 1)", "col of é = 1"),
            (
                "(= ((buffer/lsp-position K 13) 1) 3)",
                "col at EOF: x+é+… = 3 units",
            ),
        ];
        for (expr, why) in checks {
            let e = expr.replace('K', &key.to_string());
            assert_eq!(
                scripting::eval(&format!("(assert {e} \"{why}\") {e}")),
                "ok",
                "{why}"
            );
        }
    });
}

// ── Server-exit recovery ────────────────────────────────────────────────

#[test]
fn lsp_server_exited_resets_the_attach_state() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        // The attach tables key languages by string id (:language-id values).
        scripting::eval(r#"(put *lsp-servers* "testlang" :ready)"#);
        scripting::eval(&format!(r#"(put *buffer-languages* {key} "testlang")"#));
        scripting::eval(&format!(r#"(put *attached-buffers* {key} true)"#));

        let ok = scripting::eval_result(
            r#"(do (event/emit "lsp-server-exited" {"language" "testlang"}) true)"#,
        )
        .unwrap();
        assert_eq!(ok, "true");
        // emit only queues; the handler runs on the drain.
        ed.dispatch_events_scoped();

        let gone = scripting::eval_result(r#"(nil? (get *lsp-servers* "testlang"))"#).unwrap();
        assert_eq!(
            gone, "true",
            "a dead server's attach record must be dropped"
        );

        let reopenable =
            scripting::eval_result(&format!(r#"(nil? (get *attached-buffers* {key}))"#)).unwrap();
        assert_eq!(
            reopenable, "true",
            "didOpen must fire again once the replacement server is ready"
        );
    });
}

// ── Document-sync URIs ──────────────────────────────────────────────────

#[test]
fn buffer_uri_encodes_reserved_bytes() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("x");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap().path = Some("/tmp/my notes.rs".to_string());
    }
    // didOpen sends the percent-encoded URI (kernel path_to_uri); didChange
    // must reference the same string or the server never matches the doc.
    let ok = scripting::eval_result(&format!(
        r#"(= (buffer-uri {key}) "file:///tmp/my%20notes.rs")"#
    ))
    .unwrap();
    assert_eq!(ok, "true", "buffer-uri must encode like path_to_uri");
}

// ── Dead-code dimming (hint-diagnostics fallback) ────────────────────────

#[test]
fn dead_code_scope_resolves_to_the_unused_face() {
    // The diagnostics fallback writes the "unused" scope; presentation has
    // to claim it or the range falls through to the generic highlight box
    // instead of greying out.
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("aaa\nbbb\nccc\n");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    // `eval_result` prints the Janet description, so the string is quoted.
    assert_eq!(
        scripting::eval_result(r#"(face/resolve-scope "unused")"#).unwrap(),
        "\"unused-face\""
    );
}

#[test]
fn dead_code_refresh_dims_unused_hint_words() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("aaa\nbbb\nccc\n");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    // A hint whose message says the code is unused, pointing at line 2
    // col 1 ("ccc" — the display is 1-based).
    use kernel::text_engine::diagnostic::{Diagnostic, DiagnosticSeverity};
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap().set_lsp_diagnostics(vec![Diagnostic {
            message: "unused variable `x`".into(),
            severity: DiagnosticSeverity::Hint,
            line: 1,
            column: 0,
            end_line: 1,
            end_column: 3,
        }]);
    }

    let result = scripting::eval(&format!("(dead-code/refresh {key})"));
    assert_eq!(result, "ok");

    let layer = ed
        .views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("dead-code")
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        layer,
        vec![(4, 7, "unused".to_string())],
        "the word after the hint's position dims"
    );
}

#[test]
fn dead_code_refresh_dims_only_dead_code_messages() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("aaa\nbbb\nccc\n");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    use kernel::text_engine::diagnostic::{Diagnostic, DiagnosticSeverity};
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap().set_lsp_diagnostics(vec![
            Diagnostic {
                // A hint, but a style suggestion, not dead code.
                message: "consider adding documentation".into(),
                severity: DiagnosticSeverity::Hint,
                line: 0,
                column: 0,
                end_line: 0,
                end_column: 3,
            },
            Diagnostic {
                // The rust dead-code lints arrive as warnings — this one
                // dims (cargo's "unused import" family).
                message: "unused import: std::fmt".into(),
                severity: DiagnosticSeverity::Warning,
                line: 2,
                column: 0,
                end_line: 2,
                end_column: 3,
            },
        ]);
    }

    let _ = scripting::eval(&format!("(dead-code/refresh {key})"));
    let layer = ed
        .views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("dead-code")
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        layer,
        vec![(8, 11, "unused".to_string())],
        "the dead-code warning dims, the style hint does not: got {layer:?}"
    );

    // A warning that is not about dead code stays untouched.
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap().set_lsp_diagnostics(vec![Diagnostic {
            message: "mismatched types".into(),
            severity: DiagnosticSeverity::Warning,
            line: 1,
            column: 0,
            end_line: 1,
            end_column: 3,
        }]);
    }
    let _ = scripting::eval(&format!("(dead-code/refresh {key})"));
    let layer = ed
        .views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("dead-code")
        .cloned()
        .unwrap_or_default();
    assert!(
        layer.is_empty(),
        "ordinary warnings must not dim: {layer:?}"
    );
}

#[test]
fn dead_code_refresh_clears_the_layer_when_diagnostics_leave() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("aaa\nbbb\n");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    use kernel::text_engine::diagnostic::{Diagnostic, DiagnosticSeverity};
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap().set_lsp_diagnostics(vec![Diagnostic {
            message: "unused variable `x`".into(),
            severity: DiagnosticSeverity::Hint,
            line: 1,
            column: 0,
            end_line: 1,
            end_column: 3,
        }]);
    }
    let _ = scripting::eval(&format!("(dead-code/refresh {key})"));
    // The server withdrew the hint.
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap().set_lsp_diagnostics(vec![]);
    }
    let _ = scripting::eval(&format!("(dead-code/refresh {key})"));

    let layer = ed
        .views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("dead-code")
        .cloned()
        .unwrap_or_default();
    assert!(layer.is_empty(), "a withdrawn hint must undim");
}
