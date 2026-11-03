//! MySQL / MariaDB engine over mysql_async: catalog introspection through
//! information_schema, bounded script execution, KILL QUERY cancellation,
//! and transactional table edits.  One engine covers both families — they
//! share the wire protocol and system catalogs.

use std::time::{Duration, Instant};

use mysql_async::prelude::*;
use mysql_async::{Conn, Opts, OptsBuilder, SslOpts, Value};

use crate::{
    Child, ColumnMeta, Connection as DbConnection, Dialect, DbEngine, DbValue, DataSource,
    EditResult, FolderKind, ForeignKeyMeta, FunctionMeta, IndexMeta, NodeRef, PendingEdits,
    QueryOutcome, TableKind, TableMeta, block_on, split_statements,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONFLICT: &str = "no rows changed — the row was modified concurrently";
const SYSTEM_SCHEMAS: [&str; 4] = ["information_schema", "mysql", "performance_schema", "sys"];

struct MySqlEngine {
    conn: Conn,
    version: String,
}

pub struct MySqlConnection;

impl MySqlConnection {
    pub fn open(source: &DataSource, password: Option<&str>) -> Result<DbConnection, String> {
        if source.host.trim().is_empty() {
            return Err("MySQL needs a host".to_string());
        }
        let database = source.database.trim().to_string();
        let ssl = ssl_opts(source);
        let builder = OptsBuilder::default()
            .ip_or_hostname(source.host.clone())
            .tcp_port(if source.port == 0 { 3306 } else { source.port })
            .prefer_socket(false)
            .user(if source.user.is_empty() { None } else { Some(source.user.clone()) })
            .pass(password)
            .db_name(if database.is_empty() { None } else { Some(database.clone()) })
            .ssl_opts(ssl.clone());
        let opts: Opts = builder.into();
        let mut conn = block_on(async {
            tokio::time::timeout(CONNECT_TIMEOUT, Conn::new(opts.clone()))
                .await
                .map_err(|_| "connection timed out".to_string())?
                .map_err(err)
        })
        // `Prefer` degrades to plaintext after one TLS failure.
        .or_else(|first| {
            if source.ssl_mode == crate::SslMode::Prefer && ssl.is_some() {
                let plain: Opts = OptsBuilder::default()
                    .ip_or_hostname(source.host.clone())
                    .tcp_port(if source.port == 0 { 3306 } else { source.port })
                    .prefer_socket(false)
                    .user(if source.user.is_empty() { None } else { Some(source.user.clone()) })
                    .pass(password)
                    .db_name(if database.is_empty() { None } else { Some(database.clone()) })
                    .into();
                block_on(async {
                    tokio::time::timeout(CONNECT_TIMEOUT, Conn::new(plain))
                        .await
                        .map_err(|_| "connection timed out".to_string())?
                        .map_err(err)
                })
            } else {
                Err(first)
            }
        })?;
        let server_id = conn.id();
        let version = block_on(async {
            conn.query_first::<String, _>("SELECT VERSION()")
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        });
        let family = source.kind.label();
        // KILL QUERY ends the running statement and keeps the session; a
        // second connection to the same server issues it.
        Ok(DbConnection::new(
            Box::new(MySqlEngine { conn, version: version.clone() }),
            Box::new(move || {
                let _ = block_on(async {
                    let mut killer = Conn::new(opts.clone()).await.ok()?;
                    let _: Vec<mysql_async::Row> =
                        killer.exec(format!("KILL QUERY {server_id}"), ()).await.ok()?;
                    Some(())
                });
            }),
            format!("{family} {version}"),
        ))
    }
}

/// Disabled → no TLS; Require/Prefer → TLS, skipping certificate
/// validation only when the source says so.
fn ssl_opts(source: &DataSource) -> Option<SslOpts> {
    match source.ssl_mode {
        crate::SslMode::Disabled => None,
        crate::SslMode::Prefer | crate::SslMode::Require => {
            let mut o = SslOpts::default();
            if source.trust_cert {
                o = o
                    .with_danger_accept_invalid_certs(true)
                    .with_danger_skip_domain_validation(true);
            }
            Some(o)
        }
    }
}

fn err(e: mysql_async::Error) -> String {
    match &e {
        mysql_async::Error::Server(s) => s.message.to_string(),
        other => other.to_string(),
    }
}

fn value_of(v: Value) -> DbValue {
    match v {
        Value::NULL => DbValue::Null,
        Value::Int(i) => DbValue::Integer(i),
        Value::UInt(u) => DbValue::Integer(u as i64),
        Value::Float(f) => DbValue::Real(f as f64),
        Value::Double(f) => DbValue::Real(f),
        Value::Date(y, m, d, hh, mm, ss, _) => {
            DbValue::Text(format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}"))
        }
        Value::Time(neg, d, h, m, s, _) => {
            let sign = if neg { "-" } else { "" };
            DbValue::Text(format!("{sign}{d:02}d {h:02}:{m:02}:{s:02}"))
        }
        Value::Bytes(b) => {
            if looks_binary(&b) {
                DbValue::Blob(b)
            } else {
                DbValue::Text(String::from_utf8_lossy(&b).into_owned())
            }
        }
    }
}

fn looks_binary(b: &[u8]) -> bool {
    b.len() > 8
        && b.iter()
            .take(64)
            .any(|&c| c == 0 || (c < 0x20 && c != b'\t' && c != b'\n' && c != b'\r'))
}

impl DbEngine for MySqlEngine {
    fn introspect(&mut self, node: &NodeRef) -> Result<Vec<Child>, String> {
        block_on(self.introspect_async(node))
    }

    fn table_detail(&mut self, node: &NodeRef) -> Result<TableMeta, String> {
        block_on(self.table_detail_async(node))
    }

    fn execute_script(&mut self, sql: &str, max_rows: usize) -> Result<Vec<QueryOutcome>, String> {
        block_on(self.execute_script_async(sql, max_rows))
    }

    fn apply_edits(&mut self, table: &TableMeta, edits: &PendingEdits) -> Result<EditResult, String> {
        let statements = edits.statements(table, Dialect::MySql)?;
        block_on(async {
            let mut tx = self
                .conn
                .start_transaction(Default::default())
                .await
                .map_err(err)?;
            let mut result = EditResult::default();
            for (i, sql) in statements.iter().enumerate() {
                tx.exec_drop(sql, ()).await.map_err(err)?;
                let changed = tx.affected_rows();
                if changed == 0 && !matches!(edits.ops[i], crate::EditOp::InsertRow { .. }) {
                    tx.rollback().await.map_err(err)?;
                    return Err(CONFLICT.to_string());
                }
                result.rows_affected += changed as i64;
                result.statements_run += 1;
            }
            tx.commit().await.map_err(err)?;
            Ok(result)
        })
    }

    fn server_version(&self) -> String {
        self.version.clone()
    }

    fn dialect(&self) -> Dialect {
        Dialect::MySql
    }
}

impl MySqlEngine {
    async fn introspect_async(&mut self, node: &NodeRef) -> Result<Vec<Child>, String> {
        Ok(match node {
            NodeRef::Root => {
                let dbs: Vec<mysql_async::Row> = self
                    .conn
                    .exec("SHOW DATABASES", ())
                    .await
                    .map_err(err)?;
                dbs.into_iter()
                    .filter_map(|r| r.get::<String, _>(0))
                    .filter(|d| !SYSTEM_SCHEMAS.contains(&d.as_str()))
                    .map(|name| Child::Database {
                        label: name.clone(),
                        name,
                    })
                    .collect()
            }
            NodeRef::Database(name) => vec![Child::Schema { name: name.clone() }],
            NodeRef::Schema { database, .. } => {
                // A MySQL "schema" is its database; show the folder set.
                let _ = database;
                vec![
                    Child::Folder { kind: FolderKind::Tables },
                    Child::Folder { kind: FolderKind::Views },
                    Child::Folder { kind: FolderKind::Functions },
                ]
            }
            NodeRef::Folder { database, kind, .. } => {
                let db = database.as_str();
                if *kind == FolderKind::Functions {
                    let rows: Vec<mysql_async::Row> = self
                        .conn
                        .exec(
                            "SELECT routine_name, routine_type FROM information_schema.routines \
                             WHERE routine_schema = ? ORDER BY routine_name",
                            (db,),
                        )
                        .await
                        .map_err(err)?;
                    rows.into_iter()
                        .map(|r| {
                            let name: Option<String> = r.get(0);
                            let rtype: Option<String> = r.get(1);
                            let name = name.unwrap_or_default();
                            Child::Function {
                                meta: FunctionMeta {
                                    schema: db.to_string(),
                                    signature: format!("{name}()"),
                                    is_procedure: rtype.as_deref() == Some("PROCEDURE"),
                                    name,
                                },
                            }
                        })
                        .collect()
                } else {
                    let kind_filter = if *kind == FolderKind::Tables {
                        "BASE TABLE"
                    } else {
                        "VIEW"
                    };
                    let rows: Vec<mysql_async::Row> = self
                        .conn
                        .exec(
                            "SELECT table_name FROM information_schema.tables \
                             WHERE table_schema = ? AND table_type = ? ORDER BY table_name",
                            (db, kind_filter),
                        )
                        .await
                        .map_err(err)?;
                    rows.into_iter()
                        .map(|r| Child::Table {
                            name: r.get::<String, _>(0).unwrap_or_default(),
                            kind: if *kind == FolderKind::Tables {
                                TableKind::Table
                            } else {
                                TableKind::View
                            },
                        })
                        .collect()
                }
            }
            NodeRef::Table { .. } => {
                let detail = self.table_detail_async(node).await?;
                let mut children: Vec<Child> = detail
                    .columns
                    .into_iter()
                    .map(|meta| Child::Column { meta })
                    .collect();
                children.extend(detail.foreign_keys.into_iter().map(|meta| Child::ForeignKey { meta }));
                children.extend(detail.indexes.into_iter().map(|meta| Child::Index { meta }));
                children
            }
        })
    }

    async fn table_detail_async(&mut self, node: &NodeRef) -> Result<TableMeta, String> {
        let NodeRef::Table { database, schema, name, is_view } = node else {
            return Err("not a table node".to_string());
        };
        let db = database.as_str();
        let table = name.as_str();

        let rows: Vec<mysql_async::Row> = self
            .conn
            .exec(
                "SELECT column_name, column_type, is_nullable, column_default, column_key, extra \
                 FROM information_schema.columns \
                 WHERE table_schema = ? AND table_name = ? ORDER BY ordinal_position",
                (db, table),
            )
            .await
            .map_err(err)?;
        let columns: Vec<ColumnMeta> = rows
            .into_iter()
            .map(|r| {
                let name: Option<String> = r.get(0);
                let col_type: Option<String> = r.get(1);
                let nullable: Option<String> = r.get(2);
                let default: Option<String> = r.get(3);
                let key: Option<String> = r.get(4);
                let extra: Option<String> = r.get(5);
                ColumnMeta {
                    name: name.unwrap_or_default(),
                    data_type: col_type.unwrap_or_default(),
                    nullable: nullable.as_deref() != Some("NO"),
                    default,
                    primary_key: key.as_deref() == Some("PRI"),
                    auto_increment: extra
                        .as_deref()
                        .map(|e| e.to_ascii_lowercase().contains("auto_increment"))
                        .unwrap_or(false),
                }
            })
            .collect();
        let primary_key: Vec<String> = columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name.clone())
            .collect();

        let fk_rows: Vec<mysql_async::Row> = self
            .conn
            .exec(
                "SELECT constraint_name, column_name, referenced_table_schema, \
                        referenced_table_name, referenced_column_name \
                 FROM information_schema.key_column_usage \
                 WHERE table_schema = ? AND table_name = ? \
                   AND referenced_table_name IS NOT NULL \
                 ORDER BY constraint_name, ordinal_position",
                (db, table),
            )
            .await
            .map_err(err)?;
        let mut foreign_keys: Vec<ForeignKeyMeta> = Vec::new();
        for r in fk_rows {
            let cname: Option<String> = r.get(0);
            let col: Option<String> = r.get(1);
            let rschema: Option<String> = r.get(2);
            let rtable: Option<String> = r.get(3);
            let rcol: Option<String> = r.get(4);
            let cname = cname.unwrap_or_default();
            if let Some(fk) = foreign_keys.iter_mut().find(|f| f.name == cname) {
                fk.columns.push(col.unwrap_or_default());
                fk.ref_columns.push(rcol.unwrap_or_default());
            } else {
                foreign_keys.push(ForeignKeyMeta {
                    name: cname,
                    columns: vec![col.unwrap_or_default()],
                    ref_schema: rschema.unwrap_or_default(),
                    ref_table: rtable.unwrap_or_default(),
                    ref_columns: vec![rcol.unwrap_or_default()],
                });
            }
        }

        let idx_rows: Vec<mysql_async::Row> = self
            .conn
            .exec(
                "SELECT index_name, non_unique, GROUP_CONCAT(column_name ORDER BY seq_in_index) \
                 FROM information_schema.statistics \
                 WHERE table_schema = ? AND table_name = ? \
                 GROUP BY index_name, non_unique ORDER BY index_name",
                (db, table),
            )
            .await
            .map_err(err)?;
        let indexes: Vec<IndexMeta> = idx_rows
            .into_iter()
            .filter_map(|r| {
                let name: Option<String> = r.get(0);
                let unique: Option<u8> = r.get(1);
                let cols: Option<String> = r.get(2);
                let name = name?;
                if name == "PRIMARY" {
                    return None;
                }
                Some(IndexMeta {
                    name,
                    unique: unique.unwrap_or(0) != 0,
                    columns: cols?.split(',').map(|s| s.trim().to_string()).collect(),
                })
            })
            .collect();

        Ok(TableMeta {
            database: db.to_string(),
            schema: schema.clone(),
            name: table.to_string(),
            kind: if *is_view { TableKind::View } else { TableKind::Table },
            columns,
            primary_key,
            foreign_keys,
            indexes,
        })
    }

    async fn execute_script_async(
        &mut self,
        sql: &str,
        max_rows: usize,
    ) -> Result<Vec<QueryOutcome>, String> {
        let mut outcomes = Vec::new();
        for stmt in split_statements(sql) {
            let started = Instant::now();
            let text = stmt.text.clone();
            if is_select_shaped(&text) {
                let result = self.conn.query_iter(text.clone()).await.map_err(err)?;
                let mut outcome = QueryOutcome {
                    statement: text,
                    is_query: true,
                    ..Default::default()
                };
                if let Some(cols) = result.columns() {
                    outcome.columns = cols
                        .iter()
                        .map(|c| crate::ColumnOut {
                            name: c.name_str().to_string(),
                            type_name: String::new(),
                        })
                        .collect();
                }
                let mut result = result;
                result
                    .for_each(|row| {
                        if outcome.rows.len() >= max_rows {
                            outcome.truncated = true;
                        } else {
                            let cells = (0..outcome.columns.len())
                                .map(|i| {
                                    row.as_ref(i)
                                        .map(|v| value_of(v.clone()))
                                        .unwrap_or(DbValue::Null)
                                })
                                .collect();
                            outcome.rows.push(cells);
                        }
                    })
                    .await
                    .map_err(err)?;
                outcome.elapsed_ms = started.elapsed().as_millis() as u64;
                outcomes.push(outcome);
            } else {
                self.conn.exec_drop(&text, ()).await.map_err(err)?;
                outcomes.push(QueryOutcome {
                    statement: text,
                    rows_affected: self.conn.affected_rows() as i64,
                    is_query: false,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    ..Default::default()
                });
            }
        }
        Ok(outcomes)
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
    let word: String = rest.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    matches!(
        word.to_ascii_lowercase().as_str(),
        "select" | "values" | "table" | "show" | "describe" | "desc" | "explain" | "with"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_map_to_grid_kinds() {
        assert_eq!(value_of(Value::NULL), DbValue::Null);
        assert_eq!(value_of(Value::Int(-5)), DbValue::Integer(-5));
        assert_eq!(value_of(Value::UInt(9)), DbValue::Integer(9));
        assert_eq!(value_of(Value::Double(1.5)), DbValue::Real(1.5));
        assert_eq!(
            value_of(Value::Bytes(b"hello".to_vec())),
            DbValue::Text("hello".into())
        );
        assert!(matches!(
            value_of(Value::Bytes(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])),
            DbValue::Blob(_)
        ));
        assert_eq!(
            value_of(Value::Date(2024, 3, 14, 9, 30, 5, 0)),
            DbValue::Text("2024-03-14 09:30:05".into())
        );
    }

    #[test]
    fn select_shaped_covers_the_read_keywords() {
        assert!(is_select_shaped("SELECT 1"));
        assert!(is_select_shaped("  WITH c AS (SELECT 1) SELECT * FROM c"));
        assert!(is_select_shaped("SHOW TABLES"));
        assert!(is_select_shaped("-- note\nDESCRIBE t"));
        assert!(!is_select_shaped("UPDATE t SET a = 1"));
        assert!(!is_select_shaped("INSERT INTO t VALUES (1)"));
    }

    #[test]
    fn system_schemas_are_hidden_from_the_tree() {
        assert!(SYSTEM_SCHEMAS.contains(&"information_schema"));
        assert!(!SYSTEM_SCHEMAS.contains(&"shop"));
    }
}
