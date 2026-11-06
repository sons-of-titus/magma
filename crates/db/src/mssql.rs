//! SQL Server engine over tiberius: database/schema tree introspection
//! through the sys catalogs, bounded script execution, and transactional
//! table edits.  Cancellation is fetch-side: the row loop checks a flag
//! between rows (the server statement keeps running — the TDS attention
//! path is not wired yet).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::TryStreamExt;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel};
use tokio_util::compat::TokioAsyncWriteCompatExt;

use crate::{
    Child, ColumnMeta, Connection as DbConnection, Dialect, DbEngine, DbValue, DataSource,
    EditResult, FolderKind, ForeignKeyMeta, FunctionMeta, IndexMeta, NodeRef, PendingEdits,
    QueryOutcome, TableKind, TableMeta, block_on, split_statements,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONFLICT: &str = "no rows changed — the row was modified concurrently";

type TdsClient = Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

struct SqlServerEngine {
    client: TdsClient,
    version: String,
    cancel: Arc<AtomicBool>,
}

pub struct SqlServerConnection;

impl SqlServerConnection {
    pub fn open(source: &DataSource, password: Option<&str>) -> Result<DbConnection, String> {
        if source.host.trim().is_empty() {
            return Err("SQL Server needs a host".to_string());
        }
        let mut config = Config::new();
        config.host(&source.host);
        config.port(if source.port == 0 { 1433 } else { source.port });
        config.authentication(AuthMethod::sql_server(
            &source.user,
            password.unwrap_or_default(),
        ));
        if !source.database.trim().is_empty() {
            config.database(&source.database);
        }
        if source.trust_cert {
            config.trust_cert();
        }
        config.encryption(match source.ssl_mode {
            crate::SslMode::Disabled => EncryptionLevel::NotSupported,
            crate::SslMode::Prefer => EncryptionLevel::Off,
            crate::SslMode::Require => EncryptionLevel::Required,
        });

        let addr = config.get_addr().to_string();
        let mut client = block_on(async {
            tokio::time::timeout(CONNECT_TIMEOUT, async {
                let tcp = tokio::net::TcpStream::connect(&addr).await.map_err(|e| e.to_string())?;
                tcp.set_nodelay(true).ok();
                Client::connect(config, tcp.compat_write())
                    .await
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|_| "connection timed out".to_string())?
        })?;
        let version = block_on(version_of(&mut client));
        let cancel = Arc::new(AtomicBool::new(false));
        let interrupt_flag = Arc::clone(&cancel);
        Ok(DbConnection::new(
            Box::new(SqlServerEngine {
                client,
                version: version.clone(),
                cancel,
            }),
            Box::new(move || interrupt_flag.store(true, Ordering::SeqCst)),
            format!("SQL Server {version}"),
        ))
    }
}

async fn version_of(client: &mut TdsClient) -> String {
    let Ok(stream) = client.simple_query("SELECT @@VERSION").await else {
        return String::new();
    };
    let mut out = String::new();
    let mut rows = stream.into_row_stream();
    if let Ok(Ok(Some(row))) = tokio::time::timeout(Duration::from_secs(5), rows.try_next()).await
        && let Ok(Some(s)) = row.try_get::<&str, _>(0)
    {
        out = s.to_string();
    }
    // "Microsoft SQL Server 2019 (RTM-CU16) ..." — the year token reads best.
    out.split_whitespace()
        .find(|t| t.chars().all(|c| c.is_ascii_digit()) && t.len() == 4)
        .unwrap_or("")
        .to_string()
}

fn bracket(s: &str) -> String {
    format!("[{}]", s.replace(']', "]]"))
}

fn is_fixed_schema(name: &str) -> bool {
    matches!(name, "sys" | "INFORMATION_SCHEMA" | "guest") || name.starts_with("db_")
}

impl DbEngine for SqlServerEngine {
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
        let statements = edits.statements(table, Dialect::SqlServer)?;
        block_on(async {
            let mut result = EditResult::default();
            for (i, sql) in statements.iter().enumerate() {
                // Literal-only statements run fine under sp_executesql,
                // which gives exact affected-row counts for the conflict
                // check.
                let changed = self
                    .client
                    .execute(sql, &[])
                    .await
                    .map_err(|e| e.to_string())?
                    .total();
                if changed == 0 && !matches!(edits.ops[i], crate::EditOp::InsertRow { .. }) {
                    return Err(CONFLICT.to_string());
                }
                result.rows_affected += changed as i64;
                result.statements_run += 1;
            }
            Ok(result)
        })
    }

    fn server_version(&self) -> String {
        self.version.clone()
    }

    fn dialect(&self) -> Dialect {
        Dialect::SqlServer
    }
}

async fn text_rows(client: &mut TdsClient, sql: &str) -> Result<Vec<String>, String> {
    let stream = client.simple_query(sql).await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut rows = stream.into_row_stream();
    while let Some(row) = rows.try_next().await.map_err(|e| e.to_string())? {
        out.push(
            row.try_get::<&str, _>(0)
                .ok()
                .flatten()
                .unwrap_or("")
                .to_string(),
        );
    }
    Ok(out)
}

async fn text_rows_param(
    client: &mut TdsClient,
    sql: &str,
    param: &str,
) -> Result<Vec<String>, String> {
    let stream = client.query(sql, &[&param]).await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut rows = stream.into_row_stream();
    while let Some(row) = rows.try_next().await.map_err(|e| e.to_string())? {
        out.push(
            row.try_get::<&str, _>(0)
                .ok()
                .flatten()
                .unwrap_or("")
                .to_string(),
        );
    }
    Ok(out)
}

async fn param_rows(
    client: &mut TdsClient,
    sql: &str,
    param: &str,
) -> Result<Vec<tiberius::Row>, String> {
    let stream = client.query(sql, &[&param]).await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut rows = stream.into_row_stream();
    while let Some(row) = rows.try_next().await.map_err(|e| e.to_string())? {
        out.push(row);
    }
    Ok(out)
}

impl SqlServerEngine {
    async fn introspect_async(&mut self, node: &NodeRef) -> Result<Vec<Child>, String> {
        Ok(match node {
            NodeRef::Root => {
                let mut dbs = text_rows(
                    &mut self.client,
                    "SELECT name FROM sys.databases WHERE database_id > 4 AND state = 0 ORDER BY name",
                )
                .await?;
                // The connected database leads so its tree is one expand away.
                if let Some(current) = text_rows(&mut self.client, "SELECT db_name()").await?.pop()
                    && let Some(i) = dbs.iter().position(|d| *d == current)
                {
                    let cur = dbs.remove(i);
                    dbs.insert(0, cur);
                }
                dbs.into_iter()
                    .map(|name| Child::Database {
                        label: name.clone(),
                        name,
                    })
                    .collect()
            }
            NodeRef::Database(db) => {
                let sql = format!(
                    "SELECT name FROM {}.sys.schemas WHERE name NOT LIKE 'db\\_%' ESCAPE '\\' ORDER BY name",
                    bracket(db)
                );
                let mut schemas = text_rows(&mut self.client, &sql).await?;
                schemas.retain(|s| !is_fixed_schema(s));
                schemas
                    .into_iter()
                    .map(|name| Child::Schema { name })
                    .collect()
            }
            NodeRef::Schema { .. } => vec![
                Child::Folder { kind: FolderKind::Tables },
                Child::Folder { kind: FolderKind::Views },
                Child::Folder { kind: FolderKind::Functions },
            ],
            NodeRef::Folder { database, schema, kind } => {
                if *kind == FolderKind::Functions {
                    let sql = format!(
                        "SELECT o.name FROM {}.sys.objects o \
                         JOIN {}.sys.schemas s ON s.schema_id = o.schema_id \
                         WHERE s.name = @P1 AND o.type IN ('P','FN','IF','TF') ORDER BY o.name",
                        bracket(database),
                        bracket(database)
                    );
                    let names = text_rows_param(&mut self.client, &sql, schema).await?;
                    return Ok(names
                        .into_iter()
                        .map(|name| Child::Function {
                            meta: FunctionMeta {
                                schema: schema.clone(),
                                signature: format!("{name}()"),
                                is_procedure: true,
                                name,
                            },
                        })
                        .collect());
                }
                let (cat, kinds): (&str, &str) = match kind {
                    FolderKind::Tables => ("sys.tables", "('U')"),
                    FolderKind::Views => ("sys.views", "('V')"),
                    FolderKind::Sequences => return Ok(Vec::new()),
                    FolderKind::Functions => unreachable!(),
                };
                let sql = format!(
                    "SELECT o.name FROM {cat} o \
                     JOIN {}.sys.schemas s ON s.schema_id = o.schema_id \
                     WHERE s.name = @P1 AND o.type IN {kinds} ORDER BY o.name",
                    bracket(database)
                );
                let names = text_rows_param(&mut self.client, &sql, schema).await?;
                names
                    .into_iter()
                    .map(|name| Child::Table {
                        kind: if *kind == FolderKind::Tables {
                            TableKind::Table
                        } else {
                            TableKind::View
                        },
                        name,
                    })
                    .collect()
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
        let obj = format!("{}.{}.{}", bracket(database), bracket(schema), bracket(name));
        let db = bracket(database);

        let col_sql = format!(
            "SELECT c.name, ty.name, c.is_nullable, object_definition(c.default_object_id), c.is_identity \
             FROM {db}.sys.columns c \
             JOIN {db}.sys.types ty ON ty.user_type_id = c.user_type_id \
             WHERE c.object_id = OBJECT_ID(@P1) ORDER BY c.column_id"
        );
        let col_rows = param_rows(&mut self.client, &col_sql, &obj).await?;
        let columns: Vec<ColumnMeta> = col_rows
            .into_iter()
            .map(|r| {
                let nm: Option<&str> = r.try_get(0).ok().flatten();
                let ty: Option<&str> = r.try_get(1).ok().flatten();
                let nullable: Option<bool> = r.try_get(2).ok().flatten();
                let default: Option<&str> = r.try_get(3).ok().flatten();
                let identity: Option<bool> = r.try_get(4).ok().flatten();
                ColumnMeta {
                    name: nm.unwrap_or_default().to_string(),
                    data_type: ty.unwrap_or_default().to_string(),
                    nullable: nullable.unwrap_or(true),
                    default: default.map(|s| s.to_string()),
                    primary_key: false,
                    auto_increment: identity.unwrap_or(false),
                }
            })
            .collect();

        let pk_sql = format!(
            "SELECT c.name FROM {db}.sys.key_constraints kc \
             JOIN {db}.sys.index_columns ic ON ic.object_id = kc.parent_object_id \
                 AND ic.index_id = kc.unique_index_id \
             JOIN {db}.sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
             WHERE kc.parent_object_id = OBJECT_ID(@P1) AND kc.type = 'PK' ORDER BY ic.key_ordinal"
        );
        let pk_rows = param_rows(&mut self.client, &pk_sql, &obj).await?;
        let primary_key: Vec<String> = pk_rows
            .into_iter()
            .map(|r| {
                r.try_get::<&str, _>(0)
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect();

        let fk_sql = format!(
            "SELECT fk.name, pc.name, s2.name, t2.name, rc.name \
             FROM {db}.sys.foreign_keys fk \
             JOIN {db}.sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id \
             JOIN {db}.sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id \
             JOIN {db}.sys.tables t2 ON t2.object_id = fk.referenced_object_id \
             JOIN {db}.sys.schemas s2 ON s2.schema_id = t2.schema_id \
             JOIN {db}.sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id \
             WHERE fk.parent_object_id = OBJECT_ID(@P1) ORDER BY fk.name, fkc.constraint_column_id"
        );
        let fk_rows = param_rows(&mut self.client, &fk_sql, &obj).await?;
        let mut foreign_keys: Vec<ForeignKeyMeta> = Vec::new();
        for r in fk_rows {
            let get = |i: usize| -> String {
                r.try_get::<&str, _>(i).ok().flatten().unwrap_or_default().to_string()
            };
            let cname = get(0);
            if let Some(fk) = foreign_keys.iter_mut().find(|f| f.name == cname) {
                fk.columns.push(get(1));
                fk.ref_columns.push(get(4));
            } else {
                foreign_keys.push(ForeignKeyMeta {
                    name: cname,
                    columns: vec![get(1)],
                    ref_schema: get(2),
                    ref_table: get(3),
                    ref_columns: vec![get(4)],
                });
            }
        }

        let idx_sql = format!(
            "SELECT i.name, i.is_unique, c.name \
             FROM {db}.sys.indexes i \
             JOIN {db}.sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
             JOIN {db}.sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
             WHERE i.object_id = OBJECT_ID(@P1) AND i.is_primary_key = 0 AND i.name IS NOT NULL \
             ORDER BY i.name, ic.key_ordinal"
        );
        let idx_rows = param_rows(&mut self.client, &idx_sql, &obj).await?;
        let mut indexes: Vec<IndexMeta> = Vec::new();
        for r in idx_rows {
            let name = r.try_get::<&str, _>(0).ok().flatten().unwrap_or_default().to_string();
            let unique = r.try_get::<bool, _>(1).ok().flatten().unwrap_or(false);
            let col = r.try_get::<&str, _>(2).ok().flatten().unwrap_or_default().to_string();
            if let Some(ix) = indexes.iter_mut().find(|i| i.name == name) {
                ix.columns.push(col);
            } else {
                indexes.push(IndexMeta {
                    name,
                    unique,
                    columns: vec![col],
                });
            }
        }

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

    async fn execute_script_async(
        &mut self,
        sql: &str,
        max_rows: usize,
    ) -> Result<Vec<QueryOutcome>, String> {
        let mut outcomes = Vec::new();
        self.cancel.store(false, Ordering::SeqCst);
        for stmt in split_statements(sql) {
            if self.cancel.load(Ordering::SeqCst) {
                return Err("query interrupted".to_string());
            }
            let started = Instant::now();
            let text = stmt.text.clone();
            if is_select_shaped(&text) {
                let stream = self.client.query(&text, &[]).await.map_err(|e| e.to_string())?;
                let mut outcome = QueryOutcome {
                    statement: text,
                    is_query: true,
                    ..Default::default()
                };
                let mut width = 0usize;
                let mut rows = stream.into_row_stream();
                while let Some(row) = rows.try_next().await.map_err(|e| e.to_string())? {
                    if width == 0 {
                        width = row.columns().len();
                        outcome.columns = row
                            .columns()
                            .iter()
                            .map(|c| crate::ColumnOut {
                                name: c.name().to_string(),
                                type_name: String::new(),
                            })
                            .collect();
                    }
                    if outcome.rows.len() >= max_rows {
                        outcome.truncated = true;
                        break;
                    }
                    outcome
                        .rows
                        .push((0..width).map(|i| cell_value(&row, i)).collect::<Vec<_>>());
                }
                outcome.elapsed_ms = started.elapsed().as_millis() as u64;
                outcomes.push(outcome);
            } else {
                // Arbitrary batches (DDL, USE, …) only run through the text
                // protocol; affected rows come from @@ROWCOUNT appended to
                // the same batch.
                let probe = format!("{text}; SELECT @@ROWCOUNT AS _magma_affected");
                let stream = self
                    .client
                    .simple_query(&probe)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut rows = stream.into_row_stream();
                let mut affected: i64 = 0;
                while let Some(row) = rows.try_next().await.map_err(|e| e.to_string())? {
                    affected = row.try_get::<i32, _>(0).ok().flatten().unwrap_or(0) as i64;
                }
                outcomes.push(QueryOutcome {
                    statement: text,
                    rows_affected: affected,
                    is_query: false,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    ..Default::default()
                });
            }
        }
        Ok(outcomes)
    }
}

fn cell_value(row: &tiberius::Row, i: usize) -> DbValue {
    // Try in narrowing order; a type mismatch surfaces as Err, SQL NULL as
    // Ok(None) only for the column's true type.
    macro_rules! typed {
        ($t:ty, $f:expr) => {
            match row.try_get::<$t, _>(i) {
                Ok(Some(v)) => return $f(v),
                Ok(None) => return DbValue::Null,
                Err(e) => { logging::error!("mssql.rs:508: {e}"); {} }
            }
        };
    }
    typed!(bool, |v: bool| DbValue::Bool(v));
    typed!(i64, |v: i64| DbValue::Integer(v));
    typed!(i32, |v: i32| DbValue::Integer(v as i64));
    typed!(i16, |v: i16| DbValue::Integer(v as i64));
    typed!(f64, |v: f64| DbValue::Real(v));
    typed!(f32, |v: f32| DbValue::Real(v as f64));
    typed!(tiberius::numeric::Numeric, |v: tiberius::numeric::Numeric| DbValue::Text(
        v.to_string()
    ));
    typed!(chrono::NaiveDateTime, |v: chrono::NaiveDateTime| DbValue::Text(
        v.format("%Y-%m-%d %H:%M:%S").to_string()
    ));
    typed!(tiberius::Uuid, |v: tiberius::Uuid| DbValue::Text(v.to_string()));
    typed!(&str, |v: &str| DbValue::Text(v.to_string()));
    typed!(&[u8], |v: &[u8]| if v.len() > 8
        && v.iter()
            .take(64)
            .any(|&c| c == 0 || (c < 0x20 && c != b'\t' && c != b'\n' && c != b'\r'))
    {
        DbValue::Blob(v.to_vec())
    } else {
        DbValue::Text(String::from_utf8_lossy(v).into_owned())
    });
    DbValue::Null
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
        "select" | "values" | "table" | "with"
    )
}
