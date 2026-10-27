#[macro_use] mod helpers;
// Janet-side end-to-end scenarios: gutter-click breakpoints through the
// real plugin, cross-project isolation, and the unsupported-project
// explanation.  Scenario numbering follows the audit's cross-feature list.

use kernel::command::execute_command;
use scripting;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("e2e_janet_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ── Scenario 6: click the gutter to create (and remove) a breakpoint ─────────

#[test]
fn s6_gutter_click_toggles_a_real_breakpoint() {
    janet_test!(ed, {
        let tmp = temp_root("s6");
        let file = tmp.join("main.rs");
        std::fs::write(&file, "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n").unwrap();
        let path = file.to_string_lossy().into_owned();
        open_file_cmd(&mut ed, &path);

        // A real click in the breakpoints gutter column (first column,
        // width 2), routed through the mouse layer exactly like the GUI.
        // The built-in toggle is native Rust inside dispatch_gutter_click.
        ed.options.insert("mouse-support".to_string(), "true".to_string());
        kernel::render::frame::sync_viewport_scroll(&mut ed);
        let pane = ed.view_tree.panes()[0].clone();
        let click = kernel::input::event::MouseEvent {
            kind: kernel::input::event::MouseEventKind::Click,
            x: pane.x + 1,
            y: pane.y + 3, // viewport row 3 → 0-based line 3 → stored 4
            button: kernel::input::event::MouseButton::Left,
            modifiers: String::new(),
        };
        let surface = kernel::render::surface::Surface::new(pane.width, pane.height);
        kernel::input::mouse::dispatch_mouse(&mut ed, &surface, &click);

        let bps = ed.debug.breakpoints_for(&path);
        assert_eq!(bps.len(), 1, "the gutter click registered the breakpoint");
        assert_eq!(bps[0].line, 4);

        // Second click on the same line removes it.
        kernel::input::mouse::dispatch_mouse(&mut ed, &surface, &click);
        assert!(ed.debug.breakpoints_for(&path).is_empty(), "the second click clears it");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

fn open_file_cmd(ed: &mut kernel::state::Editor, path: &str) {
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        kernel::command::args::ArgValue::Path(path.to_string()));
    execute_command(ed, "open-file", &args).unwrap();
}

// ── Scenario 11: switching projects isolates configuration and breakpoints ───

#[test]
fn s11_switching_projects_isolates_state() {
    janet_test!(ed, {
        let p1 = temp_root("s11_p1");
        let p2 = temp_root("s11_p2");
        std::fs::write(p1.join("Cargo.toml"), "").unwrap();
        std::fs::write(p2.join("go.mod"), "module x\n").unwrap();

        scripting::eval(&format!("(task/detect-project \"{}\")", p1.to_string_lossy()));
        assert_eq!(ed.project_manager.project.language, "rust");
        let rust_configs = ed.project_manager.run_configs.configs.clone();

        scripting::eval(&format!("(task/detect-project \"{}\")", p2.to_string_lossy()));
        assert_eq!(ed.project_manager.project.language, "go");
        assert_ne!(ed.project_manager.run_configs.configs, rust_configs,
            "each project's detection seeds its own configurations");

        // Breakpoints follow sessions, not projects: the active store is
        // per-project session state restored on open.
        let file = p2.join("main.go");
        std::fs::write(&file, "package main\n").unwrap();
        open_file_cmd(&mut ed, file.to_string_lossy().as_ref());
        scripting::eval(&format!("(debug/add-breakpoint \"{}\" 1)", file.to_string_lossy()));
        assert_eq!(ed.debug.breakpoints.len(), 1);
        kernel::session::save_if_project(&ed);

        std::fs::remove_dir_all(&p1).ok();
        std::fs::remove_dir_all(&p2).ok();
    });
}

// ── Scenario 13: an unrecognized project explains what is missing ────────────

#[test]
fn s13_unsupported_project_says_so() {
    janet_test!(ed, {
        let empty = temp_root("s13");
        scripting::eval(&format!("(task/detect-project \"{}\")", empty.to_string_lossy()));

        assert!(ed.project_manager.project.language.is_empty());
        assert!(ed.project_manager.run_configs.configs.is_empty(),
            "no configs are invented for an unrecognized folder");
        let msg = ed.status_message.as_ref().expect("the user is told, not silenced");
        assert!(msg.text.contains("No recognized build system"),
            "the message names the gap: {}", msg.text);

        std::fs::remove_dir_all(&empty).ok();
    });
}
