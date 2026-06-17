//! Janet integration tests for Phase 8 — Workspace Persistence.

use crate::kernel::scripting;

// ── workspace/save ────────────────────────────────────────────────────────────

#[test]
fn workspace_save_returns_ok_status() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_save"); }
        let result = scripting::eval_result("(get (workspace/save) :status)").unwrap();
        assert_eq!(result, r#""ok""#);
        std::fs::remove_dir_all("/tmp/magma_janet_ws_save").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

#[test]
fn workspace_save_writes_json_file() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_write"); }
        scripting::eval("(workspace/save)");
        let path = std::path::PathBuf::from(
            "/tmp/magma_janet_ws_write/magma/workspace/workspace.json",
        );
        assert!(path.exists(), "workspace.json must be written to disk");
        std::fs::remove_dir_all("/tmp/magma_janet_ws_write").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

// ── workspace/restore ─────────────────────────────────────────────────────────

#[test]
fn workspace_restore_error_on_missing_file() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_no_file_xyz"); }
        let result = scripting::eval_result("(get (workspace/restore) :status)").unwrap();
        assert_eq!(result, r#""error""#);
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

#[test]
fn workspace_restore_after_save_returns_ok() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_restore"); }
        scripting::eval("(workspace/save)");
        let result = scripting::eval_result("(get (workspace/restore) :status)").unwrap();
        assert_eq!(result, r#""ok""#);
        std::fs::remove_dir_all("/tmp/magma_janet_ws_restore").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

#[test]
fn workspace_restore_returns_count_key() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_count"); }
        scripting::eval("(workspace/save)");
        let result = scripting::eval_result(
            "(number? (get (workspace/restore) :count))",
        )
        .unwrap();
        assert_eq!(result, "true");
        std::fs::remove_dir_all("/tmp/magma_janet_ws_count").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

// ── workspace/session-save / workspace/session-load ──────────────────────────

#[test]
fn session_save_returns_ok() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_sess_save"); }
        let result =
            scripting::eval_result(r#"(get (workspace/session-save "test-session") :status)"#)
                .unwrap();
        assert_eq!(result, r#""ok""#);
        std::fs::remove_dir_all("/tmp/magma_janet_ws_sess_save").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

#[test]
fn session_load_after_save_returns_ok() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_sess_rt"); }
        scripting::eval(r#"(workspace/session-save "rt")"#);
        let result =
            scripting::eval_result(r#"(get (workspace/session-load "rt") :status)"#).unwrap();
        assert_eq!(result, r#""ok""#);
        std::fs::remove_dir_all("/tmp/magma_janet_ws_sess_rt").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

#[test]
fn session_load_missing_returns_error() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_sess_missing"); }
        let result =
            scripting::eval_result(r#"(get (workspace/session-load "nonexistent") :status)"#)
                .unwrap();
        assert_eq!(result, r#""error""#);
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

// ── workspace/session-list ───────────────────────────────────────────────────

#[test]
fn session_list_empty_when_no_sessions() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_list_empty"); }
        let result = scripting::eval_result("(length (workspace/session-list))").unwrap();
        assert_eq!(result, "0");
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

#[test]
fn session_list_shows_saved_sessions() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_list_sessions"); }
        scripting::eval(r#"(workspace/session-save "s1")"#);
        scripting::eval(r#"(workspace/session-save "s2")"#);
        let result = scripting::eval_result("(length (workspace/session-list))").unwrap();
        assert_eq!(result, "2");
        std::fs::remove_dir_all("/tmp/magma_janet_ws_list_sessions").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

// ── Workspace events ──────────────────────────────────────────────────────────

#[test]
fn workspace_save_emits_event() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_event_save"); }
        scripting::eval(
            r#"(var *ws-saved* false)
               (event/on "workspace-saved" (fn [_] (set *ws-saved* true)))"#,
        );
        scripting::eval("(workspace/save)");
        ed.events.drain_and_dispatch();
        let result = scripting::eval_result("*ws-saved*").unwrap();
        assert_eq!(
            result, "true",
            "workspace-saved event must fire after (workspace/save)"
        );
        std::fs::remove_dir_all("/tmp/magma_janet_ws_event_save").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}

#[test]
fn workspace_restore_emits_event() {
    janet_test!(ed, {
        unsafe { std::env::set_var("MAGMA_DIR", "/tmp/magma_janet_ws_event_restore"); }
        scripting::eval("(workspace/save)");
        scripting::eval(
            r#"(var *ws-restored* false)
               (event/on "workspace-restored" (fn [_] (set *ws-restored* true)))"#,
        );
        scripting::eval("(workspace/restore)");
        ed.events.drain_and_dispatch();
        let result = scripting::eval_result("*ws-restored*").unwrap();
        assert_eq!(
            result, "true",
            "workspace-restored event must fire after (workspace/restore)"
        );
        std::fs::remove_dir_all("/tmp/magma_janet_ws_event_restore").ok();
        unsafe { std::env::remove_var("MAGMA_DIR"); }
    });
}
