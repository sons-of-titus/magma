//! Janet API tests for Sprint 12: editor/eval, fs/cwd, fs/chdir, eval-region,
//! eval-buffer, process/spawn with cwd, and MShell primitives.

#[cfg(feature = "janet")]
mod tests {
    use crate::state::Editor;
    use crate::state::id::BufferId;
    use crate::state::mode::Selection;
    use crate::buffer::Buffer;
    use crate::command::{builtin, execute_command};
    use crate::fs::disk::DiskFileSystem;
    use crate::janet_bridge;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let id = ed.allocate_buffer_id();
        let buf = Buffer::new(BufferId(id), "test");
        let entry = ed.buffers.vacant_entry();
        let key = entry.key();
        entry.insert(buf);
        if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
        ed
    }

    // ── editor/eval C function ────────────────────────────────────────────────

    #[test]
    fn editor_eval_arithmetic_via_c_function() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // Call editor/eval from Janet and verify result is returned as string
        let r = janet_bridge::eval(&mut ed, r#"(editor/eval "(+ 1 2)")"#);
        assert_eq!(r, "ok");
    }

    #[test]
    fn eval_result_arithmetic() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, "(+ 10 5)").unwrap();
        assert_eq!(r, "15");
    }

    #[test]
    fn eval_result_string() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(string "foo" "bar")"#).unwrap();
        assert_eq!(r, "\"foobar\"");
    }

    #[test]
    fn eval_result_nil() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, "nil").unwrap();
        assert_eq!(r, "nil");
    }

    #[test]
    fn eval_result_syntax_error_returns_err() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, "(+ 1");
        assert!(r.is_err());
    }

    #[test]
    fn editor_eval_c_function_returns_value_string() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // editor/eval returns the description of the result
        janet_bridge::eval(&mut ed,
            r#"(plugin-state/set "ev" (editor/eval "(* 3 4)"))"#);
        let val = ed.plugin_state.get("ev").cloned().unwrap_or_default();
        assert_eq!(val, "12");
    }

    // ── fs/cwd ────────────────────────────────────────────────────────────────

    #[test]
    fn fs_cwd_returns_non_empty_string() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed,
            r#"(plugin-state/set "cwd" (fs/cwd))"#);
        let cwd = ed.plugin_state.get("cwd").cloned().unwrap_or_default();
        assert!(!cwd.is_empty());
    }

    #[test]
    fn fs_cwd_matches_actual_cwd() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let expected = std::env::current_dir().unwrap().to_string_lossy().to_string();
        janet_bridge::eval(&mut ed,
            r#"(plugin-state/set "cwd" (fs/cwd))"#);
        let cwd = ed.plugin_state.get("cwd").cloned().unwrap_or_default();
        assert_eq!(cwd, expected);
    }

    // ── fs/chdir ──────────────────────────────────────────────────────────────

    #[test]
    fn fs_chdir_changes_working_directory() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let original = std::env::current_dir().unwrap();
        janet_bridge::eval(&mut ed, r#"(fs/chdir "/tmp")"#);
        janet_bridge::eval(&mut ed,
            r#"(plugin-state/set "new-cwd" (fs/cwd))"#);
        let new_cwd = ed.plugin_state.get("new-cwd").cloned().unwrap_or_default();
        // /tmp on macOS is /private/tmp via symlink
        assert!(new_cwd.contains("tmp"));
        std::env::set_current_dir(&original).ok();
    }

    // ── eval-region ──────────────────────────────────────────────────────────

    #[test]
    fn eval_region_evaluates_selection_and_emits_event() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);

        // Rust command handler to capture eval-result without nested fiber
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
        janet_bridge::eval(&mut ed, r#"
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
    }

    // ── eval-buffer ──────────────────────────────────────────────────────────

    #[test]
    fn eval_buffer_evaluates_entire_buffer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);

        janet_bridge::eval(&mut ed, r#"
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
    }

    // ── janet-output/write ────────────────────────────────────────────────────

    #[test]
    fn janet_output_write_appends_to_buffer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(janet-output/write "sprint12-test\n")"#);
        let buf_key = ed.buffers.iter()
            .find(|(_, b)| b.name == "*janet-output*")
            .map(|(k, _)| k);
        assert!(buf_key.is_some(), "*janet-output* buffer was not created");
        let buf = ed.buffers.get(buf_key.unwrap()).unwrap();
        let content = buf.slice(0, buf.len());
        assert!(content.contains("sprint12-test"));
    }

    // ── :janet colon verb ─────────────────────────────────────────────────────

    #[test]
    fn janet_colon_verb_is_in_plugins_table() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed,
            r#"(truthy? (get *colon-plugins* "janet"))"#).unwrap();
        assert_eq!(r, "true");
    }

    #[test]
    fn janet_repl_colon_verb_is_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed,
            r#"(truthy? (get *colon-plugins* "janet-repl"))"#).unwrap();
        assert_eq!(r, "true");
    }

    // ── MShell state ─────────────────────────────────────────────────────────

    #[test]
    fn mshell_cwd_initialized_to_process_cwd() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed,
            r#"(plugin-state/set "ms-cwd" *mshell-cwd*)"#);
        let cwd = ed.plugin_state.get("ms-cwd").cloned().unwrap_or_default();
        assert!(!cwd.is_empty());
    }

    #[test]
    fn mshell_builtins_table_exists() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed,
            r#"(truthy? *mshell-builtins*)"#).unwrap();
        assert_eq!(r, "true");
    }

    #[test]
    fn mshell_parse_line_janet_expr() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed,
            r#"(get (mshell/parse-line "(+ 1 2)") 0)"#).unwrap();
        assert!(r.contains("janet"), "expected :janet keyword, got: {r}");
    }

    #[test]
    fn mshell_parse_line_builtin_command() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed,
            r#"(get (mshell/parse-line "cd /tmp") 0)"#).unwrap();
        assert!(r.contains("builtin"), "expected :builtin keyword, got: {r}");
    }

    #[test]
    fn mshell_parse_line_shell_command() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed,
            r#"(get (mshell/parse-line "ls -la") 0)"#).unwrap();
        assert!(r.contains("shell"), "expected :shell keyword, got: {r}");
    }

    #[test]
    fn mshell_echo_builtin_works() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"
            (plugin-state/set "echo-out"
              ((get *mshell-builtins* "echo") ["hello" "world"]))"#);
        let out = ed.plugin_state.get("echo-out").cloned().unwrap_or_default();
        assert_eq!(out, "hello world");
    }

    #[test]
    fn mshell_colon_verbs_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r1 = janet_bridge::eval_result(&mut ed,
            r#"(truthy? (get *colon-plugins* "mshell"))"#).unwrap();
        let r2 = janet_bridge::eval_result(&mut ed,
            r#"(truthy? (get *colon-plugins* "ms"))"#).unwrap();
        assert_eq!(r1, "true");
        assert_eq!(r2, "true");
    }
}
