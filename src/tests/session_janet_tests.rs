//! Janet-side tests for project sessions: opening a project restores the
//! recorded working state automatically.

use crate::kernel::scripting;

#[test]
fn open_project_restores_the_previous_session() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("session_janet_restore");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();
        let file = tmp.join("src_lib.rs");
        std::fs::write(&file, "alpha\nbeta\ngamma\n").unwrap();
        let file_path = file.to_string_lossy().into_owned();

        // Open the project once, open a file, set a breakpoint; session
        // state is saved on close — here we snapshot it manually the way
        // close_editor_session does.
        scripting::eval(&format!("(editor/run-command \"open-project\" \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(editor/run-command \"open-file\" \"{}\")", file_path));
        scripting::eval(&format!("(debug/add-breakpoint \"{}\" 2)", file_path));
        scripting::eval("(debug/watch-add \"len\")");

        let state = crate::kernel::session::capture(&ed);
        crate::kernel::session::save(&ed, &tmp).unwrap();

        // A fresh editor, same project: open-project must restore.  The
        // fresh editor has no Janet runtime, so detection runs explicitly
        // (the bridge test for the handler chain is the `ed` side above).
        let mut fresh = crate::tests::helpers::make_editor();
        let mut args = std::collections::HashMap::new();
        args.insert("path".to_string(),
            crate::kernel::command::args::ArgValue::Path(tmp.to_string_lossy().into_owned()));
        crate::kernel::command::execute_command(&mut fresh, "open-project", &args).unwrap();
        fresh.project_manager.detect(&tmp.to_string_lossy());

        let reopened: Vec<String> = fresh.buffers.iter()
            .filter_map(|(_, arc)| arc.lock().unwrap().path.clone())
            .collect();
        assert!(reopened.contains(&file_path),
            "the recorded file reopens with the project: {reopened:?}");
        assert_eq!(fresh.debug.breakpoints_for(&file_path).len(), 1,
            "the recorded breakpoint returns");
        assert_eq!(fresh.debug.breakpoints_for(&file_path)[0].line, 2);
        assert_eq!(fresh.debug.watches, vec!["len".to_string()]);
        assert_eq!(fresh.project_manager.project.language, "rust");

        let _ = state;
        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_switch_resets_run_configs_and_restores_the_other_session() {
    janet_test!(ed, {
        let p1 = std::env::temp_dir().join("session_janet_p1");
        let p2 = std::env::temp_dir().join("session_janet_p2");
        let _ = std::fs::remove_dir_all(&p1);
        let _ = std::fs::remove_dir_all(&p2);
        std::fs::create_dir_all(&p1).unwrap();
        std::fs::create_dir_all(&p2).unwrap();
        std::fs::write(p1.join("Cargo.toml"), "").unwrap();
        std::fs::write(p2.join("go.mod"), "module x").unwrap();
        let p2_file = p2.join("main.go");
        std::fs::write(&p2_file, "package main\n").unwrap();

        // Project 1 with a breakpoint and a file open.  Detection follows
        // open-project exactly as the project-opened handler runs it.
        scripting::eval(&format!("(editor/run-command \"open-project\" \"{}\")", p1.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", p1.to_string_lossy()));
        let p1_marker = p1.join("marker.rs");
        std::fs::write(&p1_marker, "fn main() {}\n").unwrap();
        scripting::eval(&format!("(editor/run-command \"open-file\" \"{}\")", p1_marker.to_string_lossy()));
        scripting::eval(&format!("(debug/add-breakpoint \"{}\" 1)", p1_marker.to_string_lossy()));
        crate::kernel::session::save(&ed, &p1).unwrap();

        // Switching to project 2: project-1 state leaves the active store…
        scripting::eval(&format!("(editor/run-command \"open-project\" \"{}\")", p2.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", p2.to_string_lossy()));
        assert_eq!(ed.project_manager.project.language, "go");
        assert_eq!(ed.project_manager.run_configs.configs.len(), 3,
            "go detection seeds its own defaults");
        assert!(ed.debug.breakpoints.is_empty(),
            "the other project's breakpoints do not leak into the active store");

        // …and returns when project 1 reopens.
        scripting::eval(&format!("(editor/run-command \"open-project\" \"{}\")", p1.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", p1.to_string_lossy()));
        assert_eq!(ed.project_manager.project.language, "rust");
        assert_eq!(ed.debug.breakpoints_for(&p1_marker.to_string_lossy()).len(), 1,
            "project 1's breakpoint is restored");

        std::fs::remove_dir_all(&p1).ok();
        std::fs::remove_dir_all(&p2).ok();
    });
}
