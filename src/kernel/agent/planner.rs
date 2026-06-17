//! Planner implementations (Phase 11 — AI/Agent Architecture).
//!
//! `BuiltinPlanner` — a deterministic tool-use planner that maps simple goal
//! strings to editor command sequences without an LLM call.  It is the default
//! planner when no LLM backend is configured.

use std::collections::HashMap;
use super::types::{AgentContext, Plan, PlanStep, Planner};

/// Simple deterministic planner using prefix-based goal parsing.
///
/// Recognised goal prefixes:
///   `run:<task-name>`         → task/run
///   `goto:<symbol-name>`      → semantic/definitions
///   `diagnose:<buffer-path>`  → emits diagnostics summary (no-op steps)
///   (anything else)           → echo-style summary plan with no steps
pub struct BuiltinPlanner;

impl Planner for BuiltinPlanner {
    fn plan(&self, goal: &str, ctx: &AgentContext) -> Plan {
        let goal = goal.trim();

        if let Some(task_name) = goal.strip_prefix("run:") {
            let task_name = task_name.trim().to_string();
            Plan {
                steps: vec![PlanStep {
                    command: "task/run".to_string(),
                    args: HashMap::from([("name".to_string(), task_name.clone())]),
                    description: format!("Run task: {task_name}"),
                }],
                rationale: format!("Dispatching task '{task_name}' via the task scheduler"),
            }
        } else if let Some(symbol) = goal.strip_prefix("goto:") {
            let symbol = symbol.trim().to_string();
            Plan {
                steps: vec![PlanStep {
                    command: "semantic/definitions".to_string(),
                    args: HashMap::from([("symbol".to_string(), symbol.clone())]),
                    description: format!("Find definition of: {symbol}"),
                }],
                rationale: format!("Looking up definition of '{symbol}' in the symbol index"),
            }
        } else if let Some(path) = goal.strip_prefix("diagnose:") {
            let path = path.trim();
            let diag_count = ctx.diagnostics.len();
            Plan::empty(format!(
                "Diagnostics for '{path}': {diag_count} issue(s).\n{}",
                ctx.diagnostics.join("\n")
            ))
        } else {
            // Default: summarise context, no editor mutations.
            Plan::empty(format!(
                "Goal: {goal}\nActive file: {}\nLanguage: {}\nDiagnostics: {}\nSymbols known: {}",
                ctx.active_buffer_path.as_deref().unwrap_or("<scratch>"),
                if ctx.active_buffer_language.is_empty() { "unknown" } else { &ctx.active_buffer_language },
                ctx.diagnostics.len(),
                ctx.symbols.len(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ctx() -> AgentContext {
        AgentContext::default()
    }

    #[test]
    fn plan_run_produces_task_step() {
        let p = BuiltinPlanner;
        let plan = p.plan("run:build", &empty_ctx());
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].command, "task/run");
        assert_eq!(plan.steps[0].args.get("name").map(|s| s.as_str()), Some("build"));
    }

    #[test]
    fn plan_goto_produces_definition_step() {
        let p = BuiltinPlanner;
        let plan = p.plan("goto:MyStruct", &empty_ctx());
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].command, "semantic/definitions");
    }

    #[test]
    fn plan_diagnose_produces_empty_steps() {
        let p = BuiltinPlanner;
        let plan = p.plan("diagnose:/foo/bar.rs", &empty_ctx());
        assert!(plan.steps.is_empty());
        assert!(plan.rationale.contains("Diagnostics for '/foo/bar.rs'"));
    }

    #[test]
    fn plan_unknown_goal_returns_summary() {
        let p = BuiltinPlanner;
        let plan = p.plan("explain the codebase", &empty_ctx());
        assert!(plan.steps.is_empty());
        assert!(plan.rationale.contains("Goal: explain the codebase"));
    }

    #[test]
    fn plan_rationale_not_empty() {
        let p = BuiltinPlanner;
        for goal in ["run:test", "goto:Foo", "diagnose:bar", "just asking"] {
            let plan = p.plan(goal, &empty_ctx());
            assert!(!plan.rationale.is_empty(), "rationale empty for goal={goal}");
        }
    }
}
