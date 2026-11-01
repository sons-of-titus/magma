//! Picker entry sources: what each surface lists and how a query ranks
//! against it.  Pure collection + scoring; the state machine is in `mod.rs`.

use kernel::render::tabs;

use super::{PickerItem, PickerKind, PAYLOAD_SEP};
use crate::AppState;

/// Case-insensitive match with a substring bonus over subsequence matches.
fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let n: Vec<char> = needle.to_lowercase().chars().collect();
    let h: Vec<char> = haystack.to_lowercase().chars().collect();
    if let Some(pos) = h.windows(n.len()).position(|w| w == n.as_slice()) {
        // Substring: earlier is better.
        return Some(10_000 - pos as i32);
    }
    // Subsequence: total spread of the matched letters, tighter is better.
    let mut score = 0;
    let mut h_idx = 0;
    for c in &n {
        let mut found = None;
        while h_idx < h.len() {
            if h[h_idx] == *c {
                found = Some(h_idx);
                h_idx += 1;
                break;
            }
            h_idx += 1;
        }
        score += found? as i32;
    }
    Some(1_000 - score)
}

fn title_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Collect the entries for `kind` from live editor state.
pub(super) fn collect_items(s: &mut AppState, kind: &PickerKind) -> Vec<PickerItem> {
    let ed = &mut s.editor;
    match kind {
        PickerKind::None => Vec::new(),
        PickerKind::Files => ed
            .project_manager
            .project
            .files
            .iter()
            .map(|rel| PickerItem {
                title: title_of(rel).to_string(),
                detail: String::new(),
                meta: rel.to_string(),
                payload: rel.to_string(),
            })
            .collect(),
        PickerKind::Symbols => {
            // The index ranks by name; an empty query shows the hint instead
            // of an arbitrary slice of every symbol.
            let query = s.picker_query.trim().to_string();
            if query.is_empty() {
                return Vec::new();
            }
            let root = ed.project_manager.project.root.clone();
            let matches = ed.index.symbols_matching(&query, 200);
            matches
                .into_iter()
                .map(|(path, sym)| {
                    let rel = kernel::project::tree::relative_to_project(root.as_deref(), &path);
                    PickerItem {
                        title: sym.name.clone(),
                        detail: sym.kind.clone(),
                        meta: format!("{}:{}", rel, sym.line + 1),
                        payload: format!("{path}{}{}", PAYLOAD_SEP, sym.line + 1),
                    }
                })
                .collect()
        }
        PickerKind::Buffers => ed
            .buffers
            .iter()
            .map(|(key, arc)| {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let name = buf.name.clone();
                let modified = buf.modified();
                PickerItem {
                    title: tabs::display_label(&name),
                    detail: name.clone(),
                    meta: if modified { "modified".into() } else { String::new() },
                    payload: key.to_string(),
                }
            })
            .collect(),
        PickerKind::Commands => {
            let query = s.picker_query.clone();
            let mut items: Vec<PickerItem> = ed
                .commands
                .list_detailed()
                .into_iter()
                .map(|(name, description, needs_args)| PickerItem {
                    title: name.clone(),
                    detail: description,
                    meta: if needs_args { "needs args".into() } else { String::new() },
                    payload: name,
                })
                .collect();
            // A typed line that is not a bare command name keeps the old
            // command-line escape hatch — as an explicit, labelled row.
            let trimmed = query.trim().to_string();
            if !trimmed.is_empty()
                && !ed.commands.exists(&trimmed)
            {
                items.insert(
                    0,
                    PickerItem {
                        title: format!("Run “{trimmed}”"),
                        detail: "Execute as a command line".into(),
                        meta: String::new(),
                        payload: trimmed,
                    },
                );
            }
            items
        }
        PickerKind::Branches => ed
            .vc
            .branches
            .iter()
            .map(|b| PickerItem {
                title: b.clone(),
                detail: String::new(),
                meta: String::new(),
                payload: b.clone(),
            })
            .collect(),
        PickerKind::GotoLine | PickerKind::NewFile(_) | PickerKind::NewFolder(_)
        | PickerKind::Rename(_) => Vec::new(),
    }
}

pub(super) fn header(kind: &PickerKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        PickerKind::None => ("", "", ""),
        PickerKind::Files => ("Go to File", "Type a file name", ""),
        PickerKind::Symbols => ("Go to Symbol", "Type a symbol name", ""),
        PickerKind::Buffers => ("Switch Buffer", "Type a buffer name", ""),
        PickerKind::Commands => ("Command Palette", "Type a command — Enter runs it", ""),
        PickerKind::Branches => ("Switch Branch", "Type a branch name", ""),
        PickerKind::GotoLine => ("Go to Line", "Line number", "Enter jumps"),
        PickerKind::NewFile(_) => ("New File", "Name of the file to create", "Enter creates"),
        PickerKind::NewFolder(_) => ("New Folder", "Name of the folder to create", "Enter creates"),
        PickerKind::Rename(_) => ("Rename", "New name", "Enter renames"),
    }
}

/// Rank the items against the query: returns item indices in row order.
pub(super) fn ranked(s: &AppState, query: &str, prompt: bool) -> Vec<usize> {
    if prompt {
        return Vec::new();
    }
    let mut scored: Vec<(i32, usize)> = s
        .picker_items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            fuzzy_score(query, &item.title)
                .or_else(|| fuzzy_score(query, &item.detail))
                .map(|score| (score + if i == 0 { -1 } else { 0 }, i))
        })
        .collect();
    if query.is_empty() {
        scored = s
            .picker_items
            .iter()
            .enumerate()
            .map(|(i, _)| (0, i))
            .collect();
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.truncate(200);
    scored.into_iter().map(|(_, i)| i).collect()
}

