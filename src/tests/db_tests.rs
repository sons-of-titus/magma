//! Database tool: data sources, connections, schema, and bounded queries
//! through the command surface (synchronous path — no background pool).

use crate::kernel::command::execute_command;
use crate::kernel::state::{Editor, MessageKind};
use std::collections::HashMap;

use kernel::command::args::ArgValue;

fn editor() -> Editor {
    let mut ed = Editor::new(Box::new(crate::kernel::storage::disk::DiskFileSystem::new()));
    crate::kernel::command::builtin::register_builtin_commands(&mut ed);
    ed
}

fn tmp_sqlite(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "magma-db-cmd-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("demo.sqlite")
}

fn args(pairs: &[(&str, ArgValue)]) -> HashMap<String, ArgValue> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[test]
fn source_add_and_remove_updates_state() {
    let mut ed = editor();
    let path = tmp_sqlite("add");
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("demo".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    assert_eq!(ed.db.sources.len(), 1);
    assert_eq!(ed.db.sources[0].name, "demo");
    assert_eq!(ed.db.active, Some(ed.db.sources[0].id));

    let id = ed.db.sources[0].id;
    execute_command(
        &mut ed,
        "db-source-remove",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    )
    .unwrap();
    assert!(ed.db.sources.is_empty());
    assert_eq!(ed.db.active, None);
}

#[test]
fn source_add_requires_name_and_path() {
    let mut ed = editor();
    assert!(execute_command(&mut ed, "db-source-add", &args(&[])).is_err());
    assert!(
        execute_command(
            &mut ed,
            "db-source-add",
            &args(&[("name", ArgValue::String("x".into()))])
        )
        .is_err()
    );
}

#[test]
fn connect_loads_schema_and_query_fills_results() {
    let mut ed = editor();
    let path = tmp_sqlite("query");
    {
        let conn = ::db::SqliteConnection::open(&path.to_string_lossy()).unwrap();
        conn.execute(
            "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT); \
             INSERT INTO items VALUES (1, 'a'), (2, 'b'), (3, 'c');",
            10,
        )
        .unwrap();
    }

    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("demo".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    let id = ed.db.active.unwrap();
    execute_command(
        &mut ed,
        "db-connect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    )
    .unwrap();

    assert!(ed.db.is_connected(id), "the connection is live");
    let tables = ed.db.tables.get(&id).unwrap();
    let items = tables.iter().find(|t| t.name == "items").unwrap();
    assert_eq!(items.columns.len(), 2);

    execute_command(
        &mut ed,
        "db-query",
        &args(&[(
            "sql",
            ArgValue::String("SELECT * FROM items ORDER BY id".into()),
        )]),
    )
    .unwrap();
    let result = ed.db.last_result.as_ref().unwrap();
    assert_eq!(result.columns, vec!["id", "label"]);
    assert_eq!(result.rows.len(), 3);
    assert!(
        ed.db
            .history
            .iter()
            .any(|h| h.contains("SELECT * FROM items"))
    );
    assert_eq!(ed.db.last_error, None);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn connect_error_surfaces_in_state() {
    let mut ed = editor();
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("bad".into())),
            (
                "path",
                ArgValue::Path("/nonexistent-dir-xyz/nope.sqlite".into()),
            ),
        ]),
    )
    .unwrap();
    let id = ed.db.active.unwrap();
    // SQLite creates missing files; a truly unreachable path is one whose
    // parent directory does not exist.
    let _ = execute_command(
        &mut ed,
        "db-connect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    );
    assert!(
        !ed.db.is_connected(id),
        "connect into a missing directory must fail"
    );
    assert!(ed.db.last_error.is_some());
}

#[test]
fn query_requires_connection_and_sql() {
    let mut ed = editor();
    assert!(
        execute_command(
            &mut ed,
            "db-query",
            &args(&[("sql", ArgValue::String("SELECT 1".into()))])
        )
        .is_err()
    );
    let path = tmp_sqlite("noconn");
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("d".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    assert!(
        execute_command(
            &mut ed,
            "db-query",
            &args(&[("sql", ArgValue::String("SELECT 1".into()))])
        )
        .is_err()
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn query_error_lands_in_last_error() {
    let mut ed = editor();
    let path = tmp_sqlite("err");
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("d".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    let id = ed.db.active.unwrap();
    execute_command(
        &mut ed,
        "db-connect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-query",
        &args(&[("sql", ArgValue::String("SELECT * FROM ghost".into()))]),
    )
    .unwrap();
    assert!(ed.db.last_error.as_deref().unwrap().contains("ghost"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn disconnect_drops_connection_and_schema() {
    let mut ed = editor();
    let path = tmp_sqlite("disc");
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("d".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    let id = ed.db.active.unwrap();
    execute_command(
        &mut ed,
        "db-connect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    )
    .unwrap();
    assert!(ed.db.is_connected(id));
    execute_command(
        &mut ed,
        "db-disconnect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    )
    .unwrap();
    assert!(!ed.db.is_connected(id));
    assert!(ed.db.tables.get(&id).is_none());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn data_sources_survive_a_workspace_round_trip() {
    use kernel::storage::workspace::WorkspaceManager;

    let mut ed = editor();
    let path = tmp_sqlite("persist");
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("kept".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();

    let snapshot = WorkspaceManager::snapshot(&ed);
    let mut fresh = editor();
    WorkspaceManager::apply(&mut fresh, &snapshot).unwrap();
    assert_eq!(fresh.db.sources.len(), 1);
    assert_eq!(fresh.db.sources[0].name, "kept");
    assert_eq!(fresh.db.sources[0].kind.as_str(), "sqlite");
    // Adding after restore continues past the restored ids.
    execute_command(
        &mut fresh,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("second".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    let ids: Vec<u64> = fresh.db.sources.iter().map(|s| s.id).collect();
    assert!(
        ids[1] > ids[0],
        "ids keep increasing after restore: {ids:?}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn cancel_without_connection_reports_cleanly() {
    let mut ed = editor();
    assert!(execute_command(&mut ed, "db-cancel", &args(&[])).is_err());
    let _ = MessageKind::Info;
}

#[test]
fn console_opens_a_sql_mode_buffer_in_the_editor() {
    let mut ed = editor();
    execute_command(&mut ed, "db-console", &args(&[])).unwrap();
    let (_name, mode) = ed
        .buffers
        .iter()
        .map(|(_, a)| {
            let buf = a.lock().unwrap_or_else(|e| e.into_inner());
            (buf.name.clone(), buf.major_mode.name().to_string())
        })
        .find(|(name, _)| name == "*sql*")
        .expect("the console buffer exists");
    assert_eq!(mode, "sql");
    // The focused pane shows the console — it renders in the EditorView
    // like any other buffer.
    let focused = ed
        .view_tree
        .focused_window_mut()
        .and_then(|w| w.buffer_id)
        .map(|k| {
            ed.buffers
                .get(k)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).name.clone())
        })
        .flatten();
    assert_eq!(focused.as_deref(), Some("*sql*"));
}

#[test]
fn query_buffer_executes_the_console_content() {
    let mut ed = editor();
    let path = tmp_sqlite("buffer");
    connect_demo(&mut ed, &path);
    execute_command(&mut ed, "db-console", &args(&[])).unwrap();
    let key = ed
        .buffers
        .iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).name == "*sql*")
        .map(|(k, _)| k)
        .unwrap();
    if let Some(arc) = ed.buffers.get(key) {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let at = buf.len();
        buf.insert(at, "SELECT 41 + 1 AS answer");
    }
    execute_command(&mut ed, "db-query-buffer", &args(&[])).unwrap();
    let result = ed.db.last_result.as_ref().unwrap();
    assert_eq!(result.columns, vec!["answer"]);
    assert_eq!(result.rows.len(), 1);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn query_buffer_rejects_empty_console() {
    let mut ed = editor();
    execute_command(&mut ed, "db-console", &args(&[])).unwrap();
    // The seeded hint line is a comment; a comment-only buffer is empty SQL.
    assert!(execute_command(&mut ed, "db-query-buffer", &args(&[])).is_err());
}

#[test]
fn table_sql_stages_a_select_into_the_console() {
    let mut ed = editor();
    let path = tmp_sqlite("stage");
    connect_demo(&mut ed, &path);
    let id = ed.db.active.unwrap();
    let table = ed
        .db
        .tables
        .get(&id)
        .and_then(|t| t.first().map(|x| x.name.clone()))
        .unwrap_or_else(|| "items".to_string());
    execute_command(
        &mut ed,
        "db-table-sql",
        &args(&[("table", ArgValue::String(table))]),
    )
    .unwrap();
    let console = ed
        .buffers
        .iter()
        .map(|(_, a)| {
            let buf = a.lock().unwrap_or_else(|e| e.into_inner());
            (buf.name.clone(), buf.slice(0, buf.len()))
        })
        .find(|(name, _)| name == "*sql*")
        .unwrap();
    assert!(console.1.contains("SELECT * FROM"), "got: {}", console.1);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Register a source on `path`, connect, and create a two-column table.
fn connect_demo(ed: &mut Editor, path: &std::path::Path) {
    {
        let conn = ::db::SqliteConnection::open(&path.to_string_lossy()).unwrap();
        conn.execute(
            "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT); \
             INSERT INTO items VALUES (1, 'a');",
            10,
        )
        .unwrap();
    }
    execute_command(
        ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("demo".into())),
            ("path", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    let id = ed.db.active.unwrap();
    execute_command(
        ed,
        "db-connect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    )
    .unwrap();
}
