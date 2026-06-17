//! Core trait definitions for the AI/Agent Architecture (Phase 11).
//!
//! Three traits form the agent pipeline:
//!   ContextProvider — snapshots relevant editor state.
//!   Planner         — turns a goal + context into a sequence of PlanSteps.
//!   Executor        — dispatches PlanSteps through the command registry.

use std::collections::HashMap;
use crate::kernel::state::Editor;

/// A snapshot of editor state passed to the Planner on the background thread.
#[derive(Debug, Clone, Default)]
pub struct AgentContext {
    pub active_buffer_content: String,
    pub active_buffer_path: Option<String>,
    pub active_buffer_language: String,
    pub diagnostics: Vec<String>,
    pub symbols: Vec<String>,
    pub project_name: Option<String>,
    pub project_language: String,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

impl AgentContext {
    /// Format the context as a human/LLM-readable summary string.
    pub fn to_prompt_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(ref p) = self.active_buffer_path {
            parts.push(format!("File: {p}"));
        }
        if !self.active_buffer_language.is_empty() {
            parts.push(format!("Language: {}", self.active_buffer_language));
        }
        if let Some(ref name) = self.project_name {
            parts.push(format!("Project: {name} ({})", self.project_language));
        }
        if !self.diagnostics.is_empty() {
            parts.push(format!("Diagnostics:\n{}", self.diagnostics.join("\n")));
        }
        if !self.symbols.is_empty() {
            parts.push(format!("Known symbols: {}", self.symbols.join(", ")));
        }
        if !self.active_buffer_content.is_empty() {
            let preview: String = self.active_buffer_content.chars().take(2048).collect();
            parts.push(format!("Buffer (first 2048 chars):\n{preview}"));
        }
        parts.join("\n\n")
    }
}

/// Gathers a snapshot of editor state for the planner.
///
/// Implementations may not store a reference to the editor — the result
/// `AgentContext` must be `Send + 'static` so it can cross the thread boundary.
pub trait ContextProvider: Send + Sync {
    fn gather(&self, ed: &Editor) -> AgentContext;
}

/// A single atomic action the agent wants the editor to perform.
#[derive(Debug, Clone)]
pub struct PlanStep {
    /// Name of the editor command (must exist in `CommandRegistry`).
    pub command: String,
    /// Arguments to pass, serialised as strings.
    pub args: HashMap<String, String>,
    /// Human-readable description of the intent.
    pub description: String,
}

/// An ordered sequence of steps produced by the Planner.
#[derive(Debug, Clone)]
pub struct Plan {
    pub steps: Vec<PlanStep>,
    /// Human-readable explanation of why these steps were chosen.
    pub rationale: String,
}

impl Plan {
    pub fn empty(rationale: impl Into<String>) -> Self {
        Plan { steps: Vec::new(), rationale: rationale.into() }
    }
}

/// Converts a goal string and editor context into a Plan.
pub trait Planner: Send + Sync {
    fn plan(&self, goal: &str, ctx: &AgentContext) -> Plan;
}

/// The result of executing a Plan.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub steps_executed: usize,
    pub steps_failed: usize,
    pub output: String,
}

/// Executes a Plan's steps against the editor.
///
/// All dispatch goes through `execute_command` so capability checks, event
/// hooks, and logging apply to every agent action.
pub trait Executor {
    fn execute(&mut self, plan: &Plan, ed: &mut Editor) -> ExecutionResult;
}
