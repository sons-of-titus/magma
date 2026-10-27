#[macro_use] mod helpers;
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
        let result = scripting::eval(
            r#"(lsp/request "rust" "textDocument/hover" "{\"pos\":{}}")"#,
        );
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
        let described = ed.commands.list().into_iter()
            .any(|name| name == "lsp-hover");
        assert!(described, "lsp-hover command must stay registered");
    });
}

// ── lsp-definition event dispatch ────────────────────────────────────────

#[test]
fn lsp_definition_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-definition");
        assert!(count > 0, "lsp-definition should be subscribed in lsp.janet");
    });
}

// ── lsp-code-actions event dispatch ──────────────────────────────────────

#[test]
fn lsp_code_actions_command_stays_registered() {
    // Actions open the kernel's quick-fix picker on arrival; the trigger
    // command (alt-return) remains.
    janet_test!(ed, {
        let described = ed.commands.list().into_iter()
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
        let described = ed.commands.list().into_iter()
            .any(|name| name == "lsp-completion");
        assert!(described, "lsp-completion command must stay registered");
    });
}

// ── lsp-rename-result event dispatch ─────────────────────────────────────

#[test]
fn lsp_rename_result_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-rename-result");
        assert!(count > 0, "lsp-rename-result should be subscribed in lsp.janet");
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
            ("(= ((buffer/lsp-position K 5) 1) 3)", "col after emoji = 3 utf16 units"),
            ("(= ((buffer/lsp-position K 7) 0) 1)", "line of x = 1"),
            ("(= ((buffer/lsp-position K 8) 1) 1)", "col of é = 1"),
            ("(= ((buffer/lsp-position K 13) 1) 3)", "col at EOF: x+é+… = 3 units"),
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
