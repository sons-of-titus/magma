//! Database tool: data sources, connections, tree introspection, queries,
//! table browsing, and pending grid edits through the command surface
//! (synchronous path — no background pool).

use crate::kernel::command::execute_command;
use crate::kernel::state::Editor;
use std::collections::HashMap;

use kernel::command::args::ArgValue;
use db::NodeRef;
use kernel::db::{DataTabKind, NodeStatus};

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

/// A registered, connected SQLite source with one table of three rows.
fn connected_sqlite(ed: &mut Editor, tag: &str) -> (u64, std::path::PathBuf) {
    let path = tmp_sqlite(tag);
    {
        let conn = ::db::connect(
            &::db::DataSource {
                id: 0,
                name: "probe".into(),
                kind: db::DriverKind::Sqlite,
                host: path.to_string_lossy().to_string(),
                port: 0,
                database: String::new(),
                user: String::new(),
                ssl_mode: Default::default(),
                trust_cert: false,
                save_password: false,
                url: None,
            },
            None,
        )
        .unwrap();
        conn.execute_script(
            "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT NOT NULL, score REAL); \
             INSERT INTO items VALUES (1, 'a', 1.5), (2, 'b', NULL), (3, 'c', 3.0);",
            10,
        )
        .unwrap();
    }
    execute_command(
        ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("demo".into())),
            ("kind", ArgValue::String("sqlite".into())),
            ("host", ArgValue::Path(path.to_string_lossy().to_string())),
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
    assert!(ed.db.is_connected(id));
    (id, path)
}

#[test]
fn source_add_select_and_remove_updates_state() {
    let mut ed = editor();
    let path = tmp_sqlite("add");
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("demo".into())),
            ("kind", ArgValue::String("sqlite".into())),
            ("host", ArgValue::Path(path.to_string_lossy().to_string())),
        ]),
    )
    .unwrap();
    assert_eq!(ed.db.sources.len(), 1);
    assert_eq!(ed.db.sources[0].name, "demo");
    assert_eq!(ed.db.sources[0].kind, db::DriverKind::Sqlite);
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
fn source_add_requires_name_and_file() {
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
    assert!(
        execute_command(
            &mut ed,
            "db-source-add",
            &args(&[
                ("name", ArgValue::String("x".into())),
                ("kind", ArgValue::String("postgres".into())),
            ])
        )
        .is_err(),
        "networked sources need a host"
    );
}

#[test]
fn connect_populates_the_tree_root() {
    let mut ed = editor();
    let (id, path) = connected_sqlite(&mut ed, "tree");
    let tree = ed.db.tree(id);
    let root = tree.node("/").unwrap();
    assert!(root.expanded, "connect expands the root");
    assert!(
        root.children
            .iter()
            .any(|c| matches!(c, db::Child::Database { name, .. } if name == "main")),
        "SQLite surfaces one database"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn expand_walks_schema_folders_and_tables() {
    let mut ed = editor();
    let (id, path) = connected_sqlite(&mut ed, "walk");

    execute_command(
        &mut ed,
        "db-expand",
        &args(&[("path", ArgValue::String("/main".into()))]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-expand",
        &args(&[("path", ArgValue::String("/main/main".into()))]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-expand",
        &args(&[("path", ArgValue::String("/main/main/tables".into()))]),
    )
    .unwrap();

    let tree = ed.db.tree(id);
    let tables = tree.node("/main/main/tables").unwrap();
    assert!(
        tables
            .children
            .iter()
            .any(|c| matches!(c, db::Child::Table { name, .. } if name == "items")),
        "items is introspected: {:?}",
        tables.children
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn open_table_loads_detail_and_page() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "open");

    execute_command(
        &mut ed,
        "db-open-table",
        &args(&[("path", ArgValue::String("/main/main/tables/items".into()))]),
    )
    .unwrap();
    let tab_id = ed.db.active_tab.unwrap();
    let tab = ed.db.tab(tab_id).unwrap();
    assert!(matches!(tab.kind, DataTabKind::Table { .. }));
    let detail = tab.table_detail().expect("detail loaded");
    assert_eq!(detail.primary_key, vec!["id"]);
    assert_eq!(detail.columns.len(), 3);

    let result = tab.result().unwrap();
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.columns.len(), 3);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn query_script_runs_every_statement_into_one_tab() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "script");

    // The console command needs a focused buffer; open one first.
    execute_command(&mut ed, "db-console", &args(&[])).unwrap();
    let console = ed.focused_buffer_key();
    if let Some(arc) = ed.buffers.get(console) {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let at = buf.len();
        buf.insert(at, "SELECT count(*) FROM items; SELECT 40 + 2;");
    }

    execute_command(&mut ed, "db-execute-buffer", &args(&[])).unwrap();
    let tab = ed.db.active_tab().unwrap();
    assert_eq!(tab.results.len(), 2, "one outcome per statement");
    assert_eq!(tab.results[0].rows[0][0], db::DbValue::Integer(3));
    assert_eq!(tab.results[1].rows[0][0], db::DbValue::Integer(42));
    assert!(ed.db.history.iter().any(|h| h.contains("SELECT count(*)")));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn execute_current_runs_only_the_statement_under_the_cursor() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "current");

    execute_command(&mut ed, "db-console", &args(&[])).unwrap();
    let console = ed.focused_buffer_key();
    {
        let arc = ed.buffers.get(console).unwrap();
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        buf.insert(0, "SELECT 1; SELECT 22 + 20;");
    }
    // Park the cursor inside the second statement.
    if let Some(view) = ed.focused_view_mut() {
        view.set_cursor(13);
    }

    execute_command(&mut ed, "db-execute-current", &args(&[])).unwrap();
    let tab = ed.db.active_tab().unwrap();
    assert_eq!(tab.results.len(), 1);
    assert_eq!(tab.results[0].rows[0][0], db::DbValue::Integer(42));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn grid_edits_stage_and_apply_transactionally() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "edit");

    execute_command(
        &mut ed,
        "db-open-table",
        &args(&[("path", ArgValue::String("/main/main/tables/items".into()))]),
    )
    .unwrap();
    let tab_id = ed.db.active_tab.unwrap();

    // Update one cell, delete one row, insert one row — then apply.
    execute_command(
        &mut ed,
        "db-grid-edit-cell",
        &args(&[
            ("row", ArgValue::Integer(1)),
            ("column", ArgValue::Integer(1)),
            ("value", ArgValue::String("bee".into())),
        ]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-grid-delete-row",
        &args(&[("row", ArgValue::Integer(2))]),
    )
    .unwrap();
    execute_command(&mut ed, "db-grid-new-row", &args(&[])).unwrap();
    execute_command(
        &mut ed,
        "db-grid-new-cell",
        &args(&[
            ("column", ArgValue::Integer(1)),
            ("value", ArgValue::String("dee".into())),
        ]),
    )
    .unwrap();

    let tab = ed.db.tab(tab_id).unwrap();
    assert_eq!(tab.pending.len(), 3);

    execute_command(&mut ed, "db-grid-apply", &args(&[])).unwrap();
    assert!(ed.db.tab(tab_id).unwrap().pending.is_empty());

    // The page refetched after apply shows the new contents.
    let rows = ed.db.tab(tab_id).unwrap().result().unwrap().rows.clone();
    assert_eq!(rows.len(), 3, "2 originals − 1 delete + 1 insert");
    assert_eq!(rows[1][1], db::DbValue::Text("bee".into()));
    assert_eq!(rows[2][1], db::DbValue::Text("dee".into()));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn grid_revert_drops_pending_edits() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "revert");

    execute_command(
        &mut ed,
        "db-open-table",
        &args(&[("path", ArgValue::String("/main/main/tables/items".into()))]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-grid-edit-cell",
        &args(&[
            ("row", ArgValue::Integer(0)),
            ("column", ArgValue::Integer(1)),
            ("value", ArgValue::String("zzz".into())),
        ]),
    )
    .unwrap();
    assert!(!ed.db.active_tab().unwrap().pending.is_empty());
    execute_command(&mut ed, "db-grid-revert", &args(&[])).unwrap();
    assert!(ed.db.active_tab().unwrap().pending.is_empty());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn views_and_keyless_tables_refuse_edits() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "readonly");
    {
        let conn = ed.db.connections.values().next().unwrap().clone();
        conn.execute_script("CREATE VIEW vw AS SELECT id FROM items", 10)
            .unwrap();
        conn.execute_script("CREATE TABLE keyless (a TEXT, b TEXT)", 10)
            .unwrap();
    }
    execute_command(
        &mut ed,
        "db-open-table",
        &args(&[("path", ArgValue::String("/main/main/views/vw".into()))]),
    )
    .unwrap();
    let err = execute_command(
        &mut ed,
        "db-grid-edit-cell",
        &args(&[
            ("row", ArgValue::Integer(0)),
            ("column", ArgValue::Integer(0)),
            ("value", ArgValue::String("1".into())),
        ]),
    )
    .unwrap_err();
    assert!(err.contains("read-only"), "got: {err}");

    execute_command(
        &mut ed,
        "db-open-table",
        &args(&[("path", ArgValue::String("/main/main/tables/keyless".into()))]),
    )
    .unwrap();
    let err = execute_command(
        &mut ed,
        "db-grid-edit-cell",
        &args(&[
            ("row", ArgValue::Integer(0)),
            ("column", ArgValue::Integer(0)),
            ("value", ArgValue::String("x".into())),
        ]),
    )
    .unwrap_err();
    assert!(err.contains("primary key"), "got: {err}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn sort_and_filter_refetch_the_page() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "sort");

    execute_command(
        &mut ed,
        "db-open-table",
        &args(&[("path", ArgValue::String("/main/main/tables/items".into()))]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-grid-sort",
        &args(&[("column", ArgValue::Integer(0))]),
    )
    .unwrap();
    // Sort cycles asc → desc; after one click the page refetched asc —
    // same rows for this data, so assert through the second click: none.
    execute_command(
        &mut ed,
        "db-grid-sort",
        &args(&[("column", ArgValue::Integer(0))]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-grid-sort",
        &args(&[("column", ArgValue::Integer(0))]),
    )
    .unwrap();
    assert_eq!(ed.db.active_tab().unwrap().sort, None);

    execute_command(
        &mut ed,
        "db-grid-filter",
        &args(&[("filter", ArgValue::String("id = 2".into()))]),
    )
    .unwrap();
    let rows = ed.db.active_tab().unwrap().result().unwrap().rows.clone();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], db::DbValue::Integer(2));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn concurrent_modification_refuses_the_apply() {
    let mut ed = editor();
    let (_id, path) = connected_sqlite(&mut ed, "conflict");

    execute_command(
        &mut ed,
        "db-open-table",
        &args(&[("path", ArgValue::String("/main/main/tables/items".into()))]),
    )
    .unwrap();
    execute_command(
        &mut ed,
        "db-grid-edit-cell",
        &args(&[
            ("row", ArgValue::Integer(0)),
            ("column", ArgValue::Integer(1)),
            ("value", ArgValue::String("changed".into())),
        ]),
    )
    .unwrap();
    // Someone else wins the race behind the editor's back.
    {
        let conn = ed.db.connections.values().next().unwrap().clone();
        conn.execute_script(
            "UPDATE items SET label = 'stolen' WHERE id = 1",
            10,
        )
        .unwrap();
    }
    let err = execute_command(&mut ed, "db-grid-apply", &args(&[])).unwrap_err();
    assert!(err.contains("concurrently"), "got: {err}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn connect_failure_marks_the_tree_root_failed() {
    let mut ed = editor();
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("bad".into())),
            ("kind", ArgValue::String("sqlite".into())),
            (
                "host",
                ArgValue::Path("/nonexistent-dir-xyz/nope.sqlite".into()),
            ),
        ]),
    )
    .unwrap();
    let id = ed.db.active.unwrap();
    // SQLite creates missing files when the parent exists; this parent
    // does not, so connect fails.
    let _ = execute_command(
        &mut ed,
        "db-connect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    );
    assert!(!ed.db.is_connected(id));
    let root = ed.db.tree(id).node("/").unwrap();
    assert!(
        matches!(root.status, NodeStatus::Failed(_)),
        "the root carries the failure: {:?}",
        root.status
    );
    assert!(ed.db.last_error.is_some());
}

#[test]
fn disconnect_drops_the_tree() {
    let mut ed = editor();
    let (id, path) = connected_sqlite(&mut ed, "disconnect");
    execute_command(
        &mut ed,
        "db-disconnect",
        &args(&[("source", ArgValue::Integer(id as i64))]),
    )
    .unwrap();
    assert!(!ed.db.is_connected(id));
    assert!(ed.db.tree(id).node("/").is_none(), "tree cache dropped");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn persistence_round_trips_connection_facts_without_secrets() {
    let mut ed = editor();
    execute_command(
        &mut ed,
        "db-source-add",
        &args(&[
            ("name", ArgValue::String("pg".into())),
            ("kind", ArgValue::String("postgres".into())),
            ("host", ArgValue::String("db.example.com".into())),
            ("port", ArgValue::Integer(5433)),
            ("database", ArgValue::String("app".into())),
            ("user", ArgValue::String("app".into())),
            ("password", ArgValue::String("hunter2".into())),
            ("save-password", ArgValue::Boolean(true)),
            ("ssl", ArgValue::String("require".into())),
        ]),
    )
    .unwrap();
    let id = ed.db.active.unwrap();
    let snapshot = crate::kernel::storage::workspace::WorkspaceManager::snapshot(&ed);
    let persisted = &snapshot.data_sources[0];
    assert_eq!(persisted.kind, "postgres");
    assert_eq!(persisted.host, "db.example.com");
    assert_eq!(persisted.port, 5433);
    assert_eq!(persisted.database, "app");
    assert_eq!(persisted.user, "app");
    assert_eq!(persisted.ssl_mode, "require");
    assert!(persisted.save_password);

    // The password itself never lands in the snapshot.
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(!json.contains("hunter2"), "secrets stay out of config");

    // And the source survives a restore.
    let mut restored = editor();
    crate::kernel::storage::workspace::WorkspaceManager::apply(&mut restored, &snapshot)
        .unwrap();
    assert_eq!(restored.db.sources.len(), 1);
    let src = &restored.db.sources[0];
    assert_eq!(src.kind, db::DriverKind::Postgres);
    assert_eq!(src.ssl_mode, db::SslMode::Require);
    assert_eq!(src.id, id);
}

#[test]
fn console_buffers_associate_with_their_source() {
    let mut ed = editor();
    let (id, path) = connected_sqlite(&mut ed, "console");
    execute_command(&mut ed, "db-console", &args(&[])).unwrap();
    let key = ed.focused_buffer_key();
    assert_eq!(ed.db.console_source(key), Some(id));
    let arc = ed.buffers.get(key).unwrap();
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.name, "*sql: demo*");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn node_keys_round_trip() {
    for key in [
        "/",
        "/main",
        "/main/main",
        "/main/main/tables",
        "/main/main/tables/items",
    ] {
        let parsed = NodeRef::parse(key).unwrap_or(NodeRef::Root);
        assert_eq!(parsed.key(), key, "round trip for {key}");
    }
}
