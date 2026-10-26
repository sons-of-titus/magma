use super::*;

fn frame(id: FrameId, line: usize) -> StackFrame {
    StackFrame {
        id,
        name: format!("frame{id}"),
        source: Some("/proj/src/main.rs".into()),
        line,
        column: 1,
    }
}

#[test]
fn a_new_session_starts_empty_and_not_stopped() {
    let s = DebugSession::new(7, "codelldb".into());
    assert_eq!(s.id, 7);
    assert_eq!(s.adapter, "codelldb");
    assert!(s.threads.is_empty());
    assert!(s.frames.is_empty());
    assert!(!s.stopped);
    assert!(s.stopped_line.is_none());
    assert!(s.verified_breakpoints.is_empty());
}

#[test]
fn sequence_numbers_are_monotonic_from_one() {
    let mut s = DebugSession::new(1, "adapter".into());
    assert_eq!(s.next_seq(), 1);
    assert_eq!(s.next_seq(), 2);
    assert_eq!(s.next_seq(), 3);
}

#[test]
fn console_lines_split_and_drop_the_oldest_past_the_cap() {
    let mut s = DebugSession::new(1, "adapter".into());
    s.push_console("one\ntwo\n\nthree\n");
    assert_eq!(s.console, vec!["one", "two", "three"]);

    for i in 0..600 {
        s.push_console(&format!("line{i}"));
    }
    assert!(s.console.len() <= 500, "console stays capped");
    assert_eq!(s.console.last().unwrap(), "line599");
}

#[test]
fn stopping_resets_frames_scopes_and_watches() {
    let mut s = DebugSession::new(1, "adapter".into());
    s.set_frames(vec![frame(1, 10)]);
    s.set_variables(100, vec![Variable {
        name: "x".into(),
        value: "1".into(),
        type_name: Some("u8".into()),
        variables_reference: 0,
    }]);
    s.scope_ref = Some(100);
    s.stopped_line = Some(("/proj/src/main.rs".into(), 10));

    s.note_stopped(3);

    assert!(s.stopped);
    assert!(s.frames.is_empty());
    assert!(s.variable_scopes.is_empty());
    assert!(s.scope_ref.is_none());
    assert!(s.stopped_line.is_none());
    assert_eq!(s.watch_results, vec![None, None, None]);
    assert!(s.pending_watch_evals.is_empty());
}

#[test]
fn frames_are_looked_up_by_id() {
    let mut s = DebugSession::new(1, "adapter".into());
    s.set_frames(vec![frame(1, 10), frame(2, 20)]);
    assert_eq!(s.frame(2).map(|f| f.line), Some(20));
    assert!(s.frame(3).is_none());
}

#[test]
fn threads_and_verified_breakpoints_replace_wholesale() {
    let mut s = DebugSession::new(1, "adapter".into());
    s.set_threads(vec![Thread { id: 1, name: "main".into() }]);
    s.set_threads(vec![
        Thread { id: 2, name: "worker".into() },
        Thread { id: 3, name: "idle".into() },
    ]);
    assert_eq!(s.threads.len(), 2);
    assert_eq!(s.threads[0].id, 2);

    s.set_verified_breakpoints(vec![(true, 4), (false, 9)]);
    assert_eq!(s.verified_breakpoints, vec![(true, 4), (false, 9)]);
    s.set_verified_breakpoints(vec![]);
    assert!(s.verified_breakpoints.is_empty());
}

#[test]
fn breakpoints_carry_disabled_state_and_conditions() {
    let bp = Breakpoint {
        id: 3,
        file: "/proj/src/main.rs".into(),
        line: 12,
        verified: false,
        enabled: false,
        condition: Some("i > 3".into()),
    };
    assert!(!bp.enabled && !bp.verified);
    assert_eq!(bp.condition.as_deref(), Some("i > 3"));
}
