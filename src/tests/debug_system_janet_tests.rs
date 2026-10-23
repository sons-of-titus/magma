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

// ── Breakpoint line base ──────────────────────────────────────────────────────
// Stored lines are 1-based (DAP convention, and what the :breakpoints gutter
// provider matches against); buffer/line-number is 0-based.

#[test]
fn toggle_breakpoint_stores_one_based_line() {
    janet_test!(ed, {
        let key = crate::tests::helpers::focused_key(&ed);
        if let Some(view) = ed.view_of_buffer_mut(key) {
            view.insert(0, "alpha\nbeta\ngamma\ndelta\n");
        }
        ed.buffers.get_mut(key).unwrap()
            .lock().unwrap()
            .path = Some("/tmp/bp_line_base.rs".to_string());
        // Cursor to line index 2 (third line).
        let offset = ed.buffers.get(key).unwrap().lock().unwrap()
            .line_start_offset(2).unwrap();
        if let Some(view) = ed.view_of_buffer_mut(key) {
            view.set_cursor(offset);
        }
        scripting::eval(r#"(debug/toggle-breakpoint)"#);
        let bps = ed.debug.breakpoints_for("/tmp/bp_line_base.rs");
        assert_eq!(bps.len(), 1, "toggle should add one breakpoint");
        assert_eq!(bps[0].line, 3, "cursor on 0-based line 2 stores 1-based line 3");
        // Second toggle removes it again — the convention must be symmetric.
        scripting::eval(r#"(debug/toggle-breakpoint)"#);
        assert!(ed.debug.breakpoints_for("/tmp/bp_line_base.rs").is_empty());
    });
}

#[test]
fn gutter_click_breakpoint_stores_one_based_line() {
    janet_test!(ed, {
        let key = crate::tests::helpers::focused_key(&ed);
        if let Some(view) = ed.view_of_buffer_mut(key) {
            view.insert(0, "alpha\nbeta\ngamma\ndelta\necho\nfoxtrot\n");
        }
        ed.buffers.get_mut(key).unwrap()
            .lock().unwrap()
            .path = Some("/tmp/bp_gutter.rs".to_string());
        ed.options.insert("mouse-support".to_string(), "true".to_string());
        // Park the cursor at the top so the synced viewport starts at line 0
        // (the insert above left it at the end of the inserted text).
        if let Some(view) = ed.view_of_buffer_mut(key) {
            view.set_cursor(0);
        }
        crate::kernel::render::frame::sync_viewport_scroll(&mut ed);

        // A real gutter click on the :breakpoints column, resolved through
        // dispatch_mouse exactly like the GUI does.  Providers start after
        // the prefix margin; the breakpoints column is first, width 2.
        let pane = ed.view_tree.panes()[0].clone();
        let click = crate::kernel::input::event::MouseEvent {
            kind: crate::kernel::input::event::MouseEventKind::Click,
            x: pane.x + 1,
            y: pane.y + 4, // viewport row 4 → 0-based line 4
            button: crate::kernel::input::event::MouseButton::Left,
            modifiers: String::new(),
        };
        let surface = crate::kernel::render::surface::Surface::new(pane.width, pane.height);
        crate::kernel::input::mouse::dispatch_mouse(&mut ed, &surface, &click);

        let bps = ed.debug.breakpoints_for("/tmp/bp_gutter.rs");
        assert_eq!(bps.len(), 1, "gutter click should add one breakpoint");
        assert_eq!(bps[0].line, 5, "click on 0-based line 4 stores 1-based line 5");

        // Clicking the same row again toggles it off.
        crate::kernel::input::mouse::dispatch_mouse(&mut ed, &surface, &click);
        assert!(ed.debug.breakpoints_for("/tmp/bp_gutter.rs").is_empty());
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
