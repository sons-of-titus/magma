//! SQLite engine: metadata through `sqlite_master` and PRAGMAs, bounded
//! script execution, transactional table edits, and interruptible long
//! statements.  The tree is uniform with the networked drivers: connection
//! → database → schema → folders → objects.

use std::sync::Arc;
use std::time::Instant;

use rusqlite::Connection;

use crate::{
    Child, ColumnMeta, Connection as DbConnection, Dialect, DbEngine, DbValue, EditResult,
    FolderKind, ForeignKeyMeta, IndexMeta, NodeRef, PendingEdits, QueryOutcome, TableKind,
    TableMeta, split_statements,
};

const MAX_ROWS_EDIT_GUARD: &str = "no rows changed — the row was modified concurrently";

struct SqliteEngine {
    conn: Connection,
}

impl DbEngine for SqliteEngine {
    fn introspect(&mut self, node: &NodeRef) -> Result<Vec<Child>, String> {
        let conn = &self.conn;
        match node {
            NodeRef::Root => Ok(vec![Child::Database {
                name: "main".to_string(),
                label: "main".to_string(),
            }]),
            NodeRef::Database(_) => {
                let mut stmt = conn
                    .prepare("PRAGMA database_list")
                    .map_err(|e| e.to_string())?;
                let schemas: Vec<String> = stmt
                    .query_map([], |row| row.get::<_, String>(1))
                    .map_err(|e| e.to_string())?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(schemas.into_iter().map(|name| Child::Schema { name }).collect())
            }
            NodeRef::Schema { .. } => Ok(vec![
                Child::Folder { kind: FolderKind::Tables },
                Child::Folder { kind: FolderKind::Views },
            ]),
            NodeRef::Folder { kind, .. } => {
                let wanted = match kind {
                    FolderKind::Tables => "table",
                    FolderKind::Views => "view",
                    _ => return Ok(Vec::new()),
                };
                let mut stmt = conn
                    .prepare(
                        "SELECT name FROM sqlite_master \
                         WHERE type = ?1 AND name NOT LIKE 'sqlite_%' ORDER BY name",
                    )
                    .map_err(|e| e.to_string())?;
                let names: Vec<String> = stmt
                    .query_map([wanted], |row| row.get(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(names
                    .into_iter()
                    .map(|name| Child::Table {
                        name,
                        kind: if *kind == FolderKind::Tables {
                            TableKind::Table
                        } else {
                            TableKind::View
                        },
                    })
                    .collect())
            }
            NodeRef::Table { .. } => {
                let detail = self.table_detail(node)?;
                let mut children: Vec<Child> = detail
                    .columns
                    .iter()
                    .map(|c| Child::Column { meta: c.clone() })
                    .collect();
                for meta in detail.foreign_keys {
                    children.push(Child::ForeignKey { meta });
                }
                for meta in detail.indexes {
                    children.push(Child::Index { meta });
                }
                Ok(children)
            }
        }
    }

    fn table_detail(&mut self, node: &NodeRef) -> Result<TableMeta, String> {
        let NodeRef::Table { database, schema, name, is_view } = node else {
            return Err("not a table node".to_string());
        };
        let conn = &self.conn;
        let columns = table_columns(conn, name)?;
        let primary_key: Vec<String> = columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name.clone())
            .collect();
        let foreign_keys = table_foreign_keys(conn, name);
        let indexes = if *is_view { Vec::new() } else { table_indexes(conn, name) };
        Ok(TableMeta {
            database: database.clone(),
            schema: schema.clone(),
            name: name.clone(),
            kind: if *is_view { TableKind::View } else { TableKind::Table },
            columns,
            primary_key,
            foreign_keys,
            indexes,
        })
    }

    fn execute_script(&mut self, sql: &str, max_rows: usize) -> Result<Vec<QueryOutcome>, String> {
        let conn = &self.conn;
        let mut outcomes = Vec::new();
        for stmt in split_statements(sql) {
            let started = Instant::now();
            if is_select_shaped(&stmt.text) {
                let mut prepared = conn.prepare(&stmt.text).map_err(|e| classify(&e))?;
                let column_count = prepared.column_count();
                let column_names: Vec<String> = (0..column_count)
                    .map(|i| prepared.column_name(i).unwrap_or("").to_string())
                    .collect();
                if prepared.readonly() && column_count > 0 {
                    let mut outcome = QueryOutcome {
                        statement: stmt.text.clone(),
                        is_query: true,
                        ..Default::default()
                    };
                    let mut rows = prepared.query([]).map_err(|e| classify(&e))?;
                    while let Some(row) = rows.next().map_err(|e| classify(&e))? {
                        if outcome.rows.len() >= max_rows {
                            outcome.truncated = true;
                            break;
                        }
                        let mut cells = Vec::with_capacity(column_count);
                        for i in 0..column_count {
                            cells.push(read_value(row, i)?);
                        }
                        outcome.rows.push(cells);
                    }
                    outcome.columns = column_names
                        .into_iter()
                        .map(|name| crate::ColumnOut { name, type_name: String::new() })
                        .collect();
                    outcome.elapsed_ms = started.elapsed().as_millis() as u64;
                    outcomes.push(outcome);
                    continue;
                }
            }
            conn.execute_batch(&stmt.text).map_err(|e| classify(&e))?;
            outcomes.push(QueryOutcome {
                statement: stmt.text.clone(),
                rows_affected: conn.changes() as i64,
                is_query: false,
                elapsed_ms: started.elapsed().as_millis() as u64,
                ..Default::default()
            });
        }
        Ok(outcomes)
    }

    fn apply_edits(&mut self, table: &TableMeta, edits: &PendingEdits) -> Result<EditResult, String> {
        let statements = edits.statements(table, Dialect::Sqlite)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        let mut result = EditResult::default();
        for (i, sql) in statements.iter().enumerate() {
            let changed = tx.execute(sql, []).map_err(|e| e.to_string())?;
            if changed == 0 && !matches!(edits.ops[i], crate::EditOp::InsertRow { .. }) {
                return Err(MAX_ROWS_EDIT_GUARD.to_string());
            }
            result.rows_affected += changed as i64;
            result.statements_run += 1;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }

    fn server_version(&self) -> String {
        format!("SQLite {}", rusqlite::version())
    }

    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }
}

pub struct SqliteConnection;

impl SqliteConnection {
    pub fn open(path: &str) -> Result<DbConnection, String> {
        if path.trim().is_empty() {
            return Err("SQLite needs a database file path".to_string());
        }
        let conn = Connection::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        // Foreign-key enforcement is off by default in SQLite; the tool
        // should behave like a strict client.
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        let interrupt = Arc::new(conn.get_interrupt_handle());
        let engine = SqliteEngine { conn };
        let label = format!("SQLite {}", rusqlite::version());
        Ok(DbConnection::new(
            Box::new(engine),
            Box::new(move || interrupt.interrupt()),
            label,
        ))
    }
}

fn is_select_shaped(sql: &str) -> bool {
    let mut rest = sql.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix("--") {
            rest = after.split_once('\n').map(|(_, tail)| tail).unwrap_or("");
            rest = rest.trim_start();
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").map(|(_, tail)| tail).unwrap_or("");
            rest = rest.trim_start();
        } else {
            break;
        }
    }
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let word = word.to_ascii_lowercase();
    matches!(
        word.as_str(),
        "select" | "values" | "explain" | "pragma" | "with" | "table"
    )
}

fn classify(e: &rusqlite::Error) -> String {
    if let rusqlite::Error::SqliteFailure(ffi, _) = e
        && ffi.code == rusqlite::ErrorCode::OperationInterrupted
    {
        return "query interrupted".to_string();
    }
    if let rusqlite::Error::SqliteFailure(_, Some(msg)) = e {
        return msg.clone();
    }
    e.to_string()
}

fn read_value(row: &rusqlite::Row<'_>, i: usize) -> Result<DbValue, String> {
    let v = row.get_ref(i).map_err(|e| e.to_string())?;
    Ok(match v {
        rusqlite::types::ValueRef::Null => DbValue::Null,
        rusqlite::types::ValueRef::Integer(i) => DbValue::Integer(i),
        rusqlite::types::ValueRef::Real(f) => DbValue::Real(f),
        rusqlite::types::ValueRef::Text(t) => {
            DbValue::Text(String::from_utf8_lossy(t).into_owned())
        }
        rusqlite::types::ValueRef::Blob(b) => DbValue::Blob(b.to_vec()),
    })
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<ColumnMeta>, String> {
    let d = Dialect::Sqlite;
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", d.quote_ident(table)))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let pk_pos: i64 = row.get(5)?;
            Ok(ColumnMeta {
                name: row.get(1)?,
                data_type: row.get::<_, String>(2).unwrap_or_default(),
                nullable: row.get::<_, i64>(3)? == 0,
                default: row.get::<_, Option<String>>(4)?,
                primary_key: pk_pos > 0,
                auto_increment: false,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn table_foreign_keys(conn: &Connection, table: &str) -> Vec<ForeignKeyMeta> {
    let d = Dialect::Sqlite;
    let Ok(mut stmt) = conn.prepare(&format!(
        "PRAGMA foreign_key_list({})",
        d.quote_ident(table)
    )) else {
        return Vec::new();
    };
    struct RawFk {
        id: i64,
        table: String,
        from: String,
        to: Option<String>,
    }
    let raw: Vec<RawFk> = stmt
        .query_map([], |row| {
            Ok(RawFk {
                id: row.get(0)?,
                table: row.get(2)?,
                from: row.get(3)?,
                to: row.get(4)?,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect();
    raw.into_iter()
        .map(|fk| {
            let ref_col = fk.to.filter(|s| !s.is_empty()).unwrap_or_else(|| {
                table_columns(conn, &fk.table)
                    .ok()
                    .and_then(|cols| cols.into_iter().find(|c| c.primary_key).map(|c| c.name))
                    .unwrap_or_default()
            });
            ForeignKeyMeta {
                name: format!("fk_{}", fk.id),
                columns: vec![fk.from],
                ref_schema: "main".to_string(),
                ref_table: fk.table,
                ref_columns: vec![ref_col],
            }
        })
        .collect()
}

fn table_indexes(conn: &Connection, table: &str) -> Vec<IndexMeta> {
    let d = Dialect::Sqlite;
    let Ok(mut stmt) = conn.prepare(&format!(
        "PRAGMA index_list({})",
        d.quote_ident(table)
    )) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0))
    }) else {
        return Vec::new();
    };
    let indexes: Vec<(String, bool)> = rows
        .filter_map(|r| r.ok())
        .filter(|(name, _)| !name.starts_with("sqlite_autoindex_"))
        .collect();

    indexes
        .into_iter()
        .map(|(name, unique)| {
            let columns = conn
                .prepare(&format!(
                    "PRAGMA index_info({})",
                    d.quote_ident(&name)
                ))
                .and_then(|mut s| {
                    s.query_map([], |row| row.get::<_, String>(2))
                        .map(|it| it.filter_map(|c| c.ok()).collect::<Vec<String>>())
                })
                .unwrap_or_default();
            IndexMeta { name, unique, columns }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataSource, DriverKind};

    fn source(path: &str) -> DataSource {
        DataSource {
            id: 1,
            name: "test".into(),
            kind: DriverKind::Sqlite,
            host: path.into(),
            port: 0,
            database: String::new(),
            user: String::new(),
            ssl_mode: Default::default(),
            trust_cert: false,
            save_password: false,
            url: None,
        }
    }

    fn demo_connection() -> DbConnection {
        let conn = SqliteConnection::open(":memory:").unwrap();
        conn.execute_script(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL); \
             INSERT INTO users VALUES (1, 'ada', 9.5), (2, 'grace', NULL), (3, '', 1.0);",
            10,
        )
        .unwrap();
        conn
    }

    #[test]
    fn open_rejects_empty_path() {
        assert!(SqliteConnection::open("").is_err());
        assert!(SqliteConnection::open("   ").is_err());
    }

    #[test]
    fn query_returns_bounded_rows_with_kinds() {
        let conn = demo_connection();
        let outs = conn
            .execute_script("SELECT * FROM users ORDER BY id", 2)
            .unwrap();
        let out = &outs[0];
        assert_eq!(
            out.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["id", "name", "score"]
        );
        assert_eq!(out.rows.len(), 2);
        assert!(out.truncated, "2 of 3 rows means the fetch was capped");
        assert_eq!(out.rows[0][1], DbValue::Text("ada".into()));
        assert_eq!(out.rows[1][2], DbValue::Null);
        assert!(out.elapsed_ms < 10_000);
    }

    #[test]
    fn null_and_empty_text_stay_distinct() {
        let conn = demo_connection();
        let out = &conn
            .execute_script("SELECT name, score FROM users ORDER BY id", 10)
            .unwrap()[0];
        assert_eq!(out.rows[1][1], DbValue::Null);
        assert_eq!(out.rows[2][0], DbValue::Text(String::new()));
        assert_eq!(out.rows[1][1].render(), "NULL");
        assert_eq!(out.rows[2][0].render(), "");
    }

    #[test]
    fn writes_report_affected_rows_without_result_rows() {
        let conn = demo_connection();
        let out = &conn
            .execute_script("UPDATE users SET score = 0 WHERE score IS NULL", 10)
            .unwrap()[0];
        assert!(!out.is_query);
        assert_eq!(out.rows_affected, 1);
        assert!(out.rows.is_empty());
    }

    #[test]
    fn script_runs_every_statement_and_stops_at_error() {
        let conn = demo_connection();
        let outs = conn
            .execute_script(
                "CREATE TABLE t (a); INSERT INTO t VALUES (1); INSERT INTO t VALUES (2); SELECT count(a) FROM t",
                10,
            )
            .unwrap();
        assert_eq!(outs.len(), 4);
        assert_eq!(outs[3].rows[0][0], DbValue::Integer(2));
        let err = conn
            .execute_script("SELECT * FROM missing_table", 10)
            .unwrap_err();
        assert!(err.contains("missing_table"), "got: {err}");
    }

    #[test]
    fn tree_walks_database_schema_folders_tables() {
        let conn = demo_connection();
        let dbs = conn.introspect(&NodeRef::Root).unwrap();
        assert_eq!(dbs.len(), 1);
        let Child::Database { name, .. } = &dbs[0] else { panic!() };
        let schemas = conn.introspect(&NodeRef::Database(name.clone())).unwrap();
        assert!(schemas.iter().any(|c| matches!(c, Child::Schema { name } if name == "main")));
        let folders = conn
            .introspect(&NodeRef::Schema { database: name.clone(), schema: "main".into() })
            .unwrap();
        assert_eq!(folders.len(), 2);
        let tables = conn
            .introspect(&NodeRef::Folder {
                database: name.clone(),
                schema: "main".into(),
                kind: FolderKind::Tables,
            })
            .unwrap();
        assert_eq!(tables.len(), 1);
    }

    #[test]
    fn table_node_expands_columns_indexes_and_foreign_keys() {
        let conn = demo_connection();
        conn.execute_script("CREATE INDEX idx_users_name ON users(name)", 10)
            .unwrap();
        let children = conn
            .introspect(&NodeRef::Table {
                database: "main".into(),
                schema: "main".into(),
                name: "users".into(),
                is_view: false,
            })
            .unwrap();
        assert!(children.iter().any(|c| matches!(c, Child::Column { meta } if meta.primary_key)));
        assert!(children
            .iter()
            .any(|c| matches!(c, Child::Index { meta } if meta.columns == vec!["name"])));

        let detail = conn
            .table_detail(&NodeRef::Table {
                database: "main".into(),
                schema: "main".into(),
                name: "users".into(),
                is_view: false,
            })
            .unwrap();
        assert_eq!(detail.primary_key, vec!["id"]);
        assert!(detail.pk_columns().is_some());
    }

    #[test]
    fn views_are_listed_as_views() {
        let conn = demo_connection();
        conn.execute_script("CREATE VIEW adults AS SELECT * FROM users WHERE score > 0", 10)
            .unwrap();
        let views = conn
            .introspect(&NodeRef::Folder {
                database: "main".into(),
                schema: "main".into(),
                kind: FolderKind::Views,
            })
            .unwrap();
        assert_eq!(views.len(), 1);
    }

    #[test]
    fn test_connection_probes_a_real_file() {
        let dir = std::env::temp_dir().join(format!(
            "magma-db-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe.sqlite");
        assert!(crate::test_connection(&source(&path.to_string_lossy()), None).is_ok());
        assert!(
            crate::test_connection(&source(&dir.join("no/dir/here/x.sqlite").to_string_lossy()), None)
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn interrupt_cancels_a_running_query() {
        let conn = std::sync::Arc::new(SqliteConnection::open(":memory:").unwrap());
        let query_conn = std::sync::Arc::clone(&conn);
        let handle = std::thread::spawn(move || {
            query_conn.execute_script(
                "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) \
                 SELECT count(x) FROM c",
                10,
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(150));
        conn.interrupt();
        let result = handle.join().unwrap();
        assert_eq!(result.unwrap_err(), "query interrupted");
    }

    #[test]
    fn apply_edits_updates_inserts_and_deletes_in_one_transaction() {
        let conn = demo_connection();
        let detail = conn
            .table_detail(&NodeRef::Table {
                database: "main".into(),
                schema: "main".into(),
                name: "users".into(),
                is_view: false,
            })
            .unwrap();
        let page = conn
            .execute_script("SELECT id, name, score FROM users ORDER BY id", 10)
            .unwrap()[0]
            .rows
            .clone();
        let edits = PendingEdits {
            ops: vec![
                crate::EditOp::UpdateCell {
                    row: 1,
                    col: 1,
                    value: DbValue::Text("turing".into()),
                    original_row: page[1].clone(),
                },
                crate::EditOp::DeleteRow {
                    row: 2,
                    original_row: page[2].clone(),
                },
                crate::EditOp::InsertRow {
                    values: vec![DbValue::Null, DbValue::Text("knuth".into()), DbValue::Real(3.5)],
                },
            ],
        };
        let result = conn.apply_edits(&detail, &edits).unwrap();
        assert_eq!(result.statements_run, 3);
        let check = &conn
            .execute_script("SELECT name FROM users ORDER BY id", 10)
            .unwrap()[0];
        assert_eq!(check.rows.len(), 3);
        assert_eq!(check.rows[1][0], DbValue::Text("turing".into()));
        assert_eq!(check.rows[2][0], DbValue::Text("knuth".into()));
    }

    #[test]
    fn apply_edits_detects_concurrent_modification() {
        let conn = demo_connection();
        let detail = conn
            .table_detail(&NodeRef::Table {
                database: "main".into(),
                schema: "main".into(),
                name: "users".into(),
                is_view: false,
            })
            .unwrap();
        // Original row claims a name that no longer matches the database.
        let edits = PendingEdits {
            ops: vec![crate::EditOp::UpdateCell {
                row: 0,
                col: 1,
                value: DbValue::Text("x".into()),
                original_row: vec![DbValue::Integer(1), DbValue::Text("stale".into()), DbValue::Null],
            }],
        };
        let err = conn.apply_edits(&detail, &edits).unwrap_err();
        assert!(err.contains("concurrently"), "got: {err}");
    }

    #[test]
    fn connect_dispatches_by_kind() {
        let src = source(":memory:");
        assert!(crate::connect(&src, None).is_ok());
        assert_eq!(src.kind.label(), "SQLite");
        assert_eq!(DriverKind::parse("sqlite"), Some(DriverKind::Sqlite));
        assert_eq!(DriverKind::parse("oracle"), None);
    }

    #[test]
    fn endpoint_label_matches_engine_family() {
        assert_eq!(source("/tmp/x.sqlite").endpoint_label(), "/tmp/x.sqlite");
    }
}
