use crate::kernel::state::mode::Selection;
use crate::kernel::command::execute_command;
use crate::kernel::scripting;

// ── editor/eval C function ────────────────────────────────────────────────

#[test]
fn editor_eval_arithmetic_via_c_function() {
    janet_test!(ed, {
        let r = scripting::eval(r#"(editor/eval "(+ 1 2)")"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn eval_result_arithmetic() {
    janet_test!(ed, {
        let r = scripting::eval_result("(+ 10 5)").unwrap();
        assert_eq!(r, "15");
    });
}

#[test]
fn eval_result_string() {
    janet_test!(ed, {
        let r = scripting::eval_result(r#"(string "foo" "bar")"#).unwrap();
        assert_eq!(r, "\"foobar\"");
    });
}

#[test]
fn eval_result_nil() {
    janet_test!(ed, {
        let r = scripting::eval_result("nil").unwrap();
        assert_eq!(r, "nil");
    });
}

#[test]
fn eval_result_syntax_error_returns_err() {
    janet_test!(ed, {
        let r = scripting::eval_result("(+ 1");
        assert!(r.is_err());
    });
}

#[test]
fn editor_eval_c_function_returns_value_string() {
    janet_test!(ed, {
        scripting::eval(
            r#"(plugin-state/set "ev" (editor/eval "(* 3 4)"))"#);
        let val = ed.plugin_state.get("ev").cloned().unwrap_or_default();
        assert_eq!(val, "12");
    });
}

// ── fs/cwd ────────────────────────────────────────────────────────────────

#[test]
fn fs_cwd_returns_non_empty_string() {
    janet_test!(ed, {
        scripting::eval(
            r#"(plugin-state/set "cwd" (fs/cwd))"#);
        let cwd = ed.plugin_state.get("cwd").cloned().unwrap_or_default();
        assert!(!cwd.is_empty());
    });
}

#[test]
fn fs_cwd_matches_actual_cwd() {
    janet_test!(ed, {
        let expected = std::env::current_dir().unwrap().to_string_lossy().to_string();
        scripting::eval(
            r#"(plugin-state/set "cwd" (fs/cwd))"#);
        let cwd = ed.plugin_state.get("cwd").cloned().unwrap_or_default();
        assert_eq!(cwd, expected);
    });
}

// ── fs/chdir ──────────────────────────────────────────────────────────────

#[test]
fn fs_chdir_changes_working_directory() {
    janet_test!(ed, {
        let original = std::env::current_dir().unwrap();
        scripting::eval(r#"(fs/chdir "/tmp")"#);
        scripting::eval(
            r#"(plugin-state/set "new-cwd" (fs/cwd))"#);
        let new_cwd = ed.plugin_state.get("new-cwd").cloned().unwrap_or_default();
        assert!(new_cwd.contains("tmp"));
        std::env::set_current_dir(&original).ok();
    });
}

// ── eval-region ──────────────────────────────────────────────────────────

#[test]
fn eval_region_evaluates_selection_and_emits_event() {
    janet_test!(ed, {
        ed.commands.register_fn(
            "store-eval-result",
            "store eval result",
            vec![],
            |editor, _| {
                let v = editor.plugin_state.get("_ev_val").cloned().unwrap_or_default();
                editor.plugin_state.insert("ev-captured".to_string(), v);
                Ok(())
            },
        );
        scripting::eval(r#"
            (event/on "eval-result"
              (fn [d]
                (plugin-state/set "_ev_val" (get d :value ""))))"#);

        let buf_id = ed.windows.focused_window_mut().unwrap().buffer_id.unwrap();
        {
            let buf = ed.buffers.get_mut(buf_id).unwrap();
            buf.insert(0, "(+ 7 8)");
            let end = buf.len();
            buf.set_cursor(end);
        }
        ed.selection = Some(Selection { anchor: 0, kind: "char".to_string() });

        execute_command(&mut ed, "eval-region", &std::collections::HashMap::new()).unwrap();
        ed.events.drain_and_dispatch();

        let value = ed.plugin_state.get("_ev_val").cloned().unwrap_or_default();
        assert_eq!(value, "15");
    });
}

// ── eval-buffer ──────────────────────────────────────────────────────────

#[test]
fn eval_buffer_evaluates_entire_buffer() {
    janet_test!(ed, {
        scripting::eval(r#"
            (event/on "eval-result"
              (fn [d]
                (plugin-state/set "buf-val" (get d :value ""))))"#);

        let buf_id = ed.windows.focused_window_mut().unwrap().buffer_id.unwrap();
        {
            let buf = ed.buffers.get_mut(buf_id).unwrap();
            buf.insert(0, "(* 6 7)");
        }

        execute_command(&mut ed, "eval-buffer", &std::collections::HashMap::new()).unwrap();
        ed.events.drain_and_dispatch();

        let value = ed.plugin_state.get("buf-val").cloned().unwrap_or_default();
        assert_eq!(value, "42");
    });
}

// ── janet-output/write ────────────────────────────────────────────────────

#[test]
fn janet_output_write_appends_to_buffer() {
    janet_test!(ed, {
        scripting::eval(r#"(janet-output/write "sprint12-test\n")"#);
        let buf_key = ed.buffers.iter()
            .find(|(_, b)| b.name == "*janet-output*")
            .map(|(k, _)| k);
        assert!(buf_key.is_some(), "*janet-output* buffer was not created");
        let buf = ed.buffers.get(buf_key.unwrap()).unwrap();
        let content = buf.slice(0, buf.len());
        assert!(content.contains("sprint12-test"));
    });
}

// ── :janet colon verb ─────────────────────────────────────────────────────

#[test]
fn janet_colon_verb_is_in_plugins_table() {
    janet_test!(ed, {
        let r = scripting::eval_result(
            r#"(truthy? (get *colon-plugins* "janet"))"#).unwrap();
        assert_eq!(r, "true");
    });
}

#[test]
fn janet_repl_colon_verb_is_registered() {
    janet_test!(ed, {
        let r = scripting::eval_result(
            r#"(truthy? (get *colon-plugins* "janet-repl"))"#).unwrap();
        assert_eq!(r, "true");
    });
}

// ── MShell state ─────────────────────────────────────────────────────────

#[test]
fn mshell_cwd_initialized_to_process_cwd() {
    janet_test!(ed, {
        scripting::eval(
            r#"(plugin-state/set "ms-cwd" *mshell-cwd*)"#);
        let cwd = ed.plugin_state.get("ms-cwd").cloned().unwrap_or_default();
        assert!(!cwd.is_empty());
    });
}

#[test]
fn mshell_builtins_table_exists() {
    janet_test!(ed, {
        let r = scripting::eval_result(
            r#"(truthy? *mshell-builtins*)"#).unwrap();
        assert_eq!(r, "true");
    });
}

#[test]
fn mshell_parse_line_janet_expr() {
    janet_test!(ed, {
        let r = scripting::eval_result(
            r#"(get (mshell/parse-line "(+ 1 2)") 0)"#).unwrap();
        assert!(r.contains("janet"), "expected :janet keyword, got: {r}");
    });
}

#[test]
fn mshell_parse_line_builtin_command() {
    janet_test!(ed, {
        let r = scripting::eval_result(
            r#"(get (mshell/parse-line "cd /tmp") 0)"#).unwrap();
        assert!(r.contains("builtin"), "expected :builtin keyword, got: {r}");
    });
}

#[test]
fn mshell_parse_line_shell_command() {
    janet_test!(ed, {
        let r = scripting::eval_result(
            r#"(get (mshell/parse-line "ls -la") 0)"#).unwrap();
        assert!(r.contains("shell"), "expected :shell keyword, got: {r}");
    });
}

#[test]
fn mshell_echo_builtin_works() {
    janet_test!(ed, {
        scripting::eval(r#"
            (plugin-state/set "echo-out"
              ((get *mshell-builtins* "echo") ["hello" "world"]))"#);
        let out = ed.plugin_state.get("echo-out").cloned().unwrap_or_default();
        assert_eq!(out, "hello world");
    });
}

#[test]
fn mshell_colon_verbs_registered() {
    janet_test!(ed, {
        let r1 = scripting::eval_result(
            r#"(truthy? (get *colon-plugins* "mshell"))"#).unwrap();
        let r2 = scripting::eval_result(
            r#"(truthy? (get *colon-plugins* "ms"))"#).unwrap();
        assert_eq!(r1, "true");
        assert_eq!(r2, "true");
    });
}
