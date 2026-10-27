//! Agent state model and core data types (editor-independent).

use std::collections::HashMap;

/// Maximum model round-trips per session.
pub const MAX_TURNS: u8 = 12;

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

/// One entry in a session's transcript — what the agent panel renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptEntry {
    /// A reasoning/plan step.
    Thought(String),
    /// A command the agent dispatched.
    Action { command: String, description: String },
    /// The outcome of a foreground-executed step.
    StepOutput(String),
}

/// State for a single agent session.
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: u64,
    pub prompt: String,
    pub status: AgentStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub transcript: Vec<TranscriptEntry>,
    /// LLM chat transcript (OpenAI message objects) for live sessions.
    pub messages: Vec<serde_json::Value>,
    /// Remaining model turns before the loop must stop.
    pub turns_left: u8,
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
            transcript: Vec::new(),
            messages: Vec::new(),
            turns_left: MAX_TURNS,
        });
        id
    }

    // ── LLM loop bookkeeping ───────────────────────────────────────────

    pub fn begin_messages(&mut self, id: u64, messages: Vec<serde_json::Value>) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.messages = messages;
        }
    }

    pub fn append_messages(&mut self, id: u64, messages: Vec<serde_json::Value>) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.messages.extend(messages);
        }
    }

    pub fn messages_of(&self, id: u64) -> Option<Vec<serde_json::Value>> {
        self.sessions.get(&id).map(|s| s.messages.clone())
    }

    pub fn turns_left_of(&self, id: u64) -> Option<u8> {
        self.sessions.get(&id).map(|s| s.turns_left)
    }

    pub fn spend_turn(&mut self, id: u64) -> Option<u8> {
        self.sessions.get_mut(&id).map(|s| {
            s.turns_left = s.turns_left.saturating_sub(1);
            s.turns_left
        })
    }

    pub fn request_stop(&mut self, id: u64) -> bool {
        self.sessions.get_mut(&id).map(|s| {
            if s.status == AgentStatus::Running {
                s.turns_left = 0;
                true
            } else {
                false
            }
        }).unwrap_or(false)
    }

    pub fn note_transcript(&mut self, id: u64, entry: TranscriptEntry) {
        if let Some(s) = self.sessions.get_mut(&id)
            && s.status == AgentStatus::Running {
                s.transcript.push(entry);
            }
    }

    pub fn note_thought(&mut self, id: u64, thought: &str) {
        self.note_transcript(id, TranscriptEntry::Thought(thought.to_string()));
    }

    pub fn note_action(&mut self, id: u64, command: &str, description: &str) {
        self.note_transcript(id, TranscriptEntry::Action {
            command: command.to_string(),
            description: description.to_string(),
        });
    }

    pub fn note_step_output(&mut self, id: u64, output: &str) {
        self.note_transcript(id, TranscriptEntry::StepOutput(output.to_string()));
    }

    pub fn complete_session(&mut self, id: u64, result: impl Into<String>) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.status = AgentStatus::Completed;
            s.result = Some(result.into());
        }
    }

    pub fn fail_session(&mut self, id: u64, error: impl Into<String>) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.status = AgentStatus::Failed;
            s.error = Some(error.into());
        }
    }

    pub fn status(&self, id: u64) -> Option<&AgentStatus> {
        self.sessions.get(&id).map(|s| &s.status)
    }

    pub fn result(&self, id: u64) -> Option<&str> {
        self.sessions.get(&id).and_then(|s| s.result.as_deref())
    }

    pub fn list(&self) -> Vec<&AgentSession> {
        let mut v: Vec<_> = self.sessions.values().collect();
        v.sort_by_key(|s| s.id);
        v
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

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

/// A single atomic action the agent wants the editor to perform.
#[derive(Debug, Clone)]
pub struct PlanStep {
    pub command: String,
    pub args: HashMap<String, String>,
    pub description: String,
}

/// An ordered sequence of steps produced by the Planner.
#[derive(Debug, Clone)]
pub struct Plan {
    pub steps: Vec<PlanStep>,
    pub rationale: String,
}

impl Plan {
    pub fn empty(rationale: impl Into<String>) -> Self {
        Plan { steps: Vec::new(), rationale: rationale.into() }
    }
}

/// The result of executing a Plan.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub steps_executed: usize,
    pub steps_failed: usize,
    pub output: String,
}

/// Converts a goal string and editor context into a Plan.
pub trait Planner: Send + Sync {
    fn plan(&self, goal: &str, ctx: &AgentContext) -> Plan;
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

    #[test]
    fn session_budget() {
        let mut mgr = AgentManager::new();
        let sid = mgr.create_session("x");
        assert_eq!(mgr.turns_left_of(sid), Some(MAX_TURNS));
        assert_eq!(mgr.spend_turn(sid), Some(MAX_TURNS - 1));
        assert!(mgr.request_stop(sid));
        assert_eq!(mgr.turns_left_of(sid), Some(0));
        mgr.complete_session(sid, "done");
        mgr.note_thought(sid, "late");
        assert!(mgr.sessions[&sid].transcript.is_empty());
    }
}
