//! SQLite engine: metadata through `sqlite_master` and PRAGMAs, bounded
//! query execution, and interruptible long statements.

use std::sync::Arc;
use std::time::Instant;

use rusqlite::Connection;

use crate::{ColumnMeta, DbConnection, DbEngine, DbValue, IndexMeta, QueryOutcome, TableMeta};

/// The engine half: statements run against one connection behind the
/// shared engine lock; the interrupt handle sits outside that lock.
struct SqliteEngine {
    conn: Connection,
}

impl DbEngine for SqliteEngine {
    fn tables(&mut self) -> Result<Vec<TableMeta>, String> {
        let conn = &self.conn;
        let mut stmt = conn
            .prepare(
                "SELECT name, type FROM sqlite_master \
                 WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' \
                 ORDER BY type, name",
            )
            .map_err(|e| e.to_string())?;
        let names: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;

        let mut tables = Vec::new();
        for (name, kind) in names {
            let columns = table_columns(&conn, &name)?;
            let indexes = if kind == "table" {
                table_indexes(&conn, &name)
            } else {
                Vec::new()
            };
            tables.push(TableMeta {
                is_view: kind == "view",
                name,
                columns,
                indexes,
            });
        }
        Ok(tables)
    }

    fn execute(&mut self, sql: &str, max_rows: usize) -> Result<QueryOutcome, String> {
        let conn = &self.conn;
        let started = Instant::now();
        let mut outcome = QueryOutcome::default();

        // Queries prepare and fetch; everything else (writes, DDL,
        // multi-statement scripts) runs through execute_batch, which needs
        // no prepare.  rusqlite's prepare refuses multi-statement text, so
        // the split is by leading keyword, and a WITH that turns out to be
        // a write falls through to script mode.
        if matches!(
            leading_keyword(sql).as_str(),
            "select" | "values" | "explain" | "pragma" | "with"
        ) {
            let mut prepared = conn.prepare(sql).map_err(|e| classify(&e))?;
            let column_count = prepared.column_count();
            if prepared.readonly() && column_count > 0 {
                let column_names: Vec<String> = (0..column_count)
                    .map(|i| prepared.column_name(i).unwrap_or("").to_string())
                    .collect();
                let mut rows_out = Vec::new();
                let mut rows = prepared.query([]).map_err(|e| classify(&e))?;
                while let Some(row) = rows.next().map_err(|e| classify(&e))? {
                    if rows_out.len() >= max_rows {
                        outcome.truncated = true;
                        break;
                    }
                    let mut cells = Vec::with_capacity(column_count);
                    for i in 0..column_count {
                        cells.push(read_value(row, i)?);
                    }
                    rows_out.push(cells);
                }
                outcome.columns = column_names;
                outcome.rows = rows_out;
                outcome.is_query = true;
                outcome.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(outcome);
            }
        }

        conn.execute_batch(sql).map_err(|e| classify(&e))?;
        outcome.rows_affected = conn.changes() as i64;
        outcome.is_query = false;
        outcome.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(outcome)
    }

    fn label(&self) -> &'static str {
        "SQLite"
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
        Ok(DbConnection::new(
            Box::new(engine),
            Box::new(move || interrupt.interrupt()),
        ))
    }
}

/// The first bare keyword of a statement, lowercased; comments and
/// whitespace are skipped.
fn leading_keyword(sql: &str) -> String {
    let mut rest = sql.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix("--") {
            rest = after.split_once('\n').map(|(_, tail)| tail).unwrap_or("");
            rest = rest.trim_start();
        } else {
            break;
        }
    }
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    word.to_ascii_lowercase()
}

fn classify(e: &rusqlite::Error) -> String {
    match e {
        rusqlite::Error::SqliteFailure(ffi, msg)
            if ffi.code == rusqlite::ErrorCode::OperationInterrupted =>
        {
            "query interrupted".to_string()
        }
        _ => {
            let text = e.to_string();
            match msg_of(e) {
                Some(m) => m,
                None => text,
            }
        }
    }
}

fn msg_of(e: &rusqlite::Error) -> Option<String> {
    if let rusqlite::Error::SqliteFailure(_, msg) = e {
        msg.clone()
    } else {
        None
    }
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
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", crate::quote_ident(table)))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ColumnMeta {
                name: row.get::<_, String>(1)?,
                declared_type: row.get::<_, String>(2).unwrap_or_default(),
                not_null: row.get::<_, i64>(3)? != 0,
                primary_key: row.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn table_indexes(conn: &Connection, table: &str) -> Vec<IndexMeta> {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA index_list({})", crate::quote_ident(table)))
    else {
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
                .prepare(&format!("PRAGMA index_info({})", crate::quote_ident(&name)))
                .and_then(|mut s| {
                    s.query_map([], |row| row.get::<_, String>(2))
                        .map(|it| it.filter_map(|c| c.ok()).collect::<Vec<String>>())
                })
                .unwrap_or_default();
            IndexMeta {
                name,
                unique,
                columns,
            }
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
            path: path.into(),
        }
    }

    fn demo_connection() -> DbConnection {
        let conn = SqliteConnection::open(":memory:").unwrap();
        conn.execute(
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
        let out = conn.execute("SELECT * FROM users ORDER BY id", 2).unwrap();
        assert_eq!(out.columns, vec!["id", "name", "score"]);
        assert_eq!(out.rows.len(), 2);
        assert!(out.truncated, "2 of 3 rows means the fetch was capped");
        assert_eq!(out.rows[0][1], DbValue::Text("ada".into()));
        assert_eq!(out.rows[1][2], DbValue::Null);
        assert!(out.elapsed_ms < 10_000);
    }

    #[test]
    fn null_and_empty_text_stay_distinct() {
        let conn = demo_connection();
        let out = conn
            .execute("SELECT name, score FROM users ORDER BY id", 10)
            .unwrap();
        assert_eq!(out.rows[1][1], DbValue::Null);
        assert_eq!(out.rows[2][0], DbValue::Text(String::new()));
        assert_eq!(out.rows[1][1].render(), "NULL");
        assert_eq!(out.rows[2][0].render(), "");
    }

    #[test]
    fn writes_report_affected_rows_without_result_rows() {
        let conn = demo_connection();
        let out = conn
            .execute("UPDATE users SET score = 0 WHERE score IS NULL", 10)
            .unwrap();
        assert!(!out.is_query);
        assert_eq!(out.rows_affected, 1);
        assert!(out.rows.is_empty());
    }

    #[test]
    fn errors_surface_the_database_message() {
        let conn = demo_connection();
        let err = conn.execute("SELECT * FROM missing_table", 10).unwrap_err();
        assert!(err.contains("missing_table"), "got: {err}");
    }

    #[test]
    fn tables_list_columns_and_indexes() {
        let conn = demo_connection();
        conn.execute("CREATE INDEX idx_users_name ON users(name)", 10)
            .unwrap();
        let tables = conn.tables().unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        assert!(!users.is_view);
        assert_eq!(users.columns.len(), 3);
        assert!(users.columns[0].primary_key);
        assert!(users.columns[1].not_null);
        assert_eq!(users.indexes.len(), 1);
        assert_eq!(users.indexes[0].columns, vec!["name"]);
    }

    #[test]
    fn views_are_listed_as_views() {
        let conn = demo_connection();
        conn.execute(
            "CREATE VIEW adults AS SELECT * FROM users WHERE score > 0",
            10,
        )
        .unwrap();
        let tables = conn.tables().unwrap();
        let view = tables.iter().find(|t| t.name == "adults").unwrap();
        assert!(view.is_view);
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
        assert!(crate::test_connection(&source(&path.to_string_lossy())).is_ok());
        assert!(
            crate::test_connection(&source(&dir.join("no/dir/here/x.sqlite").to_string_lossy()))
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn interrupt_cancels_a_running_query() {
        let conn = std::sync::Arc::new(SqliteConnection::open(":memory:").unwrap());
        let query_conn = std::sync::Arc::clone(&conn);
        let handle = std::thread::spawn(move || {
            // A count over an unbounded recursion would run for hours.
            query_conn.execute(
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
    fn select_all_quotes_identifiers() {
        let conn = SqliteConnection::open(":memory:").unwrap();
        conn.execute("CREATE TABLE \"odd name\" (a INTEGER)", 10)
            .unwrap();
        let out = conn
            .execute(&crate::select_all("odd name", 10), 10)
            .unwrap();
        assert_eq!(out.columns, vec!["a"]);
    }

    #[test]
    fn connect_dispatches_by_kind() {
        let mut src = source(":memory:");
        assert!(crate::connect(&src).is_ok());
        src.kind = DriverKind::Sqlite;
        assert_eq!(src.kind.label(), "SQLite");
        assert_eq!(DriverKind::parse("sqlite"), Some(DriverKind::Sqlite));
        assert_eq!(DriverKind::parse("oracle"), None);
    }
}
