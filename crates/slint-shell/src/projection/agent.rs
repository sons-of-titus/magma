//! Agent tool-window projections: sessions, transcript entries, status,
//! provider onboarding, and composer controls.

use std::collections::HashSet;

use kernel::agent::{AgentStatus, EntryKind};

use crate::{AgentEntry, AgentSessionChip};
use kernel::state::Editor;

// ── Agent tool window ────────────────────────────────────────────────────────

/// The session the dock should render: the pinned one, else the newest.
fn agent_session<'a>(
    ed: &'a Editor,
    active: Option<u64>,
) -> (Option<&'a kernel::agent::AgentSession>, u64) {
    if active == Some(0) {
        return (None, 0);
    }
    let latest = ed.agent.sessions.keys().copied().max().unwrap_or(0);
    let id = active
        .and_then(|a| ed.agent.sessions.get(&a).map(|_| a))
        .unwrap_or(latest);
    (ed.agent.sessions.get(&id), latest)
}

/// The id backing the dock's current view (pinned or latest).
pub fn active_session_id(ed: &Editor, active: Option<u64>) -> Option<u64> {
    if active == Some(0) {
        return None;
    }
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
            title: short_title(&s.prompt).into(),
            status: s.status.as_str().into(),
            selected: s.id == latest,
        })
        .collect()
}

/// First line of the prompt, capped — enough to tell sessions apart.
fn short_title(prompt: &str) -> String {
    let first_line = prompt.lines().next().unwrap_or("").trim();
    let mut title: String = first_line.chars().take(28).collect();
    if first_line.chars().count() > 28 {
        title.push('…');
    }
    if title.is_empty() {
        title = "untitled".to_string();
    }
    title
}

/// Human width for a step duration: "320ms", "2.4s", "1m04s".
fn format_elapsed(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}m{:02}s", ms / 60_000, (ms % 60_000) / 1_000)
    }
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
    let status_done = !matches!(
        session.status,
        AgentStatus::Running | AgentStatus::WaitingApproval
    );
    let waiting = session.status == AgentStatus::WaitingApproval;
    let mut out = Vec::new();
    out.push(AgentEntry {
        kind: "user".into(),
        text: session.prompt.clone().into(),
        detail: "".into(),
        status: "".into(),
        elapsed: "".into(),
        expanded: false,
        idx: -1,
    });
    for (i, entry) in session.transcript.iter().enumerate() {
        let expanded = expanded.contains(&(session.id, i));
        let next_ms = session.transcript.get(i + 1).map(|e| e.at_ms);
        let elapsed = next_ms.map(format_elapsed).unwrap_or_default();
        match &entry.kind {
            EntryKind::User(text) => out.push(AgentEntry {
                kind: "user".into(),
                text: text.clone().into(),
                detail: "".into(),
                status: "".into(),
                elapsed: "".into(),
                expanded: false,
                idx: i as i32,
            }),
            EntryKind::Thought(text) => out.push(AgentEntry {
                kind: "thought".into(),
                text: text.clone().into(),
                detail: "".into(),
                status: "".into(),
                elapsed: "".into(),
                expanded: false,
                idx: i as i32,
            }),
            EntryKind::Action {
                command,
                description,
            } => {
                let status = match session.transcript.get(i + 1).map(|e| &e.kind) {
                    Some(EntryKind::StepOutput(o)) if o.starts_with("error:") => "error",
                    Some(EntryKind::StepOutput(o)) if o.contains("user declined this action") => {
                        "declined"
                    }
                    Some(EntryKind::StepOutput(_)) => "ok",
                    _ if waiting && session.transcript.len() == i + 1 => "pending",
                    _ if !status_done => "running",
                    _ => "planned",
                };
                out.push(AgentEntry {
                    kind: "action".into(),
                    text: description.clone().into(),
                    detail: command.clone().into(),
                    status: status.into(),
                    elapsed: if status == "ok" || status == "error" || status == "declined" {
                        elapsed.into()
                    } else {
                        "".into()
                    },
                    expanded: expanded
                        && status != "running"
                        && status != "pending"
                        && status != "planned",
                    idx: i as i32,
                });
            }
            EntryKind::StepOutput(text) => {
                let follows_action = i > 0
                    && matches!(
                        session.transcript.get(i - 1).map(|e| &e.kind),
                        Some(EntryKind::Action { .. })
                    );
                if follows_action {
                    continue;
                }
                out.push(AgentEntry {
                    kind: "output".into(),
                    text: preview_output(text).into(),
                    detail: "".into(),
                    status: "".into(),
                    elapsed: "".into(),
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
            elapsed: "".into(),
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
            elapsed: "".into(),
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
        .any(|s| s.status == AgentStatus::Running)
}

/// One-word phase for the active session: what the agent is doing now.
pub fn agent_phase(ed: &Editor, active: Option<u64>) -> String {
    let (session, _) = agent_session(ed, active);
    let Some(s) = session else {
        return "ready".to_string();
    };
    match s.status {
        AgentStatus::Running => "working".to_string(),
        AgentStatus::WaitingApproval => "waiting-approval".to_string(),
        AgentStatus::Completed => "completed".to_string(),
        AgentStatus::Failed => {
            if s.error.as_deref() == Some("stopped by user")
                || s.result.as_deref() == Some("stopped by user")
            {
                "stopped".to_string()
            } else {
                "failed".to_string()
            }
        }
    }
}

/// The header's fuller readout: phase plus turn budget or elapsed time.
pub fn agent_phase_detail(ed: &Editor, active: Option<u64>) -> String {
    let (session, _) = agent_session(ed, active);
    let Some(s) = session else {
        return String::new();
    };
    match s.status {
        AgentStatus::Running => {
            let left = ed.agent.turns_left_of(s.id).unwrap_or(0);
            format!(
                "working · {} of {} turns left",
                left,
                kernel::agent::MAX_TURNS
            )
        }
        AgentStatus::WaitingApproval => "waiting for your approval".to_string(),
        AgentStatus::Completed | AgentStatus::Failed => {
            format!(
                "{} · ran for {}",
                phase_word(s),
                format_elapsed(ed.agent.elapsed_ms(s.id).unwrap_or(0))
            )
        }
    }
}

fn phase_word(s: &kernel::agent::AgentSession) -> &'static str {
    match s.status {
        AgentStatus::Failed => "failed",
        _ => "completed",
    }
}

/// The command summary awaiting approval, empty when nothing is parked.
pub fn agent_approval_summary(ed: &Editor, active: Option<u64>) -> String {
    let (session, _) = agent_session(ed, active);
    session
        .and_then(|s| ed.agent.approval_of(s.id))
        .map(|p| p.summary.clone())
        .unwrap_or_default()
}

/// Files the active session's agent opened, for the context strip.
pub fn agent_files(ed: &Editor, active: Option<u64>) -> Vec<String> {
    let (session, _) = agent_session(ed, active);
    session.map(|s| s.files.clone()).unwrap_or_default()
}
