//! Janet integration tests for the Debug System (Phase 6).

use crate::kernel::scripting;

// ── debug/session ─────────────────────────────────────────────────────────────

#[test]
fn debug_session_nil_when_no_session() {
    janet_test!(ed, {
        let result = scripting::eval_result("(debug/session)").unwrap();
        assert_eq!(result, "nil", "no active session should yield nil");
    });
}

// ── debug/breakpoints ─────────────────────────────────────────────────────────

#[test]
fn debug_breakpoints_empty_for_unknown_file() {
    janet_test!(ed, {
        let result = scripting::eval_result(r#"(length (debug/breakpoints "/nonexistent.rs"))"#).unwrap();
        assert_eq!(result, "0");
    });
}

// ── debug/add-breakpoint / debug/remove-breakpoint ────────────────────────────

#[test]
fn add_breakpoint_stores_in_rust() {
    janet_test!(ed, {
        scripting::eval(r#"(debug/add-breakpoint "/tmp/test.rs" 10)"#);
        let bps = ed.debug.breakpoints_for("/tmp/test.rs");
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0].line, 10);
    });
}

#[test]
fn add_breakpoint_janet_api_returns_line_list() {
    janet_test!(ed, {
        scripting::eval(r#"(debug/add-breakpoint "/tmp/foo.rs" 5)"#);
        scripting::eval(r#"(debug/add-breakpoint "/tmp/foo.rs" 15)"#);
        let result = scripting::eval_result(r#"(length (debug/breakpoints "/tmp/foo.rs"))"#).unwrap();
        assert_eq!(result, "2");
    });
}

#[test]
fn remove_breakpoint_via_janet() {
    janet_test!(ed, {
        scripting::eval(r#"(debug/add-breakpoint "/tmp/r.rs" 42)"#);
        scripting::eval(r#"(debug/remove-breakpoint "/tmp/r.rs" 42)"#);
        let bps = ed.debug.breakpoints_for("/tmp/r.rs");
        assert!(bps.is_empty());
    });
}

#[test]
fn remove_nonexistent_breakpoint_is_noop() {
    janet_test!(ed, {
        scripting::eval(r#"(debug/remove-breakpoint "/tmp/no.rs" 99)"#);
        // Must not panic or error
        let bps = ed.debug.breakpoints_for("/tmp/no.rs");
        assert!(bps.is_empty());
    });
}

// ── Multiple breakpoints ──────────────────────────────────────────────────────

#[test]
fn multiple_files_independent_breakpoints() {
    janet_test!(ed, {
        scripting::eval(r#"(debug/add-breakpoint "/a.rs" 1)"#);
        scripting::eval(r#"(debug/add-breakpoint "/a.rs" 2)"#);
        scripting::eval(r#"(debug/add-breakpoint "/b.rs" 10)"#);
        assert_eq!(ed.debug.breakpoints_for("/a.rs").len(), 2);
        assert_eq!(ed.debug.breakpoints_for("/b.rs").len(), 1);
    });
}

// ── debug/evaluate (no session active — must be a no-op) ─────────────────────

#[test]
fn evaluate_without_session_is_noop() {
    janet_test!(ed, {
        // Should not panic or error — simply returns nil.
        let result = scripting::eval_result(r#"(debug/evaluate "x + 1")"#).unwrap();
        assert_eq!(result, "nil");
    });
}

// ── debug/continue / step-in / step-over / step-out without session ───────────

#[test]
fn step_commands_without_session_are_noop() {
    janet_test!(ed, {
        let cmds = [
            "(debug/continue)",
            "(debug/step-in)",
            "(debug/step-over)",
            "(debug/step-out)",
        ];
        for cmd in cmds {
            let result = scripting::eval_result(cmd).unwrap();
            assert_eq!(result, "nil", "{cmd} should return nil when no session is active");
        }
    });
}

// ── debug/ functions are registered ──────────────────────────────────────────

#[test]
fn debug_api_functions_exist() {
    janet_test!(ed, {
        let fns = [
            "debug/start",
            "debug/continue",
            "debug/step-in",
            "debug/step-over",
            "debug/step-out",
            "debug/add-breakpoint",
            "debug/remove-breakpoint",
            "debug/evaluate",
            "debug/session",
            "debug/breakpoints",
        ];
        for f in fns {
            // C functions registered via janet_cfuns are :cfunction, not :function.
            // Use (not (nil? f)) to verify the binding exists.
            let check = format!("(not (nil? {f}))");
            let result = scripting::eval_result(&check).unwrap();
            assert_eq!(result, "true", "{f} should be a registered C function");
        }
    });
}
