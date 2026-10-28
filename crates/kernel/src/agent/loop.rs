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

    process_calls(ed, session_id, calls);
}

/// Execute a batch of tool calls, suspending before any that need the
/// user's approval.  When the whole batch has run (or the session parked),
/// `finish_batch` closes the turn for whatever actually executed.
fn process_calls(ed: &mut Editor, session_id: u64, calls: Vec<ToolCall>) {
    let mut results: Vec<Value> = Vec::new();
    for (i, call) in calls.iter().enumerate() {
        let summary = tool_summary(call);
        ed.agent.note_action(session_id, &call.name, &summary);
        ed.events.emit_typed(
            keys::events::AGENT_ACTION,
            crate::event::payload::AgentActionPayload {
                session_id: session_id.to_string(),
                command: call.name.clone(),
                description: summary.clone(),
            },
        );
        if requires_approval(call) {
            let queued: Vec<ToolCall> = calls[i + 1..].to_vec();
            ed.agent
                .park_for_approval(session_id, call.clone(), summary.clone(), queued);
            ed.events.emit_typed(
                keys::events::AGENT_APPROVAL,
                crate::event::payload::AgentApprovalPayload {
                    session_id: session_id.to_string(),
                    tool: call.name.clone(),
                    summary,
                },
            );
            return;
        }
        let output = execute_tool(ed, call);
        record_side_effects(ed, session_id, call);
        // The UI renders tool results straight from the transcript; without
        // this the only consumer of a tool's output is the model.
        ed.agent.note_step_output(session_id, &output);
        results.push(json!({
            "role": "tool",
            "tool_call_id": call.id,
            "content": llm::truncate_for_context(&output),
        }));
    }
    finish_batch(ed, session_id, results);
}

/// Feed executed results back to the model and start the next turn,
/// honoring the turn budget.
fn finish_batch(ed: &mut Editor, session_id: u64, results: Vec<Value>) {
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

/// Resolve the session's pending approval: `true` executes the parked call,
/// `false` reports the decline to the model.  Either way the rest of the
/// parked batch continues.
pub fn resolve_approval(ed: &mut Editor, approve: bool) -> bool {
    let Some(session_id) = ed.agent.any_pending() else {
        return false;
    };
    let Some(pending) = ed.agent.approval_of(session_id).cloned() else {
        return false;
    };
    let queued = ed.agent.take_approval(session_id).unwrap_or_default();

    let content = if approve {
        let call = ToolCall {
            id: pending.call_id.clone(),
            name: pending.tool.clone(),
            arguments: pending.arguments.clone(),
        };
        let output = execute_tool(ed, &call);
        record_side_effects(ed, session_id, &call);
        ed.agent.note_step_output(session_id, &output);
        llm::truncate_for_context(&output)
    } else {
        let declined = "user declined this action".to_string();
        ed.agent.note_step_output(session_id, &declined);
        declined
    };

    let results = vec![json!({
        "role": "tool",
        "tool_call_id": pending.call_id,
        "content": content,
    })];

    if queued.is_empty() {
        finish_batch(ed, session_id, results);
    } else {
        ed.agent.append_messages(session_id, results);
        process_calls(ed, session_id, queued);
    }
    true
}

/// The read-only commands the agent may run without asking.  Everything
/// else dispatched through `run_editor_command` — file writes, shell
/// commands, VC mutations — parks the session until the user approves.
const APPROVAL_EXEMPT_COMMANDS: &[&str] = &[
    "get-diagnostics",
    "list-commands",
    "describe-mode",
    "buffer-info",
    "vc-status",
    "project-info",
];

fn requires_approval(call: &ToolCall) -> bool {
    if call.name != "run_editor_command" {
        return false;
    }
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
    match args.get("command").and_then(Value::as_str) {
        Some(cmd) => !APPROVAL_EXEMPT_COMMANDS.contains(&cmd),
        None => true,
    }
}

/// Track observable side effects of an executed call for the session's
/// file list (the context panel's "files" chips).
fn record_side_effects(ed: &mut Editor, session_id: u64, call: &ToolCall) {
    if call.name != "run_editor_command" {
        return;
    }
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
    let command = args.get("command").and_then(Value::as_str).unwrap_or("");
    if command == "open-file" {
        if let Some(path) = args
            .get("args")
            .and_then(|a| a.get("path"))
            .and_then(Value::as_str)
        {
            ed.agent.record_file(session_id, path);
        }
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
    use crate::agent::EntryKind;
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
        // open-file through the agent = the real command path, but it must
        // be approved first — the session parks until the user decides.
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
        handle_reply(&mut ed, sid, "Opening the file.".into(), &calls_json, None);
        assert_eq!(
            ed.agent.status(sid),
            Some(&AgentStatus::WaitingApproval),
            "mutating commands park the session"
        );
        assert!(
            !ed.buffers.iter().any(|(_, a)| {
                a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref()
                    == Some(path.to_string_lossy().as_ref())
            }),
            "nothing executes before approval"
        );

        assert!(resolve_approval(&mut ed, true));
        assert!(
            ed.buffers.iter().any(|(_, a)| {
                a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref()
                    == Some(path.to_string_lossy().as_ref())
            }),
            "the approved call opened the buffer"
        );
        assert_eq!(
            ed.agent.sessions[&sid].files,
            vec![path.to_string_lossy().to_string()],
            "the session tracked the touched file"
        );
        assert_eq!(
            ed.agent.status(sid),
            Some(&AgentStatus::Failed),
            "without a background pool the next turn cannot start"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn declining_a_call_reports_it_to_the_model_and_continues() {
        let mut ed = editor();
        let sid = ed.agent.create_session("run something");
        ed.agent
            .begin_messages(sid, vec![json!({"role": "user", "content": "go"})]);
        let calls = vec![call(
            "run_editor_command",
            r#"{"command":"shell-command","args":{"command":"rm -rf /"}}"#,
        )];
        let calls_json = serde_json::to_string(&calls).unwrap();
        handle_reply(&mut ed, sid, String::new(), &calls_json, None);
        assert_eq!(ed.agent.status(sid), Some(&AgentStatus::WaitingApproval));

        assert!(resolve_approval(&mut ed, false));
        let transcript = &ed.agent.sessions[&sid].transcript;
        assert!(
            transcript
                .iter()
                .any(|e| matches!(&e.kind, EntryKind::StepOutput(t) if t.contains("declined"))),
            "the decline lands in the transcript: {transcript:?}"
        );
        assert_eq!(ed.agent.approval_of(sid), None);
    }

    #[test]
    fn read_only_commands_run_without_approval() {
        let mut ed = editor();
        let sid = ed.agent.create_session("check diagnostics");
        let calls = vec![call(
            "run_editor_command",
            r#"{"command":"get-diagnostics"}"#,
        )];
        let calls_json = serde_json::to_string(&calls).unwrap();
        handle_reply(&mut ed, sid, String::new(), &calls_json, None);
        assert_ne!(
            ed.agent.status(sid),
            Some(&AgentStatus::WaitingApproval),
            "exempt commands never park the session"
        );
    }

    #[test]
    fn calls_queued_behind_an_approval_run_after_it() {
        let mut ed = editor();
        let sid = ed.agent.create_session("open then read");
        ed.agent
            .begin_messages(sid, vec![json!({"role": "user", "content": "go"})]);
        let calls = vec![
            call("run_editor_command", r#"{"command":"get-diagnostics"}"#),
            call("read_active_buffer", "{}"),
        ];
        // No approval in this mix — both run in order, output follows each.
        let calls_json = serde_json::to_string(&calls).unwrap();
        handle_reply(&mut ed, sid, String::new(), &calls_json, None);
        let transcript = &ed.agent.sessions[&sid].transcript;
        let outputs = transcript
            .iter()
            .filter(|e| matches!(e.kind, EntryKind::StepOutput(_)))
            .count();
        assert_eq!(outputs, 2, "one output per executed call: {transcript:?}");
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
            matches!(&transcript.first().map(|e| &e.kind), Some(EntryKind::Action { command, .. }) if command == "read_active_buffer"),
            "action recorded: {transcript:?}"
        );
        assert!(
            matches!(&transcript.last().map(|e| &e.kind), Some(EntryKind::StepOutput(text)) if text.contains("buffer: scratch")),
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
