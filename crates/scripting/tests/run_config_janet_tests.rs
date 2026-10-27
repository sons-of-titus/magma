#[macro_use] mod helpers;
// Janet-side tests for run configurations: detection seeding through the
// bridge, the run-config commands, and `project/current` shape.

use scripting;

/// Like the shared `janet_test!`, but the editor carries a background
/// runtime — run-config launches need one.
macro_rules! janet_bg_test { ($ed:ident, $body:block) => {{
    let _lock = helpers::acquire_janet_lock();
    let mut $ed = helpers::make_editor();
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (bg_sender, _bg_receiver) = tokio::sync::mpsc::unbounded_channel();
    $ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, bg_sender));
    scripting::init(&mut $ed);
    let _editor_scope = scripting::EditorScope::new(&mut $ed);
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
        ed.project_manager.run_configs.upsert(kernel::run_config::RunConfig {
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

#[test]
fn dbg_start_launches_the_active_run_config() {
    if !helpers::python3_available() { return; }
    janet_bg_test!(ed, {
        let tmp = std::env::temp_dir().join("run_config_janet_dbg_start");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();
        let _ = std::fs::remove_file(tmp.join(".magma/run-configs.json"));
        let fake = helpers::spawn_fake_dap("janet_dbg_start");

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));
        ed.project_manager.run_configs.upsert(kernel::run_config::RunConfig {
            name: "run".to_string(),
            command: "/bin/echo".to_string(),
            args: vec!["hi".to_string()],
            cwd: None,
            env: Default::default(),
            build_before_run: None,
            debuggable: true,
        });
        ed.project_manager.run_configs.active = Some("run".to_string());

        let r = scripting::eval_result(&format!(
            r#"(editor/run-command "dbg-start" "{}")"#,
            fake.script.to_string_lossy()));
        assert!(r.is_ok(), "dbg-start must reach debug-config: {r:?}");

        // The active config's program must arrive at the adapter as the launch.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let lines = loop {
            let lines = helpers::dap_log_lines(&fake.log);
            if lines.iter().any(|l| l == "launch") || std::time::Instant::now() >= deadline {
                break lines;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        let idx = lines.iter().position(|l| l == "launch")
            .unwrap_or_else(|| panic!("dbg-start never sent launch: {lines:?}"));
        let args_line = lines.get(idx + 1).map(String::as_str).unwrap_or("");
        assert!(args_line.contains("/bin/echo"),
            "active config program must be the debuggee: {args_line:?}");

        ed.debug.disconnect_all();
        std::fs::remove_dir_all(&tmp).ok();
    });
}
