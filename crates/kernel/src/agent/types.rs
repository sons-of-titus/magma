//! Agent types — re-exports from the `agent` crate plus editor-coupled traits.

use crate::state::Editor;

pub use agent::types::*;

/// Gathers a snapshot of editor state for the planner.
pub trait ContextProvider: Send + Sync {
    fn gather(&self, ed: &Editor) -> agent::types::AgentContext;
}

/// Executes a Plan's steps against the editor.
pub trait Executor {
    fn execute(&mut self, plan: &agent::types::Plan, ed: &mut Editor) -> agent::types::ExecutionResult;
}
