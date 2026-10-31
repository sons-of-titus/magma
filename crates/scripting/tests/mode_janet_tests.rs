#[macro_use]
mod helpers;
use scripting;

// ── buffer-created mode assignment ──────────────────────────────────────
//
// buffer-created events queue and drain in batches (project open, session
// restore).  The mode must land on the buffer the event describes — a
// drain-time-focused rust file used to come out in toml-mode because the
// activation targeted whatever buffer was focused when the queue drained.

#[test]
fn buffer_created_assigns_mode_to_the_event_buffer_not_the_focused_one() {
    janet_test!(ed, {
        // A mode that exists in the bare environment (plugin modes like
        // rust-mode only exist once plugins load); the define also exercises
        // the buffer-targeted activation closure.
        assert_eq!(
            scripting::eval(r#"(major-mode/define "ztest-mode" {:parent "prog"})"#),
            "ok"
        );
        assert_eq!(
            scripting::eval(r#"(put *extension-mode-map* "ztest" "ztest-mode")"#),
            "ok"
        );
        let first = ed.create_buffer_from_str("main.ztest", "fn main() {}\n");
        let second = ed.create_buffer_from_str("data.ztest", "k = 1\n");
        // Focus the first file: the drain must not retarget modes onto it.
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(first);
        }

        // The second creation drains last — the old bug stamped its mode
        // on the focused first buffer right here.
        for (id, path) in [(second, "/proj/data.ztest"), (first, "/proj/main.ztest")] {
            let ok = scripting::eval_result(&format!(
                r#"(do (event/emit "buffer-created" {{"buffer-id" "{id}" "path" "{path}"}}) true)"#
            ))
            .unwrap();
            assert_eq!(ok, "true");
        }
        ed.dispatch_events_scoped();

        let first_mode = ed
            .buffers
            .get(first)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name()
            .to_string();
        let second_mode = ed
            .buffers
            .get(second)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name()
            .to_string();
        assert_eq!(
            first_mode, "ztest-mode",
            "the focused buffer keeps the mode its extension maps to"
        );
        assert_eq!(
            second_mode, "ztest-mode",
            "the other buffer got the mode from its own event"
        );

        // The critical crossover property: a later event for ANOTHER
        // buffer must not touch the focused one.
        assert_eq!(
            scripting::eval(r#"(major-mode/define "zother-mode" {:parent "prog"})"#),
            "ok"
        );
        assert_eq!(
            scripting::eval(r#"(put *extension-mode-map* "zother" "zother-mode")"#),
            "ok"
        );
        let ok = scripting::eval_result(&format!(
            r#"(do (event/emit "buffer-created" {{"buffer-id" "{second}" "path" "/proj/data.zother"}}) true)"#
        ))
        .unwrap();
        assert_eq!(ok, "true");
        ed.dispatch_events_scoped();
        let first_mode = ed
            .buffers
            .get(first)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name()
            .to_string();
        let second_mode = ed
            .buffers
            .get(second)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name()
            .to_string();
        assert_eq!(
            first_mode, "ztest-mode",
            "focused buffer untouched by another buffer's event"
        );
        assert_eq!(
            second_mode, "zother-mode",
            "the event buffer itself is retargeted"
        );
    });
}

#[test]
fn set_major_mode_targets_the_named_buffer_from_the_command_line() {
    janet_test!(ed, {
        let a = ed.create_buffer_from_str("a.txt", "a\n");
        let b = ed.create_buffer_from_str("b.md", "b\n");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(a);
        }
        // editor/run-command joins the positional arguments onto the
        // command line; the declared Buffer argument coerces the id back —
        // the command line and event handlers share this path.
        let ok = scripting::eval_result(&format!(
            r#"(do (editor/run-command "set-major-mode" "markdown-mode" {b}) true)"#
        ))
        .unwrap();
        assert_eq!(ok, "true");

        let a_mode = ed
            .buffers
            .get(a)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name()
            .to_string();
        let b_mode = ed
            .buffers
            .get(b)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name()
            .to_string();
        assert_eq!(a_mode, "fundamental", "the focused buffer is untouched");
        assert_eq!(b_mode, "markdown-mode", "the named buffer got the mode");
    });
}
