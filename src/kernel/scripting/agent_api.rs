//! Janet API for the AI/Agent Architecture (Phase 11).
//!
//! Registered C functions:
//!   (agent/ask prompt)            → session-id
//!   (agent/run-task description)  → session-id
//!   (agent/on-event kind callback)→ nil
//!   (agent/status id)             → keyword
//!   (agent/result id)             → string or nil
//!   (agent/list)                  → array of {:id :prompt :status :result}

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::agent::{BuiltinPlanner, EditorContextProvider};
use crate::kernel::agent::types::{ContextProvider, Planner};

/// (agent/ask prompt) → session-id
///
/// Snapshot the editor context, submit the goal to the BuiltinPlanner on the
/// background thread, and return a numeric session ID.  Results arrive via
/// `agent-thought`, `agent-action`, and `agent-result` events.
unsafe extern "C-unwind" fn c_agent_ask(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let prompt = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("agent/ask: prompt string required"),
        };

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("agent/ask: no background runtime available"),
        };

        let ctx = EditorContextProvider.gather(ed);
        let session_id = ed.agent.create_session(&prompt);
        let sender = bg.sender.clone();

        bg.spawn_blocking(move || {
            let plan = BuiltinPlanner.plan(&prompt, &ctx);

            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::AgentThought {
                session_id,
                thought: plan.rationale.clone(),
            });

            for step in &plan.steps {
                let _ = sender.send(crate::kernel::runtime::BackgroundEvent::AgentAction {
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

            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::AgentResult {
                session_id,
                result,
            });
        });

        conv::integer(session_id as i32)
    })
}

/// (agent/run-task description) → session-id
///
/// Submit a structured task description.  The BuiltinPlanner looks for
/// `run:`, `goto:`, `diagnose:` prefixes in `description` and generates
/// corresponding plan steps.  Returns a session ID.
unsafe extern "C-unwind" fn c_agent_run_task(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let description = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("agent/run-task: description string required"),
        };

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("agent/run-task: no background runtime available"),
        };

        let ctx = EditorContextProvider.gather(ed);
        let session_id = ed.agent.create_session(&description);
        let sender = bg.sender.clone();

        // For run-task, the planner dispatches steps; each step arrives as an
        // AgentAction event that the Janet handler can choose to execute.
        bg.spawn_blocking(move || {
            let plan = BuiltinPlanner.plan(&description, &ctx);

            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::AgentThought {
                session_id,
                thought: plan.rationale.clone(),
            });

            for step in &plan.steps {
                let _ = sender.send(crate::kernel::runtime::BackgroundEvent::AgentAction {
                    session_id,
                    command: step.command.clone(),
                    description: step.description.clone(),
                });
            }

            let result = if plan.steps.is_empty() {
                plan.rationale
            } else {
                format!("{} action(s) dispatched", plan.steps.len())
            };

            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::AgentResult {
                session_id,
                result,
            });
        });

        conv::integer(session_id as i32)
    })
}

/// (agent/status id) → keyword
///
/// Return `:running`, `:completed`, `:failed`, or `:not-found`.
unsafe extern "C-unwind" fn c_agent_status(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(-1) as u64;
        use crate::kernel::agent::AgentStatus;
        let kw = match ed.agent.status(id) {
            None                          => "not-found",
            Some(AgentStatus::Running)    => "running",
            Some(AgentStatus::Completed)  => "completed",
            Some(AgentStatus::Failed)     => "failed",
        };
        conv::keyword(kw)
    })
}

/// (agent/result id) → string or nil
///
/// Return the result string for a completed session, or nil.
unsafe extern "C-unwind" fn c_agent_result(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(-1) as u64;
        match ed.agent.result(id) {
            None => conv::nil(),
            Some(r) => conv::string(r),
        }
    })
}

/// (agent/list) → array of {:id :prompt :status :result}
///
/// Return all agent sessions.
unsafe extern "C-unwind" fn c_agent_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let sessions = ed.agent.list();
        let arr = janet_wrap_array(janet_array(sessions.len() as i32));
        for s in sessions {
            let tbl = janet_wrap_table(janet_table(4));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("id"),     conv::integer(s.id as i32));
            janet_table_put(t, conv::keyword("prompt"), conv::string(&s.prompt));
            janet_table_put(t, conv::keyword("status"), conv::keyword(s.status.as_str()));
            let result_val = s.result.as_deref()
                .map(|r| conv::string(r))
                .unwrap_or_else(conv::nil);
            janet_table_put(t, conv::keyword("result"), result_val);
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"agent/ask".as_ptr() as *const _,
            cfun: Some(c_agent_ask as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Submit an agent query; returns session-id; results arrive as agent-* events".as_ptr() as *const _,
        },
        JanetReg {
            name: c"agent/run-task".as_ptr() as *const _,
            cfun: Some(c_agent_run_task as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Submit a structured task description; returns session-id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"agent/status".as_ptr() as *const _,
            cfun: Some(c_agent_status as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return :running :completed :failed or :not-found for a session".as_ptr() as *const _,
        },
        JanetReg {
            name: c"agent/result".as_ptr() as *const _,
            cfun: Some(c_agent_result as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the result string for a completed agent session, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"agent/list".as_ptr() as *const _,
            cfun: Some(c_agent_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return an array of {:id :prompt :status :result} for all agent sessions".as_ptr() as *const _,
        },
    ]
}
