//! DAP background-event handlers — session lifecycle, stop events, console
//! output, and command-result application.

use crate::kernel::state::Editor;
use crate::kernel::event::keys;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Dap(event) => handle_dap_event(ed, event),
        _ => {}
    }
}

fn handle_dap_event(ed: &mut Editor, event: crate::kernel::debug::Event) {
    use crate::kernel::debug::Event;
    match event {
        Event::Started { session_id, result } => {
            match result {
                Ok(handle) => {
                    ed.debug.connections.insert(session_id, handle);
                    // Sync every project breakpoint to the fresh adapter,
                    // then release it with configurationDone — adapters that
                    // require the sequence (e.g. js-debug) hang without it.
                    crate::kernel::debug::sync_breakpoint_lines(ed);
                    let files: Vec<String> = ed.debug.breakpoints.keys().cloned().collect();
                    #[cfg(feature = "janet")]
                    for file in files {
                        crate::kernel::scripting::send_dap_command(
                            ed, session_id, "setBreakpoints",
                            &ed.debug.set_breakpoints_args(&file));
                    }
                    #[cfg(not(feature = "janet"))]
                    let _ = files;
                    #[cfg(feature = "janet")]
                    crate::kernel::scripting::send_dap_command(
                        ed, session_id, "configurationDone", "{}");
                    ed.events.emit_typed(keys::events::DEBUG_SESSION_STARTED,
                        crate::kernel::event::payload::DebugSessionStartedPayload {
                            session_id: session_id.to_string(),
                            adapter: ed.debug.sessions.get(&session_id)
                                .map(|s| s.adapter.clone()).unwrap_or_default(),
                        });
                }
                Err(e) => {
                    ed.debug.sessions.remove(&session_id);
                    ed.show_message(crate::kernel::state::MessageKind::Error, e);
                }
            }
        }
        Event::Stopped { session_id, reason, thread_id } => {
            let watch_count = ed.debug.watches.len();
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.note_stopped(watch_count);
                session.stopped_thread = Some(thread_id);
            }
            // A stopped debuggee with no stack is a facade — fetch it.
            #[cfg(feature = "janet")]
            crate::kernel::scripting::send_dap_command(
                ed, session_id, "stackTrace", &format!(r#"{{"threadId":{thread_id},"startFrame":0,"levels":20}}"#),
            );
            // The thread list changes across stops; refresh it for the pane.
            #[cfg(feature = "janet")]
            crate::kernel::scripting::send_dap_command(ed, session_id, "threads", "{}");
            ed.events.emit_typed(keys::events::DEBUG_STOPPED, crate::kernel::event::payload::DebugStoppedPayload {
                session_id: session_id.to_string(),
                reason,
                thread_id: thread_id.to_string(),
            });
        }
        Event::Output { session_id, category, output } => {
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.push_console(&output);
            }
            ed.events.emit_typed(keys::events::DEBUG_OUTPUT, crate::kernel::event::payload::DebugOutputPayload {
                session_id: session_id.to_string(),
                category,
                output,
            });
        }
        Event::Terminated { session_id } => {
            ed.debug.connections.remove(&session_id);
            ed.debug.sessions.remove(&session_id);
            if ed.debug.active_session == Some(session_id) {
                ed.debug.active_session = None;
            }
            ed.events.emit_typed(keys::events::DEBUG_SESSION_ENDED, crate::kernel::event::payload::DebugSessionEndedPayload {
                session_id: session_id.to_string(),
            });
        }
        Event::Response { session_id, event_type, data } => {
            if event_type == "evaluate" {
                crate::kernel::runtime_dap::apply_dap_response(ed, session_id, &data);
                ed.events.emit_typed(keys::events::DEBUG_EVALUATE_RESULT, crate::kernel::event::payload::DebugEvaluateResultPayload {
                    session_id: session_id.to_string(),
                    result: data,
                });
            } else {
                let mut payload = std::collections::HashMap::new();
                payload.insert("session-id".into(), session_id.to_string());
                payload.insert("event-type".into(), event_type);
                payload.insert("data".into(), data);
                ed.events.emit("debug-dap-event", payload);
            }
        }
        Event::CommandResult { session_id, command, success, body } => {
            if !success {
                crate::kernel::runtime_dap::note_request_error(ed, session_id, &command, &body);
            }
            crate::kernel::runtime_dap::apply_dap_command_result(ed, session_id, &command, success, &body);
        }
    }
}
