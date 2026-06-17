use crate::kernel::scripting;

// ── agent/status ──────────────────────────────────────────────────────────────

#[test]
fn agent_status_unknown_returns_not_found() {
    janet_test!(ed, {
        let result = scripting::eval_result("(agent/status 9999)").unwrap_or_default();
        assert_eq!(result, ":not-found");
    });
}

// ── agent/result ──────────────────────────────────────────────────────────────

#[test]
fn agent_result_unknown_returns_nil() {
    janet_test!(ed, {
        let result = scripting::eval_result("(agent/result 9999)").unwrap_or_default();
        assert_eq!(result, "nil");
    });
}

// ── agent/list ────────────────────────────────────────────────────────────────

#[test]
fn agent_list_returns_array() {
    janet_test!(ed, {
        let result = scripting::eval_result("(type (agent/list))").unwrap_or_default();
        assert_eq!(result, ":array");
    });
}

#[test]
fn agent_list_initially_empty() {
    janet_test!(ed, {
        let result = scripting::eval_result("(length (agent/list))").unwrap_or_default();
        assert_eq!(result, "0");
    });
}

// ── agent/on-event ────────────────────────────────────────────────────────────

#[test]
fn agent_on_event_accepts_thought_callback() {
    janet_test!(ed, {
        let result = scripting::eval(
            r#"(event/on "agent-thought" (fn [p] nil))"#
        );
        assert!(!result.contains("error"), "event/on agent-thought failed: {result}");
    });
}

#[test]
fn agent_on_event_accepts_result_callback() {
    janet_test!(ed, {
        let result = scripting::eval(
            r#"(event/on "agent-result" (fn [p] nil))"#
        );
        assert!(!result.contains("error"), "event/on agent-result failed: {result}");
    });
}

#[test]
fn agent_on_event_janet_helper_works() {
    janet_test!(ed, {
        let result = scripting::eval(
            r#"(agent/on-event "action" (fn [p] nil))"#
        );
        assert!(!result.contains("error"), "agent/on-event failed: {result}");
    });
}

// ── agent/status after manual session ─────────────────────────────────────────

#[test]
fn agent_status_round_trip_via_rust() {
    janet_test!(ed, {
        let id = ed.agent.create_session("test from rust");
        ed.agent.complete_session(id, "rust result");

        let code = format!("(agent/status {id})");
        let result = scripting::eval_result(&code).unwrap_or_default();
        assert_eq!(result, ":completed");
    });
}

#[test]
fn agent_result_round_trip_via_rust() {
    janet_test!(ed, {
        let id = ed.agent.create_session("test");
        ed.agent.complete_session(id, "hello from janet");

        let code = format!("(agent/result {id})");
        let result = scripting::eval_result(&code).unwrap_or_default();
        assert!(result.contains("hello from janet"), "result was: {result}");
    });
}

#[test]
fn agent_list_reflects_rust_created_session() {
    janet_test!(ed, {
        ed.agent.create_session("check this");
        let result = scripting::eval_result("(length (agent/list))").unwrap_or_default();
        assert_eq!(result, "1");
    });
}
