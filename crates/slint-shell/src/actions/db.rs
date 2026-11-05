//! Database tool-window wiring: toolbar, explorer tree, data tabs, the
//! grid, and the connection dialog.  All state changes run through kernel
//! commands; this layer only translates UI events into them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use slint::ComponentHandle;

use crate::app_state::DbDialog;
use crate::actions::context_menu;
use crate::{AppState, AppWindow, context, sync::refresh};

const DB_KINDS: [db::DriverKind; 5] = [
    db::DriverKind::Sqlite,
    db::DriverKind::Postgres,
    db::DriverKind::MySql,
    db::DriverKind::MariaDb,
    db::DriverKind::SqlServer,
];

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_toggle(move || {
            let mut s = state.borrow_mut();
            s.db_open = !s.db_open;
            if s.db_open {
                s.agent_open = false;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_resized(move |delta| {
            let mut s = state.borrow_mut();
            s.db_height = (s.db_height + delta).clamp(160.0, 900.0);
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
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_toolbar(move |action| {
            let mut s = state.borrow_mut();
            toolbar(&weak, &mut s, &action);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_tree_clicked(move |key| {
            let mut s = state.borrow_mut();
            tree_clicked(&weak, &mut s, &key);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_tree_opened(move |key| {
            let mut s = state.borrow_mut();
            tree_opened(&weak, &mut s, &key);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_tree_context(move |key, x, y| {
            let mut s = state.borrow_mut();
            s.context_menu = Some(context::ContextTarget::DbNode(key.to_string()));
            s.context_path = key.to_string();
            context_menu::raise(&weak, &mut s, x, y);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_tree_filter_edited(move || {
            let mut s = state.borrow_mut();
            s.db_tree_filter = query_of(&weak, "db-tree-filter");
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_tab_selected(move |id| {
            let mut s = state.borrow_mut();
            run_s(&mut s, "db-select-tab", &[("tab", ArgValue::Integer(id as i64))]);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_tab_closed(move |id| {
            let mut s = state.borrow_mut();
            run_s(&mut s, "db-close-tab", &[("tab", ArgValue::Integer(id as i64))]);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_sort_clicked(move |col| {
            let mut s = state.borrow_mut();
            run_s(&mut s,
                "db-grid-sort",
                &[("column", ArgValue::Integer(col as i64))],
            );
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_filter_accepted(move || {
            let mut s = state.borrow_mut();
            let filter = query_of(&weak, "db-filter-text");
            run_s(&mut s,
                "db-grid-filter",
                &[("filter", ArgValue::String(filter))],
            );
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_grid_select(move |row, col| {
            let mut s = state.borrow_mut();
            run_s(&mut s,
                "db-grid-select",
                &[
                    ("row", ArgValue::Integer(row as i64)),
                    ("column", ArgValue::Integer(col as i64)),
                ],
            );
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_grid_edit_committed(move |row, col, text| {
            let mut s = state.borrow_mut();
            // An empty box commits NULL; the new-row path stages cells.
            let editing_insert = s
                .editor
                .db
                .active_tab()
                .map(|t| {
                    t.pending
                        .ops
                        .iter()
                        .any(|o| matches!(o, db::EditOp::InsertRow { .. }))
                })
                .unwrap_or(false);
            let row_is_insert = {
                let rows = s
                    .editor
                    .db
                    .active_tab()
                    .and_then(|t| t.result())
                    .map(|r| r.rows.len())
                    .unwrap_or(0);
                (row as usize) == rows
            };
            let cmd = if editing_insert && row_is_insert {
                "db-grid-new-cell"
            } else {
                "db-grid-edit-cell"
            };
            run_s(&mut s,
                cmd,
                &[
                    ("row", ArgValue::Integer(row as i64)),
                    ("column", ArgValue::Integer(col as i64)),
                    ("value", ArgValue::String(text.to_string())),
                ],
            );
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_grid_action(move |action| {
            let mut s = state.borrow_mut();
            grid_action(&weak, &mut s, &action);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_col_resized(move |col, delta| {
            let mut s = state.borrow_mut();
            let Some(tab) = s.editor.db.active_tab else {
                return;
            };
            let count = s
                .editor
                .db
                .active_tab()
                .and_then(|t| t.result())
                .map(|r| r.columns.len())
                .unwrap_or(0);
            let widths = s.db_col_widths.entry(tab).or_default();
            if widths.is_empty() {
                widths.resize(count, 120.0);
            }
            if let Some(w) = widths.get_mut(col.max(0) as usize) {
                *w = (*w + delta).max(40.0);
            }
            refresh(&weak, &mut s);
        });
    }

    // ── Connection dialog ────────────────────────────────────────────────
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_dialog_opened(move |editing| {
            let mut s = state.borrow_mut();
            let editing = (editing > 0).then_some(editing as u64);
            s.db_dialog = Some(open_dialog(&mut s.editor, editing));
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_dialog_cancelled(move || {
            let mut s = state.borrow_mut();
            s.db_dialog = None;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_dialog_kind_changed(move |label| {
            let mut s = state.borrow_mut();
            if let Some(d) = &mut s.db_dialog
                && let Some(kind) = DB_KINDS.iter().position(|k| k.label() == label.as_str())
            {
                d.kind_index = kind;
                d.port = kind_default_port(kind).to_string();
                d.port_push = true;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_dialog_ssl_changed(move |label| {
            let mut s = state.borrow_mut();
            if let Some(d) = &mut s.db_dialog {
                d.ssl_index = match label.as_str() {
                    "disabled" => 0,
                    "require" => 2,
                    _ => 1,
                };
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_dialog_url_toggled(move || {
            let mut s = state.borrow_mut();
            if let Some(d) = &mut s.db_dialog {
                d.url_mode = !d.url_mode;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_dialog_tested(move || {
            let mut s = state.borrow_mut();
            test_dialog_connection(&weak, &mut s);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_db_dialog_saved(move || {
            let mut s = state.borrow_mut();
            save_dialog(&weak, &mut s);
            refresh(&weak, &mut s);
        });
    }
}

// ── Toolbar ──────────────────────────────────────────────────────────────────

fn toolbar(weak: &slint::Weak<AppWindow>, s: &mut AppState, action: &str) {
    match action {
        "add" => {
            s.db_dialog = Some(open_dialog(&mut s.editor, None));
        }
        "properties" => {
            let id = s.editor.db.active;
            s.db_dialog = Some(open_dialog(&mut s.editor, id));
        }
        "connect" => {
            let id = s.editor.db.active;
            let connected = id.map(|i| s.editor.db.is_connected(i)).unwrap_or(false);
            let cmd = if connected { "db-disconnect" } else { "db-connect" };
            run_s(s, cmd, &[]);
        }
        "refresh" => {
            run_s(s, "db-refresh-source", &[]);
        }
        "console" => {
            run_s(s, "db-console", &[]);
        }
        "run" => {
            // The stop button doubles as run/cancel: a running query on
            // the active source cancels, otherwise it runs the console.
            let running = s.editor.db.active
                .map(|id| s.editor.db.tabs.iter().any(|t| t.source_id == id && t.busy))
                .unwrap_or(false);
            run_s(s, if running { "db-cancel" } else { "db-execute-current" }, &[]);
        }
        "collapse" => {
            let id = s.editor.db.active;
            if let Some(id) = id {
                s.editor.db.tree_mut(id).collapse_all();
            }
        }
        "filter" => {
            s.db_tree_filter_active = !s.db_tree_filter_active;
            if !s.db_tree_filter_active {
                s.db_tree_filter.clear();
            }
        }
        _ => {}
    }
    let _ = weak;
}

// ── Explorer tree ────────────────────────────────────────────────────────────

fn tree_clicked(weak: &slint::Weak<AppWindow>, s: &mut AppState, key: &str) {
    let _ = weak;
    // Only addressable nodes expand; UI-only leaves (columns, indexes)
    // ignore clicks.
    if db::NodeRef::parse(key).is_some() {
        run_s(s, "db-expand", &[("path", ArgValue::String(key.to_string()))]);
    }
}

fn tree_opened(weak: &slint::Weak<AppWindow>, s: &mut AppState, key: &str) {
    // Tables and views open their data editor; other nodes just expand.
    let is_object = {
        let parts: Vec<&str> = key.split('/').filter(|p| !p.is_empty()).collect();
        matches!(parts.get(2), Some(&"tables") | Some(&"views")) && parts.len() == 4
    };
    if is_object {
        run_s(s, "db-open-table", &[("path", ArgValue::String(key.to_string()))]);
    } else {
        tree_clicked(weak, s, key);
    }
}

// ── Grid actions ─────────────────────────────────────────────────────────────

fn grid_action(weak: &slint::Weak<AppWindow>, s: &mut AppState, action: &str) {
    match action {
        "insert" => run_s(s, "db-grid-new-row", &[]),
        "delete" => {
            let row = s
                .editor
                .db
                .active_tab()
                .and_then(|t| t.selected)
                .map(|(r, _)| r as i64);
            match row {
                Some(row) => run_s(s, "db-grid-delete-row", &[("row", ArgValue::Integer(row))]),
                None => s.message = "select a row to delete first".into(),
            }
        }
        "apply" => run_s(s, "db-grid-apply", &[]),
        "revert" => run_s(s, "db-grid-revert", &[]),
        "copy-cell" => run_s(s, "db-grid-copy", &[("scope", ArgValue::String("cell".into()))]),
        "copy-row" => run_s(s, "db-grid-copy", &[("scope", ArgValue::String("row".into()))]),
        "refresh" => run_s(s, "db-grid-refresh", &[]),
        "prev" => run_s(s, "db-grid-page", &[("delta", ArgValue::Integer(-1))]),
        "next" => run_s(s, "db-grid-page", &[("delta", ArgValue::Integer(1))]),
        _ => {}
    }
    let _ = weak;
}

// ── Connection dialog ────────────────────────────────────────────────────────

fn open_dialog(ed: &mut kernel::state::Editor, editing: Option<u64>) -> DbDialog {
    let kind_index = editing
        .and_then(|id| ed.db.source(id))
        .map(|s| {
            DB_KINDS
                .iter()
                .position(|k| *k == s.kind)
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let source = editing.and_then(|id| ed.db.source(id));
    DbDialog {
        editing,
        kind_index,
        name: source.map(|s| s.name.clone()).unwrap_or_default(),
        host: source.map(|s| s.host.clone()).unwrap_or_default(),
        port: source
            .map(|s| s.port.to_string())
            .unwrap_or_else(|| kind_default_port(kind_index).to_string()),
        database: source.map(|s| s.database.clone()).unwrap_or_default(),
        user: source.map(|s| s.user.clone()).unwrap_or_default(),
        password: String::new(),
        save_password: source.map(|s| s.save_password).unwrap_or(false),
        ssl_index: source
            .map(|s| match s.ssl_mode {
                db::SslMode::Disabled => 0usize,
                db::SslMode::Prefer => 1,
                db::SslMode::Require => 2,
            })
            .unwrap_or(1),
        trust_cert: source.map(|s| s.trust_cert).unwrap_or(false),
        url_mode: source.map(|s| s.url.is_some()).unwrap_or(false),
        url: source.and_then(|s| s.url.clone()).unwrap_or_default(),
        testing: false,
        needs_hydration: true,
        port_push: false,
        test_token: ed.db.test_token,
    }
}

fn kind_default_port(index: usize) -> u16 {
    DB_KINDS
        .get(index)
        .map(|k| k.default_port())
        .unwrap_or(0)
}

fn dialog_fields(app: &AppWindow, s: &mut AppState) {
    let Some(d) = &mut s.db_dialog else {
        return;
    };
    d.name = app.get_db_dialog_name().to_string();
    d.host = app.get_db_dialog_host().to_string();
    d.port = app.get_db_dialog_port().to_string();
    d.database = app.get_db_dialog_database().to_string();
    d.user = app.get_db_dialog_user().to_string();
    d.password = app.get_db_dialog_password().to_string();
    d.save_password = app.get_db_dialog_save_password();
    d.trust_cert = app.get_db_dialog_trust_cert();
    d.url = app.get_db_dialog_url().to_string();
}

fn dialog_args(d: &DbDialog) -> Vec<(&'static str, ArgValue)> {
    let kind = DB_KINDS
        .get(d.kind_index)
        .copied()
        .unwrap_or(db::DriverKind::Sqlite);
    let mut args = vec![
        ("name", ArgValue::String(d.name.clone())),
        ("kind", ArgValue::String(kind.as_str().to_string())),
        ("host", ArgValue::String(d.host.clone())),
        ("port", ArgValue::Integer(d.port.parse::<i64>().unwrap_or(0))),
        ("database", ArgValue::String(d.database.clone())),
        ("user", ArgValue::String(d.user.clone())),
        ("save-password", ArgValue::Boolean(d.save_password)),
        (
            "ssl",
            ArgValue::String(
                match d.ssl_index {
                    0 => db::SslMode::Disabled,
                    2 => db::SslMode::Require,
                    _ => db::SslMode::Prefer,
                }
                .as_str()
                .to_string(),
            ),
        ),
        ("trust-cert", ArgValue::Boolean(d.trust_cert)),
    ];
    if d.url_mode {
        args.push(("url", ArgValue::String(d.url.clone())));
    }
    if !d.password.is_empty() {
        args.push(("password", ArgValue::String(d.password.clone())));
    }
    args
}

fn test_dialog_connection(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    dialog_fields(&weak.upgrade().expect("app window"), s);
    let Some(d) = &mut s.db_dialog else {
        return;
    };
    d.testing = true;
    let probe_source = {
        let mut args = HashMap::new();
        for (k, v) in dialog_args(d) {
            args.insert(k.to_string(), v);
        }
        // Build a probe source without registering it.
        let kind = DB_KINDS
            .get(d.kind_index)
            .copied()
            .unwrap_or(db::DriverKind::Sqlite);
        let _ = &mut args;
        db::DataSource {
            id: 0,
            name: "probe".into(),
            kind,
            host: d.host.clone(),
            port: d.port.parse().unwrap_or(kind.default_port()),
            database: d.database.clone(),
            user: d.user.clone(),
            ssl_mode: match d.ssl_index {
                0 => db::SslMode::Disabled,
                2 => db::SslMode::Require,
                _ => db::SslMode::Prefer,
            },
            trust_cert: d.trust_cert,
            save_password: false,
            url: if d.url_mode && !d.url.is_empty() {
                Some(d.url.clone())
            } else {
                None
            },
        }
    };
    let password = s
        .db_dialog
        .as_ref()
        .map(|d| d.password.clone())
        .filter(|p| !p.is_empty());
    let token = s.editor.db.take_next_test_token();
    if let Some(d) = &mut s.db_dialog {
        d.test_token = token;
    }
    match s.editor.background.as_ref() {
        Some(bg) => {
            let sender = bg.sender.clone();
            bg.spawn_blocking(move || {
                let result = db::test_connection(&probe_source, password.as_deref());
                let _ = sender.send(kernel::runtime::BackgroundEvent::Db(
                    kernel::db::DbEvent::TestDone { token, result },
                ));
            });
        }
        None => {
            let result = db::test_connection(&probe_source, password.as_deref());
            s.editor.db.last_test = Some(result);
            if let Some(d) = &mut s.db_dialog {
                d.testing = false;
            }
        }
    }
    let _ = weak;
}

fn save_dialog(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    dialog_fields(&weak.upgrade().expect("app window"), s);
    let Some(d) = s.db_dialog.take() else {
        return;
    };
    let editing = d.editing;
    let cmd = if editing.is_some() {
        "db-source-update"
    } else {
        "db-source-add"
    };
    let mut args = HashMap::new();
    for (k, v) in dialog_args(&d) {
        args.insert(k.to_string(), v);
    }
    if let Some(id) = editing {
        args.insert("source".to_string(), ArgValue::Integer(id as i64));
    }
    match execute_command(&mut s.editor, cmd, &args) {
        Ok(()) => {
            s.message.clear();
            // The usual flow is add → explore: connect right away.
            let id = editing.unwrap_or_else(|| {
                s.editor
                    .db
                    .sources
                    .last()
                    .map(|s| s.id)
                    .unwrap_or(0)
            });
            s.editor.db.active = Some(id);
            if !s.editor.db.is_connected(id) {
                run(&mut s.editor, "db-connect", &[("source", ArgValue::Integer(id as i64))]);
            }
        }
        Err(e) => {
            // Keep the form open with the error visible.
            s.db_dialog = Some(d);
            s.message = e;
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn run(ed: &mut kernel::state::Editor, command: &str, args: &[(&str, ArgValue)]) {
    let mut map = HashMap::new();
    for (k, v) in args {
        map.insert(k.to_string(), v.clone());
    }
    if let Err(e) = execute_command(ed, command, &map) {
        ed.show_message(kernel::state::MessageKind::Error, e);
    }
}

fn run_s(s: &mut AppState, command: &str, args: &[(&str, ArgValue)]) {
    run(&mut s.editor, command, args);
}

/// Read back an in-out Field's text from the generated component.
fn query_of(weak: &slint::Weak<AppWindow>, property: &str) -> String {
    let Some(a) = weak.upgrade() else {
        return String::new();
    };
    match property {
        "db-tree-filter" => a.get_db_tree_filter().to_string(),
        "db-filter-text" => a.get_db_filter_text().to_string(),
        _ => String::new(),
    }
}
