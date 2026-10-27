//! Editor state → Slint models.
//!
//! Pure projections: no widget handles, no mutation.  Keeping them here makes
//! the shell's `refresh` a straight assignment pass.

use std::collections::HashSet;

use magma::kernel::project::tree::{
    relative_to_project, segments, visible_rows, FileNode,
};
use magma::kernel::state::{Editor, MessageKind};

use crate::{AgentLine, ChangeRow, FindRow, MenuItem, TreeItem};

pub fn focused_key(ed: &Editor) -> Option<usize> {
    ed.view_tree
        .focused_window()
        .and_then(|window| ed.view_tree.buffer(window))
}

pub fn focused_file(ed: &Editor) -> (String, bool) {
    let Some(key) = focused_key(ed) else {
        return ("No buffer".into(), false);
    };
    match ed.buffers.get(key) {
        Some(arc) => {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            (buf.name.clone(), buf.modified())
        }
        None => ("No buffer".into(), false),
    }
}

pub fn current_path(ed: &Editor) -> String {
    let Some(key) = focused_key(ed) else { return String::new() };
    let Some(arc) = ed.buffers.get(key) else { return String::new() };
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let path = buf.path.clone().unwrap_or_else(|| buf.name.clone());
    relative_to_project(ed.project_manager.project.root.as_deref(), &path)
}

pub fn breadcrumb_segments(ed: &Editor) -> Vec<String> {
    segments(&current_path(ed))
}

pub fn cursor_position(ed: &Editor) -> String {
    let Some(key) = focused_key(ed) else { return String::new() };
    let offset = ed.view_of_buffer(key).map(|v| v.cursor.offset).unwrap_or(0);
    let Some(arc) = ed.buffers.get(key) else { return String::new() };
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let pos = buf.position_of(offset.min(buf.len()));
    format!("Ln {}, Col {}", pos.line + 1, pos.column + 1)
}

pub fn branch_label(ed: &Editor) -> String {
    ed.vc
        .last_status
        .as_ref()
        .map(|s| s.branch.clone())
        .unwrap_or_default()
}

pub fn status_message(ed: &Editor, fallback: &str, ttl: std::time::Duration) -> (String, bool) {
    if let Some(msg) = &ed.status_message
        && msg.at.elapsed() < ttl
    {
        return (msg.text.clone(), msg.kind == MessageKind::Error);
    }
    (fallback.to_string(), false)
}


pub fn pane_message(ed: &Editor) -> String {
    if focused_key(ed).is_none() {
        "No buffer open. Use File › Open Folder…".to_string()
    } else {
        String::new()
    }
}

pub fn tree_rows(nodes: &[FileNode], expanded: &HashSet<String>) -> Vec<TreeItem> {
    visible_rows(nodes, expanded)
        .into_iter()
        .map(|row| TreeItem {
            name: row.name.into(),
            path: row.path.into(),
            depth: row.depth as i32,
            is_dir: row.is_dir,
            expanded: row.expanded,
        })
        .collect()
}

pub fn menu_title(title: &str) -> &'static str {
    match title {
        "Edit" => "Edit",
        "View" => "View",
        "Navigate" => "Navigate",
        "Run" => "Run",
        _ => "File",
    }
}

pub fn menu_items(title: &'static str) -> Vec<MenuItem> {
    let rows: &[(&str, &str)] = match title {
        "File" => &[
            ("Open Folder…", "open-project-dialog"),
            ("", ""),
            ("Save", "save-buffer"),
            ("Save All", "save-all"),
            ("", ""),
            ("Close Buffer", "close-buffer"),
            ("Quit", "quit"),
        ],
        "Edit" => &[
            ("Undo", "undo"),
            ("Redo", "redo"),
            ("", ""),
            ("Find in Buffers…", "dialog:find"),
        ],
        "View" => &[
            ("Project", "rail:0"),
            ("Commit", "rail:1"),
            ("Debugger", "rail:2"),
            ("", ""),
            ("Toggle Theme", "toggle-theme"),
            ("Settings…", "dialog:settings"),
        ],
        "Navigate" => &[
            ("Next Tab", "buffer-next"),
            ("Previous Tab", "buffer-prev"),
            ("", ""),
            ("Go to File…", "dialog:goto-file"),
            ("Go to Symbol…", "dialog:goto-symbol"),
        ],
        _ => &[
            ("Run", "run-config"),
            ("Debug", "debug-config"),
            ("Stop", "stop-config"),
            ("", ""),
            ("Refresh Version Control", "vc-refresh"),
        ],
    };
    rows.iter()
        .map(|(label, command)| MenuItem {
            label: (*label).into(),
            command: (*command).into(),
            separator: label.is_empty(),
        })
        .collect()
}

// ── Tool windows ──────────────────────────────────────────────────────────────

pub fn terminal_text(ed: &Editor) -> String {
    let manager = &ed.io.terminals;
    let id = manager
        .focused
        .or_else(|| manager.sessions.first().map(|s| s.id));
    let Some(id) = id else { return String::new() };
    let Some(session) = manager.get(id) else { return String::new() };
    session.screen.tail(60).join("\n")
}

pub fn find_rows(ed: &Editor) -> Vec<FindRow> {
    ed.quickfix_list
        .iter()
        .take(200)
        .map(|entry| FindRow {
            title: format!("{}:{}", short_name(&entry.filename), entry.line).into(),
            detail: entry.message.clone().into(),
            path: entry.filename.clone().into(),
            line: entry.line as i32,
        })
        .collect()
}

pub fn output(ed: &Editor) -> (String, String) {
    match &ed.last_task {
        Some((name, output)) => (name.clone(), output.clone()),
        None => (String::new(), String::new()),
    }
}

pub fn agent_lines(ed: &Editor) -> Vec<AgentLine> {
    let Some(session) = ed.agent.sessions.values().max_by_key(|s| s.id) else {
        return Vec::new();
    };
    let mut lines = vec![AgentLine {
        text: format!("You: {}", session.prompt).into(),
        role: "user".into(),
    }];
    for entry in &session.transcript {
        let (text, role) = match entry {
            magma::kernel::agent::TranscriptEntry::Thought(text) => (text.clone(), "agent"),
            magma::kernel::agent::TranscriptEntry::Action { command, description } => {
                (format!("⚒ {command} — {description}"), "action")
            }
            magma::kernel::agent::TranscriptEntry::StepOutput(text) => (text.clone(), "agent"),
        };
        lines.push(AgentLine { text: text.into(), role: role.into() });
    }
    lines.push(AgentLine {
        text: format!("[{}]", session.status.as_str()).into(),
        role: "agent".into(),
    });
    if let Some(result) = &session.result {
        lines.push(AgentLine { text: result.clone().into(), role: "agent".into() });
    }
    if let Some(error) = &session.error {
        lines.push(AgentLine { text: error.clone().into(), role: "error".into() });
    }
    lines
}

pub fn agent_running(ed: &Editor) -> bool {
    ed.agent
        .sessions
        .values()
        .any(|s| s.status == magma::kernel::agent::AgentStatus::Running)
}

pub fn running_session(ed: &Editor) -> Option<u64> {
    ed.agent
        .sessions
        .values()
        .filter(|s| s.status == magma::kernel::agent::AgentStatus::Running)
        .max_by_key(|s| s.id)
        .map(|s| s.id)
}

pub fn changes(ed: &Editor) -> Vec<ChangeRow> {
    let Some(status) = &ed.vc.last_status else { return Vec::new() };
    let mut rows = Vec::new();
    for file in &status.staged {
        rows.push(ChangeRow {
            path: file.path.clone().into(),
            code: file.code.to_string().into(),
            staged: true,
        });
    }
    for file in &status.unstaged {
        rows.push(ChangeRow {
            path: file.path.clone().into(),
            code: file.code.to_string().into(),
            staged: false,
        });
    }
    for path in &status.untracked {
        rows.push(ChangeRow {
            path: path.clone().into(),
            code: "?".into(),
            staged: false,
        });
    }
    rows
}

pub fn debug_state(ed: &Editor) -> (bool, Vec<String>, String) {
    let Some(session) = ed.debug.active_session() else {
        return (false, Vec::new(), String::new());
    };
    let frames = session
        .frames
        .iter()
        .map(|frame| {
            let location = frame
                .source
                .as_deref()
                .map(short_name)
                .unwrap_or_default();
            if location.is_empty() {
                frame.name.clone()
            } else {
                format!("{}  {}:{}", frame.name, location, frame.line)
            }
        })
        .collect();
    (true, frames, session.console.join("\n"))
}

pub fn recent_projects(ed: &Editor) -> Vec<String> {
    ed.project_manager
        .recent_projects
        .iter()
        .take(8)
        .cloned()
        .collect()
}

pub fn status_mode(ed: &Editor) -> String {
    let Some(key) = focused_key(ed) else { return String::new() };
    let Some(arc) = ed.buffers.get(key) else { return String::new() };
    arc.lock()
        .unwrap_or_else(|e| e.into_inner())
        .major_mode
        .name()
        .to_string()
}

fn short_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}
