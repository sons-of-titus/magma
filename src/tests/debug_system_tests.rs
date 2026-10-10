//! Pure-Rust tests for the Debug System (Phase 6).

use crate::kernel::debug::{DebugManager, DebugSession};

// ── DebugManager basics ──────────────────────────────────────────────────────

#[test]
fn manager_starts_empty() {
    let mgr = DebugManager::new();
    assert!(mgr.sessions.is_empty());
    assert!(mgr.active_session.is_none());
    assert!(mgr.breakpoints.is_empty());
}

#[test]
fn new_session_returns_incrementing_ids() {
    let mut mgr = DebugManager::new();
    let id1 = mgr.new_session("codelldb".into());
    let id2 = mgr.new_session("debugpy".into());
    assert_ne!(id1, id2);
    assert!(id2 > id1);
}

#[test]
fn first_session_becomes_active() {
    let mut mgr = DebugManager::new();
    let id = mgr.new_session("codelldb".into());
    assert_eq!(mgr.active_session, Some(id));
}

#[test]
fn second_session_does_not_replace_active() {
    let mut mgr = DebugManager::new();
    let first = mgr.new_session("codelldb".into());
    mgr.new_session("debugpy".into());
    assert_eq!(mgr.active_session, Some(first), "second session must not displace the active one");
}

#[test]
fn active_session_accessor() {
    let mut mgr = DebugManager::new();
    assert!(mgr.active_session().is_none());
    let id = mgr.new_session("adapter".into());
    assert_eq!(mgr.active_session().unwrap().id, id);
}

#[test]
fn active_session_mut_returns_correct_session() {
    let mut mgr = DebugManager::new();
    let id = mgr.new_session("adapter".into());
    {
        let s = mgr.active_session_mut().unwrap();
        s.stopped = true;
    }
    assert!(mgr.sessions[&id].stopped);
}

// ── DebugSession ─────────────────────────────────────────────────────────────

#[test]
fn session_starts_not_stopped() {
    let s = DebugSession::new(1, "codelldb".into());
    assert!(!s.stopped);
    assert!(s.stopped_thread.is_none());
    assert!(s.threads.is_empty());
    assert!(s.frames.is_empty());
}

#[test]
fn session_next_seq_increments() {
    let mut s = DebugSession::new(1, "adapter".into());
    assert_eq!(s.next_seq(), 1);
    assert_eq!(s.next_seq(), 2);
    assert_eq!(s.next_seq(), 3);
}

#[test]
fn session_set_frames() {
    use crate::kernel::debug::StackFrame;
    let mut s = DebugSession::new(1, "adapter".into());
    s.set_frames(vec![StackFrame { id: 1, name: "main".into(), source: None, line: 10, column: 0 }]);
    assert_eq!(s.frames.len(), 1);
    assert_eq!(s.frame(1).unwrap().name, "main");
    assert!(s.frame(2).is_none());
}

#[test]
fn session_set_threads() {
    use crate::kernel::debug::Thread;
    let mut s = DebugSession::new(1, "adapter".into());
    s.set_threads(vec![Thread { id: 1, name: "main".into() }]);
    assert_eq!(s.threads.len(), 1);
    assert_eq!(s.threads[0].name, "main");
}

// ── Breakpoints ───────────────────────────────────────────────────────────────

#[test]
fn add_breakpoint_returns_id() {
    let mut mgr = DebugManager::new();
    let id1 = mgr.add_breakpoint("src/main.rs".into(), 10);
    let id2 = mgr.add_breakpoint("src/main.rs".into(), 20);
    assert_ne!(id1, id2);
}

#[test]
fn breakpoints_for_empty_file() {
    let mgr = DebugManager::new();
    assert!(mgr.breakpoints_for("src/main.rs").is_empty());
}

#[test]
fn breakpoints_stored_per_file() {
    let mut mgr = DebugManager::new();
    mgr.add_breakpoint("a.rs".into(), 5);
    mgr.add_breakpoint("a.rs".into(), 10);
    mgr.add_breakpoint("b.rs".into(), 1);
    assert_eq!(mgr.breakpoints_for("a.rs").len(), 2);
    assert_eq!(mgr.breakpoints_for("b.rs").len(), 1);
    assert!(mgr.breakpoints_for("c.rs").is_empty());
}

#[test]
fn remove_breakpoint_by_line() {
    let mut mgr = DebugManager::new();
    mgr.add_breakpoint("a.rs".into(), 5);
    mgr.add_breakpoint("a.rs".into(), 10);
    let removed = mgr.remove_breakpoint("a.rs", 5);
    assert_eq!(removed.len(), 1, "the removed breakpoint's id is returned");
    assert_eq!(mgr.breakpoints_for("a.rs").len(), 1);
    assert_eq!(mgr.breakpoints_for("a.rs")[0].line, 10);
}

#[test]
fn remove_nonexistent_breakpoint_returns_no_ids() {
    let mut mgr = DebugManager::new();
    assert!(mgr.remove_breakpoint("a.rs", 99).is_empty());
}

#[test]
fn remove_only_matching_line() {
    let mut mgr = DebugManager::new();
    mgr.add_breakpoint("a.rs".into(), 5);
    mgr.add_breakpoint("a.rs".into(), 5);
    mgr.remove_breakpoint("a.rs", 5);
    assert!(mgr.breakpoints_for("a.rs").is_empty());
}

// ── set_breakpoints_args JSON ─────────────────────────────────────────────────

#[test]
fn set_breakpoints_args_empty_file() {
    let mgr = DebugManager::new();
    let json = mgr.set_breakpoints_args("src/main.rs");
    assert!(json.contains("\"breakpoints\":[]"));
    assert!(json.contains("\"path\":\"src/main.rs\""));
}

#[test]
fn set_breakpoints_args_with_lines() {
    let mut mgr = DebugManager::new();
    mgr.add_breakpoint("a.rs".into(), 7);
    mgr.add_breakpoint("a.rs".into(), 42);
    let json = mgr.set_breakpoints_args("a.rs");
    assert!(json.contains(r#"{"line":7}"#));
    assert!(json.contains(r#"{"line":42}"#));
}

// ── DAP types ─────────────────────────────────────────────────────────────────

#[test]
fn breakpoint_struct() {
    use crate::kernel::debug::Breakpoint;
    let bp = Breakpoint { id: 1, file: "a.rs".into(), line: 10, verified: false, enabled: true, condition: None };
    assert_eq!(bp.line, 10);
    assert!(!bp.verified);
}

#[test]
fn variable_struct() {
    use crate::kernel::debug::Variable;
    let v = Variable {
        name: "x".into(),
        value: "42".into(),
        type_name: Some("i32".into()),
        variables_reference: 0,
    };
    assert_eq!(v.name, "x");
    assert_eq!(v.type_name.unwrap(), "i32");
}

#[test]
fn memory_region_struct() {
    use crate::kernel::debug::MemoryRegion;
    let r = MemoryRegion { address: "0x1000".into(), size: 4096, name: Some("stack".into()) };
    assert_eq!(r.size, 4096);
}

// ── Enabled flag ──────────────────────────────────────────────────────────────

#[test]
fn disabled_breakpoints_are_withheld_from_the_adapter() {
    let mut mgr = DebugManager::new();
    mgr.add_breakpoint("a.rs".into(), 7);
    mgr.add_breakpoint("a.rs".into(), 42);
    mgr.set_enabled("a.rs", 7, false);
    let json = mgr.set_breakpoints_args("a.rs");
    assert!(!json.contains(r#"{"line":7}"#), "disabled breakpoints do not reach the adapter");
    assert!(json.contains(r#"{"line":42}"#));
    // They stay in the model for the UI.
    assert_eq!(mgr.breakpoints_for("a.rs").len(), 2);
    assert!(!mgr.breakpoints_for("a.rs")[0].enabled);
    assert!(mgr.breakpoints_for("a.rs")[1].enabled);
}

#[test]
fn all_breakpoints_spans_files_sorted() {
    let mut mgr = DebugManager::new();
    mgr.add_breakpoint("b.rs".into(), 2);
    mgr.add_breakpoint("a.rs".into(), 9);
    mgr.add_breakpoint("a.rs".into(), 1);
    let all = mgr.all_breakpoints();
    let lines: Vec<(String, usize)> = all.iter().map(|bp| (bp.file.clone(), bp.line)).collect();
    assert_eq!(lines, vec![("a.rs".into(), 1), ("a.rs".into(), 9), ("b.rs".into(), 2)]);
}

// ── Line tracking through edits ───────────────────────────────────────────────

#[test]
fn breakpoint_lines_track_edits_above_them() {
    let mut ed = crate::tests::helpers::make_editor();
    let key = crate::tests::helpers::focused_key(&ed);
    let path = "/tmp/bp_track.rs".to_string();
    {
        let arc = ed.buffers.get_mut(key).unwrap();
        let mut buf = arc.lock().unwrap();
        buf.path = Some(path.clone());
        buf.insert(0, "l1\nl2\nl3\n");
    }
    ed.debug.add_breakpoint(path.clone(), 3);
    let bp_id = ed.debug.breakpoints_for(&path)[0].id;
    {
        let arc = ed.buffers.get_mut(key).unwrap();
        let mut buf = arc.lock().unwrap();
        let offset = buf.line_start_offset(2).unwrap();
        buf.attach_breakpoint(bp_id, offset);
    }
    {
        // Two lines inserted above push the breakpoint down.
        let arc = ed.buffers.get_mut(key).unwrap();
        let mut buf = arc.lock().unwrap();
        buf.insert(0, "new1\nnew2\n");
        assert_eq!(buf.breakpoint_line(bp_id), Some(5));
    }
    crate::kernel::debug::sync_breakpoint_lines(&mut ed);
    assert_eq!(ed.debug.breakpoints_for(&path)[0].line, 5,
        "the manager's line follows the anchor after an edit");
}

// ── Session model ─────────────────────────────────────────────────────────────

#[test]
fn note_stopped_resets_per_stop_state() {
    let mut s = DebugSession::new(1, "codelldb".into());
    s.variable_scopes.insert(5, vec![]);
    s.scope_ref = Some(5);
    s.stopped_line = Some(("/a.rs".into(), 3));
    s.note_stopped(2);
    assert!(s.stopped);
    assert!(s.variable_scopes.is_empty());
    assert_eq!(s.scope_ref, None);
    assert_eq!(s.stopped_line, None);
    assert_eq!(s.watch_results.len(), 2);
}

#[test]
fn console_is_capped() {
    let mut s = DebugSession::new(1, "codelldb".into());
    for i in 0..600 {
        s.push_console(&format!("line {i}"));
    }
    assert_eq!(s.console.len(), 500);
    assert!(s.console.last().unwrap().contains("599"));
}

// ── Bootstrap ordering against a real (fake) adapter ──────────────────────────

#[test]
fn session_start_syncs_breakpoints_then_configuration_done() {
    if !crate::tests::helpers::python3_available() {
        return;
    }
    use crate::kernel::runtime::BackgroundHandle;

    let fake = crate::tests::helpers::spawn_fake_dap("ordering");
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let bg = BackgroundHandle::new(runtime.clone(), tx);
    let mut ed = crate::tests::helpers::make_editor();
    ed.background = Some(bg);

    ed.debug.add_breakpoint("/tmp/some_file.rs".into(), 5);
    crate::kernel::debug::spawn_session(&mut ed, fake.script.to_string_lossy().as_ref(), None)
        .expect("fake adapter spawns");

    // The DapStarted event runs the bootstrap: breakpoint sync + configDone.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        while let Ok(ev) = rx.try_recv() {
            crate::kernel::runtime::process_background_event(&mut ed, ev);
        }
        let lines = crate::tests::helpers::dap_log_lines(&fake.log);
        let has = |cmd: &str| lines.iter().any(|l| l == cmd);
        if has("initialize") && has("setBreakpoints") && has("configurationDone") {
            let idx = |cmd: &str| lines.iter().position(|l| l == cmd).unwrap();
            assert!(idx("initialize") < idx("setBreakpoints")
                && idx("setBreakpoints") < idx("configurationDone"),
                "initialize → setBreakpoints → configurationDone, got {lines:?}");
            break;
        }
        assert!(std::time::Instant::now() < deadline,
            "fake adapter never received the bootstrap sequence");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }

    ed.debug.disconnect_all();
}
