//! The agent turn loop — foreground state machine + background LLM calls.
//!
//! One turn: the background thread calls the model; the reply lands as
//! `BackgroundEvent::AgentLlmReply`; the foreground executes the requested
//! tool calls through the command registry, appends the results to the
//! session transcript, and starts the next turn.  Sessions are bounded by
//! `MAX_TURNS` so a looping model cannot spin forever.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::command::args::ArgValue;
use crate::command::execute_command;
use crate::event::keys;
use crate::state::Editor;
use crate::state::MessageKind;

use super::TranscriptEntry;
use super::context::EditorContextProvider;
use super::llm::{self, LlmConfig, ToolCall};
use super::types::{AgentContext, ContextProvider, PlanStep, Planner};

/// Maximum model round-trips per session.
pub use super::MAX_TURNS;

/// Create a session and kick off its first turn.  With an API key this is
/// the real LLM loop; without one the deterministic BuiltinPlanner runs
/// (same events, no network).  Safe to call from the UI and from Janet.
pub fn start_session(ed: &mut Editor, prompt: &str) -> Result<u64, String> {
    let session_id = ed.agent.create_session(prompt);
    let ctx = EditorContextProvider.gather(ed);
    if LlmConfig::is_configured(&ed.options) {
        begin_session(ed, session_id, prompt, &ctx);
        start_turn(ed, session_id)?;
    } else {
        run_builtin_session(ed, session_id, prompt, &ctx)?;
    }
    Ok(session_id)
}

// ── LLM loop ─────────────────────────────────────────────────────────────────

/// Seed the chat transcript for a session.
pub fn begin_session(ed: &mut Editor, session_id: u64, prompt: &str, ctx: &AgentContext) {
    ed.agent.begin_messages(
        session_id,
        vec![
            json!({ "role": "system", "content": llm::system_prompt() }),
            json!({ "role": "user", "content": llm::context_message(ctx) }),
            json!({ "role": "user", "content": prompt }),
        ],
    );
}

/// Fire one model call on the background pool.  The reply (or error)
/// arrives as `AgentLlmReply` on the foreground.
pub fn start_turn(ed: &mut Editor, session_id: u64) -> Result<(), String> {
    let Some(cfg) = LlmConfig::from_options(&ed.options) else {
        return Err("agent: no API key configured".to_string());
    };
    let Some(bg) = ed.background.clone() else {
        return Err("agent: no background runtime available".to_string());
    };
    let messages = ed
        .agent
        .messages_of(session_id)
        .ok_or_else(|| "agent: session vanished".to_string())?;
    let sender = bg.sender.clone();
    bg.spawn_blocking(move || {
        let (content, calls, error) = match llm::chat(&cfg, &messages) {
            Ok((text, calls)) => (text, calls, None),
            Err(e) => (String::new(), Vec::new(), Some(e)),
        };
        let _ = sender.send(crate::runtime::BackgroundEvent::AgentLlmReply {
            session_id,
            content,
            tool_calls_json: serde_json::to_string(&calls).unwrap_or_else(|_| "[]".into()),
            error,
        });
    });
    Ok(())
}

/// Foreground half of the turn: publish the assistant text, execute the
/// tool calls, feed results back (or finish the session).
pub fn handle_reply(
    ed: &mut Editor,
    session_id: u64,
    content: String,
    tool_calls_json: &str,
    error: Option<String>,
) {
    if let Some(e) = error {
        ed.agent.fail_session(session_id, e.clone());
        ed.events.emit_typed(
            keys::events::AGENT_ERROR,
            crate::event::payload::AgentErrorPayload {
                session_id: session_id.to_string(),
                error: e,
            },
        );
        return;
    }
    if !content.trim().is_empty() {
        ed.agent.note_thought(session_id, &content);
        ed.events.emit_typed(
            keys::events::AGENT_THOUGHT,
            crate::event::payload::AgentThoughtPayload {
                session_id: session_id.to_string(),
                thought: content.clone(),
            },
        );
    }

    let calls: Vec<ToolCall> = serde_json::from_str(tool_calls_json).unwrap_or_default();
    if calls.is_empty() {
        // No tool calls: this was the final answer.
        let final_text = if content.trim().is_empty() {
            "done".to_string()
        } else {
            content
        };
        ed.agent.complete_session(session_id, final_text.clone());
        ed.events.emit_typed(
            keys::events::AGENT_RESULT,
            crate::event::payload::AgentResultPayload {
                session_id: session_id.to_string(),
                result: final_text,
            },
        );
        return;
    }

    // The user asked to stop: acknowledge instead of executing more work.
    if ed.agent.turns_left_of(session_id) == Some(0) {
        ed.agent
            .complete_session(session_id, "stopped by user".to_string());
        ed.events.emit_typed(
            keys::events::AGENT_RESULT,
            crate::event::payload::AgentResultPayload {
                session_id: session_id.to_string(),
                result: "stopped by user".to_string(),
            },
        );
        return;
    }

    let mut results: Vec<Value> = Vec::new();
    for call in &calls {
        ed.agent
            .note_action(session_id, &call.name, &tool_summary(call));
        ed.events.emit_typed(
            keys::events::AGENT_ACTION,
            crate::event::payload::AgentActionPayload {
                session_id: session_id.to_string(),
                command: call.name.clone(),
                description: tool_summary(call),
            },
        );
        let output = execute_tool(ed, call);
        // The UI renders tool results straight from the transcript; without
        // this the only consumer of a tool's output is the model.
        ed.agent.note_step_output(session_id, &output);
        results.push(json!({
            "role": "tool",
            "tool_call_id": call.id,
            "content": llm::truncate_for_context(&output),
        }));
    }
    ed.agent.append_messages(session_id, results);

    // Budget: each executed batch consumes one turn.
    match ed.agent.spend_turn(session_id) {
        Some(remaining) if remaining > 0 => {
            if let Err(e) = start_turn(ed, session_id) {
                ed.agent.fail_session(session_id, e.clone());
                ed.show_message(MessageKind::Error, e);
            }
        }
        Some(_) => {
            ed.agent
                .complete_session(session_id, format!("stopped after {MAX_TURNS} tool turns"));
            ed.events.emit_typed(
                keys::events::AGENT_RESULT,
                crate::event::payload::AgentResultPayload {
                    session_id: session_id.to_string(),
                    result: format!("stopped after {MAX_TURNS} tool turns"),
                },
            );
        }
        None => {}
    }
}

/// Human-readable one-liner for a tool call (transcript + events).
fn tool_summary(call: &ToolCall) -> String {
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
    match call.name.as_str() {
        "run_editor_command" => {
            let cmd = args.get("command").and_then(Value::as_str).unwrap_or("?");
            let rest = args.get("args").map(|a| a.to_string()).unwrap_or_default();
            if rest == "{}" {
                cmd.to_string()
            } else {
                format!("{cmd} {rest}")
            }
        }
        _ => call.name.clone(),
    }
}

/// Execute one tool call on the foreground.  Every editor mutation goes
/// through the command registry — the agent has no private path.
pub fn execute_tool(ed: &mut Editor, call: &ToolCall) -> String {
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
    match call.name.as_str() {
        "read_active_buffer" => {
            let key = ed.focused_buffer_key();
            ed.buffers
                .get(key)
                .map(|arc| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    let text = buf.slice(0, buf.len().min(65_536));
                    format!("buffer: {}\n{}", buf.name, llm::truncate_for_context(&text))
                })
                .unwrap_or_else(|| "no active buffer".to_string())
        }
        "get_diagnostics" => {
            let key = ed.focused_buffer_key();
            ed.buffers
                .get(key)
                .map(|arc| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    let diags: Vec<String> = buf.diagnostic_strings();
                    if diags.is_empty() {
                        "no diagnostics".to_string()
                    } else {
                        diags.join("\n")
                    }
                })
                .unwrap_or_else(|| "no active buffer".to_string())
        }
        "list_editor_commands" => {
            let mut lines: Vec<String> = ed
                .commands
                .list_detailed()
                .into_iter()
                .map(|(name, desc, _has_args)| format!("{name}: {desc}"))
                .collect();
            lines.sort();
            lines.join("\n")
        }
        "run_editor_command" => {
            let Some(command) = args.get("command").and_then(Value::as_str) else {
                return "error: run_editor_command requires a command".to_string();
            };
            let mut cmd_args: HashMap<String, ArgValue> = HashMap::new();
            if let Some(obj) = args.get("args").and_then(Value::as_object) {
                for (k, v) in obj {
                    let val = match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        other => other.to_string(),
                    };
                    cmd_args.insert(k.clone(), ArgValue::String(val));
                }
            }
            match execute_command(ed, command, &cmd_args) {
                Ok(()) => "ok".to_string(),
                Err(e) => format!("error: {e}"),
            }
        }
        other => format!("error: unknown tool {other}"),
    }
}

// ── Deterministic fallback (no API key) ─────────────────────────────────────

/// The pre-LLM behavior: plan with `BuiltinPlanner` on the background
/// thread and publish the plan as events.  No model, no execution.
fn run_builtin_session(
    ed: &mut Editor,
    session_id: u64,
    prompt: &str,
    ctx: &AgentContext,
) -> Result<(), String> {
    use crate::agent::BuiltinPlanner;
    let Some(bg) = ed.background.clone() else {
        return Err("agent: no background runtime available".to_string());
    };
    let prompt = prompt.to_string();
    let ctx = ctx.clone();
    let sender = bg.sender.clone();
    bg.spawn_blocking(move || {
        let plan = BuiltinPlanner.plan(&prompt, &ctx);

        let _ = sender.send(crate::runtime::BackgroundEvent::AgentThought {
            session_id,
            thought: plan.rationale.clone(),
        });

        for step in &plan.steps {
            let _ = sender.send(crate::runtime::BackgroundEvent::AgentAction {
                session_id,
                command: step.command.clone(),
                description: step.description.clone(),
            });
        }

        let result = if plan.steps.is_empty() {
            plan.rationale.clone()
        } else {
            format!("{} step(s) planned", plan.steps.len())
        };

        let _ = sender.send(crate::runtime::BackgroundEvent::AgentResult { session_id, result });
    });
    Ok(())
}

/// Plan steps visible to tests of the fallback.
#[allow(dead_code)]
pub(crate) fn plan_step(command: &str, arg: (&str, &str)) -> PlanStep {
    PlanStep {
        command: command.to_string(),
        args: HashMap::from([(arg.0.to_string(), arg.1.to_string())]),
        description: format!("{command} {}", arg.1),
    }
}

// keep TranscriptEntry imported for the note_* helpers above
#[allow(dead_code)]
fn _transcript_witness(_: TranscriptEntry) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentManager, AgentStatus};

    fn editor() -> Editor {
        let mut ed = Editor::new(Box::new(crate::storage::disk::DiskFileSystem::new()));
        crate::command::builtin::register_builtin_commands(&mut ed);
        let key = ed.create_buffer_from_str("scratch", "hello agent\n");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        ed
    }

    fn call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: "t1".into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }

    #[test]
    fn reply_without_calls_completes_the_session_with_the_answer() {
        let mut ed = editor();
        let sid = ed.agent.create_session("goal");
        handle_reply(
            &mut ed,
            sid,
            "All done: three files changed.".into(),
            "[]",
            None,
        );
        assert_eq!(ed.agent.status(sid), Some(&AgentStatus::Completed));
        assert_eq!(ed.agent.result(sid), Some("All done: three files changed."));
    }

    #[test]
    fn error_reply_fails_the_session() {
        let mut ed = editor();
        let sid = ed.agent.create_session("goal");
        handle_reply(&mut ed, sid, String::new(), "[]", Some("boom".into()));
        assert_eq!(ed.agent.status(sid), Some(&AgentStatus::Failed));
        assert_eq!(ed.agent.sessions[&sid].error.as_deref(), Some("boom"));
    }

    #[test]
    fn tool_calls_execute_through_the_registry_and_consume_budget() {
        let mut ed = editor();
        // open-file through the agent = the real command path.
        let dir = std::env::temp_dir().join(format!(
            "agent-tool-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("opened.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        let sid = ed.agent.create_session("open the file");
        ed.agent
            .begin_messages(sid, vec![json!({"role": "user", "content": "go"})]);

        let calls = vec![call(
            "run_editor_command",
            &format!(
                r#"{{"command":"open-file","args":{{"path":"{}"}}}}"#,
                path.to_string_lossy()
            ),
        )];
        let calls_json = serde_json::to_string(&calls).unwrap();
        // No background runtime: the follow-up turn reports the failure but
        // the tool call itself already ran.
        handle_reply(&mut ed, sid, "Opening the file.".into(), &calls_json, None);
        assert!(
            ed.buffers.iter().any(|(_, a)| {
                a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref()
                    == Some(path.to_string_lossy().as_ref())
            }),
            "the agent's open-file call opened the buffer"
        );
        assert_eq!(
            ed.agent.status(sid),
            Some(&AgentStatus::Failed),
            "without a background pool the next turn cannot start"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tool_output_lands_in_the_transcript_next_to_its_action() {
        let mut ed = editor();
        let sid = ed.agent.create_session("read the buffer");
        let calls = vec![call("read_active_buffer", "{}")];
        let calls_json = serde_json::to_string(&calls).unwrap();
        handle_reply(&mut ed, sid, String::new(), &calls_json, None);

        let transcript = &ed.agent.sessions[&sid].transcript;
        assert!(
            matches!(transcript.first(), Some(super::super::TranscriptEntry::Action { command, .. }) if command == "read_active_buffer"),
            "action recorded: {transcript:?}"
        );
        assert!(
            matches!(transcript.last(), Some(super::super::TranscriptEntry::StepOutput(text)) if text.contains("buffer: scratch")),
            "tool output recorded: {transcript:?}"
        );
    }

    #[test]
    fn read_active_buffer_returns_the_focused_text() {
        let mut ed = editor();
        let out = execute_tool(&mut ed, &call("read_active_buffer", "{}"));
        assert!(out.contains("hello agent"), "got: {out}");
        assert!(out.contains("buffer: scratch"));
    }

    #[test]
    fn get_diagnostics_reports_none_cleanly() {
        let mut ed = editor();
        let out = execute_tool(&mut ed, &call("get_diagnostics", "{}"));
        assert_eq!(out, "no diagnostics");
    }

    #[test]
    fn list_editor_commands_includes_the_registry() {
        let mut ed = editor();
        crate::command::builtin::register_builtin_commands(&mut ed);
        let out = execute_tool(&mut ed, &call("list_editor_commands", "{}"));
        assert!(out.contains("open-file:"));
        assert!(out.contains("save-buffer:"));
    }

    #[test]
    fn unknown_tool_is_an_error_string_not_a_panic() {
        let mut ed = editor();
        let out = execute_tool(&mut ed, &call("self_destruct", "{}"));
        assert!(out.starts_with("error:"));
    }

    #[test]
    fn malformed_arguments_degrade_to_empty_object() {
        let mut ed = editor();
        let out = execute_tool(&mut ed, &call("run_editor_command", "not json"));
        assert!(
            out.starts_with("error:"),
            "missing command arg is an error, got: {out}"
        );
    }

    #[test]
    fn session_budget_and_stop_bookkeeping() {
        let mut mgr = AgentManager::new();
        let sid = mgr.create_session("x");
        assert_eq!(mgr.turns_left_of(sid), Some(MAX_TURNS));
        assert_eq!(mgr.spend_turn(sid), Some(MAX_TURNS - 1));
        assert!(mgr.request_stop(sid));
        assert_eq!(mgr.turns_left_of(sid), Some(0));
        // Transcript entries only land while running.
        mgr.complete_session(sid, "done");
        mgr.note_thought(sid, "late");
        assert!(
            mgr.sessions[&sid].transcript.is_empty(),
            "no entries after completion"
        );
    }
}
