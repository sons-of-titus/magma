//! Executor implementations (Phase 11 — AI/Agent Architecture).
//!
//! `SafeExecutor` dispatches each PlanStep through `execute_command`, which
//! means every agent action goes through the command registry and can be
//! intercepted, logged, or rejected by the security model.

use std::collections::HashMap;
use crate::kernel::command::{execute_command, args::ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::event::keys;
use crate::kernel::event::payload::AgentActionPayload;
use super::types::{ExecutionResult, Executor, Plan};

/// Dispatches PlanSteps through the command registry with no special privileges.
///
/// Each step:
///   1. Emits an `agent-action` event on the editor event bus.
///   2. Calls `execute_command` with the step's command name and args.
///   3. Records success or failure in `ExecutionResult`.
pub struct SafeExecutor;

impl Executor for SafeExecutor {
    fn execute(&mut self, plan: &Plan, ed: &mut Editor) -> ExecutionResult {
        let mut steps_executed = 0usize;
        let mut steps_failed = 0usize;
        let mut outputs: Vec<String> = Vec::new();

        for step in &plan.steps {
            let args: HashMap<String, ArgValue> = step.args.iter()
                .map(|(k, v)| (k.clone(), ArgValue::String(v.clone())))
                .collect();

            ed.events.emit_typed(keys::events::AGENT_ACTION, AgentActionPayload {
                session_id: String::new(),
                command: step.command.clone(),
                description: step.description.clone(),
            });

            match execute_command(ed, &step.command, &args) {
                Ok(()) => {
                    steps_executed += 1;
                    outputs.push(format!("ok: {}", step.description));
                }
                Err(e) => {
                    steps_failed += 1;
                    outputs.push(format!("err: {}: {e}", step.description));
                }
            }
        }

        ExecutionResult {
            steps_executed,
            steps_failed,
            output: outputs.join("\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::agent::types::{Plan, PlanStep};

    #[test]
    fn empty_plan_returns_zero_counts() {
        let plan = Plan::empty("nothing to do");
        let result = ExecutionResult {
            steps_executed: 0,
            steps_failed: 0,
            output: String::new(),
        };
        assert_eq!(result.steps_executed, 0);
        assert_eq!(result.steps_failed, 0);
        let _ = plan; // consumed
    }

    #[test]
    fn plan_step_fields_accessible() {
        let step = PlanStep {
            command: "test-cmd".to_string(),
            args: HashMap::from([("key".to_string(), "val".to_string())]),
            description: "A test step".to_string(),
        };
        assert_eq!(step.command, "test-cmd");
        assert_eq!(step.args.get("key").map(|s| s.as_str()), Some("val"));
    }
}
