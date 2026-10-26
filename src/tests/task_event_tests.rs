//! Regression tests for the task subsystem's namespaced background events.
//!
//! The task vocabulary is `task::Event`, wrapped in the central
//! `BackgroundEvent` so it keeps sharing the one foreground channel.  These
//! tests pin the payloads, the delivery order across the wrapper, and the
//! dispatch effects so the split cannot silently drop, duplicate or reorder
//! events.

use crate::kernel::runtime::{BackgroundEvent, process_background_event};
use crate::kernel::task::{Event as TaskEvent, TaskStatus};

#[test]
fn task_events_round_trip_through_the_shared_channel_in_send_order() {
    let (_ed, bg, mut rx) = crate::tests::helpers::editor_with_background();

    bg.send_event(BackgroundEvent::Task(TaskEvent::OutputLine {
        id: 7,
        stream: "stdout".into(),
        line: "first".into(),
    }));
    bg.send_event(BackgroundEvent::FileReadResult {
        path: "/tmp/x".into(),
        content: Ok("body".into()),
        request_id: 11,
    });
    bg.send_event(BackgroundEvent::Task(TaskEvent::OutputLine {
        id: 7,
        stream: "stderr".into(),
        line: "second".into(),
    }));
    bg.send_event(BackgroundEvent::Task(TaskEvent::RunCompleted {
        id: 7,
        exit_code: 0,
        stdout: "first\n".into(),
        stderr: "second\n".into(),
        duration_ms: 42,
    }));
    bg.send_event(BackgroundEvent::Custom("ping".into(), "pong".into()));

    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }

    assert_eq!(seen.len(), 5, "no event may be dropped or duplicated");
    assert!(matches!(
        &seen[0],
        BackgroundEvent::Task(TaskEvent::OutputLine { id: 7, stream, line })
            if stream.as_str() == "stdout" && line.as_str() == "first"
    ));
    assert!(matches!(
        &seen[1],
        BackgroundEvent::FileReadResult { request_id: 11, content: Ok(body), .. }
            if body.as_str() == "body"
    ));
    assert!(matches!(
        &seen[2],
        BackgroundEvent::Task(TaskEvent::OutputLine { id: 7, stream, line })
            if stream.as_str() == "stderr" && line.as_str() == "second"
    ));
    assert!(matches!(
        &seen[3],
        BackgroundEvent::Task(TaskEvent::RunCompleted {
            id: 7, exit_code: 0, stdout, stderr, duration_ms: 42
        }) if stdout.as_str() == "first\n" && stderr.as_str() == "second\n"
    ));
    assert!(matches!(
        &seen[4],
        BackgroundEvent::Custom(name, payload)
            if name.as_str() == "ping" && payload.as_str() == "pong"
    ));
}

#[test]
fn output_lines_dispatch_into_the_sink_in_order_without_loss() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let buffer_key = ed.create_buffer_from_str("sink", "");
    ed.task_scheduler.output_sinks.insert(1, crate::kernel::task::OutputSink {
        buffer_key,
        parse_quickfix: false,
        command: "true".into(),
    });

    for line in ["one", "two", "three"] {
        process_background_event(&mut ed, BackgroundEvent::Task(TaskEvent::OutputLine {
            id: 1,
            stream: "stdout".into(),
            line: line.into(),
        }));
    }

    let arc = ed.buffers.get(buffer_key).unwrap();
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.slice(0, buf.len()), "one\ntwo\nthree\n");
}

#[test]
fn successful_task_completion_dispatch_updates_scheduler_state() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let id = ed.task_scheduler.define(
        "build".into(), "true".into(), Default::default(), vec![]);
    ed.task_scheduler.tasks.get_mut(&id).unwrap().status = TaskStatus::Running;
    ed.frame_dirty = false;

    process_background_event(&mut ed, BackgroundEvent::Task(TaskEvent::RunCompleted {
        id,
        exit_code: 0,
        stdout: "ok\n".into(),
        stderr: String::new(),
        duration_ms: 5,
    }));

    assert!(ed.frame_dirty, "dispatch must invalidate the frame");
    assert_eq!(
        ed.task_scheduler.tasks.get(&id).map(|t| &t.status),
        Some(&TaskStatus::Completed),
    );
    assert_eq!(
        ed.task_scheduler.output(id).map(|o| o.stdout.as_str()),
        Some("ok\n"),
    );
}

#[test]
fn failed_task_completion_dispatch_carries_the_exit_code() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let id = ed.task_scheduler.define(
        "build".into(), "false".into(), Default::default(), vec![]);
    ed.task_scheduler.tasks.get_mut(&id).unwrap().status = TaskStatus::Running;

    process_background_event(&mut ed, BackgroundEvent::Task(TaskEvent::RunCompleted {
        id,
        exit_code: 2,
        stdout: String::new(),
        stderr: "boom\n".into(),
        duration_ms: 9,
    }));

    assert_eq!(
        ed.task_scheduler.tasks.get(&id).map(|t| &t.status),
        Some(&TaskStatus::Failed("exit code 2".to_string())),
    );
    assert_eq!(
        ed.task_scheduler.output(id).map(|o| o.stderr.as_str()),
        Some("boom\n"),
    );
}
