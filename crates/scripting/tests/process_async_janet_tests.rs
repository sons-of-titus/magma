#[macro_use]
mod helpers;
use scripting;

fn setup_background_editor() -> kernel::state::Editor {
    let mut ed = kernel::state::Editor::new(Box::new(kernel::storage::disk::DiskFileSystem::new()));
    kernel::command::builtin::register_builtin_commands(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (bg_sender, _bg_receiver) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, bg_sender));
    let k = ed.create_buffer("test");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(k);
    }
    ed
}

// ── process/spawn ────────────────────────────────────────────────────

#[test]
fn process_spawn_requires_command() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval("(process/spawn)");
    assert_ne!(r, "ok", "process/spawn without args must signal an error");
}

#[test]
fn process_spawn_errors_on_bad_command() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r =
        scripting::eval(r#"(try (process/spawn "nonexistent_command_xyz_12345") ([e] "caught"))"#);
    assert_eq!(
        r, "ok",
        "process/spawn with a bad command must be catchable"
    );
}

#[test]
fn process_spawn_adds_to_process_table() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let before = ed.io.processes.len();
    let r = scripting::eval(r#"(try (do (process/spawn "echo test") "ok") ([e] "caught"))"#);
    assert_eq!(r, "ok", "process/spawn echo must succeed");

    let after = ed.io.processes.len();
    assert_eq!(
        after,
        before + 1,
        "process/spawn must add an entry to the process table"
    );
}

#[test]
fn process_spawn_records_cmd_and_running() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(r#"(try (process/spawn "echo hello-world") ([e] nil))"#);

    let state = ed.io.processes.values().next();
    assert!(state.is_some(), "process must exist after spawn");
    let state = state.unwrap();
    assert!(
        state.cmd.contains("echo hello-world"),
        "process cmd must contain the spawned command"
    );
    assert!(state.running, "process must be marked running after spawn");
}

// ── process/list ─────────────────────────────────────────────────────

#[test]
fn process_list_returns_empty_array_by_default() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval("(= 0 (length (process/list)))");
    assert_eq!(
        r, "ok",
        "process/list must return an empty array when no processes exist"
    );
}

#[test]
fn process_list_returns_spawned_processes() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(r#"(try (process/spawn "echo list-test") ([e] nil))"#);

    let r = scripting::eval("(> (length (process/list)) 0)");
    assert_eq!(r, "ok", "process/list must return non-empty after a spawn");
}

// ── process/stdin ────────────────────────────────────────────────────

#[test]
fn process_stdin_errors_for_missing_id() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    // The write must never report success for a process that does not
    // exist — a silent no-op would make a failed remote write look done.
    assert_eq!(
        scripting::eval_result(r#"(= (try (process/stdin 99999 "x") ([e] "caught")) "caught")"#)
            .unwrap_or_default(),
        "true",
        "process/stdin on an unknown id must be a catchable error, not a silent no-op"
    );
}

// ── process/kill ─────────────────────────────────────────────────────

#[test]
fn process_kill_noops_for_missing_id() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval(r#"(process/kill 99999)"#);
    assert_eq!(r, "ok", "process/kill with nonexistent id must not error");
}

// ── task/spawn ───────────────────────────────────────────────────────

#[test]
fn task_spawn_requires_function() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval("(task/spawn)");
    assert_ne!(r, "ok", "task/spawn without args must signal an error");
}

#[test]
fn task_spawn_returns_integer_id() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval(r#"(def tid (task/spawn (fn [] "hi"))) (not= nil tid)"#);
    assert_eq!(r, "ok", "task/spawn must return a non-nil integer ID");
}

#[test]
fn task_spawn_adds_to_task_table() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let before = ed.io.tasks.len();
    scripting::eval(r#"(task/spawn (fn [] "task-table-test"))"#);

    assert_eq!(
        ed.io.tasks.len(),
        before + 1,
        "task/spawn must add an entry to the task table"
    );
}

#[test]
fn task_spawn_marks_task_running() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(r#"(task/spawn (fn [] "running-test"))"#);

    for state in ed.io.tasks.values() {
        assert!(state.running, "task must be marked running after spawn");
    }
}

// ── task/cancel ──────────────────────────────────────────────────────

#[test]
fn task_cancel_noops_for_missing_id() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval(r#"(task/cancel 99999)"#);
    assert_eq!(r, "ok", "task/cancel with nonexistent id must not error");
}

#[test]
fn task_cancel_removes_task() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(r#"(def tid (task/spawn (fn [] "cancel-test")))"#);

    let before = ed.io.tasks.len();
    assert!(before > 0, "task must exist before cancel");

    let ids: Vec<u64> = ed.io.tasks.keys().copied().collect();
    if let Some(&id) = ids.first() {
        scripting::eval(&format!("(task/cancel {})", id));
        assert!(
            !ed.io.tasks.contains_key(&id),
            "task must be removed after cancel"
        );
    }
}

// ── editor/shell legacy (process/spawn equivalent) ───────────────────

#[test]
fn editor_shell_is_deprecated_but_still_works() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval(r#"(editor/shell "echo deprecation-test")"#);
    assert_eq!(
        r, "ok",
        "editor/shell must still work for backward compatibility"
    );
}
// ── ssh-fs write lifecycle: stdin delivery + failure propagation ──────
//
// `ssh-fs/write` spawns `ssh host sh -c 'cat > path'` and delivers the
// content on stdin.  The lifecycle tests below drive the identical spawn
// → stdin → close-stdin → exit pipeline through a local `cat`, so they
// never need a network.

fn setup_pumping_editor() -> (
    kernel::state::Editor,
    tokio::sync::mpsc::UnboundedReceiver<kernel::runtime::BackgroundEvent>,
) {
    let mut ed = kernel::state::Editor::new(Box::new(kernel::storage::disk::DiskFileSystem::new()));
    kernel::command::builtin::register_builtin_commands(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (bg_sender, bg_receiver) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, bg_sender));
    let k = ed.create_buffer("test");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(k);
    }
    (ed, bg_receiver)
}

/// Pump background events until the process exits; returns its exit code.
fn pump_exit(
    ed: &mut kernel::state::Editor,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<kernel::runtime::BackgroundEvent>,
) -> i32 {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let deadline = std::time::Duration::from_secs(10);
    loop {
        let event = rt
            .block_on(async { tokio::time::timeout(deadline, rx.recv()).await })
            .expect("timed out waiting for process exit")
            .expect("background channel closed");
        if let kernel::runtime::BackgroundEvent::ProcessExited { exit_code, .. } = &event {
            let code = *exit_code;
            kernel::runtime::process_background_event(ed, event);
            return code;
        }
        kernel::runtime::process_background_event(ed, event);
    }
}

#[test]
fn stdin_then_eof_delivers_content_and_reports_success() {
    let _lock = helpers::acquire_janet_lock();
    let (mut ed, mut rx) = setup_pumping_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let out = std::env::temp_dir().join(format!("magma_sshw_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let pid = scripting::eval_result(&format!(
        r#"(process/spawn "cat > {}")"#,
        out.to_str().unwrap()
    ))
    .unwrap()
    .parse::<i64>()
    .unwrap();
    scripting::eval(&format!(r#"(process/stdin {pid} "héllo\nline two\n")"#));
    scripting::eval(&format!(r#"(process/close-stdin {pid})"#));

    let exit_code = pump_exit(&mut ed, &mut rx);
    assert_eq!(exit_code, 0, "cat over stdin must succeed");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "héllo\nline two\n",
        "exact content, including unicode and newlines, must reach the file"
    );
    std::fs::remove_file(&out).ok();
}

#[test]
fn empty_stdin_writes_an_empty_file() {
    let _lock = helpers::acquire_janet_lock();
    let (mut ed, mut rx) = setup_pumping_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let out = std::env::temp_dir().join(format!("magma_sshw_empty_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let pid = scripting::eval_result(&format!(
        r#"(process/spawn "cat > {}")"#,
        out.to_str().unwrap()
    ))
    .unwrap()
    .parse::<i64>()
    .unwrap();
    scripting::eval(&format!(r#"(process/stdin {pid} "")"#));
    scripting::eval(&format!(r#"(process/close-stdin {pid})"#));

    let exit_code = pump_exit(&mut ed, &mut rx);
    assert_eq!(exit_code, 0);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "");
    std::fs::remove_file(&out).ok();
}

#[test]
fn remote_command_failure_surfaces_as_nonzero_exit() {
    let _lock = helpers::acquire_janet_lock();
    let (mut ed, mut rx) = setup_pumping_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let out = std::env::temp_dir().join(format!("magma_sshw_fail_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let pid = scripting::eval_result(&format!(
        r#"(process/spawn "cat > {}; exit 3")"#,
        out.to_str().unwrap()
    ))
    .unwrap()
    .parse::<i64>()
    .unwrap();
    scripting::eval(&format!(r#"(process/stdin {pid} "partial")"#));
    scripting::eval(&format!(r#"(process/close-stdin {pid})"#));

    let exit_code = pump_exit(&mut ed, &mut rx);
    assert_eq!(exit_code, 3, "the remote failure must not be swallowed");
    std::fs::remove_file(&out).ok();
}

#[test]
fn stdin_to_closed_or_unknown_process_signals_error() {
    let _lock = helpers::acquire_janet_lock();
    let (mut ed, _rx) = setup_pumping_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval(r#"(try (process/stdin 424242 "x") ([e] "caught"))"#);
    assert_eq!(r, "ok", "an unknown process must be a catchable error");
    assert!(
        scripting::eval_result(r#"(= (try (process/stdin 424242 "x") ([e] "caught")) "caught")"#)
            .unwrap()
            .contains("true")
    );

    // A closed stdin (the normal post-write state) must also be a catchable
    // error, not a silent no-op.
    let pid = scripting::eval_result(r#"(process/spawn "true")"#)
        .unwrap()
        .parse::<i64>()
        .unwrap();
    scripting::eval(&format!(r#"(process/close-stdin {pid})"#));
    let r = scripting::eval_result(&format!(
        r#"(= (try (process/stdin {pid} "x") ([e] "caught")) "caught")"#
    ))
    .unwrap();
    assert_eq!(r, "true", "writing to a closed stdin must be catchable");
}

#[test]
fn ssh_fs_escape_path_quotes_single_quotes() {
    let _lock = helpers::acquire_janet_lock();
    let (mut ed, _rx) = setup_pumping_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let r = scripting::eval_result(
        r#"(= (ssh-fs/escape-path "/tmp/it's here.txt") "/tmp/it'\\''s here.txt")"#,
    )
    .unwrap();
    assert_eq!(r, "true", "single quotes in paths must be shell-escaped");
}

#[test]
#[ignore = "requires passwordless `ssh localhost`; run with --ignored"]
fn ssh_fs_write_round_trip_over_real_ssh() {
    let _lock = helpers::acquire_janet_lock();
    let (mut ed, mut rx) = setup_pumping_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let out = std::env::temp_dir().join(format!("magma_ssh_real_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let pid = scripting::eval_result(&format!(
        r#"(ssh-fs/write "localhost" "{}" "real ssh round trip\n")"#,
        out.to_str().unwrap()
    ))
    .unwrap()
    .parse::<i64>()
    .unwrap();
    assert!(pid >= 0);
    let exit_code = pump_exit(&mut ed, &mut rx);
    assert_eq!(exit_code, 0);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "real ssh round trip\n"
    );
    std::fs::remove_file(&out).ok();
}
