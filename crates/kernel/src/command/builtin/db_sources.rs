//! Data source commands: registration, selection, connect/disconnect,
//! connection testing.  Slow work runs on the background pool and lands
//! via `DbEvent`; without a pool (tests, headless) it runs inline.

use std::collections::HashMap;
use std::sync::Arc;

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::db::DbEvent;
use crate::runtime::BackgroundEvent;
use crate::state::secrets;
use crate::state::Editor;
use db::{DataSource, DriverKind, SslMode};

fn conn_specs(with_name: bool) -> Vec<ArgSpec> {
    let mut specs = Vec::new();
    if with_name {
        specs.push(ArgSpec::new("name", ArgType::String));
    }
    for (n, t) in [
        ("kind", ArgType::String),
        ("host", ArgType::String),
        ("port", ArgType::Integer),
        ("database", ArgType::String),
        ("user", ArgType::String),
        ("password", ArgType::String),
        ("save-password", ArgType::Boolean),
        ("ssl", ArgType::String),
        ("trust-cert", ArgType::Boolean),
        ("url", ArgType::String),
    ] {
        specs.push(ArgSpec::optional(
            n,
            t.clone(),
            if matches!(t, ArgType::Integer) {
                ArgValue::Integer(0)
            } else if matches!(t, ArgType::Boolean) {
                ArgValue::Boolean(false)
            } else {
                ArgValue::String(String::new())
            },
        ));
    }
    specs
}

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "db-source-add",
        "Register a data source (name, kind, host/port/database/user, …)",
        conn_specs(true),
        |editor, args| {
            let mut source = source_from_args(editor, args, None)?;
            let password = string_arg(args, "password").map(|s| s.to_string());
            let id = editor.db.add_source(source.clone());
            source.id = id;
            store_password(editor, id, password, source.save_password);
            editor.show_message(
                crate::state::MessageKind::Info,
                format!("data source {} added", source.name),
            );
            editor.frame_dirty = true;
            Ok(())
        },
    );

    let mut update_specs = vec![ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0))];
    update_specs.extend(conn_specs(true));
    cmds.register_fn(
        "db-source-update",
        "Update a data source's connection settings",
        update_specs,
        |editor, args| {
            let id = source_id(editor, args)?;
            let mut updated = source_from_args(editor, args, Some(id))?;
            updated.id = id;
            // Changing the connection facts invalidates the live session.
            editor.db.connections.remove(&id);
            editor.db.trees.remove(&id);
            editor.db.catalogs.remove(&id);
            let password = string_arg(args, "password").map(|s| s.to_string());
            store_password(editor, id, password, updated.save_password);
            let name = updated.name.clone();
            if let Some(src) = editor.db.source_mut(id) {
                *src = updated;
            }
            editor.show_message(
                crate::state::MessageKind::Info,
                format!("data source {name} updated"),
            );
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-source-remove",
        "Remove a data source and drop its connection",
        vec![ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let id = source_id(editor, args)?;
            secrets::delete_key(&source_account(id));
            editor.db.remove_source(id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-select",
        "Point the Database tool at a data source",
        vec![ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let id = source_id(editor, args)?;
            editor.db.active = Some(id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-connect",
        "Connect to a data source and load its catalog tree",
        vec![ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let id = source_id(editor, args)?;
            let source = editor
                .db
                .source(id)
                .cloned()
                .ok_or_else(|| "no such data source".to_string())?;
            let password = editor.db.password_for(
                id,
                || secrets::load_key(&source_account(id)),
            );
            editor.db.tree_mut(id).mark_loading("/");
            connect_dispatch(editor, source, password);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-disconnect",
        "Close a data source's connection and drop its tree",
        vec![ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let id = source_id(editor, args)?;
            editor.db.connections.remove(&id);
            editor.db.trees.remove(&id);
            editor.db.catalogs.remove(&id);
            editor.db.loading.retain(|(s, _)| *s != id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-test-connection",
        "Probe a connection without registering it; the result lands via DbEvent",
        conn_specs(false),
        |editor, args| {
            let probe = source_from_args(editor, args, None)?;
            let password = string_arg(args, "password").map(|s| s.to_string());
            let token = editor.db.take_next_test_token();
            match editor.background.as_ref() {
                Some(bg) => {
                    let sender = bg.sender.clone();
                    bg.spawn_blocking(move || {
                        let result = db::test_connection(&probe, password.as_deref());
                        let _ = sender.send(BackgroundEvent::Db(DbEvent::TestDone {
                            token,
                            result,
                        }));
                    });
                }
                None => {
                    editor.db.last_test = Some(db::test_connection(&probe, password.as_deref()));
                }
            }
            Ok(())
        },
    );
}

/// Dispatch a connect on the background pool, fetching the root tree level
/// in the same task so the explorer has something to show on landing.
pub(super) fn connect_dispatch(editor: &mut Editor, source: DataSource, password: Option<String>) {
    match editor.background.as_ref() {
        Some(bg) => {
            let sender = bg.sender.clone();
            bg.spawn_blocking(move || match db::connect(&source, password.as_deref()) {
                Ok(conn) => {
                    let conn = Arc::new(conn);
                    let steps = introspect_cascade(&conn, &db::NodeRef::Root);
                    let _ = sender.send(BackgroundEvent::Db(DbEvent::Connected {
                        source_id: source.id,
                        connection: Some(Arc::clone(&conn)),
                        error: None,
                    }));
                    for (path, children) in steps {
                        let _ = sender.send(BackgroundEvent::Db(DbEvent::Introspected {
                            source_id: source.id,
                            path,
                            children,
                        }));
                    }
                }
                Err(e) => {
                    let _ = sender.send(BackgroundEvent::Db(DbEvent::Connected {
                        source_id: source.id,
                        connection: None,
                        error: Some(e),
                    }));
                }
            });
        }
        None => {
            // No background pool: connect inline so tests can drive it.
            match db::connect(&source, password.as_deref()) {
                Ok(conn) => {
                    let conn = Arc::new(conn);
                    let steps = introspect_cascade(&conn, &db::NodeRef::Root);
                    editor.db.connections.insert(source.id, Arc::clone(&conn));
                    editor.db.last_error = None;
                    for (path, children) in steps {
                        editor.db.apply_introspected(source.id, &path, children);
                    }
                    crate::db::dispatch_catalog(editor, source.id);
                }
                Err(e) => {
                    editor.db.tree_mut(source.id).apply_children(
                        "/",
                        Vec::new(),
                        Some(e.clone()),
                    );
                    editor.db.last_error = Some(e);
                }
            }
            editor.frame_dirty = true;
        }
    }
}

/// Re-run an introspection against the live connection (background when a
/// pool exists).  Shared by expand, refresh-node, and refresh-source.
pub(super) fn introspect_dispatch(editor: &mut Editor, source_id: u64, path: &str) -> Result<(), String> {
    let conn = editor
        .db
        .connections
        .get(&source_id)
        .cloned()
        .ok_or_else(|| "source is not connected".to_string())?;
    editor.db.loading.insert((source_id, path.to_string()));
    let path = path.to_string();
    match editor.background.as_ref() {
        Some(bg) => {
            let sender = bg.sender.clone();
            bg.spawn_blocking(move || {
                let node = db::NodeRef::parse(&path).unwrap_or(db::NodeRef::Root);
                for (step_path, children) in introspect_cascade(&conn, &node) {
                    let _ = sender.send(BackgroundEvent::Db(DbEvent::Introspected {
                        source_id,
                        path: step_path,
                        children,
                    }));
                }
            });
        }
        None => {
            let node = db::NodeRef::parse(&path).unwrap_or(db::NodeRef::Root);
            editor.db.loading.remove(&(source_id, path));
            for (step_path, children) in introspect_cascade(&conn, &node) {
                editor.db.apply_introspected(source_id, &step_path, children);
            }
            editor.frame_dirty = true;
        }
    }
    Ok(())
}

fn introspect_cascade(
    conn: &db::Connection,
    start: &db::NodeRef,
) -> Vec<(String, Result<Vec<db::Child>, String>)> {
    let mut out = Vec::new();
    let mut current = Some(start.clone());
    while let Some(node) = current.take() {
        let key = node.key();
        let res = conn.introspect(&node);
        let next = match (&node, &res) {
            (db::NodeRef::Root, Ok(children)) => children.iter().find_map(|c| match c {
                db::Child::Database { name, .. } => Some(db::NodeRef::Database(name.clone())),
                _ => None,
            }),
            (db::NodeRef::Database(db_name), Ok(children)) => {
                let schemas: Vec<&str> = children
                    .iter()
                    .filter_map(|c| match c {
                        db::Child::Schema { name } => Some(name.as_str()),
                        _ => None,
                    })
                    .collect();
                let pick = schemas
                    .iter()
                    .copied()
                    .find(|s| matches!(*s, "public" | "main" | "dbo"))
                    .or_else(|| (schemas.len() == 1).then(|| schemas[0]));
                pick.map(|s| db::NodeRef::Schema {
                    database: db_name.clone(),
                    schema: s.to_string(),
                })
            }
            (db::NodeRef::Schema { database, schema }, Ok(children)) => children
                .iter()
                .any(|c| matches!(c, db::Child::Folder { kind: db::FolderKind::Tables }))
                .then(|| db::NodeRef::Folder {
                    database: database.clone(),
                    schema: schema.clone(),
                    kind: db::FolderKind::Tables,
                }),
            _ => None,
        };
        out.push((key, res));
        current = next;
    }
    out
}

pub(super) fn source_id(editor: &mut Editor, args: &HashMap<String, ArgValue>) -> Result<u64, String> {
    // Optional args arrive with their injected default (0); real source
    // ids start at 1, so 0 means "not provided" — fall back to active.
    if let Some(v) = args.get("source").and_then(|a| a.as_integer())
        && v > 0
    {
        return Ok(v as u64);
    }
    editor
        .db
        .active
        .ok_or_else(|| "no active data source".to_string())
}

fn string_arg<'a>(args: &'a HashMap<String, ArgValue>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|a| a.as_string())
}

fn bool_arg(args: &HashMap<String, ArgValue>, key: &str) -> bool {
    args.get(key).and_then(|a| a.as_bool()).unwrap_or(false)
}

/// Build a `DataSource` from the connection form's arguments.  `id` is the
/// existing source when updating, `None` when probing/creating.
fn source_from_args(
    editor: &mut Editor,
    args: &HashMap<String, ArgValue>,
    id: Option<u64>,
) -> Result<DataSource, String> {
    let name = string_arg(args, "name").map(|s| s.trim().to_string());
    let name = match (id, name) {
        (Some(_), Some(n)) if !n.is_empty() => n,
        (Some(id), _) => editor
            .db
            .source(id)
            .map(|s| s.name.clone())
            .unwrap_or_default(),
        (None, Some(n)) if !n.is_empty() => n,
        (None, _) => return Err("a data source name is required".to_string()),
    };
    let kind = string_arg(args, "kind")
        .and_then(DriverKind::parse)
        .or_else(|| id.and_then(|id| editor.db.source(id)).map(|s| s.kind))
        .unwrap_or(DriverKind::Sqlite);
    let host = string_arg(args, "host")
        .map(|s| s.trim().to_string())
        .or_else(|| id.and_then(|id| editor.db.source(id)).map(|s| s.host.clone()))
        .unwrap_or_default();
    if kind.is_embedded() && host.is_empty() {
        return Err("SQLite needs a database file path".to_string());
    }
    if !kind.is_embedded() && host.is_empty() {
        return Err("a host is required".to_string());
    }
    let port = args
        .get("port")
        .and_then(|a| a.as_integer())
        .map(|p| p.max(0) as u16)
        .unwrap_or_else(|| kind.default_port());
    let ssl_mode = string_arg(args, "ssl")
        .and_then(SslMode::parse)
        .unwrap_or(SslMode::Prefer);
    Ok(DataSource {
        id: id.unwrap_or(0),
        name,
        kind,
        host,
        port,
        database: string_arg(args, "database").unwrap_or_default().to_string(),
        user: string_arg(args, "user").unwrap_or_default().to_string(),
        ssl_mode,
        trust_cert: bool_arg(args, "trust-cert"),
        save_password: bool_arg(args, "save-password"),
        url: string_arg(args, "url").map(|s| s.to_string()).filter(|s| !s.is_empty()),
    })
}

/// Saved passwords go to the OS keychain; unsaved ones stay session-only
/// so re-connecting within the session still works.  Neither touches
/// persisted configuration.  A typed password is always kept for the
/// session as well, because the keychain can refuse a write.  An empty
/// field on an existing source means "unchanged": the properties dialog
/// never pre-fills the password.
fn store_password(editor: &mut Editor, id: u64, password: Option<String>, save: bool) {
    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        if save {
            secrets::save_key(&source_account(id), &pw);
        }
        editor.db.session_passwords.insert(id, pw);
    }
    if !save {
        secrets::delete_key(&source_account(id));
    }
}

pub(super) fn source_account(id: u64) -> String {
    format!("db-source-{id}")
}
