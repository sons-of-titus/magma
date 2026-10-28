//! Agent state model and core data types (editor-independent).

use std::collections::HashMap;
use std::time::Instant;

/// Maximum model round-trips per session.
pub const MAX_TURNS: u8 = 12;

/// Status of an in-flight or completed agent session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Running,
    /// A tool call needs the user's approval before it executes; the session
    /// resumes (or declines) through the same loop.
    WaitingApproval,
    Completed,
    Failed,
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentStatus::Running => "running",
            AgentStatus::WaitingApproval => "waiting-approval",
            AgentStatus::Completed => "completed",
            AgentStatus::Failed => "failed",
        }
    }
}

/// One entry in a session's transcript — what the agent panel renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptEntry {
    /// Milliseconds since the session started; lets the UI show per-step
    /// elapsed time without wall-clock assumptions.
    pub at_ms: u64,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// A reasoning/plan step.
    Thought(String),
    /// A command the agent dispatched.
    Action {
        command: String,
        description: String,
    },
    /// The outcome of a foreground-executed step.
    StepOutput(String),
}

/// A tool call parked until the user approves or declines it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub call_id: String,
    pub tool: String,
    /// The call's raw arguments JSON, replayed verbatim on approval.
    pub arguments: String,
    /// What will run, as a single human-readable line.
    pub summary: String,
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
    /// When the session started; drives elapsed-time displays.
    pub created: Instant,
    /// The tool call awaiting approval, while status is `WaitingApproval`.
    pub pending: Option<PendingApproval>,
    /// Tool calls parked behind the pending one, to run after it resolves.
    pub queued_calls: Vec<crate::llm::ToolCall>,
    /// File paths the agent opened through editor commands, in order.
    pub files: Vec<String>,
}

/// Tracks all active and completed agent sessions.
#[derive(Default)]
pub struct AgentManager {
    pub sessions: HashMap<u64, AgentSession>,
    next_id: u64,
}

impl AgentManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new session in `Running` state and return its ID.
    pub fn create_session(&mut self, prompt: impl Into<String>) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.sessions.insert(
            id,
            AgentSession {
                id,
                prompt: prompt.into(),
                status: AgentStatus::Running,
                result: None,
                error: None,
                transcript: Vec::new(),
                messages: Vec::new(),
                turns_left: MAX_TURNS,
                created: Instant::now(),
                pending: None,
                queued_calls: Vec::new(),
                files: Vec::new(),
            },
        );
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
        self.sessions
            .get_mut(&id)
            .map(|s| {
                if s.status == AgentStatus::Running {
                    s.turns_left = 0;
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false)
    }

    pub fn note_transcript(&mut self, id: u64, kind: EntryKind) {
        if let Some(s) = self.sessions.get_mut(&id)
            && matches!(
                s.status,
                AgentStatus::Running | AgentStatus::WaitingApproval
            )
        {
            let at_ms = s.created.elapsed().as_millis() as u64;
            s.transcript.push(TranscriptEntry { at_ms, kind });
        }
    }

    pub fn note_thought(&mut self, id: u64, thought: &str) {
        self.note_transcript(id, EntryKind::Thought(thought.to_string()));
    }

    pub fn note_action(&mut self, id: u64, command: &str, description: &str) {
        self.note_transcript(
            id,
            EntryKind::Action {
                command: command.to_string(),
                description: description.to_string(),
            },
        );
    }

    pub fn note_step_output(&mut self, id: u64, output: &str) {
        self.note_transcript(id, EntryKind::StepOutput(output.to_string()));
    }

    // ── Approval gate ──────────────────────────────────────────────────

    /// Park `call` for user approval, keeping `queued` (the calls after it)
    /// for when the decision lands.  Returns false when the session is not
    /// in a state that can be parked.
    pub fn park_for_approval(
        &mut self,
        id: u64,
        call: crate::llm::ToolCall,
        summary: String,
        queued: Vec<crate::llm::ToolCall>,
    ) -> bool {
        match self.sessions.get_mut(&id) {
            Some(s) if s.status == AgentStatus::Running && s.pending.is_none() => {
                s.pending = Some(PendingApproval {
                    call_id: call.id.clone(),
                    tool: call.name.clone(),
                    arguments: call.arguments.clone(),
                    summary,
                });
                s.queued_calls = queued;
                s.status = AgentStatus::WaitingApproval;
                true
            }
            _ => false,
        }
    }

    /// The call awaiting approval, if any.
    pub fn approval_of(&self, id: u64) -> Option<&PendingApproval> {
        self.sessions.get(&id).and_then(|s| s.pending.as_ref())
    }

    /// Most recent session with a decision pending, if any.
    pub fn any_pending(&self) -> Option<u64> {
        self.sessions
            .values()
            .filter(|s| s.pending.is_some())
            .map(|s| s.id)
            .max()
    }

    /// Resolve the pending call: back to `Running` with the queued calls
    /// handed to the caller.  Returns false when nothing was parked.
    pub fn take_approval(&mut self, id: u64) -> Option<Vec<crate::llm::ToolCall>> {
        let s = self.sessions.get_mut(&id)?;
        s.pending.take()?;
        s.status = AgentStatus::Running;
        Some(std::mem::take(&mut s.queued_calls))
    }

    // ── File tracking ──────────────────────────────────────────────────

    /// Record a file path the agent touched, deduplicated, order kept.
    pub fn record_file(&mut self, id: u64, path: &str) {
        if let Some(s) = self.sessions.get_mut(&id) {
            let canonical = path.trim().to_string();
            if !canonical.is_empty() && !s.files.iter().any(|f| f == &canonical) {
                s.files.push(canonical);
            }
        }
    }

    /// Elapsed milliseconds for a live or finished session.
    pub fn elapsed_ms(&self, id: u64) -> Option<u64> {
        self.sessions
            .get(&id)
            .map(|s| s.created.elapsed().as_millis() as u64)
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
        Plan {
            steps: Vec::new(),
            rationale: rationale.into(),
        }
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
        assert_eq!(AgentStatus::Running.as_str(), "running");
        assert_eq!(AgentStatus::WaitingApproval.as_str(), "waiting-approval");
        assert_eq!(AgentStatus::Completed.as_str(), "completed");
        assert_eq!(AgentStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn approval_park_and_take_round_trip() {
        let mut mgr = AgentManager::new();
        let sid = mgr.create_session("x");
        let call = crate::llm::ToolCall {
            id: "c1".into(),
            name: "run_editor_command".into(),
            arguments: r#"{"command":"shell-command"}"#.into(),
        };
        let queued = vec![call.clone()];
        assert!(mgr.park_for_approval(sid, call, "shell-command".into(), queued));
        assert_eq!(mgr.status(sid), Some(&AgentStatus::WaitingApproval));
        assert_eq!(mgr.approval_of(sid).unwrap().tool, "run_editor_command");
        assert_eq!(mgr.any_pending(), Some(sid));
        // Transcript still lands while waiting (the action entry precedes).
        mgr.note_thought(sid, "mid-flight");
        assert_eq!(mgr.sessions[&sid].transcript.len(), 1);
        let queued = mgr.take_approval(sid).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(mgr.status(sid), Some(&AgentStatus::Running));
        assert!(mgr.approval_of(sid).is_none());
        assert_eq!(mgr.any_pending(), None);
    }

    #[test]
    fn park_rejected_when_not_running() {
        let mut mgr = AgentManager::new();
        let sid = mgr.create_session("x");
        mgr.complete_session(sid, "done");
        let call = crate::llm::ToolCall {
            id: "c1".into(),
            name: "run_editor_command".into(),
            arguments: "{}".into(),
        };
        assert!(!mgr.park_for_approval(sid, call, "x".into(), Vec::new()));
    }

    #[test]
    fn transcript_entries_carry_session_elapsed_time() {
        let mut mgr = AgentManager::new();
        let sid = mgr.create_session("x");
        mgr.note_thought(sid, "thinking");
        mgr.note_action(sid, "open-file", "open it");
        let entries = &mgr.sessions[&sid].transcript;
        assert_eq!(entries.len(), 2);
        assert!(entries[0].at_ms <= entries[1].at_ms);
    }

    #[test]
    fn record_file_deduplicates_in_order() {
        let mut mgr = AgentManager::new();
        let sid = mgr.create_session("x");
        mgr.record_file(sid, "src/a.rs");
        mgr.record_file(sid, "src/b.rs");
        mgr.record_file(sid, "src/a.rs");
        mgr.record_file(sid, "  ");
        assert_eq!(mgr.sessions[&sid].files, vec!["src/a.rs", "src/b.rs"]);
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
