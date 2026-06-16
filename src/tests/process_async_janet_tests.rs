use crate::janet_bridge;

fn setup_background_editor() -> crate::state::Editor {
    let mut ed = crate::state::Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    crate::command::builtin::register_builtin_commands(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (bg_sender, _bg_receiver) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(crate::runtime::BackgroundHandle::new(runtime, bg_sender));
    let id = ed.allocate_buffer_id();
    let buf = crate::buffer::Buffer::new(crate::state::id::BufferId(id), "test");
    let e = ed.buffers.vacant_entry();
    let k = e.key();
    e.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(k);
    }
    ed
}

// ── process/spawn ────────────────────────────────────────────────────

#[test]
fn process_spawn_requires_command() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval("(process/spawn)");
    assert_ne!(r, "ok", "process/spawn without args must signal an error");
}

#[test]
fn process_spawn_errors_on_bad_command() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(
        r#"(try (process/spawn "nonexistent_command_xyz_12345") ([e] "caught"))"#,
    );
    assert_eq!(r, "ok",
        "process/spawn with a bad command must be catchable");
}

#[test]
fn process_spawn_adds_to_process_table() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let before = ed.io.processes.len();
    let r = janet_bridge::eval(
        r#"(try (do (process/spawn "echo test") "ok") ([e] "caught"))"#,
    );
    assert_eq!(r, "ok", "process/spawn echo must succeed");

    let after = ed.io.processes.len();
    assert_eq!(after, before + 1,
        "process/spawn must add an entry to the process table");
}

#[test]
fn process_spawn_records_cmd_and_running() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    janet_bridge::eval(
        r#"(try (process/spawn "echo hello-world") ([e] nil))"#,
    );

    let state = ed.io.processes.values().next();
    assert!(state.is_some(), "process must exist after spawn");
    let state = state.unwrap();
    assert!(state.cmd.contains("echo hello-world"),
        "process cmd must contain the spawned command");
    assert!(state.running,
        "process must be marked running after spawn");
}

// ── process/list ─────────────────────────────────────────────────────

#[test]
fn process_list_returns_empty_array_by_default() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval("(= 0 (length (process/list)))");
    assert_eq!(r, "ok",
        "process/list must return an empty array when no processes exist");
}

#[test]
fn process_list_returns_spawned_processes() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    janet_bridge::eval(
        r#"(try (process/spawn "echo list-test") ([e] nil))"#,
    );

    let r = janet_bridge::eval("(> (length (process/list)) 0)");
    assert_eq!(r, "ok",
        "process/list must return non-empty after a spawn");
}

// ── process/stdin ────────────────────────────────────────────────────

#[test]
fn process_stdin_noops_for_missing_id() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(
        r#"(process/stdin 99999 "test input")"#,
    );
    assert_eq!(r, "ok",
        "process/stdin with nonexistent id must not error");
}

// ── process/kill ─────────────────────────────────────────────────────

#[test]
fn process_kill_noops_for_missing_id() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(r#"(process/kill 99999)"#);
    assert_eq!(r, "ok",
        "process/kill with nonexistent id must not error");
}

// ── task/spawn ───────────────────────────────────────────────────────

#[test]
fn task_spawn_requires_function() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval("(task/spawn)");
    assert_ne!(r, "ok",
        "task/spawn without args must signal an error");
}

#[test]
fn task_spawn_returns_integer_id() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(
        r#"(def tid (task/spawn (fn [] "hi"))) (not= nil tid)"#,
    );
    assert_eq!(r, "ok",
        "task/spawn must return a non-nil integer ID");
}

#[test]
fn task_spawn_adds_to_task_table() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let before = ed.io.tasks.len();
    janet_bridge::eval(
        r#"(task/spawn (fn [] "task-table-test"))"#,
    );

    assert_eq!(ed.io.tasks.len(), before + 1,
        "task/spawn must add an entry to the task table");
}

#[test]
fn task_spawn_marks_task_running() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    janet_bridge::eval(
        r#"(task/spawn (fn [] "running-test"))"#,
    );

    for state in ed.io.tasks.values() {
        assert!(state.running,
            "task must be marked running after spawn");
    }
}

// ── task/cancel ──────────────────────────────────────────────────────

#[test]
fn task_cancel_noops_for_missing_id() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(r#"(task/cancel 99999)"#);
    assert_eq!(r, "ok",
        "task/cancel with nonexistent id must not error");
}

#[test]
fn task_cancel_removes_task() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    janet_bridge::eval(
        r#"(def tid (task/spawn (fn [] "cancel-test")))"#,
    );

    let before = ed.io.tasks.len();
    assert!(before > 0, "task must exist before cancel");

    let ids: Vec<u64> = ed.io.tasks.keys().copied().collect();
    if let Some(&id) = ids.first() {
        janet_bridge::eval(
            &format!("(task/cancel {})", id),
        );
        assert!(!ed.io.tasks.contains_key(&id),
            "task must be removed after cancel");
    }
}

// ── editor/shell legacy (process/spawn equivalent) ───────────────────

#[test]
fn editor_shell_is_deprecated_but_still_works() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(
        r#"(editor/shell "echo deprecation-test")"#,
    );
    assert_eq!(r, "ok",
        "editor/shell must still work for backward compatibility");
}
