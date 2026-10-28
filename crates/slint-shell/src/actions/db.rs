//! Database tool-window wiring: sources, connect/refresh, the SQL console,
//! and the schema tree.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use slint::ComponentHandle;

use crate::{AppState, AppWindow, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_toggle(move || {
            let mut s = state.borrow_mut();
            s.db_open = !s.db_open;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_resized(move |delta| {
            let mut s = state.borrow_mut();
            s.db_height = (s.db_height + delta).clamp(160.0, 640.0);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_source_changed(move |name| {
            let mut s = state.borrow_mut();
            if let Some(src) = s.editor.db.sources.iter().find(|x| x.name == name.as_str()) {
                s.editor.db.active = Some(src.id);
                s.db_expanded.clear();
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_connect_toggle(move || {
            let mut s = state.borrow_mut();
            let Some(id) = s.editor.db.active else { return };
            let cmd = if s.editor.db.is_connected(id) {
                "db-disconnect"
            } else {
                "db-connect"
            };
            run(&mut s, cmd, id as i64);
            s.db_expanded.clear();
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_refresh_schema(move || {
            let mut s = state.borrow_mut();
            let Some(id) = s.editor.db.active else { return };
            run(&mut s, "db-refresh", id as i64);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_form_opened(move || {
            let mut s = state.borrow_mut();
            s.db_form_open = true;
            s.db_form_note.clear();
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_form_cancelled(move || {
            let mut s = state.borrow_mut();
            s.db_form_open = false;
            s.db_form_note.clear();
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_source_created(move |name, path| {
            let mut s = state.borrow_mut();
            if name.trim().is_empty() || path.trim().is_empty() {
                s.db_form_note = "a name and a file path are both required".into();
                refresh(&weak, &mut s);
                return;
            }
            let mut args = HashMap::new();
            args.insert("name".to_string(), ArgValue::String(name.to_string()));
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, "db-source-add", &args) {
                Ok(()) => {
                    s.db_form_open = false;
                    s.db_form_note.clear();
                    // Connect immediately: the usual flow is add → explore.
                    if let Some(id) = s.editor.db.active {
                        run(&mut s, "db-connect", id as i64);
                    }
                }
                Err(e) => s.db_form_note = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_test_connection(move |_name, path| {
            let mut s = state.borrow_mut();
            // A SQLite file-open probe is quick; fine on the UI thread.
            let probe = db::DataSource {
                id: 0,
                name: "probe".into(),
                kind: db::DriverKind::Sqlite,
                path: path.to_string(),
            };
            s.db_form_note = match db::test_connection(&probe) {
                Ok(()) => "connection works".into(),
                Err(e) => e,
            };
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_toggle_table(move |idx| {
            let mut s = state.borrow_mut();
            let idx = idx.max(0) as usize;
            if !s.db_expanded.remove(&idx) {
                s.db_expanded.insert(idx);
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_table_opened(move |idx| {
            let mut s = state.borrow_mut();
            let Some(tables) = s
                .editor
                .db
                .active
                .and_then(|id| s.editor.db.tables.get(&id))
            else {
                return;
            };
            let Some(table) = tables.get(idx.max(0) as usize) else {
                return;
            };
            let name = table.name.clone();
            let mut args = HashMap::new();
            args.insert("table".to_string(), ArgValue::String(name));
            match execute_command(&mut s.editor, "db-table-sql", &args) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_run_query(move || {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, "db-query-buffer", &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_open_console(move || {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, "db-console", &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_cancel_query(move || {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, "db-cancel", &HashMap::new()) {
                Ok(()) => {}
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
}

fn run(s: &mut AppState, command: &str, source: i64) {
    let mut args = HashMap::new();
    args.insert("source".to_string(), ArgValue::Integer(source));
    match execute_command(&mut s.editor, command, &args) {
        Ok(()) => s.message.clear(),
        Err(e) => s.message = e,
    }
}
