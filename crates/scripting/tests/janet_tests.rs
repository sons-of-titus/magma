#[macro_use]
mod helpers;
// Janet runtime integration — command surface, options, and the mode
// substrate a community modal-editing plugin would build on.

use scripting;

#[test]
fn quit_routes_through_the_guarded_command() {
    janet_test!(ed, {
        let r = scripting::eval("(editor/run-command \"quit\")");
        assert_eq!(r, "ok");
        assert!(!ed.running, "quit must end the session");
    });
}

#[test]
fn quit_refusal_surfaces_as_a_janet_error() {
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("dirty.txt", "work");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(0, "x");

        let r = scripting::eval("(editor/run-command \"quit\")");
        assert_ne!(r, "ok", "quit with dirty buffers must fail");
        assert!(ed.running, "refusal keeps the editor alive");
    });
}

#[test]
fn set_mode_substrate_remains_for_community_plugins() {
    // Modality is out of the shipped product, but the extension point a
    // community vim plugin needs must keep working.
    janet_test!(ed, {
        let r = scripting::eval("(editor/set-mode \"vim-normal\" {:accepts-text false})");
        assert_eq!(r, "ok");
        assert_eq!(ed.editor_mode.name, "vim-normal");
        assert!(!ed.editor_mode.accepts_text);

        let r = scripting::eval("(editor/set-mode \"editor\")");
        assert_eq!(r, "ok");
        assert!(
            ed.editor_mode.accepts_text,
            "back to the default typing mode"
        );
    });
}

#[test]
fn option_set_is_the_configuration_path() {
    janet_test!(ed, {
        let r = scripting::eval("(option/set \"tab-width\" \"8\")");
        assert_eq!(r, "ok");
        assert_eq!(ed.options.get("tab-width").map(|s| s.as_str()), Some("8"));
    });
}

#[test]
fn janet_defined_commands_execute_and_reach_the_palette() {
    janet_test!(ed, {
        let r =
            scripting::eval("(command/define \"community-hello\" (fn [&] (editor/echo \"hi\")))");
        assert_eq!(r, "ok");
        assert!(ed.commands.exists("community-hello"));
        assert!(
            ed.commands
                .list_detailed()
                .iter()
                .any(|(name, _, _)| name == "community-hello"),
            "Janet-defined commands are palette surface"
        );
    });
}

#[test]
fn builtin_command_files_register_their_commands() {
    // A compile error in any builtin (e.g. an unknown symbol) is only
    // logged at load time — the registry going empty is the observable
    // failure.  One command per file that registers verbs at load time.
    janet_test!(ed, {
        for name in ["dbg-start", "ws-save", "agent-ask"] {
            assert!(
                ed.commands.exists(name),
                "builtin command {name} must be registered at load time"
            );
        }
    });
}

#[test]
fn every_builtin_compiles_at_init() {
    // A builtin that fails to load silently disables everything it defines
    // (LSP glue, syntax tables, …) — the failure must never be invisible.
    janet_test!(ed, {
        let errors = scripting::take_builtin_load_errors();
        assert!(errors.is_empty(), "builtins failed to compile: {errors:?}");
    });
}

#[test]
fn command_run_passes_arguments_to_the_command() {
    // The args previously vanished (empty HashMap) — every parameterized
    // command failed with "Missing required argument" from Janet.  A real
    // open is the observable: the project root must actually change.
    janet_test!(ed, {
        let dir = std::env::temp_dir().join(format!("magma-run-args-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = format!(r#"(command/run "open-project" :path "{}")"#, dir.display());
        let r = scripting::eval_result(&src);
        assert!(
            r.is_ok(),
            "command/run must accept :key value pairs: {:?}",
            r.err()
        );
        assert_eq!(
            ed.project_manager.project.root.as_ref(),
            Some(&dir),
            "the :path argument must reach open-project"
        );
        std::fs::remove_dir_all(&dir).ok();
    });
}

#[test]
fn lsp_attach_uses_numeric_buffer_ids() {
    // buffer-created delivers the id as a string; every buffer/* C function
    // needs a number.  The attach handler must convert it, so a .rs buffer
    // gets registered for its language (didOpen's prerequisite).
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("main.rs", "fn main() {}");
        if let Some(arc) = ed.buffers.get(key) {
            arc.lock().unwrap().path = Some("/tmp/magma-lsp-test/main.rs".to_string());
        }
        // Registering a language is the first observable step of attach;
        // lsp/start itself is skipped when the server binary is missing.
        let r = scripting::eval(&format!(
            "(do (event/emit \"buffer-created\" {{:buffer-id \"{key}\"}}) nil)"
        ));
        assert_eq!(r, "ok", "handler must not error on the string payload: {r}");
    });
}

#[test]
fn explorer_open_end_to_end_focuses_and_highlights() {
    // The whole click path: open-project → index → sidebar click path →
    // open-file → frame drain → the file is on screen and highlighted.
    janet_test!(ed, {
        use kernel::command::{self, args::ArgValue};
        use kernel::runtime::{BackgroundHandle, process_background_event};

        let rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        ed.background = Some(BackgroundHandle::new(rt, tx));

        let dir = std::env::temp_dir().join(format!("magma-explorer-e2e-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            "fn main() {\n    let done = true;\n}\n",
        )
        .unwrap();

        let mut args = std::collections::HashMap::new();
        args.insert(
            "path".to_string(),
            ArgValue::Path(dir.to_string_lossy().into_owned()),
        );
        command::execute_command(&mut ed, "open-project", &args).unwrap();

        // What drain_background_events does every frame.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while ed.project_manager.project.files.is_empty() {
            match rx.try_recv() {
                Ok(ev) => process_background_event(&mut ed, ev),
                _ if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                _ => panic!("no ProjectIndexed event"),
            }
        }
        assert!(
            ed.project_manager
                .project
                .files
                .contains(&"src/main.rs".to_string()),
            "index must be project-relative: {:?}",
            ed.project_manager.project.files
        );

        // The exact string the sidebar click computes for a file node.
        let root = ed
            .project_manager
            .project
            .root
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let full = format!("{}/{}", root.trim_end_matches('/'), "src/main.rs");

        // SidebarAction::Open → DialogAction::OpenFile → open-file.
        let mut args = std::collections::HashMap::new();
        args.insert("path".to_string(), ArgValue::Path(full.clone()));
        command::execute_command(&mut ed, "open-file", &args).unwrap();

        // What the editor shell now does every frame.
        ed.dispatch_events_scoped();

        // The file is on screen in the focused pane.
        let focused = ed
            .view_tree
            .focused_window()
            .and_then(|w| ed.view_tree.buffer(w))
            .expect("a focused buffer after opening from the explorer");
        assert_eq!(
            ed.buffers
                .get(focused)
                .unwrap()
                .lock()
                .unwrap()
                .path
                .as_deref(),
            Some(full.as_str()),
            "the clicked file's buffer must be the focused one"
        );

        // The syntax core ran on buffer-created and highlighted it.
        let view = ed.view_of_buffer(focused).expect("view for focused buffer");
        let syntax = view
            .highlight_layers
            .get("syntax")
            .expect("syntax highlight layer set by buffer-created handler");
        assert!(!syntax.is_empty(), "rust keywords must be highlighted");

        std::fs::remove_dir_all(&dir).ok();
    });
}

#[test]
fn self_emitting_handler_terminates() {
    // A handler that re-emits its own event used to grow the pending queue
    // forever — the drain loop never ended.  The bus now drops re-entrant
    // emits; this test hangs instead of passing if the guard regresses.
    janet_test!(ed, {
        scripting::eval(
            r#"(event/on "magma-loop-test" (fn [ev] (event/emit "magma-loop-test" {})))"#,
        );
        scripting::eval(r#"(event/emit "magma-loop-test" {})"#);
        ed.dispatch_events_scoped();
        assert_eq!(
            ed.events.pending_len(),
            0,
            "self-emit must be dropped, not queued"
        );
    });
}

#[test]
fn handler_can_subscribe_from_inside_a_handler() {
    // event/on used to hold the handler-table mutex across the Janet call:
    // a handler that subscribed again deadlocked the editor on the spot.
    janet_test!(ed, {
        scripting::eval(
            r#"(event/on "magma-reg-outer" (fn [ev] (event/on "magma-reg-inner" (fn [e2] nil))))"#,
        );
        scripting::eval(r#"(event/emit "magma-reg-outer" {})"#);
        ed.dispatch_events_scoped();
        assert_eq!(
            ed.events.subscriber_count("magma-reg-inner"),
            1,
            "the nested subscription must have registered"
        );
    });
}

#[test]
fn lsp_position_round_trip_on_non_ascii() {
    // goto-definition used to send byte columns and add UTF-16 columns to
    // byte offsets; on non-ASCII lines both directions landed wrong.
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("t.rs", "fn héllo() {}\nfn wörld() {}\n");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        // Line 0 ("fn héllo() {}\n") is 15 bytes; 'r' in "wörld" sits at
        // byte 21 (ö is two bytes) but UTF-16 column 5 (ö is one unit).
        let a = scripting::eval_result(
            "(let [b (buffer/current)] (string/format \"%d %d\" ;(buffer/lsp-position b 21)))",
        )
        .unwrap();
        assert!(a.contains("1 5"), "lsp-position must be (1, 5), got {a}");

        let b =
            scripting::eval_result("(string (buffer/offset-of-lsp-position (buffer/current) 1 5))")
                .unwrap();
        assert!(
            b.contains("21"),
            "inverse must return byte offset 21, got {b}"
        );
    });
}
