//! Janet API tests for the Task System (Phase 4).

use crate::kernel::scripting;
use crate::kernel::task::TaskStatus;

fn setup_background_editor() -> crate::kernel::state::Editor {
    let mut ed = crate::kernel::state::Editor::new(
        Box::new(crate::kernel::storage::disk::DiskFileSystem::new()),
    );
    crate::kernel::command::builtin::register_builtin_commands(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (bg_sender, _bg_receiver) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(crate::kernel::runtime::BackgroundHandle::new(runtime, bg_sender));
    let k = ed.create_buffer("test");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(k);
    }
    ed
}

// ── task/define ───────────────────────────────────────────────────────────────

#[test]
fn task_define_returns_integer_id() {
    janet_test!(ed, {
        let r = scripting::eval_result(r#"(task/define "build" "cargo build")"#);
        let id_str = r.expect("task/define must return ok");
        let id: i64 = id_str.parse().expect("id must be numeric");
        assert!(id >= 1, "task id must be >= 1");
    });
}

#[test]
fn task_define_same_name_returns_same_id() {
    janet_test!(ed, {
        let id1 = scripting::eval_result(r#"(task/define "build" "cargo build")"#).unwrap();
        let id2 = scripting::eval_result(r#"(task/define "build" "make")"#).unwrap();
        assert_eq!(id1, id2, "redefining the same name must return the same id");
    });
}

#[test]
fn task_define_multiple_tasks() {
    janet_test!(ed, {
        scripting::eval(r#"(task/define "build" "cargo build")"#);
        scripting::eval(r#"(task/define "test" "cargo test")"#);
        assert_eq!(ed.task_scheduler.tasks.len(), 2);
    });
}

#[test]
fn task_define_with_deps_array() {
    janet_test!(ed, {
        let dep_id: u64 = scripting::eval_result(r#"(task/define "lint" "cargo clippy")"#)
            .unwrap().parse().unwrap();
        scripting::eval(&format!(r#"(task/define "ci" "echo ci" @[{}])"#, dep_id));
        let ci_id = ed.task_scheduler.task_by_name("ci").unwrap();
        let ci_task = ed.task_scheduler.tasks.get(&ci_id).unwrap();
        assert_eq!(ci_task.dependencies, vec![dep_id]);
    });
}

// ── task/status ───────────────────────────────────────────────────────────────

#[test]
fn task_status_pending_after_define() {
    janet_test!(ed, {
        let id: u64 = scripting::eval_result(r#"(task/define "build" "cargo build")"#)
            .unwrap().parse().unwrap();
        let status = scripting::eval_result(&format!("(task/status {})", id)).unwrap();
        assert_eq!(status, ":pending");
    });
}

#[test]
fn task_status_not_found_for_unknown_id() {
    janet_test!(ed, {
        let r = scripting::eval_result("(task/status 99999)").unwrap();
        assert_eq!(r, ":not-found");
    });
}

// ── task/list ─────────────────────────────────────────────────────────────────

#[test]
fn task_list_empty_initially() {
    janet_test!(ed, {
        let r = scripting::eval_result("(length (task/list))").unwrap();
        assert_eq!(r, "0");
    });
}

#[test]
fn task_list_after_define() {
    janet_test!(ed, {
        scripting::eval(r#"(task/define "build" "cargo build")"#);
        scripting::eval(r#"(task/define "test" "cargo test")"#);
        let r = scripting::eval_result("(length (task/list))").unwrap();
        assert_eq!(r, "2");
    });
}

#[test]
fn task_list_entries_have_expected_keys() {
    janet_test!(ed, {
        scripting::eval(r#"(task/define "build" "cargo build")"#);
        let r = scripting::eval(
            r#"(let [entry (get (task/list) 0)] (and entry (get entry :name) (get entry :status) (get entry :command)))"#,
        );
        assert_eq!(r, "ok");
    });
}

// ── task/output ───────────────────────────────────────────────────────────────

#[test]
fn task_output_nil_for_pending_task() {
    janet_test!(ed, {
        let id: u64 = scripting::eval_result(r#"(task/define "build" "cargo build")"#)
            .unwrap().parse().unwrap();
        let r = scripting::eval_result(&format!("(nil? (task/output {}))", id)).unwrap();
        assert_eq!(r, "true");
    });
}

// ── task/cancel ───────────────────────────────────────────────────────────────

#[test]
fn task_cancel_unknown_id_is_noop() {
    janet_test!(ed, {
        let r = scripting::eval("(task/cancel 99999)");
        assert_eq!(r, "ok");
    });
}

#[test]
fn task_cancel_pending_task_sets_cancelled() {
    janet_test!(ed, {
        let id: u64 = scripting::eval_result(r#"(task/define "build" "cargo build")"#)
            .unwrap().parse().unwrap();
        scripting::eval(&format!("(task/cancel {})", id));
        let status = scripting::eval_result(&format!("(task/status {})", id)).unwrap();
        assert_eq!(status, ":cancelled");
        assert_eq!(ed.task_scheduler.status(id), Some(&TaskStatus::Cancelled));
    });
}

// ── task/on-complete ─────────────────────────────────────────────────────────

#[test]
fn task_on_complete_requires_function() {
    janet_test!(ed, {
        let r = scripting::eval(r#"(task/on-complete 1 "not-a-function")"#);
        assert_ne!(r, "ok", "task/on-complete with non-function must signal error");
    });
}

#[test]
fn task_on_complete_accepts_function() {
    janet_test!(ed, {
        let r = scripting::eval(r#"(task/on-complete 1 (fn [_] nil))"#);
        assert_eq!(r, "ok");
    });
}

// ── task/run ────────────────────────────────────────────────────────────────

#[test]
fn task_run_errors_without_background() {
    janet_test!(ed, {
        scripting::eval(r#"(task/define "build" "echo x")"#);
        let r = scripting::eval(r#"(task/run "build")"#);
        assert_ne!(r, "ok", "task/run without background must signal error");
    });
}

#[test]
fn task_run_succeeds_with_background() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);

    scripting::eval(r#"(task/define "echo-test" "echo hello")"#);
    let r = scripting::eval(r#"(task/run "echo-test")"#);
    assert_eq!(r, "ok", "task/run with background must succeed");

    let id = ed.task_scheduler.task_by_name("echo-test").unwrap();
    assert_eq!(ed.task_scheduler.status(id), Some(&TaskStatus::Running),
        "task must be Running after task/run");
}

#[test]
fn task_run_unknown_name_errors() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);

    let r = scripting::eval(r#"(task/run "nonexistent-task")"#);
    assert_ne!(r, "ok", "task/run with unknown name must signal error");
}

// ── task/detect-project ─────────────────────────────────────────────────────

#[test]
fn task_detect_project_empty_root_noops() {
    janet_test!(ed, {
        let r = scripting::eval(r#"(task/detect-project "")"#);
        assert_eq!(r, "ok");
        assert!(ed.task_scheduler.tasks.is_empty());
    });
}

#[test]
fn task_detect_project_cargo_defines_tasks() {
    janet_test!(ed, {
        let dir = std::env::temp_dir().join("magma_janet_detect_cargo");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join("Cargo.toml"), "[package]").ok();
        let _ = std::fs::remove_file(dir.join("package.json"));
        let _ = std::fs::remove_file(dir.join("Makefile"));

        scripting::eval(&format!(r#"(task/detect-project "{}")"#, dir.to_string_lossy()));
        assert!(ed.task_scheduler.task_by_name("build").is_some());
        assert!(ed.task_scheduler.task_by_name("test").is_some());
        assert!(ed.task_scheduler.task_by_name("run").is_some());

        std::fs::remove_dir_all(&dir).ok();
    });
}
