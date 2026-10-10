//! Janet-side tests for run configurations: detection seeding through the
//! bridge, the run-config commands, and `project/current` shape.

use crate::kernel::scripting;

/// Like the shared `janet_test!`, but the editor carries a background
/// runtime — run-config launches need one.
macro_rules! janet_bg_test { ($ed:ident, $body:block) => {{
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut $ed = crate::tests::helpers::make_editor();
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (bg_sender, _bg_receiver) = tokio::sync::mpsc::unbounded_channel();
    $ed.background = Some(crate::kernel::runtime::BackgroundHandle::new(runtime, bg_sender));
    crate::kernel::scripting::init(&mut $ed);
    let _editor_scope = crate::kernel::scripting::EditorScope::new(&mut $ed);
    $body
}};}

#[test]
fn project_current_reports_run_configs_and_active() {
    janet_bg_test!(ed, {
        let tmp = std::env::temp_dir().join("run_config_janet_current");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();
        let _ = std::fs::remove_file(tmp.join(".magma/run-configs.json"));

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));

        let names = scripting::eval_result(
            r#"(string/join (map |(string $) (get (project/current) :run-configs)) ",")"#)
            .unwrap();
        assert_eq!(names, "\"build,test,run\"",
            "eval_result renders string results in Janet quoting");
        let active = scripting::eval_result(r#"(get (project/current) :active-config)"#).unwrap();
        assert_eq!(active, "\"build\"");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn select_config_sets_active_and_persists() {
    janet_bg_test!(ed, {
        let tmp = std::env::temp_dir().join("run_config_janet_select");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();
        let _ = std::fs::remove_file(tmp.join(".magma/run-configs.json"));

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));

        let r = scripting::eval_result(r#"(editor/run-command "select-config" "test")"#);
        assert!(r.is_ok(), "select-config must succeed: {r:?}");
        assert_eq!(ed.project_manager.run_configs.active.as_deref(), Some("test"));

        let file = tmp.join(".magma/run-configs.json");
        assert!(file.exists(), "select-config must persist the store");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"active\": \"test\""), "active selection saved: {text}");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn run_config_with_unknown_name_is_catchable_error() {
    janet_bg_test!(ed, {
        let tmp = std::env::temp_dir().join("run_config_janet_unknown");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));

        let r = scripting::eval(r#"(try (editor/run-command "run-config" "no-such-config") ([e] "caught"))"#);
        assert_eq!(r, "ok", "the launch error must be catchable from Janet");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn run_config_launches_the_named_config_process() {
    janet_bg_test!(ed, {
        let tmp = std::env::temp_dir().join("run_config_janet_launch");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();
        let _ = std::fs::remove_file(tmp.join(".magma/run-configs.json"));

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));
        // Swap `cargo test` for an instant no-op so the suite never builds.
        scripting::eval(r#"(editor/run-command "select-config" "test")"#);
        ed.project_manager.run_configs.upsert(crate::kernel::run_config::RunConfig {
            name: "test".to_string(),
            command: "/usr/bin/true".to_string(),
            args: vec![],
            cwd: None,
            env: Default::default(),
            build_before_run: None,
            debuggable: false,
        });

        let r = scripting::eval_result(r#"(editor/run-command "run-config" "test")"#);
        assert!(r.is_ok(), "run-config must launch: {r:?}");
        assert!(ed.task_scheduler.task_by_name("test").is_some(),
            "the config launches as a named task");

        std::fs::remove_dir_all(&tmp).ok();
    });
}
