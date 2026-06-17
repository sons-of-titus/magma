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
    assert!(removed);
    assert_eq!(mgr.breakpoints_for("a.rs").len(), 1);
    assert_eq!(mgr.breakpoints_for("a.rs")[0].line, 10);
}

#[test]
fn remove_nonexistent_breakpoint_returns_false() {
    let mut mgr = DebugManager::new();
    assert!(!mgr.remove_breakpoint("a.rs", 99));
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
    let bp = Breakpoint { id: 1, file: "a.rs".into(), line: 10, verified: false, condition: None };
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
