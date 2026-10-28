//! DAP background-event handlers — session lifecycle, stop events, console
//! output, and command-result application.

use crate::event::keys;
use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Dap(event) => handle_dap_event(ed, event),
        _ => {}
    }
}

fn handle_dap_event(ed: &mut Editor, event: crate::debug::Event) {
    use crate::debug::Event;
    match event {
        Event::Started { session_id, result } => {
            match result {
                Ok(handle) => {
                    let ready = ed
                        .debug
                        .sessions
                        .get(&session_id)
                        .is_some_and(|s| s.initialized);
                    ed.debug.connections.insert(session_id, handle);
                    // The adapter may have reported `initialized` before the
                    // spawner delivered this event; configure now if so.
                    if ready {
                        configure_adapter(ed, session_id);
                    }
                    ed.events.emit_typed(
                        keys::events::DEBUG_SESSION_STARTED,
                        crate::event::payload::DebugSessionStartedPayload {
                            session_id: session_id.to_string(),
                            adapter: ed
                                .debug
                                .sessions
                                .get(&session_id)
                                .map(|s| s.adapter.clone())
                                .unwrap_or_default(),
                        },
                    );
                }
                Err(e) => {
                    ed.debug.sessions.remove(&session_id);
                    ed.show_message(crate::state::MessageKind::Error, e);
                }
            }
        }
        // The adapter's `initialized` event opens the DAP configuration
        // window: breakpoints bind only from here, and configurationDone
        // releases the debuggee.  Whichever of this and `Started` lands
        // second performs the configuration — exactly once.
        Event::Response {
            session_id,
            event_type,
            ..
        } if event_type == "initialized" => {
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.initialized = true;
            }
            if ed.debug.connections.contains_key(&session_id) {
                configure_adapter(ed, session_id);
            }
        }
        Event::Stopped {
            session_id,
            reason,
            thread_id,
        } => {
            let watch_count = ed.debug.watches.len();
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.note_stopped(watch_count);
                session.stopped_thread = Some(thread_id);
            }
            // A stopped debuggee with no stack is a facade — fetch it.
            crate::script_runtime::send_dap_command(
                ed,
                session_id,
                "stackTrace",
                &format!(r#"{{"threadId":{thread_id},"startFrame":0,"levels":20}}"#),
            );
            // The thread list changes across stops; refresh it for the pane.
            crate::script_runtime::send_dap_command(ed, session_id, "threads", "{}");
            ed.events.emit_typed(
                keys::events::DEBUG_STOPPED,
                crate::event::payload::DebugStoppedPayload {
                    session_id: session_id.to_string(),
                    reason,
                    thread_id: thread_id.to_string(),
                },
            );
        }
        Event::Output {
            session_id,
            category,
            output,
        } => {
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.push_console(&output);
            }
            ed.events.emit_typed(
                keys::events::DEBUG_OUTPUT,
                crate::event::payload::DebugOutputPayload {
                    session_id: session_id.to_string(),
                    category,
                    output,
                },
            );
        }
        Event::Terminated { session_id } => {
            ed.debug.connections.remove(&session_id);
            ed.debug.sessions.remove(&session_id);
            if ed.debug.active_session == Some(session_id) {
                ed.debug.active_session = None;
            }
            ed.events.emit_typed(
                keys::events::DEBUG_SESSION_ENDED,
                crate::event::payload::DebugSessionEndedPayload {
                    session_id: session_id.to_string(),
                },
            );
        }
        Event::Response {
            session_id,
            event_type,
            data,
        } => {
            if event_type == "evaluate" {
                crate::runtime_dap::apply_dap_response(ed, session_id, &data);
                ed.events.emit_typed(
                    keys::events::DEBUG_EVALUATE_RESULT,
                    crate::event::payload::DebugEvaluateResultPayload {
                        session_id: session_id.to_string(),
                        result: data,
                    },
                );
            } else {
                let mut payload = std::collections::HashMap::new();
                payload.insert("session-id".into(), session_id.to_string());
                payload.insert("event-type".into(), event_type);
                payload.insert("data".into(), data);
                ed.events.emit("debug-dap-event", payload);
            }
        }
        Event::CommandResult {
            session_id,
            command,
            success,
            body,
        } => {
            if !success {
                crate::runtime_dap::note_request_error(ed, session_id, &command, &body);
            }
            crate::runtime_dap::apply_dap_command_result(ed, session_id, &command, success, &body);
        }
    }
}

/// Sync every project breakpoint to the adapter — lines may have shifted
/// since they were set — then release the debuggee with configurationDone.
/// Exactly one of the `Started`/`initialized` handlers calls this per
/// session.
fn configure_adapter(ed: &mut Editor, session_id: u64) {
    let files: Vec<String> = ed.debug.breakpoints.keys().cloned().collect();
    for file in files {
        crate::debug::send_set_breakpoints(ed, session_id, &file);
    }
    crate::script_runtime::send_dap_command(ed, session_id, "configurationDone", "{}");
}
