//! Navigation wiring: the picker's UI callbacks and the status-bar
//! shortcuts.  The surfaces themselves live in `picker`, the find bar in
//! `find_bar`, context menus in `context_menu`.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

use super::context_menu::copy_absolute;
use super::picker;
use crate::{AppState, AppWindow, projection, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    picker_callbacks(app, state.clone());
    status_callbacks(app, state);
}

fn picker_callbacks(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_picker_query_changed(move || {
            let mut s = state.borrow_mut();
            picker::query_changed(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_picker_confirm(move || {
            let mut s = state.borrow_mut();
            picker::confirm(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_picker_cancel(move || {
            let mut s = state.borrow_mut();
            picker::cancel(&mut s);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_picker_move(move |delta| {
            let mut s = state.borrow_mut();
            picker::move_selection(&weak, &mut s, delta);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_picker_row_clicked(move |row| {
            let mut s = state.borrow_mut();
            picker::row_clicked(&weak, &mut s, row);
        });
    }
}

fn status_callbacks(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_copy_file_path(move || {
            let mut s = state.borrow_mut();
            let path = projection::current_path(&s.editor);
            if path.is_empty() {
                s.message = "No file open.".into();
            } else {
                copy_absolute(&mut s, &path);
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_breadcrumb_clicked(move |index| {
            let mut s = state.borrow_mut();
            let segments = projection::breadcrumb_segments(&s.editor);
            let index = index.max(0) as usize;
            if index < segments.len() {
                // Reveal the clicked directory: expand it and every ancestor
                // the tree knows about, and put the tree in front.
                let mut prefix = String::new();
                for seg in segments.iter().take(index + 1) {
                    if !prefix.is_empty() {
                        prefix.push('/');
                    }
                    prefix.push_str(seg);
                    if s.dirs.contains(&prefix) {
                        s.expanded.insert(prefix.clone());
                    }
                }
                s.rail = 0;
                s.sidebar_open = true;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_send(move |text| {
            let mut s = state.borrow_mut();
            let id = s
                .editor
                .io
                .terminals
                .focused
                .or_else(|| s.editor.io.terminals.sessions.first().map(|t| t.id));
            if let Some(id) = id {
                match kernel::terminal::send_input(&s.editor, id, &format!("{text}\r")) {
                    Ok(()) => s.message.clear(),
                    Err(e) => s.message = e,
                }
                s.editor.frame_dirty = true;
            } else {
                s.message = "No terminal session — start one first.".into();
            }
            refresh(&weak, &mut s);
        });
    }
}
