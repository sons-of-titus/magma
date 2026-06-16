use crate::janet_bridge;

// ── C function registration ─────────────────────────────────────────────

#[test]
fn lsp_request_c_function_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/request \"test-lang\" \"test/method\" \"{}\")");
        assert_eq!(result, "ok", "lsp/request should return ok");
    });
}

#[test]
fn lsp_hover_c_function_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/hover \"rust\")");
        assert_eq!(result, "ok", "lsp/hover should return ok");
    });
}

#[test]
fn lsp_code_actions_c_function_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/code-actions \"rust\")");
        assert_eq!(result, "ok", "lsp/code-actions should return ok");
    });
}

#[test]
fn lsp_completion_c_function_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/completion \"rust\")");
        assert_eq!(result, "ok", "lsp/completion should return ok");
    });
}

#[test]
fn lsp_rename_c_function_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/rename \"rust\" \"new_name\")");
        assert_eq!(result, "ok", "lsp/rename should return ok");
    });
}

#[test]
fn lsp_apply_edit_c_function_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/apply-edit \"{}\")");
        assert_eq!(result, "ok", "lsp/apply-edit should return ok");
    });
}

#[test]
fn lsp_request_with_params_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            r#"(lsp/request "rust" "textDocument/hover" "{\"pos\":{}}")"#,
        );
        assert_eq!(result, "ok", "lsp/request with params should return ok");
    });
}

#[test]
fn lsp_start_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/start \"test-lang\" \"echo\" \"arg1\")");
        assert_eq!(result, "ok", "lsp/start should return ok");
    });
}

#[test]
fn lsp_notify_registered() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(lsp/notify \"test-lang\" \"test/notification\" \"{}\")");
        assert_eq!(result, "ok", "lsp/notify should return ok");
    });
}

// ── lsp-hover event dispatch ─────────────────────────────────────────────

#[test]
fn lsp_hover_event_dispatches_from_rust() {
    janet_test!(ed, {
        let mut data = std::collections::HashMap::new();
        data.insert("path".into(), "test".into());
        data.insert("contents".into(), "hover result".into());
        ed.events.emit("lsp-hover", data);
        ed.events.drain_and_dispatch();

        let sub_count = ed.events.subscriber_count("lsp-hover");
        assert!(sub_count > 0, "lsp-hover should have subscribers from lsp.janet");
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
fn lsp_code_actions_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-code-actions");
        assert!(count > 0, "lsp-code-actions should be subscribed in lsp.janet");
    });
}

// ── lsp-completion-items event dispatch ──────────────────────────────────

#[test]
fn lsp_completion_items_event_has_subscribers() {
    janet_test!(ed, {
        let count = ed.events.subscriber_count("lsp-completion-items");
        assert!(count > 0, "lsp-completion-items should be subscribed in lsp.janet");
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
