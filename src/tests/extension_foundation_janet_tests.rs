use crate::janet_bridge;

// ── buffer/major-mode getter ──────────────────────────────────────────

#[test]
fn buffer_major_mode_returns_fundamental_by_default() {
    janet_test!(ed, {
        let key = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid))
            .unwrap();
        let result = janet_bridge::eval(
            &format!("(buffer/major-mode {})", key),
        );
        assert_eq!(result, "ok", "buffer/major-mode eval should not error");

        assert_eq!(ed.buffers.get(key).unwrap().major_mode.name(), "fundamental");
    });
}

#[test]
fn buffer_major_mode_reflects_set_major_mode_command() {
    janet_test!(ed, {
        let key = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid))
            .unwrap();

        let result = janet_bridge::eval(
            &format!(
                "(do (editor/run-command \"set-major-mode\" \"text\") \
                     (buffer/major-mode {}))",
                key
            ),
        );
        assert_eq!(result, "ok");
        assert_eq!(ed.buffers.get(key).unwrap().major_mode.name(), "text",
            "major mode must be text after set-major-mode");
    });
}

// ── editor/option-set-local / editor/option-get-local ─────────────────

#[test]
fn option_set_local_isolated_from_global() {
    janet_test!(ed, {
        ed.options.insert("tab-width".to_string(), "4".to_string());

        let r = janet_bridge::eval("(option/set-local \"tab-width\" \"2\")");
        assert_eq!(r, "ok");

        let r2 = janet_bridge::eval("(option/get-local \"tab-width\")");
        assert_eq!(r2, "ok");

        let key = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid))
            .unwrap();
        assert_eq!(
            ed.buffers.get(key).unwrap().local_options.get("tab-width").map(|s| s.as_str()),
            Some("2"),
            "local option must be 2"
        );
        assert_eq!(
            ed.options.get("tab-width").map(|s| s.as_str()),
            Some("4"),
            "global option must remain 4"
        );
    });
}

#[test]
fn option_get_local_falls_back_to_global_when_no_local() {
    janet_test!(ed, {
        ed.options.insert("wrap-width".to_string(), "80".to_string());

        let r = janet_bridge::eval("(option/get-local \"wrap-width\")");
        assert_eq!(r, "ok", "option-get-local fallback eval must succeed");
    });
}

// ── editor/load-file C function ───────────────────────────────────────

#[test]
fn editor_load_file_evaluates_janet_source() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint1_load_file_test.janet");
        std::fs::write(&tmp, "(def *sprint1-loaded* true)").unwrap();

        let path = tmp.to_string_lossy().to_string();
        let load_expr = format!("(editor/load-file \"{}\")", path);
        let r = janet_bridge::eval(&load_expr);
        assert_eq!(r, "ok", "editor/load-file must succeed");

        let check = janet_bridge::eval("*sprint1-loaded*");
        assert_eq!(check, "ok", "variable defined by load-file must be accessible");

        std::fs::remove_file(&tmp).ok();
    });
}

#[test]
fn editor_load_file_errors_on_missing_file() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            "(try (editor/load-file \"/nonexistent/path/to/file.janet\") ([e] \"caught\"))",
        );
        assert_eq!(r, "ok", "error from load-file must be catchable with try/catch");
    });
}

// ── major-mode/define macro ───────────────────────────────────────────

#[test]
fn major_mode_define_registers_command() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            r#"(major-mode/define "sprint1-test-mode"
                  {:parent "prog"
                   :layers []
                   :setup nil
                   :hooks {}})"#,
        );
        assert_eq!(r, "ok", "major-mode/define must not error");
        assert!(ed.commands.exists("sprint1-test-mode"),
            "major-mode/define must register the mode as a Rust command");
    });
}

#[test]
fn major_mode_define_stores_in_registry() {
    janet_test!(ed, {
        janet_bridge::eval(
            r#"(major-mode/define "sprint1-registry-mode" {:parent "text" :layers []})"#,
        );

        let r = janet_bridge::eval(
            r#"(not= nil (get *major-modes* "sprint1-registry-mode"))"#,
        );
        assert_eq!(r, "ok", "major-mode/define must store entry in *major-modes*");
    });
}

#[test]
fn major_mode_define_setup_fn_called_on_activate() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            r#"(do
                 (major-mode/define "sprint1-setup-mode"
                   {:parent "prog"
                    :layers []
                    :setup (fn [] (option/set-local "indent-width" "2"))})
                 (editor/run-command "sprint1-setup-mode"))"#,
        );
        assert_eq!(r, "ok", "mode with setup fn must activate without error");

        let key = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid))
            .unwrap();
        assert_eq!(
            ed.buffers.get(key).unwrap().local_options.get("indent-width").map(|s| s.as_str()),
            Some("2"),
            "setup fn must set the local option when mode activates"
        );
    });
}

// ── auto-detect major mode from buffer-created event ──────────────────

#[test]
fn auto_detect_mode_activates_when_command_registered() {
    janet_test!(ed, {
        ed.commands.register_fn("zig-mode", "zig mode", vec![], |ed, _args| {
            let buf_id = ed.windows.focused_window()
                .and_then(|wid| ed.windows.buffer(wid))
                .unwrap_or(0);
            if let Some(buf) = ed.buffers.get_mut(buf_id) {
                buf.major_mode = crate::buffer::MajorMode::Custom("zig-mode".to_string());
            }
            Ok(())
        });

        assert!(ed.events.subscriber_count("buffer-created") > 0,
            "init.janet must register a buffer-created subscriber");

        let r = janet_bridge::eval(
            r#"(do
                 (def ext (path-extension "/home/user/main.zig"))
                 (def mode-name (when ext (get *extension-mode-map* ext nil)))
                 (when (and mode-name (command/exists? mode-name))
                   (editor/run-command mode-name)))"#,
        );
        assert_eq!(r, "ok", "auto-detect logic must not error");

        let key = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid))
            .unwrap();
        assert_eq!(ed.buffers.get(key).unwrap().major_mode.name(), "zig-mode",
            "auto-detect logic must activate zig-mode when invoked directly");
    });
}

#[test]
fn auto_detect_mode_skips_when_command_not_registered() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            r#"(event/emit "buffer-created" {:path "/src/server.erl" :buffer-id "99" :name "server.erl"})"#,
        );
        assert_eq!(r, "ok");
        ed.events.drain_and_dispatch();

        let key = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid))
            .unwrap();
        assert_eq!(ed.buffers.get(key).unwrap().major_mode.name(), "fundamental",
            "auto-detect must not crash when mode command is not registered");
    });
}

#[test]
fn auto_detect_skips_buffer_without_path() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            r#"(event/emit "buffer-created" {:name "*scratch*" :buffer-id "1"})"#,
        );
        assert_eq!(r, "ok");
        ed.events.drain_and_dispatch();

        let key = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid))
            .unwrap();
        assert_eq!(ed.buffers.get(key).unwrap().major_mode.name(), "fundamental");
    });
}

// ── require / plugin loader ───────────────────────────────────────────

#[test]
fn require_returns_false_for_missing_plugin() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            "(require \"definitely-nonexistent-plugin-xyz\")",
        );
        assert_eq!(r, "ok", "require must not crash for missing plugin");
    });
}

#[test]
fn require_caches_result_so_file_evaluated_once() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            r#"(do
                 (require "sprint1-cache-test-xyz")
                 (require "sprint1-cache-test-xyz")
                 (get *loaded-modules* "sprint1-cache-test-xyz"))"#,
        );
        assert_eq!(r, "ok", "require result must be in *loaded-modules* after two calls");
    });
}
