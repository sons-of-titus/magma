//! End-to-end scenario tests — the audit's 13 cross-feature workflows,
//! walked through the headless path (commands + background events) exactly
//! as a session would experience them.  GUI-only steps (clicking the gutter)
//! live in the janet half of the pair.

use std::time::Duration;

use crate::kernel::command::{execute_command, execute_command_line};
use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::session;
use crate::tests::helpers::{
    dap_log_lines, editor_with_background, make_editor, python3_available, spawn_fake_dap,
};

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("e2e_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_project(ed: &mut crate::kernel::state::Editor, root: &std::path::Path) {
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        crate::kernel::command::args::ArgValue::Path(root.to_string_lossy().into_owned()));
    execute_command(ed, "open-project", &args).unwrap();
}

fn open_file(ed: &mut crate::kernel::state::Editor, path: &str) {
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        crate::kernel::command::args::ArgValue::Path(path.to_string()));
    execute_command(ed, "open-file", &args).unwrap();
}

fn wait_until(mut predicate: impl FnMut() -> bool, secs: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        if predicate() { return true; }
        std::thread::sleep(Duration::from_millis(25));
    }
    predicate()
}

fn pump(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
    ed: &mut crate::kernel::state::Editor,
    secs: u64,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        match rx.try_recv() {
            Ok(ev) => crate::kernel::runtime::process_background_event(ed, ev),
            Err(_) => {
                if std::time::Instant::now() >= deadline {
                    return;
                }
                std::thread::sleep(Duration::from_millis(15));
            }
        }
    }
}

// ── Scenario 1: open a project and restore the previous session ──────────────

#[test]
fn s1_open_project_restores_the_previous_session() {
    let root = temp_root("s1");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let file = root.join("lib.rs");
    std::fs::write(&file, "fn a() {}\nfn b() {}\n").unwrap();
    let path = file.to_string_lossy().into_owned();

    let (mut ed, _bg, _rx) = editor_with_background();
    open_project(&mut ed, &root);
    open_file(&mut ed, &path);
    let key = ed.buffers.iter()
        .find(|(_, a)| a.lock().unwrap().path.as_deref() == Some(path.as_str()))
        .map(|(k, _)| k).unwrap();
    let target = {
        let arc = ed.buffers.get_mut(key).unwrap();
        let buf = arc.lock().unwrap();
        buf.line_start_offset(1).unwrap()
    };
    if let Some(v) = ed.views.get_mut(&key) {
        v.set_cursor(target);
    }
    ed.debug.add_breakpoint(path.clone(), 2);
    session::save_if_project(&ed);

    let (mut fresh, _bg2, mut rx2) = editor_with_background();
    open_project(&mut fresh, &root);
    // Restore is asynchronous now: the session file loads on the pool and
    // the buffer lands as an event.
    pump(&mut rx2, &mut fresh, 10);
    assert!(fresh.buffers.iter().any(|(_, a)| a.lock().unwrap().path.as_deref() == Some(path.as_str())),
        "the file reopens");
    let key2 = fresh.buffers.iter()
        .find(|(_, a)| a.lock().unwrap().path.as_deref() == Some(path.as_str()))
        .map(|(k, _)| k).unwrap();
    assert_eq!(fresh.views.get(&key2).map(|v| v.cursor_offset()), Some(target));
    assert_eq!(fresh.debug.breakpoints_for(&path)[0].line, 2, "breakpoints return");
}

// ── Scenario 2: open a build-system project and import its structure ─────────

#[test]
fn s2_open_build_project_imports_structure_and_seeds_configs() {
    let root = temp_root("s2");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();

    let (mut ed, _bg, mut rx) = editor_with_background();
    open_project(&mut ed, &root);
    // Headless: detection normally runs from the project-opened Janet
    // handler; the janet half of this pair exercises that chain.
    ed.project_manager.detect(&root.to_string_lossy());
    pump(&mut rx, &mut ed, 10);

    assert_eq!(ed.project_manager.project.language, "rust");
    assert!(ed.project_manager.project.files.iter().any(|f| f == "src/main.rs"),
        "the index imports project files: {:?}", ed.project_manager.project.files);
    let registry = ed.project_manager.projects.get(
        ed.project_manager.current_project.as_deref().unwrap()).unwrap();
    assert_eq!(registry.language, "rust", "the registry copy tracks the active project");
    assert_eq!(ed.project_manager.run_configs.configs.len(), 3, "detection seeds configs");
}

// ── Scenario 3: select a run configuration for the project ───────────────────

#[test]
fn s3_runnable_file_gets_a_selectable_config() {
    let root = temp_root("s3");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let (mut ed, _bg, _rx) = editor_with_background();
    open_project(&mut ed, &root);
    ed.project_manager.detect(&root.to_string_lossy());

    assert!(ed.project_manager.run_configs.named("run").is_some(),
        "detection offers a runnable target");
    execute_command_line(&mut ed, "select-config run").unwrap();
    assert_eq!(ed.project_manager.run_configs.active.as_deref(), Some("run"));
}

// ── Scenario 4: run the project and observe output + exit status ─────────────

#[test]
fn s4_run_project_reports_output_and_exit() {
    let root = temp_root("s4");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let (mut ed, _bg, mut rx) = editor_with_background();
    open_project(&mut ed, &root);
    ed.project_manager.detect(&root.to_string_lossy());
    ed.project_manager.run_configs.upsert(crate::kernel::run_config::RunConfig {
        name: "run".into(),
        command: "/bin/echo".into(),
        args: vec!["built-and-ran".into()],
        cwd: None,
        env: Default::default(),
        build_before_run: None,
        debuggable: true,
    });

    execute_command_line(&mut ed, "run-config run").unwrap();
    pump(&mut rx, &mut ed, 10);

    let (name, output) = ed.last_task.clone().expect("the run leaves a transcript");
    assert_eq!(name, "run");
    assert!(output.contains("built-and-ran"));
    assert_eq!(ed.task_scheduler.status(ed.task_scheduler.task_by_name("run").unwrap()),
        Some(&crate::kernel::task::TaskStatus::Completed));
}

// ── Scenario 5: start debugging from a run configuration ─────────────────────

#[test]
fn s5_debug_config_launches_the_configured_program() {
    if !python3_available() { return; }
    let root = temp_root("s5");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let fake = spawn_fake_dap("s5");

    let (mut ed, _bg, mut rx) = editor_with_background();
    open_project(&mut ed, &root);
    ed.project_manager.run_configs.upsert(crate::kernel::run_config::RunConfig {
        name: "run".into(),
        command: "/bin/echo".into(),
        args: vec!["hi".into()],
        cwd: None,
        env: Default::default(),
        build_before_run: None,
        debuggable: true,
    });
    // debug-config resolves the adapter from the language's candidates and
    // launches the config's program as the debuggee.
    let cfg = ed.project_manager.run_configs.named("run").unwrap().clone();
    let launch = serde_json::json!({
        "request": "launch",
        "program": cfg.command,
        "args": cfg.args,
        "cwd": root.to_string_lossy(),
        "name": cfg.name,
    });
    crate::kernel::debug::spawn_session(&mut ed, fake.script.to_string_lossy().as_ref(), Some(launch))
        .unwrap();

    // The `Dap(Started)` bootstrap (configDone after breakpoint sync) is what a
    // real session experiences; the launch request precedes it.  Arguments
    // are logged on the tab-indented line after their command.
    let mut lines;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        while let Ok(ev) = rx.try_recv() {
            crate::kernel::runtime::process_background_event(&mut ed, ev);
        }
        lines = dap_log_lines(&fake.log);
        if lines.iter().any(|l| l == "launch") { break; }
        assert!(std::time::Instant::now() < deadline, "launch never reached the adapter");
        std::thread::sleep(Duration::from_millis(25));
    }
    let launch_idx = lines.iter().position(|l| l == "launch").unwrap();
    let args_line = lines.get(launch_idx + 1).map(String::as_str).unwrap_or("");
    assert!(args_line.contains("/bin/echo"),
        "the config's program is the debuggee: {args_line:?}");
    if let Some(done_idx) = lines.iter().position(|l| l == "configurationDone") {
        assert!(launch_idx < done_idx, "launch lands before configurationDone");
    }
    ed.debug.disconnect_all();
}

// ── Scenario 6: the Threads pane is fed by a `threads` request on stop ───────

#[test]
fn s6_stop_requests_threads_and_populates_the_thread_list() {
    if !python3_available() { return; }
    let root = temp_root("s6");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let fake = spawn_fake_dap("s6");

    let (mut ed, _bg, mut rx) = editor_with_background();
    open_project(&mut ed, &root);
    let launch = serde_json::json!({
        "request": "launch", "program": "/bin/echo", "args": [],
        "cwd": root.to_string_lossy(), "name": "run",
    });
    let sid = crate::kernel::debug::spawn_session(
        &mut ed, fake.script.to_string_lossy().as_ref(), Some(launch)).unwrap();

    // The connection registers when `Dap(Started)` lands; only then can a stop's
    // follow-up requests reach the adapter.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !ed.debug.connections.contains_key(&sid) {
        while let Ok(ev) = rx.try_recv() {
            crate::kernel::runtime::process_background_event(&mut ed, ev);
        }
        assert!(std::time::Instant::now() < deadline, "adapter never came up");
        std::thread::sleep(Duration::from_millis(15));
    }

    // A stop must ask the adapter for its thread list — otherwise the Threads
    // pane is permanently empty.
    crate::kernel::runtime::process_background_event(&mut ed,
        BackgroundEvent::Dap(crate::kernel::debug::Event::Stopped {
            session_id: sid, reason: "breakpoint".into(), thread_id: 1,
        }));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        while let Ok(ev) = rx.try_recv() {
            crate::kernel::runtime::process_background_event(&mut ed, ev);
        }
        let listed = ed.debug.sessions.get(&sid)
            .map(|s| s.threads.iter().any(|t| t.name == "main")).unwrap_or(false);
        if listed || std::time::Instant::now() >= deadline { break; }
        std::thread::sleep(Duration::from_millis(15));
    }
    assert!(dap_log_lines(&fake.log).iter().any(|l| l == "threads"),
        "stop must send a `threads` request");
    let threads = &ed.debug.sessions.get(&sid).unwrap().threads;
    assert!(threads.iter().any(|t| t.name == "main"),
        "the adapter's threads populate the session: {threads:?}");
    ed.debug.disconnect_all();
}

// ── Scenario 7/8: hit a breakpoint → jump + highlight; inspect frames/vars ───

#[test]
fn s7_s8_stop_opens_the_frame_and_populates_stack_and_variables() {
    let root = temp_root("s7");
    let file = root.join("main.rs");
    std::fs::write(&file, "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n").unwrap();
    let path = file.to_string_lossy().into_owned();

    let mut ed = make_editor();
    open_file(&mut ed, &path);
    let sid = ed.debug.new_session("fake".into());

    let body = serde_json::json!({"stackFrames": [
        {"id": 11, "name": "main", "source": {"path": path}, "line": 2, "column": 5},
        {"id": 12, "name": "caller", "source": {"path": path}, "line": 3, "column": 1},
    ]}).to_string();
    crate::kernel::runtime_dap::apply_dap_command_result(&mut ed, sid, "stackTrace", true, &body);

    let session = ed.debug.active_session().unwrap();
    assert_eq!(session.stopped_line, Some((path.clone(), 2)),
        "the execution point is recorded for the editor highlight");
    assert_eq!(session.frames.len(), 2, "the call stack fills");
    assert_eq!(session.pending_watch_evals.len(), 0);

    // The editor jumped to the stopped line.
    let key = ed.buffers.iter()
        .find(|(_, a)| a.lock().unwrap().path.as_deref() == Some(path.as_str()))
        .map(|(k, _)| k).unwrap();
    let line2 = ed.buffers.get_mut(key).unwrap().lock().unwrap().line_start_offset(1).unwrap();
    assert_eq!(ed.views.get(&key).map(|v| v.cursor_offset()), Some(line2));

    crate::kernel::runtime_dap::apply_dap_command_result(&mut ed, sid, "scopes", true,
        r#"{"scopes":[{"name":"Locals","variablesReference":7}]}"#);
    assert_eq!(ed.debug.sessions[&sid].pending_variables, Some(7));
    crate::kernel::runtime_dap::apply_dap_command_result(&mut ed, sid, "variables", true,
        r#"{"variables":[{"name":"x","value":"1","type":"i32","variablesReference":9}]}"#);
    let locals = &ed.debug.sessions[&sid].variable_scopes[&7];
    assert_eq!(locals[0].name, "x");

    // Expanding a nested reference fetches its children.
    crate::kernel::runtime_dap::request_variables(&mut ed, sid, 9);
    assert_eq!(ed.debug.sessions[&sid].pending_variables, Some(9));
    crate::kernel::runtime_dap::apply_dap_command_result(&mut ed, sid, "variables", true,
        r#"{"variables":[{"name":"inner","value":"2","variablesReference":0}]}"#);
    assert_eq!(ed.debug.sessions[&sid].variable_scopes[&9][0].name, "inner");
}

// ── Scenario 9: edit the source while debugging → breakpoints resync ─────────

#[test]
fn s9_editing_while_debugging_resyncs_breakpoints_on_save() {
    if !python3_available() { return; }
    let root = temp_root("s9");
    let file = root.join("main.rs");
    std::fs::write(&file, "fn a() {}\nfn b() {}\n").unwrap();
    let path = file.to_string_lossy().into_owned();
    let fake = spawn_fake_dap("s9");

    let (mut ed, _bg, mut rx) = editor_with_background();
    open_file(&mut ed, &path);
    ed.debug.add_breakpoint(path.clone(), 2);
    // The add-breakpoint path attaches a line anchor to the open buffer —
    // mirror it here (the manager call alone does not).
    let bp_key = ed.buffers.iter()
        .find(|(_, a)| a.lock().unwrap().path.as_deref() == Some(path.as_str()))
        .map(|(k, _)| k).unwrap();
    let bp_id = ed.debug.breakpoints_for(&path)[0].id;
    {
        let arc = ed.buffers.get_mut(bp_key).unwrap();
        let mut buf = arc.lock().unwrap();
        let offset = buf.line_start_offset(1).unwrap();
        buf.attach_breakpoint(bp_id, offset);
    }
    let sid = crate::kernel::debug::spawn_session(&mut ed, fake.script.to_string_lossy().as_ref(), None)
        .unwrap();
    // Drain the channel until the bootstrap registers the connection —
    // without it the later save has nothing to resync to.
    assert!(wait_until(|| {
        while let Ok(ev) = rx.try_recv() {
            crate::kernel::runtime::process_background_event(&mut ed, ev);
        }
        ed.debug.connections.contains_key(&sid)
    }, 10), "the session never came up");

    // Two lines inserted above move the breakpoint from line 2 to line 4.
    let key = ed.buffers.iter()
        .find(|(_, a)| a.lock().unwrap().path.as_deref() == Some(path.as_str()))
        .map(|(k, _)| k).unwrap();
    {
        let arc = ed.buffers.get_mut(key).unwrap();
        let mut buf = arc.lock().unwrap();
        buf.insert(0, "// one\n// two\n");
    }
    execute_command_line(&mut ed, "save-buffer").unwrap();

    // The write lands off-thread; the anchor follows the edit and the
    // resync re-sends the shifted breakpoint when it does.
    assert!(wait_until(|| {
        while let Ok(ev) = rx.try_recv() {
            crate::kernel::runtime::process_background_event(&mut ed, ev);
        }
        ed.debug.breakpoints_for(&path)[0].line == 4
    }, 10), "the anchor follows the edit once the save lands");
    assert!(wait_until(|| {
        let lines = dap_log_lines(&fake.log);
        matches!(lines.windows(2).rev().find(|w| w[0] == "setBreakpoints"),
            Some(w) if w[1].contains(r#""line": 4"#))
    }, 10), "the save re-sends the shifted breakpoint; log: {:?}",
        dap_log_lines(&fake.log));
    ed.debug.disconnect_all();
}

// ── Scenario 10: stop the session and start it again ─────────────────────────

#[test]
fn s10_stop_then_restart_debugging() {
    if !python3_available() { return; }
    let fake = spawn_fake_dap("s10");
    let (mut ed, _bg, mut rx) = editor_with_background();

    let first = crate::kernel::debug::spawn_session(
        &mut ed, fake.script.to_string_lossy().as_ref(), None).unwrap();
    assert!(wait_until(|| {
        while let Ok(ev) = rx.try_recv() { crate::kernel::runtime::process_background_event(&mut ed, ev); }
        ed.debug.connections.contains_key(&first)
    }, 10), "the first session handshakes");

    ed.debug.disconnect_all();
    assert!(ed.debug.sessions.is_empty(), "stop tears the session down");

    // The restarted adapter re-truncates its log; session identity and the
    // processed handshake are the real evidence of a second session.
    let second = crate::kernel::debug::spawn_session(
        &mut ed, fake.script.to_string_lossy().as_ref(), None).unwrap();
    assert!(second > first, "each start gets a fresh session id");
    assert!(wait_until(|| {
        while let Ok(ev) = rx.try_recv() { crate::kernel::runtime::process_background_event(&mut ed, ev); }
        ed.debug.connections.contains_key(&second)
    }, 10), "the second session runs its own handshake");
    ed.debug.disconnect_all();
}

// ── Scenario 12: close and reopen — everything persisted ─────────────────────

#[test]
fn s12_reopening_the_ide_restores_persisted_state() {
    let root = temp_root("s12");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let (mut ed, _bg, _rx) = editor_with_background();
    open_project(&mut ed, &root);
    ed.project_manager.detect(&root.to_string_lossy());
    execute_command_line(&mut ed, "select-config test").unwrap();

    assert!(root.join(".magma/run-configs.json").exists(),
        "run configurations persist per project");
    let root_str = root.to_string_lossy().into_owned();
    assert!(ed.project_manager.recent_projects.iter().any(|p| p == &root_str),
        "the project lands in recents");

    session::save_if_project(&ed);
    assert!(root.join(".magma/session.json").exists(), "the session persists");
}
