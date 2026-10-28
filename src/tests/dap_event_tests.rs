//! Regression tests for the debug subsystem's namespaced background events.
//!
//! `debug::Event` owns the DAP vocabulary; `BackgroundEvent::Dap(_)` carries
//! it on the one foreground channel.  These tests pin the wrapper's payloads,
//! cross-subsystem ordering, and dispatch effects.

use crate::kernel::debug::Event as DapEvent;
use crate::kernel::runtime::{BackgroundEvent, process_background_event};

#[test]
fn dap_events_round_trip_through_the_shared_channel_in_send_order() {
    let (_ed, bg, mut rx) = crate::tests::helpers::editor_with_background();

    bg.send_event(BackgroundEvent::Dap(DapEvent::Stopped {
        session_id: 3,
        reason: "breakpoint".into(),
        thread_id: 9,
    }));
    bg.send_event(BackgroundEvent::FileReadResult {
        path: "/tmp/a".into(),
        content: Ok("x".into()),
        request_id: 4,
    });
    bg.send_event(BackgroundEvent::Dap(DapEvent::Output {
        session_id: 3,
        category: "console".into(),
        output: "hi\n".into(),
    }));
    bg.send_event(BackgroundEvent::Dap(DapEvent::Response {
        session_id: 3,
        event_type: "evaluate".into(),
        data: "42".into(),
    }));
    bg.send_event(BackgroundEvent::Dap(DapEvent::CommandResult {
        session_id: 3,
        command: "threads".into(),
        success: true,
        body: "{}".into(),
    }));
    bg.send_event(BackgroundEvent::Dap(DapEvent::Terminated { session_id: 3 }));
    bg.send_event(BackgroundEvent::Custom("ping".into(), "pong".into()));

    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }

    assert_eq!(seen.len(), 7, "no event may be dropped or duplicated");
    assert!(matches!(
        &seen[0],
        BackgroundEvent::Dap(DapEvent::Stopped { session_id: 3, reason, thread_id: 9 })
            if reason.as_str() == "breakpoint"
    ));
    assert!(matches!(
        &seen[1],
        BackgroundEvent::FileReadResult { request_id: 4, content: Ok(body), .. }
            if body.as_str() == "x"
    ));
    assert!(matches!(
        &seen[2],
        BackgroundEvent::Dap(DapEvent::Output { session_id: 3, category, output })
            if category.as_str() == "console" && output.as_str() == "hi\n"
    ));
    assert!(matches!(
        &seen[3],
        BackgroundEvent::Dap(DapEvent::Response { session_id: 3, event_type, data })
            if event_type.as_str() == "evaluate" && data.as_str() == "42"
    ));
    assert!(matches!(
        &seen[4],
        BackgroundEvent::Dap(DapEvent::CommandResult { session_id: 3, command, success: true, body })
            if command.as_str() == "threads" && body.as_str() == "{}"
    ));
    assert!(matches!(
        &seen[5],
        BackgroundEvent::Dap(DapEvent::Terminated { session_id: 3 })
    ));
    assert!(matches!(
        &seen[6],
        BackgroundEvent::Custom(name, payload)
            if name.as_str() == "ping" && payload.as_str() == "pong"
    ));
}

#[test]
fn dap_output_dispatch_reaches_the_session_console() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let sid = ed.debug.new_session("fake".into());
    ed.frame_dirty = false;

    process_background_event(
        &mut ed,
        BackgroundEvent::Dap(DapEvent::Output {
            session_id: sid,
            category: "stdout".into(),
            output: "hello\n".into(),
        }),
    );

    assert!(ed.frame_dirty, "dispatch must invalidate the frame");
    let session = ed.debug.sessions.get(&sid).expect("session survives");
    assert_eq!(session.console.last().map(String::as_str), Some("hello"));
}

#[test]
fn dap_terminated_dispatch_removes_the_session_and_clears_active() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let sid = ed.debug.new_session("fake".into());
    assert_eq!(ed.debug.active_session, Some(sid));

    process_background_event(
        &mut ed,
        BackgroundEvent::Dap(DapEvent::Terminated { session_id: sid }),
    );

    assert!(!ed.debug.sessions.contains_key(&sid));
    assert_eq!(ed.debug.active_session, None);
}

#[test]
fn set_breakpoints_response_verifies_only_the_file_that_was_sent() {
    // The regression: the DAP response carries lines but no file, so the
    // old mirroring verified line N in *every* open file.
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let sid = ed.debug.new_session("fake".into());
    let id_a = ed.debug.add_breakpoint("/proj/a.rs".into(), 10);
    let id_b = ed.debug.add_breakpoint("/proj/b.rs".into(), 10);
    ed.debug.pending_bp_files.push((sid, "/proj/a.rs".into()));

    crate::kernel::runtime_dap::apply_dap_command_result(
        &mut ed,
        sid,
        "setBreakpoints",
        true,
        r#"{"breakpoints":[{"verified":true,"line":10}]}"#,
    );

    let bp_a = ed
        .debug
        .breakpoints
        .get("/proj/a.rs")
        .unwrap()
        .iter()
        .find(|bp| bp.id == id_a)
        .unwrap();
    let bp_b = ed
        .debug
        .breakpoints
        .get("/proj/b.rs")
        .unwrap()
        .iter()
        .find(|bp| bp.id == id_b)
        .unwrap();
    assert!(bp_a.verified, "the requested file's breakpoint verifies");
    assert!(
        !bp_b.verified,
        "an unrelated file's breakpoint must not be touched by the response"
    );
    assert!(ed.debug.pending_bp_files.is_empty(), "queue drains");
}

#[test]
fn toggle_past_eof_never_reanchors_to_line_one() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let key = ed.create_buffer_from_str("f.rs", "one\ntwo\n");
    let path = {
        let buf = ed.buffers.get(key).unwrap();
        let mut buf = buf.lock().unwrap_or_else(|e| e.into_inner());
        buf.path = Some("/proj/f.rs".into());
        buf.path.clone().unwrap()
    };
    // Line 9 does not exist; the anchor must simply not attach.
    crate::kernel::debug::toggle_breakpoint_at(&mut ed, &path, 9);
    let buf = ed.buffers.get(key).unwrap();
    let buf = buf.lock().unwrap_or_else(|e| e.into_inner());
    assert!(
        buf.bp_anchors.is_empty(),
        "out-of-range toggles leave no anchor that would drift to line 1"
    );
    assert_eq!(ed.debug.breakpoints.get(path.as_str()).unwrap()[0].line, 9);
}
