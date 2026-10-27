//! Agent background-event handlers — session bookkeeping and event-bus
//! publication of agent turns.

use crate::state::Editor;
use crate::event::keys;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::AgentThought { session_id, thought } => {
            ed.agent.note_thought(session_id, &thought);
            ed.events.emit_typed(keys::events::AGENT_THOUGHT, crate::event::payload::AgentThoughtPayload {
                session_id: session_id.to_string(),
                thought,
            });
        }
        BackgroundEvent::AgentAction { session_id, command, description } => {
            ed.agent.note_action(session_id, &command, &description);
            ed.events.emit_typed(keys::events::AGENT_ACTION, crate::event::payload::AgentActionPayload {
                session_id: session_id.to_string(),
                command,
                description,
            });
        }
        BackgroundEvent::AgentStepResult { session_id, output } => {
            ed.agent.note_step_output(session_id, &output);
            ed.events.emit("agent-step-result", {
                let mut data = std::collections::HashMap::new();
                data.insert("session-id".to_string(), session_id.to_string());
                data.insert("output".to_string(), output);
                data
            });
        }
        BackgroundEvent::AgentLlmReply { session_id, content, tool_calls_json, error } => {
            crate::agent::loop_::handle_reply(
                ed, session_id, content, &tool_calls_json, error);
        }
        BackgroundEvent::AgentResult { session_id, result } => {
            ed.agent.complete_session(session_id, result.clone());
            ed.events.emit_typed(keys::events::AGENT_RESULT, crate::event::payload::AgentResultPayload {
                session_id: session_id.to_string(),
                result,
            });
        }
        BackgroundEvent::AgentError { session_id, error } => {
            ed.agent.fail_session(session_id, error.clone());
            ed.events.emit_typed(keys::events::AGENT_ERROR, crate::event::payload::AgentErrorPayload {
                session_id: session_id.to_string(),
                error,
            });
        }
        _ => {}
    }
}
