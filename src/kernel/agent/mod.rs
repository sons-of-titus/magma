//! AI/Agent Architecture (Phase 11).
//!
//! Agents are system participants that observe editor state, reason about a
//! goal, and dispatch editor commands — not chat windows.
//!
//! Subsystem layout:
//!   types.rs    — ContextProvider, Planner, Executor traits + AgentContext, Plan, PlanStep
//!   context.rs  — EditorContextProvider, LlmContextProvider
//!   planner.rs  — BuiltinPlanner (deterministic, no LLM required)
//!   executor.rs — SafeExecutor (dispatches through CommandRegistry)
//!   mod.rs      — AgentManager (session tracking + async submission)

pub mod types;
pub mod context;
pub mod planner;
pub mod executor;

pub use types::{
    AgentContext, ContextProvider, Plan, PlanStep, Planner, ExecutionResult, Executor,
};
pub use context::{EditorContextProvider, LlmContextProvider};
pub use planner::BuiltinPlanner;
pub use executor::SafeExecutor;

use std::collections::HashMap;

/// Status of an in-flight or completed agent session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Running,
    Completed,
    Failed,
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentStatus::Running   => "running",
            AgentStatus::Completed => "completed",
            AgentStatus::Failed    => "failed",
        }
    }
}

/// State for a single agent session.
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: u64,
    pub prompt: String,
    pub status: AgentStatus,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// Tracks all active and completed agent sessions.
#[derive(Default)]
pub struct AgentManager {
    pub sessions: HashMap<u64, AgentSession>,
    next_id: u64,
}

impl AgentManager {
    pub fn new() -> Self { Self::default() }

    /// Create a new session in `Running` state and return its ID.
    pub fn create_session(&mut self, prompt: impl Into<String>) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.sessions.insert(id, AgentSession {
            id,
            prompt: prompt.into(),
            status: AgentStatus::Running,
            result: None,
            error: None,
        });
        id
    }

    /// Mark a session as successfully completed.
    pub fn complete_session(&mut self, id: u64, result: impl Into<String>) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.status = AgentStatus::Completed;
            s.result = Some(result.into());
        }
    }

    /// Mark a session as failed.
    pub fn fail_session(&mut self, id: u64, error: impl Into<String>) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.status = AgentStatus::Failed;
            s.error = Some(error.into());
        }
    }

    /// Return the status for a session, or `None` if the ID is unknown.
    pub fn status(&self, id: u64) -> Option<&AgentStatus> {
        self.sessions.get(&id).map(|s| &s.status)
    }

    /// Return the result string for a completed session, or `None`.
    pub fn result(&self, id: u64) -> Option<&str> {
        self.sessions.get(&id).and_then(|s| s.result.as_deref())
    }

    /// All sessions sorted by ascending ID.
    pub fn list(&self) -> Vec<&AgentSession> {
        let mut v: Vec<_> = self.sessions.values().collect();
        v.sort_by_key(|s| s.id);
        v
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_complete_session() {
        let mut mgr = AgentManager::new();
        let id = mgr.create_session("do something");
        assert_eq!(mgr.status(id), Some(&AgentStatus::Running));
        mgr.complete_session(id, "done");
        assert_eq!(mgr.status(id), Some(&AgentStatus::Completed));
        assert_eq!(mgr.result(id), Some("done"));
    }

    #[test]
    fn create_and_fail_session() {
        let mut mgr = AgentManager::new();
        let id = mgr.create_session("will fail");
        mgr.fail_session(id, "oops");
        assert_eq!(mgr.status(id), Some(&AgentStatus::Failed));
        assert_eq!(mgr.sessions[&id].error.as_deref(), Some("oops"));
    }

    #[test]
    fn unknown_session_returns_none() {
        let mgr = AgentManager::new();
        assert!(mgr.status(999).is_none());
        assert!(mgr.result(999).is_none());
    }

    #[test]
    fn list_sorted_by_id() {
        let mut mgr = AgentManager::new();
        let a = mgr.create_session("a");
        let b = mgr.create_session("b");
        let ids: Vec<u64> = mgr.list().iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn count_tracks_sessions() {
        let mut mgr = AgentManager::new();
        assert_eq!(mgr.count(), 0);
        mgr.create_session("x");
        mgr.create_session("y");
        assert_eq!(mgr.count(), 2);
    }

    #[test]
    fn agent_status_as_str() {
        assert_eq!(AgentStatus::Running.as_str(),   "running");
        assert_eq!(AgentStatus::Completed.as_str(), "completed");
        assert_eq!(AgentStatus::Failed.as_str(),    "failed");
    }
}
