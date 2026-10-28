//! Editor state → Slint models.
//!
//! Pure projections: no widget handles, no mutation.  Keeping them here makes
//! the shell's `refresh` a straight assignment pass.

mod agent;
mod debug_view;
mod settings;

pub use agent::{
    active_session_id, agent_approval_summary, agent_entries, agent_files, agent_phase,
    agent_phase_detail, agent_running, agent_session_chips, agent_turns_left,
};
pub use debug_view::snapshot as debug_snapshot;
pub use settings::settings_rows;

use std::collections::HashSet;

use kernel::project::tree::{FileNode, relative_to_project, segments, visible_rows};
use kernel::state::{Editor, MessageKind};

use crate::{FindRow, MenuItem, ProblemRow, TreeItem};

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
    let Some(key) = focused_key(ed) else {
        return String::new();
    };
    let Some(arc) = ed.buffers.get(key) else {
        return String::new();
    };
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let path = buf.path.clone().unwrap_or_else(|| buf.name.clone());
    relative_to_project(ed.project_manager.project.root.as_deref(), &path)
}

pub fn breadcrumb_segments(ed: &Editor) -> Vec<String> {
    segments(&current_path(ed))
}

pub fn cursor_position(ed: &Editor) -> String {
    let Some(key) = focused_key(ed) else {
        return String::new();
    };
    let offset = ed.view_of_buffer(key).map(|v| v.cursor.offset).unwrap_or(0);
    let Some(arc) = ed.buffers.get(key) else {
        return String::new();
    };
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

pub fn tree_rows(ed: &Editor, nodes: &[FileNode], expanded: &HashSet<String>) -> Vec<TreeItem> {
    // Collect modified files and files with errors.
    let mut modified_files: HashSet<String> = HashSet::new();
    let mut error_files: HashSet<String> = HashSet::new();
    for (key, arc) in &ed.buffers {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        if buf.modified() {
            let rel = relative_to_project(
                ed.project_manager.project.root.as_deref(),
                &buf.path.clone().unwrap_or_else(|| buf.name.clone()),
            );
            if !rel.is_empty() {
                modified_files.insert(rel);
            }
        }
        let has_err = buf
            .get_diagnostics()
            .iter()
            .any(|d| d.severity == kernel::text_engine::diagnostic::DiagnosticSeverity::Error);
        if has_err {
            let rel = relative_to_project(
                ed.project_manager.project.root.as_deref(),
                &buf.path.clone().unwrap_or_else(|| buf.name.clone()),
            );
            if !rel.is_empty() {
                error_files.insert(rel);
            }
        }
        let _ = key;
    }
    visible_rows(nodes, expanded)
        .into_iter()
        .map(|row| {
            let (icon, color, icon_path, icon_image) =
                icon_for(ed, &row.name, row.is_dir, row.expanded);
            TreeItem {
                name: row.name.into(),
                path: row.path.clone().into(),
                depth: row.depth as i32,
                is_dir: row.is_dir,
                ignored: row.ignored,
                modified: modified_files.contains(&row.path),
                has_error: error_files.contains(&row.path),
                expanded: row.expanded,
                icon: icon.into(),
                icon_color: color,
                icon_path: icon_path.into(),
                icon_image,
            }
        })
        .collect()
}

fn icon_for(
    ed: &Editor,
    name: &str,
    is_dir: bool,
    expanded: bool,
) -> (String, slint::Color, String, slint::Image) {
    let (glyph, color, path) = if is_dir {
        match ed.icons.folder_icon(expanded) {
            Some(kernel::icons::IconKind::Glyph { glyph, color }) => (
                *glyph,
                color.map(|(r, g, b)| slint::Color::from_rgb_u8(r, g, b)),
                None,
            ),
            Some(kernel::icons::IconKind::Svg { path }) => ('\0', None, Some(path.clone())),
            _ => ('\u{F024B}', None, None),
        }
    } else {
        match ed.icons.lookup(name) {
            Some(kernel::icons::IconKind::Glyph { glyph, color }) => (
                *glyph,
                color.map(|(r, g, b)| slint::Color::from_rgb_u8(r, g, b)),
                None,
            ),
            Some(kernel::icons::IconKind::Svg { path }) => ('\0', None, Some(path.clone())),
            _ => ('\u{EA7B}', None, None),
        }
    };
    let color = color.unwrap_or(if is_dir {
        slint::Color::from_rgb_u8(180, 190, 254)
    } else {
        slint::Color::from_rgb_u8(166, 173, 200)
    });
    let icon_path = path
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let icon_image = if !icon_path.is_empty() {
        slint::Image::load_from_path(std::path::Path::new(&icon_path)).unwrap_or_default()
    } else {
        slint::Image::default()
    };
    let glyph_str = if icon_path.is_empty() {
        glyph.to_string()
    } else {
        String::new()
    };
    (glyph_str, color, icon_path, icon_image)
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
    let Some(session) = manager.get(id) else {
        return String::new();
    };
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

pub fn problem_rows(ed: &Editor) -> Vec<ProblemRow> {
    ed.quickfix_list
        .iter()
        .map(|entry| ProblemRow {
            severity: entry.severity.as_str().into(),
            message: entry.message.trim_end().to_string().into(),
            location: format!("{}:{}:{}", entry.filename, entry.line, entry.col).into(),
            path: entry.filename.clone().into(),
            line: entry.line as i32,
        })
        .collect()
}

pub fn running_session(ed: &Editor) -> Option<u64> {
    ed.agent
        .sessions
        .values()
        .filter(|s| s.status == kernel::agent::AgentStatus::Running)
        .max_by_key(|s| s.id)
        .map(|s| s.id)
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
    let Some(key) = focused_key(ed) else {
        return String::new();
    };
    let Some(arc) = ed.buffers.get(key) else {
        return String::new();
    };
    arc.lock()
        .unwrap_or_else(|e| e.into_inner())
        .major_mode
        .name()
        .to_string()
}

fn short_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}
