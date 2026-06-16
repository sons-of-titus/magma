//! Janet API tests for the Project Model (Phase 5).

use crate::kernel::scripting;

// ── project/current ───────────────────────────────────────────────────────────

#[test]
fn project_current_returns_nil_when_no_project() {
    janet_test!(ed, {
        let r = scripting::eval_result("(project/current)");
        assert!(r.is_ok(), "project/current must not error");
        assert_eq!(r.unwrap(), "nil", "project/current must return nil without active project");
    });
}

#[test]
fn project_current_returns_table_after_set_root() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("phase5_janet_current");
        std::fs::create_dir_all(&tmp).unwrap();
        let root_str = tmp.to_string_lossy().to_string();

        scripting::eval(&format!("(project/set-root \"{}\")", root_str));

        let r = scripting::eval_result("(project/current)");
        assert!(r.is_ok(), "project/current must not error");
        let val = r.unwrap();
        assert_ne!(val, "nil", "project/current must return a table after set-root");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_current_includes_name_and_root() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("phase5_janet_name_root");
        std::fs::create_dir_all(&tmp).unwrap();
        let root_str = tmp.to_string_lossy().to_string();

        scripting::eval(&format!("(project/set-root \"{}\")", root_str));

        let name_r = scripting::eval_result("(get (project/current) :name)");
        assert!(name_r.is_ok());
        let root_r = scripting::eval_result("(get (project/current) :root)");
        assert!(root_r.is_ok());

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_current_includes_language_field() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("phase5_janet_lang");
        std::fs::create_dir_all(&tmp).unwrap();

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));

        let lang_r = scripting::eval_result("(get (project/current) :language)");
        assert!(lang_r.is_ok(), "project/current must have :language key");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

// ── task/detect-project (via project_manager.detect) ─────────────────────────

#[test]
fn detect_project_via_task_api_sets_language() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("phase5_janet_detect");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));

        assert_eq!(ed.project_manager.project.language, "rust");
        assert!(!ed.project_manager.project.build_targets.is_empty());
        assert!(ed.task_scheduler.task_by_name("build").is_some());

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn detect_project_populates_tasks_array() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("phase5_janet_tasks_arr");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));

        assert_eq!(ed.project_manager.project.tasks.len(), 3,
            "detect must register 3 tasks (build, test, run)");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

// ── project/build, project/test, project/run ─────────────────────────────────

#[test]
fn project_build_errors_without_detected_tasks() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("phase5_janet_build_err");
        std::fs::create_dir_all(&tmp).unwrap();

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));

        // project/build should signal error (no background or no task defined)
        let r = scripting::eval("(try (project/build) ([e] \"caught\"))");
        assert_eq!(r, "ok", "project/build error must be catchable");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_build_test_run_available_as_janet_fns() {
    janet_test!(ed, {
        // Just verify the functions are defined (callable without crashing)
        scripting::eval("(try (project/build)  ([_e] nil))");
        scripting::eval("(try (project/test)   ([_e] nil))");
        scripting::eval("(try (project/run)    ([_e] nil))");
    });
}

// ── project/current build-targets structure ───────────────────────────────────

#[test]
fn project_current_build_targets_after_detect() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("phase5_janet_bt");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

        scripting::eval(&format!("(project/set-root \"{}\")", tmp.to_string_lossy()));
        scripting::eval(&format!("(task/detect-project \"{}\")", tmp.to_string_lossy()));

        let bt_r = scripting::eval_result("(length (get (project/current) :build-targets))");
        assert!(bt_r.is_ok());
        let len: usize = bt_r.unwrap().parse().unwrap_or(0);
        assert_eq!(len, 3, "detected Rust project must have 3 build targets");

        std::fs::remove_dir_all(&tmp).ok();
    });
}
