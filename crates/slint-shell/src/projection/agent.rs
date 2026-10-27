//! Agent tool-window projections: sessions, transcript entries, status.

use std::collections::HashSet;

use crate::{AgentEntry, AgentSessionChip};
use kernel::state::Editor;

// ── Agent tool window ────────────────────────────────────────────────────────

/// The session the dock should render: the pinned one, else the newest.
fn agent_session<'a>(
    ed: &'a Editor,
    active: Option<u64>,
) -> (Option<&'a kernel::agent::AgentSession>, u64) {
    let latest = ed.agent.sessions.keys().copied().max().unwrap_or(0);
    let id = active
        .and_then(|a| ed.agent.sessions.get(&a).map(|_| a))
        .unwrap_or(latest);
    (ed.agent.sessions.get(&id), latest)
}

/// The id backing the dock's current view (pinned or latest).
pub fn active_session_id(ed: &Editor, active: Option<u64>) -> Option<u64> {
    let latest = ed.agent.sessions.keys().copied().max()?;
    Some(match active {
        Some(a) if ed.agent.sessions.contains_key(&a) => a,
        _ => latest,
    })
}

pub fn agent_session_chips(ed: &Editor, active: Option<u64>) -> Vec<AgentSessionChip> {
    let (_, latest) = agent_session(ed, active);
    let mut sessions: Vec<_> = ed.agent.sessions.values().collect();
    sessions.sort_by_key(|s| s.id);
    sessions
        .iter()
        .map(|s| AgentSessionChip {
            id: s.id as i32,
            status: s.status.as_str().into(),
            selected: s.id == latest,
        })
        .collect()
}

/// Render one session as a linear entry list.  Tool outputs are folded into
/// their preceding action card; a stray output (older shape) renders alone.
pub fn agent_entries(
    ed: &Editor,
    active: Option<u64>,
    expanded: &HashSet<(u64, usize)>,
) -> Vec<AgentEntry> {
    let (session_view, _) = agent_session(ed, active);
    let Some(session) = session_view else {
        return Vec::new();
    };
    let status_done = session.status != kernel::agent::AgentStatus::Running;
    let mut out = Vec::new();
    out.push(AgentEntry {
        kind: "user".into(),
        text: session.prompt.clone().into(),
        detail: "".into(),
        status: "".into(),
        expanded: false,
        idx: -1,
    });
    for (i, entry) in session.transcript.iter().enumerate() {
        let expanded = expanded.contains(&(session.id, i));
        match entry {
            kernel::agent::TranscriptEntry::Thought(text) => out.push(AgentEntry {
                kind: "thought".into(),
                text: text.clone().into(),
                detail: "".into(),
                status: "".into(),
                expanded: false,
                idx: i as i32,
            }),
            kernel::agent::TranscriptEntry::Action {
                command,
                description,
            } => {
                let status = match session.transcript.get(i + 1) {
                    Some(kernel::agent::TranscriptEntry::StepOutput(o))
                        if o.starts_with("error:") =>
                    {
                        "error"
                    }
                    Some(kernel::agent::TranscriptEntry::StepOutput(_)) => "ok",
                    _ if !status_done => "running",
                    _ => "planned",
                };
                out.push(AgentEntry {
                    kind: "action".into(),
                    text: description.clone().into(),
                    detail: command.clone().into(),
                    status: status.into(),
                    expanded: expanded && status != "running" && status != "planned",
                    idx: i as i32,
                });
            }
            kernel::agent::TranscriptEntry::StepOutput(text) => {
                let follows_action = i > 0
                    && matches!(
                        session.transcript.get(i - 1),
                        Some(kernel::agent::TranscriptEntry::Action { .. })
                    );
                if follows_action {
                    continue;
                }
                out.push(AgentEntry {
                    kind: "output".into(),
                    text: preview_output(text).into(),
                    detail: "".into(),
                    status: "".into(),
                    expanded: false,
                    idx: i as i32,
                });
            }
        }
    }
    if let Some(result) = &session.result {
        out.push(AgentEntry {
            kind: "result".into(),
            text: result.clone().into(),
            detail: "".into(),
            status: "".into(),
            expanded: false,
            idx: -1,
        });
    }
    if let Some(error) = &session.error {
        out.push(AgentEntry {
            kind: "error".into(),
            text: error.clone().into(),
            detail: "".into(),
            status: "".into(),
            expanded: false,
            idx: -1,
        });
    }
    out
}

/// Keep card bodies bounded; the kernel keeps the full text.
fn preview_output(text: &str) -> String {
    const LIMIT: usize = 1_500;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let cut: String = text.chars().take(LIMIT).collect();
    let rest = text.chars().count() - LIMIT;
    format!("{cut}\n… [{rest} more characters]")
}

pub fn agent_turns_left(ed: &Editor, active: Option<u64>) -> i32 {
    let (session, _) = agent_session(ed, active);
    session
        .and_then(|s| ed.agent.turns_left_of(s.id))
        .unwrap_or(kernel::agent::MAX_TURNS) as i32
}

/// Whether any session is mid-run (drives the composer's stop affordance).
pub fn agent_running(ed: &Editor) -> bool {
    ed.agent
        .sessions
        .values()
        .any(|s| s.status == kernel::agent::AgentStatus::Running)
}
