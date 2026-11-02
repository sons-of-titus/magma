//! The find bar (in-buffer search/replace) and project search.  Buffer
//! search routes through the kernel's `search-buffer`/`replace-*` commands;
//! project search scans the walk's files and publishes hits to
//! `search.results`, the Search panel's own model.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use kernel::task::QuickfixEntry;
use slint::ComponentHandle;

use crate::{AppState, AppWindow, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_bar_changed(move || {
            let mut s = state.borrow_mut();
            if let Some(a) = weak.upgrade() {
                s.find_query = a.get_find_bar_query().to_string();
            }
            let query = s.find_query.clone();
            find_in_buffer(&mut s, &query);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_bar_next(move || {
            let mut s = state.borrow_mut();
            let _ = execute_command(&mut s.editor, "repeat-search", &HashMap::new());
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_bar_prev(move || {
            let mut s = state.borrow_mut();
            let _ = execute_command(&mut s.editor, "reverse-search", &HashMap::new());
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_bar_replace_next(move || {
            let mut s = state.borrow_mut();
            run_replace(&weak, &mut s, false);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_bar_replace_all(move || {
            let mut s = state.borrow_mut();
            run_replace(&weak, &mut s, true);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        app.on_find_bar_toggle_replace(move || {
            let mut s = state.borrow_mut();
            s.find_replace_visible = !s.find_replace_visible;
            s.editor.frame_dirty = true;
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_bar_toggle_regex(move || {
            let mut s = state.borrow_mut();
            s.find_regex = !s.find_regex;
            // Re-run the live query under the new mode; an invalid pattern
            // shows the same find-bar error path as plain typing.
            let query = s.find_query.clone();
            find_in_buffer(&mut s, &query);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_bar_close(move || {
            let mut s = state.borrow_mut();
            s.find_visible = false;
            let _ = execute_command(&mut s.editor, "search-clear", &HashMap::new());
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_search(move |query| {
            let mut s = state.borrow_mut();
            run_project_search(&mut s, &query);
            refresh(&weak, &mut s);
        });
    }
}

fn find_in_buffer(s: &mut AppState, pattern: &str) {
    let mut args = HashMap::new();
    args.insert("pattern".to_string(), ArgValue::String(pattern.to_string()));
    if s.find_regex {
        args.insert("regex".to_string(), ArgValue::Boolean(true));
    }
    match execute_command(&mut s.editor, "search-buffer", &args) {
        Ok(()) => {
            s.find_has_error = false;
            s.find_match_text = match s.editor.search.last_match_count {
                0 => "no matches".into(),
                1 => "1 match".into(),
                n => format!("{n} matches"),
            };
        }
        // An invalid pattern is a find-bar condition, not a status error.
        Err(e) => {
            s.find_has_error = true;
            s.find_match_text = e;
        }
    }
}

fn run_replace(weak: &slint::Weak<AppWindow>, s: &mut AppState, all: bool) {
    if let Some(a) = weak.upgrade() {
        s.find_replacement = a.get_find_bar_replacement().to_string();
    }
    let mut args = HashMap::new();
    args.insert(
        "pattern".to_string(),
        ArgValue::String(s.find_query.clone()),
    );
    args.insert(
        "replacement".to_string(),
        ArgValue::String(s.find_replacement.clone()),
    );
    if s.find_regex {
        args.insert("regex".to_string(), ArgValue::Boolean(true));
    }
    let command = if all { "replace-all" } else { "replace-next" };
    match execute_command(&mut s.editor, command, &args) {
        Ok(()) => s.find_has_error = false,
        Err(e) => {
            s.find_has_error = true;
            s.find_match_text = e;
        }
    }
    let query = s.find_query.clone();
    find_in_buffer(s, &query);
}

/// Project search: scan the walk's files for the query (smart case) and
/// publish the hits to `search.results`, the Search panel's own model.
pub(crate) fn run_project_search(s: &mut AppState, needle: &str) {
    let needle = needle.trim().to_string();
    if needle.is_empty() {
        return;
    }
    let Some(root) = s.editor.project_manager.project.root.clone() else {
        s.message = "Open a project to search it.".into();
        return;
    };
    let files = s.editor.project_manager.project.files.clone();
    let smart_case = !needle.chars().any(char::is_uppercase);
    let needle_lower = needle.to_lowercase();
    let mut entries: Vec<QuickfixEntry> = Vec::new();
    let mut scanned = 0usize;
    'outer: for rel in &files {
        let path = root.join(rel);
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.len() > 1_000_000 {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        scanned += 1;
        for (i, line) in content.lines().enumerate() {
            let hit = if smart_case {
                line.to_lowercase().contains(&needle_lower)
            } else {
                line.contains(&needle)
            };
            if hit {
                let col = line
                    .to_lowercase()
                    .find(&needle_lower)
                    .map(|i| i + 1)
                    .unwrap_or(1);
                entries.push(QuickfixEntry {
                    filename: path.to_string_lossy().into_owned(),
                    line: i + 1,
                    col,
                    message: line.trim().to_string(),
                    severity: kernel::semantic::provider::DiagnosticSeverity::Info,
                });
                if entries.len() >= kernel::task::QUICKFIX_MAX_RESULTS {
                    break 'outer;
                }
            }
        }
    }
    let count = entries.len();
    s.editor.search.set_results(entries);
    s.editor.quickfix_focus = kernel::task::QuickfixList::Search;
    s.message = format!("{count} match(es) for “{needle}” in {scanned} files");
    s.editor.frame_dirty = true;
}

