//! Dock wiring: bottom tool windows (terminal, find, output, git history),
//! the left sidebar panels, and version control actions.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use slint::ComponentHandle;

use crate::{AppState, AppWindow, GitHistoryRow, projection, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_toggle_bottom(move |index| {
            let mut s = state.borrow_mut();
            if s.bottom == index {
                s.bottom = -1;
            } else {
                s.bottom = index;
                if index == 0 && s.editor.io.terminals.sessions.is_empty() {
                    let _ = execute_command(&mut s.editor, "terminal-start", &HashMap::new());
                }
                if index == 3 {
                    refresh_git_history(&mut s);
                }
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_close_bottom(move || {
            let mut s = state.borrow_mut();
            s.bottom = -1;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_focus(move |i| {
            let mut s = state.borrow_mut();
            if let Some(sess) = s.editor.io.terminals.sessions.get(i as usize) {
                s.editor.io.terminals.focused = Some(sess.id);
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_close(move |i| {
            let mut s = state.borrow_mut();
            let id = s.editor.io.terminals.sessions.get(i as usize).map(|t| t.id);
            if let Some(id) = id {
                let mut args = HashMap::new();
                args.insert("terminal".to_string(), ArgValue::Integer(id as i64));
                match execute_command(&mut s.editor, "terminal-close", &args) {
                    Ok(()) => s.message.clear(),
                    Err(e) => s.message = e,
                }
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_scroll(move |d| {
            let mut s = state.borrow_mut();
            let delta = (d / 16.0) as i32;
            if delta > 0 {
                s.terminal_scroll = s.terminal_scroll.saturating_add(delta as usize);
            } else {
                s.terminal_scroll = s.terminal_scroll.saturating_sub((-delta) as usize);
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_new(move || {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, "terminal-start", &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            s.bottom = 0;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_open(move |path, line| {
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, "open-file", &args) {
                Ok(()) => {
                    let mut args = HashMap::new();
                    args.insert("line".to_string(), ArgValue::Integer(line as i64));
                    if let Err(e) = execute_command(&mut s.editor, "goto-line", &args) {
                        s.message = e;
                    }
                }
                Err(e) => s.message = e,
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_cmd(move |cmd| {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, &cmd, &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_change_clicked(move |path| {
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, "vc-diff-file", &args) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_stage_clicked(move |path, staged| {
            let mut s = state.borrow_mut();
            let command = if staged {
                "vc-unstage-file"
            } else {
                "vc-stage-file"
            };
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, command, &args) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_commit(move || {
            let message = weak
                .upgrade()
                .map(|a| a.get_commit_message().to_string())
                .unwrap_or_default();
            if message.trim().is_empty() {
                return;
            }
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("message".to_string(), ArgValue::String(message));
            match execute_command(&mut s.editor, "vc-commit-message", &args) {
                Ok(()) => {
                    s.message.clear();
                    if let Some(app) = weak.upgrade() {
                        app.set_commit_message("".into());
                    }
                }
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_sidebar_refresh(move || {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, "index-project", &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_sidebar_collapse(move || {
            let mut s = state.borrow_mut();
            s.expanded.clear();
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_open_recent(move |path| {
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, "open-project", &args) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            s.tree_epoch = u64::MAX;
            refresh(&weak, &mut s);
        });
    }
}

/// Load `git log` for the focused project (or the focused file's directory).
fn refresh_git_history(s: &mut AppState) {
    let dir = s
        .editor
        .project_manager
        .project
        .root
        .clone()
        .or_else(|| {
            let key = projection::focused_key(&s.editor);
            key.and_then(|k| {
                s.editor
                    .buffers
                    .get(k)
                    .and_then(|a| a.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
            })
            .as_deref()
            .and_then(|p| std::path::Path::new(p).parent().map(|p| p.to_path_buf()))
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let Some((_, root)) = s.editor.vc.detect_for(&dir) else {
        s.git_history.clear();
        s.git_history.push(GitHistoryRow {
            hash: "norepo".into(),
            author: dir.display().to_string().into(),
            date: "".into(),
            message: "no repository detected".into(),
        });
        s.message = format!("no repo in {}", dir.display());
        return;
    };
    match kernel::vc::run_cmd(
        &root,
        "git",
        &["log", "-30", "--pretty=format:%h|%an|%ad|%s"],
    ) {
        Ok(out) => {
            s.git_history = out
                .lines()
                .filter_map(|line| {
                    let mut parts = line.splitn(4, '|');
                    let hash = parts.next()?.to_string();
                    let author = parts.next().unwrap_or("").to_string();
                    let date = parts.next().unwrap_or("").to_string();
                    let message = parts.next().unwrap_or("").to_string();
                    Some(GitHistoryRow {
                        hash: hash.into(),
                        author: author.into(),
                        date: date.into(),
                        message: message.into(),
                    })
                })
                .collect();
            if s.git_history.is_empty() {
                s.git_history.push(GitHistoryRow {
                    hash: "empty".into(),
                    author: root.display().to_string().into(),
                    date: "".into(),
                    message: out.into(),
                });
            }
        }
        Err(e) => {
            s.message = format!("git log failed: {}", e);
            s.git_history.clear();
            s.git_history.push(GitHistoryRow {
                hash: "err".into(),
                author: "".into(),
                date: "".into(),
                message: e.into(),
            });
        }
    }
}
