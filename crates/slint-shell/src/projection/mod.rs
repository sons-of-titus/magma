//! Editor state → Slint models.
//!
//! Pure projections: no widget handles, no mutation.  Keeping them here makes
//! the shell's `refresh` a straight assignment pass.

mod agent;
mod agent_panel;
mod db;
mod debug_view;
mod menus;
mod settings;

pub use agent::{
    active_session_id, agent_approval_summary, agent_entries, agent_files, agent_phase,
    agent_phase_detail, agent_running, agent_session_chips, agent_turns_left,
};
pub use agent_panel::{
    active_model_choice, active_provider_choice, attachment_labels, auth_panel, model_choices,
    provider_cards, provider_choices, reasoning_choices, reasoning_label, reasoning_supported,
    send_blocked_reason,
};
pub use db::{
    db_active_source_name, db_busy, db_can_edit, db_columns, db_connected_label, db_elapsed,
    db_grid_title, db_has_tab, db_page_info, db_pending_count, db_querying, db_row_count, db_rows,
    db_sources, db_status, db_tabs, db_tree, db_truncated,
};
pub use debug_view::snapshot as debug_snapshot;
pub use menus::{menu_items, menu_title};
pub use settings::{disabled_plugins, plugin_rows, settings_rows};

use std::collections::HashSet;

use kernel::project::tree::{FileNode, relative_to_project, segments, visible_rows};
use kernel::state::{Editor, MessageKind};

use kernel::render::tabs;

use crate::{FindRow, OutputLine, ProblemRow, ProgressTask, TreeItem};

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
            (tabs::display_label(&buf.name), buf.modified())
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
    if focused_key(ed).is_some() {
        return String::new();
    }
    // While the async open is still delivering, the empty pane is a state in
    // progress, not a dead end — say so instead of suggesting the dialog.
    if ed.project_manager.project.indexing || ed.session_restore_pending > 0 {
        "Opening project…".to_string()
    } else {
        "No buffer open. Use File › Open Folder…".to_string()
    }
}

/// The async-open pipeline as user-visible chips, in the order the stages
/// run.  Only stages with live kernel state appear — no synthetic progress.
pub fn progress_tasks(ed: &Editor) -> Vec<ProgressTask> {
    let proj = &ed.project_manager.project;
    let mut tasks = Vec::new();
    if proj.indexing {
        tasks.push(ProgressTask {
            label: "Scanning project files".into(),
            detail: format!("{} found", proj.files.len()).into(),
        });
    }
    let pending = ed.index.stats().pending;
    if pending > 0 {
        tasks.push(ProgressTask {
            label: "Indexing symbols".into(),
            detail: format!("{pending} left").into(),
        });
    }
    if ed.session_restore_pending > 0 {
        tasks.push(ProgressTask {
            label: "Restoring session".into(),
            detail: format!("{} buffers", ed.session_restore_pending).into(),
        });
    }
    if proj.detecting_tasks {
        tasks.push(ProgressTask {
            label: "Detecting build tasks".into(),
            detail: "".into(),
        });
    }
    if let Some(job) = &ed.template_jobs.generating {
        tasks.push(ProgressTask {
            label: "Creating project".into(),
            detail: job.name.as_str().into(),
        });
    }
    tasks
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

/// The Search panel's rows: hits of the last project search / keyword
/// context — its own dataset, never the build problems list.
pub fn find_rows(ed: &Editor) -> Vec<FindRow> {
    ed.search
        .results
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

/// Task output as per-line rows; lines carrying a `file:line:col` location
/// become clickable jumps into the code.
pub fn output_lines(ed: &Editor) -> Vec<OutputLine> {
    let Some((_, text)) = &ed.last_task else {
        return Vec::new();
    };
    text.lines()
        .map(|line| match kernel::task::diag_parse::find_location_prefix(line) {
            Some((path, line_no)) => OutputLine {
                text: line.into(),
                path: path.into(),
                line: line_no,
            },
            None => OutputLine {
                text: line.into(),
                path: "".into(),
                line: 0,
            },
        })
        .collect()
}

/// The Output pane's title: the finished task's name.  The text itself is
/// projected only into `output_lines` (cached by generation) — cloning the
/// whole log per refresh is what this exists to avoid.
pub fn output_title(ed: &Editor) -> String {
    match &ed.last_task {
        Some((name, _)) => name.clone(),
        None => String::new(),
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

