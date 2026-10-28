//! Database tool commands: data sources, connect/disconnect, schema
//! refresh, bounded queries, and cancellation.  Slow work runs on the
//! background pool and lands via `DbEvent` — the synchronous fallback
//! (tests, embedded) runs inline instead.

use std::sync::Arc;

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::db::DbEvent;
use crate::runtime::BackgroundEvent;
use crate::state::Editor;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "db-source-add",
        "Register a SQLite data source (name, path)",
        vec![
            ArgSpec::new("name", ArgType::String),
            ArgSpec::new("path", ArgType::Path),
        ],
        |editor, args| {
            let name = args
                .get("name")
                .and_then(|a| a.as_string())
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| "a data source name is required".to_string())?;
            let path = args
                .get("path")
                .and_then(|a| a.as_string())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "a database file path is required".to_string())?;
            editor.db.add_source(name, &path);
            editor.show_message(
                crate::state::MessageKind::Info,
                format!("data source {name} added"),
            );
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-source-remove",
        "Remove a data source and drop its connection",
        vec![ArgSpec::new("source", ArgType::Integer)],
        |editor, args| {
            let id = source_id(editor, args)?;
            editor.db.remove_source(id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-select",
        "Point the Database panel at a data source",
        vec![ArgSpec::new("source", ArgType::Integer)],
        |editor, args| {
            let id = source_id(editor, args)?;
            editor.db.active = Some(id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-connect",
        "Connect to a data source and load its schema",
        vec![ArgSpec::new("source", ArgType::Integer)],
        |editor, args| {
            let id = source_id(editor, args)?;
            let source = editor
                .db
                .source(id)
                .cloned()
                .ok_or_else(|| "no such data source".to_string())?;
            let bg_source = source.clone();
            editor.db.busy_metadata = true;
            let sender = match editor.background.as_ref() {
                Some(bg) => {
                    let sender = bg.sender.clone();
                    let bg = bg.clone();
                    bg.spawn_blocking(move || match ::db::connect(&bg_source) {
                        Ok(conn) => {
                            let tables = conn.tables();
                            let _ = sender.send(BackgroundEvent::Db(DbEvent::Connected {
                                source_id: source.id,
                                connection: Arc::new(conn),
                                tables,
                            }));
                        }
                        Err(e) => {
                            let _ = sender.send(BackgroundEvent::Db(DbEvent::TablesDone {
                                source_id: source.id,
                                tables: Err(e),
                            }));
                        }
                    });
                    None
                }
                None => Some(()),
            };
            if sender.is_some() {
                // No background pool: connect inline so tests can drive it.
                match ::db::connect(&source) {
                    Ok(conn) => {
                        let tables = conn.tables();
                        editor.db.apply_connected(id, Arc::new(conn), tables);
                    }
                    Err(e) => editor.db.apply_tables(id, Err(e)),
                }
                editor.frame_dirty = true;
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "db-console",
        "Open the SQL console buffer (renders in the editor like any buffer)",
        vec![],
        |editor, _args| {
            let key = open_console_buffer(editor);
            if let Some(win) = editor.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-query-buffer",
        "Execute the SQL console buffer's content against the active source",
        vec![],
        |editor, _args| {
            let key = open_console_buffer(editor);
            let sql = editor
                .buffers
                .get(key)
                .map(|a| {
                    let buf = a.lock().unwrap_or_else(|e| e.into_inner());
                    buf.slice(0, buf.len())
                })
                .unwrap_or_default();
            run_query(editor, &sql)
        },
    );

    cmds.register_fn(
        "db-table-sql",
        "Stage a SELECT for a table in the SQL console buffer",
        vec![ArgSpec::new("table", ArgType::String)],
        |editor, args| {
            let table = args
                .get("table")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "a table name is required".to_string())?;
            let stmt = ::db::select_all(table, crate::db::FETCH_LIMIT);
            let key = open_console_buffer(editor);
            if let Some(arc) = editor.buffers.get(key) {
                let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let at = buf.len();
                let prefix = if at == 0 {
                    String::new()
                } else {
                    "\n".to_string()
                };
                buf.insert(at, &format!("{prefix}{stmt}\n"));
            }
            if let Some(win) = editor.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-disconnect",
        "Close a data source's connection",
        vec![ArgSpec::new("source", ArgType::Integer)],
        |editor, args| {
            let id = source_id(editor, args)?;
            editor.db.connections.remove(&id);
            editor.db.tables.remove(&id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-refresh",
        "Reload the connected data source's schema",
        vec![ArgSpec::new("source", ArgType::Integer)],
        |editor, args| {
            let id = source_id(editor, args)?;
            let conn = editor
                .db
                .connections
                .get(&id)
                .cloned()
                .ok_or_else(|| "source is not connected".to_string())?;
            editor.db.busy_metadata = true;
            match editor.background.as_ref() {
                Some(bg) => {
                    let sender = bg.sender.clone();
                    bg.spawn_blocking(move || {
                        let tables = conn.tables();
                        let _ = sender.send(BackgroundEvent::Db(DbEvent::TablesDone {
                            source_id: id,
                            tables,
                        }));
                    });
                }
                None => {
                    let tables = conn.tables();
                    editor.db.apply_tables(id, tables);
                    editor.frame_dirty = true;
                }
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "db-query",
        "Execute SQL against the active data source",
        vec![ArgSpec::new("sql", ArgType::String)],
        |editor, args| {
            let sql = args
                .get("sql")
                .and_then(|a| a.as_string())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "SQL is required".to_string())?;
            let id = editor
                .db
                .active
                .ok_or_else(|| "no active data source".to_string())?;
            let conn = editor
                .db
                .connections
                .get(&id)
                .cloned()
                .ok_or_else(|| "source is not connected".to_string())?;
            editor.db.note_history(&sql);
            let job = editor.db.take_next_job();
            editor.db.busy_query = true;
            match editor.background.as_ref() {
                Some(bg) => {
                    let sender = bg.sender.clone();
                    bg.spawn_blocking(move || {
                        let result = conn.execute(&sql, crate::db::FETCH_LIMIT);
                        let _ =
                            sender.send(BackgroundEvent::Db(DbEvent::QueryDone { job, result }));
                    });
                }
                None => {
                    let result = conn.execute(&sql, crate::db::FETCH_LIMIT);
                    editor.db.apply_query(job, result);
                    editor.frame_dirty = true;
                }
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "db-cancel",
        "Interrupt the running query on the active source",
        vec![],
        |editor, _args| {
            if editor.db.interrupt_active() {
                editor.show_message(
                    crate::state::MessageKind::Info,
                    "interrupt sent to the database",
                );
                Ok(())
            } else {
                Err("no connected data source to interrupt".to_string())
            }
        },
    );
}

fn source_id(
    editor: &mut Editor,
    args: &std::collections::HashMap<String, ArgValue>,
) -> Result<u64, String> {
    if let Some(v) = args.get("source").and_then(|a| a.as_integer()) {
        return Ok(v.max(0) as u64);
    }
    editor
        .db
        .active
        .ok_or_else(|| "no active data source".to_string())
}

/// The SQL console is an editor buffer, not a widget: it renders in the
/// EditorView with the SQL major mode, participates in crash recovery, and
/// persists with the workspace.  Reopening focuses the existing one.
fn open_console_buffer(editor: &mut Editor) -> usize {
    if let Some((key, _)) = editor
        .buffers
        .iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).name == "*sql*")
    {
        return key;
    }
    let id = editor.allocate_buffer_id();
    let buf = crate::text_engine::Buffer::from_string(
        crate::state::id::BufferId(id),
        "*sql*",
        "-- SQL console: put a statement here and run it against the active source.\n",
    );
    let arc = std::sync::Arc::new(std::sync::Mutex::new(buf));
    let entry = editor.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(arc.clone());
    editor
        .views
        .insert(key, crate::text_engine::BufferView::new(arc));
    if let Some(a) = editor.buffers.get(key) {
        a.lock().unwrap_or_else(|e| e.into_inner()).major_mode =
            crate::text_engine::MajorMode::from_name("sql");
    }
    key
}

/// Shared tail of the query commands: dispatch to the active source's
/// connection (background when a pool exists, inline otherwise).
fn run_query(editor: &mut Editor, sql: &str) -> Result<(), String> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err("the console buffer is empty".to_string());
    }
    let id = editor
        .db
        .active
        .ok_or_else(|| "no active data source".to_string())?;
    let conn = editor
        .db
        .connections
        .get(&id)
        .cloned()
        .ok_or_else(|| "source is not connected".to_string())?;
    editor.db.note_history(sql);
    let job = editor.db.take_next_job();
    editor.db.busy_query = true;
    match editor.background.as_ref() {
        Some(bg) => {
            let sender = bg.sender.clone();
            let sql = sql.to_string();
            bg.spawn_blocking(move || {
                let result = conn.execute(&sql, crate::db::FETCH_LIMIT);
                let _ = sender.send(BackgroundEvent::Db(DbEvent::QueryDone { job, result }));
            });
        }
        None => {
            let result = conn.execute(sql, crate::db::FETCH_LIMIT);
            editor.db.apply_query(job, result);
            editor.frame_dirty = true;
        }
    }
    Ok(())
}
