//! PostgreSQL engine over tokio-postgres: schema tree introspection,
//! typed bounded queries with a text-cast fallback for exotic column
//! types, cancellation through the server's cancel protocol, and
//! transactional table edits.

use std::time::{Duration, Instant};

use tokio_postgres::types::Type;
use tokio_postgres::{Client, NoTls, Row};

use crate::{
    Child, ColumnMeta, Connection as DbConnection, Dialect, DbEngine, DbValue, DataSource,
    EditResult, FolderKind, ForeignKeyMeta, FunctionMeta, IndexMeta, NodeRef, PendingEdits,
    QueryOutcome, TableKind, TableMeta, block_on, split_statements,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CANCEL_TIMEOUT: Duration = Duration::from_secs(3);
const CONFLICT: &str = "no rows changed — the row was modified concurrently";

struct PostgresEngine {
    client: Client,
    version: String,
}

pub struct PostgresConnection;

impl PostgresConnection {
    pub fn open(source: &DataSource, password: Option<&str>) -> Result<DbConnection, String> {
        let (base_config, target) = build_pg_config(source, password)?;
        let attempts: &[Option<bool>] = match (source.ssl_mode, source.trust_cert) {
            (crate::SslMode::Disabled, _) => &[None],
            (crate::SslMode::Prefer, true) => &[Some(true), None],
            (crate::SslMode::Prefer, false) => &[Some(false), Some(true), None],
            (crate::SslMode::Require, true) => &[Some(true)],
            (crate::SslMode::Require, false) => &[Some(false), Some(true)],
        };
        let mut last_err = String::new();
        for &trust in attempts {
            let tls_cfg = trust.map(tls::client_config);
            match block_on(connect_once(base_config.clone(), tls_cfg)) {
                Ok((client, cancel)) => {
                    let version = block_on(query_version(&client));
                    let label = format!("PostgreSQL {version}");
                    let interrupt_token = cancel.clone();
                    return Ok(DbConnection::new(
                        Box::new(PostgresEngine { client, version }),
                        Box::new(move || {
                            logging::LogErr::log_err(block_on(tokio::time::timeout(
                                CANCEL_TIMEOUT,
                                interrupt_token.cancel_query(NoTls),
                            )));
                        }),
                        label,
                    ));
                }
                Err((e, is_db_err)) => {
                    last_err = e;
                    if is_db_err {
                        break;
                    }
                }
            }
        }
        Err(format!("cannot connect to {target}: {last_err}"))
    }
}

pub(crate) fn build_pg_config(
    source: &DataSource,
    password: Option<&str>,
) -> Result<(tokio_postgres::Config, String), String> {
    let raw_host = source.host.trim();
    let url = source
        .url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            (raw_host.starts_with("postgres://") || raw_host.starts_with("postgresql://"))
                .then_some(raw_host)
        });
    let mut config = if let Some(raw_url) = url {
        let mut cfg: tokio_postgres::Config = raw_url
            .parse()
            .map_err(|e| format!("invalid PostgreSQL URL: {e}"))?;
        if cfg.get_hosts().is_empty() {
            cfg.host("localhost");
        }
        if cfg.get_ports().is_empty() {
            cfg.port(if source.port == 0 { 5432 } else { source.port });
        }
        if cfg.get_user().is_none_or(|u| u.is_empty()) {
            let user = if source.user.trim().is_empty() {
                "postgres"
            } else {
                source.user.trim()
            };
            cfg.user(user);
        }
        if cfg.get_dbname().is_none_or(|d| d.is_empty()) {
            let db = if !source.database.trim().is_empty() {
                source.database.trim().to_string()
            } else {
                cfg.get_user().unwrap_or("postgres").to_string()
            };
            cfg.dbname(&db);
        }
        cfg
    } else {
        if raw_host.is_empty() {
            return Err("PostgreSQL needs a host".to_string());
        }
        let (host, port) = if !raw_host.starts_with('[')
            && raw_host.matches(':').count() == 1
            && let Some((h, p)) = raw_host.rsplit_once(':')
            && let Ok(parsed_port) = p.trim().parse::<u16>()
            && parsed_port > 0
        {
            (h.trim(), parsed_port)
        } else {
            (raw_host, if source.port == 0 { 5432 } else { source.port })
        };
        let user = if source.user.trim().is_empty() {
            "postgres"
        } else {
            source.user.trim()
        };
        let database = if source.database.trim().is_empty() {
            user
        } else {
            source.database.trim()
        };
        let mut cfg = tokio_postgres::Config::new();
        cfg.host(host).port(port).user(user).dbname(database);
        cfg
    };
    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        config.password(pw);
    }
    config.connect_timeout(CONNECT_TIMEOUT);
    let target = format!(
        "{} @ {}:{}",
        config.get_dbname().unwrap_or("postgres"),
        config
            .get_hosts()
            .first()
            .map(|h| match h {
                tokio_postgres::config::Host::Tcp(s) => s.as_str(),
                #[cfg(unix)]
                tokio_postgres::config::Host::Unix(_) => "unix",
            })
            .unwrap_or("localhost"),
        config.get_ports().first().copied().unwrap_or(5432),
    );
    Ok((config, target))
}

async fn connect_once(
    mut config: tokio_postgres::Config,
    tls_config: Option<rustls::ClientConfig>,
) -> Result<(Client, tokio_postgres::CancelToken), (String, bool)> {
    let client = match tls_config {
        Some(cfg) => {
            config.ssl_mode(tokio_postgres::config::SslMode::Prefer);
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(cfg);
            let (client, conn) = tokio::time::timeout(CONNECT_TIMEOUT, config.connect(tls))
                .await
                .map_err(|_| ("connection timed out".to_string(), false))?
                .map_err(|e| {
                    let is_db = e.as_db_error().is_some();
                    (err(e), is_db)
                })?;
            tokio::spawn(async move {
                let _ = conn.await;
            });
            client
        }
        None => {
            config.ssl_mode(tokio_postgres::config::SslMode::Disable);
            let (client, conn) = tokio::time::timeout(CONNECT_TIMEOUT, config.connect(NoTls))
                .await
                .map_err(|_| ("connection timed out".to_string(), false))?
                .map_err(|e| {
                    let is_db = e.as_db_error().is_some();
                    (err(e), is_db)
                })?;
            tokio::spawn(async move {
                let _ = conn.await;
            });
            client
        }
    };
    let cancel = client.cancel_token();
    Ok((client, cancel))
}

async fn query_version(client: &Client) -> String {
    client
        .query_one("SELECT version()", &[])
        .await
        .ok()
        .and_then(|r| r.try_get::<_, String>(0).ok())
        .and_then(|v| v.split_whitespace().nth(1).map(|s| s.to_string()))
        .unwrap_or_else(|| "?".to_string())
}

impl DbEngine for PostgresEngine {
    fn introspect(&mut self, node: &NodeRef) -> Result<Vec<Child>, String> {
        block_on(self.introspect_async(node))
    }

    fn table_detail(&mut self, node: &NodeRef) -> Result<TableMeta, String> {
        block_on(self.table_detail_async(node))
    }

    fn catalog(&mut self) -> Result<crate::Catalog, String> {
        block_on(crate::postgres_catalog::load(&self.client))
    }

    fn execute_script(&mut self, sql: &str, max_rows: usize) -> Result<Vec<QueryOutcome>, String> {
        block_on(self.execute_script_async(sql, max_rows))
    }

    fn apply_edits(&mut self, table: &TableMeta, edits: &PendingEdits) -> Result<EditResult, String> {
        let statements = edits.statements(table, Dialect::Postgres)?;
        block_on(async {
            let tx = self.client.transaction().await.map_err(err)?;
            let mut result = EditResult::default();
            for (i, sql) in statements.iter().enumerate() {
                let changed = tx.execute(sql, &[]).await.map_err(err)?;
                if changed == 0 && !matches!(edits.ops[i], crate::EditOp::InsertRow { .. }) {
                    logging::LogErr::log_err(tx.rollback().await);
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
        Dialect::Postgres
    }
}

impl PostgresEngine {
    async fn introspect_async(&mut self, node: &NodeRef) -> Result<Vec<Child>, String> {
        let client = &self.client;
        Ok(match node {
            NodeRef::Root => {
                let row = client
                    .query_one("SELECT current_database()", &[])
                    .await
                    .map_err(err)?;
                vec![Child::Database {
                    name: row.try_get::<_, String>(0).map_err(err)?,
                    label: String::new(),
                }]
            }
            NodeRef::Database(_) => {
                let rows = client
                    .query(
                        "SELECT nspname FROM pg_namespace \
                         WHERE nspname NOT LIKE 'pg\\_%' \
                           AND nspname NOT IN ('information_schema') \
                         ORDER BY nspname",
                        &[],
                    )
                    .await
                    .map_err(err)?;
                rows.into_iter()
                    .map(|r| Ok(Child::Schema { name: r.try_get(0).map_err(err)? }))
                    .collect::<Result<_, String>>()?
            }
            NodeRef::Schema { .. } => vec![
                Child::Folder { kind: FolderKind::Tables },
                Child::Folder { kind: FolderKind::Views },
                Child::Folder { kind: FolderKind::Sequences },
                Child::Folder { kind: FolderKind::Functions },
            ],
            NodeRef::Folder { schema, kind, .. } => {
                let relkinds: &str = match kind {
                    FolderKind::Tables => "'r','p'",
                    FolderKind::Views => "'v','m'",
                    FolderKind::Sequences => "",
                    FolderKind::Functions => "",
                };
                if *kind == FolderKind::Functions {
                    let rows = client
                        .query(
                            "SELECT p.proname, pg_get_function_identity_arguments(p.oid), p.prokind::text \
                             FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
                             WHERE n.nspname = $1 ORDER BY p.proname",
                            &[schema],
                        )
                        .await
                        .map_err(err)?;
                    rows.into_iter()
                        .map(|r| {
                            let name: String = r.try_get(0).map_err(err)?;
                            let args: String = r.try_get(1).map_err(err)?;
                            let prokind: String = r.try_get(2).map_err(err)?;
                            Ok(Child::Function {
                                meta: FunctionMeta {
                                    schema: schema.clone(),
                                    signature: format!("{name}({args})"),
                                    is_procedure: prokind == "p",
                                    name,
                                },
                            })
                        })
                        .collect::<Result<_, String>>()?
                } else if *kind == FolderKind::Sequences {
                    let rows = client
                        .query(
                            "SELECT c.relname FROM pg_class c \
                             JOIN pg_namespace n ON n.oid = c.relnamespace \
                             WHERE n.nspname = $1 AND c.relkind = 'S' ORDER BY c.relname",
                            &[schema],
                        )
                        .await
                        .map_err(err)?;
                    rows.into_iter()
                        .map(|r| {
                            Ok(Child::Sequence {
                                meta: crate::SequenceMeta {
                                    schema: schema.clone(),
                                    name: r.try_get(0).map_err(err)?,
                                },
                            })
                        })
                        .collect::<Result<_, String>>()?
                } else {
                    let sql = format!(
                        "SELECT c.relname, c.relkind::text FROM pg_class c \
                         JOIN pg_namespace n ON n.oid = c.relnamespace \
                         WHERE n.nspname = $1 AND c.relkind IN ({relkinds}) ORDER BY c.relname"
                    );
                    let rows = client.query(&sql, &[schema]).await.map_err(err)?;
                    rows.into_iter()
                        .map(|r| {
                            let name: String = r.try_get(0).map_err(err)?;
                            let relkind: String = r.try_get(1).map_err(err)?;
                            let tkind = match relkind.as_str() {
                                "v" => TableKind::View,
                                "m" => TableKind::MaterializedView,
                                _ => TableKind::Table,
                            };
                            Ok(Child::Table { name, kind: tkind })
                        })
                        .collect::<Result<_, String>>()?
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
        let qualified = format!("{}.{}", quote(schema), quote(name));
        let client = &self.client;

        let col_rows = client
            .query(
                "SELECT a.attname, format_type(a.atttypid, a.atttypmod), NOT a.attnotnull, \
                        pg_get_expr(d.adbin, d.adrelid)::text, a.attidentity::text <> '' \
                 FROM pg_attribute a \
                 LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
                 WHERE a.attrelid = $1::text::regclass AND a.attnum > 0 AND NOT a.attisdropped \
                 ORDER BY a.attnum",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        let mut columns: Vec<ColumnMeta> = col_rows
            .into_iter()
            .map(|r| {
                Ok(ColumnMeta {
                    name: r.try_get(0).map_err(err)?,
                    data_type: r.try_get(1).map_err(err)?,
                    nullable: r.try_get(2).map_err(err)?,
                    default: r.try_get(3).map_err(err)?,
                    primary_key: false,
                    auto_increment: r.try_get(4).map_err(err)?,
                })
            })
            .collect::<Result<_, String>>()?;

        let pk_rows = client
            .query(
                "SELECT a.attname FROM pg_index i \
                 JOIN pg_attribute a ON a.attrelid = i.indrelid \
                     AND a.attnum = ANY(i.indkey) AND a.attnum > 0 \
                 WHERE i.indrelid = $1::text::regclass AND i.indisprimary",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        // ANY() loses key order; key order for multi-column pks is a known
        // limitation of this listing.
        let primary_key: Vec<String> = pk_rows
            .into_iter()
            .map(|r| r.try_get(0).map_err(err))
            .collect::<Result<_, String>>()?;
        for col in &mut columns {
            col.primary_key = primary_key.contains(&col.name);
        }

        let fk_rows = client
            .query(
                "SELECT con.conname, con.conkey, con.confkey, nn.nspname, rc.relname \
                 FROM pg_constraint con \
                 JOIN pg_class rc ON rc.oid = con.confrelid \
                 JOIN pg_namespace nn ON nn.oid = rc.relnamespace \
                 WHERE con.contype = 'f' AND con.conrelid = $1::text::regclass ORDER BY con.conname",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        let mut foreign_keys = Vec::with_capacity(fk_rows.len());
        for r in &fk_rows {
            let cname: String = r.try_get(0).map_err(err)?;
            let conkey: Vec<i16> = r.try_get(1).map_err(err)?;
            let confkey: Vec<i16> = r.try_get(2).map_err(err)?;
            let ref_schema: String = r.try_get(3).map_err(err)?;
            let ref_table: String = r.try_get(4).map_err(err)?;
            let ref_qualified = format!("{}.{}", quote(&ref_schema), quote(&ref_table));
            foreign_keys.push(ForeignKeyMeta {
                name: cname,
                columns: column_names(client, &qualified, &conkey).await?,
                ref_schema,
                ref_table,
                ref_columns: column_names(client, &ref_qualified, &confkey).await?,
            });
        }

        let idx_rows = client
            .query(
                "SELECT ic.relname, i.indisunique, i.indkey::int2[] \
                 FROM pg_index i \
                 JOIN pg_class t ON t.oid = i.indrelid \
                 JOIN pg_class ic ON ic.oid = i.indexrelid \
                 WHERE i.indrelid = $1::text::regclass AND NOT i.indisprimary ORDER BY ic.relname",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        let mut indexes = Vec::with_capacity(idx_rows.len());
        for r in &idx_rows {
            let name: String = r.try_get(0).map_err(err)?;
            let unique: bool = r.try_get(1).map_err(err)?;
            let indkey: Vec<i16> = r.try_get(2).map_err(err)?;
            indexes.push(IndexMeta {
                name,
                unique,
                columns: column_names(client, &qualified, &indkey).await?,
            });
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
        for stmt in split_statements(sql) {
            let started = Instant::now();
            let text = stmt.text.clone();
            if is_select_shaped(&text) {
                let prepared = self.client.prepare(&text).await.map_err(err)?;
                let columns: Vec<crate::ColumnOut> = prepared
                    .columns()
                    .iter()
                    .map(|c| crate::ColumnOut {
                        name: c.name().to_string(),
                        type_name: c.type_().name().to_string(),
                    })
                    .collect();
                let all_supported = prepared.columns().iter().all(|c| supports(c.type_().clone()));
                let (rows_out, truncated) = if all_supported {
                    let rows = self.client.query(&prepared, &[]).await.map_err(err)?;
                    collect_rows(&rows, max_rows)
                } else {
                    // Exotic types (numeric, json, dates beyond SystemTime,
                    // arrays…) render through a text-cast wrapper query so
                    // the grid still shows something honest.
                    let wrapper = cast_wrapper(prepared.columns(), &text);
                    let rows = self.client.query(wrapper.as_str(), &[]).await.map_err(err)?;
                    let mut out = Vec::new();
                    let mut truncated = false;
                    for row in &rows {
                        if out.len() >= max_rows {
                            truncated = true;
                            break;
                        }
                        let mut cells = Vec::with_capacity(columns.len());
                        for i in 0..columns.len() {
                            cells.push(
                                row.try_get::<_, Option<String>>(i)
                                    .map_err(err)?
                                    .map_or(DbValue::Null, DbValue::Text),
                            );
                        }
                        out.push(cells);
                    }
                    (out, truncated)
                };
                outcomes.push(QueryOutcome {
                    statement: text,
                    columns,
                    rows: rows_out,
                    truncated,
                    is_query: true,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    ..Default::default()
                });
            } else {
                let affected = self.client.execute(&text, &[]).await.map_err(err)?;
                outcomes.push(QueryOutcome {
                    statement: text,
                    rows_affected: affected as i64,
                    is_query: false,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    ..Default::default()
                });
            }
        }
        Ok(outcomes)
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub(crate) fn err(e: tokio_postgres::Error) -> String {
    if let Some(db) = e.as_db_error() {
        return match db.detail() {
            Some(detail) if !detail.is_empty() => format!("{}: {detail}", db.message()),
            _ => db.message().to_string(),
        };
    }
    let mut msg = e.to_string();
    let mut curr: Option<&dyn std::error::Error> = std::error::Error::source(&e);
    while let Some(cause) = curr {
        let s = cause.to_string();
        if !s.is_empty() && !msg.contains(&s) {
            msg.push_str(": ");
            msg.push_str(&s);
        }
        curr = cause.source();
    }
    msg
}

fn supports(t: Type) -> bool {
    matches!(
        t,
        Type::BOOL
            | Type::INT2
            | Type::INT4
            | Type::INT8
            | Type::OID
            | Type::FLOAT4
            | Type::FLOAT8
            | Type::TEXT
            | Type::VARCHAR
            | Type::BPCHAR
            | Type::NAME
            | Type::BYTEA
            | Type::TIMESTAMPTZ
            | Type::TIMESTAMP
    )
}

fn collect_rows(rows: &[Row], max_rows: usize) -> (Vec<Vec<DbValue>>, bool) {
    let mut out = Vec::new();
    let mut truncated = false;
    if rows.is_empty() {
        return (out, false);
    }
    let width = rows[0].columns().len();
    for row in rows {
        if out.len() >= max_rows {
            truncated = true;
            break;
        }
        out.push(
            (0..width)
                .map(|i| typed_value(row, i).unwrap_or(DbValue::Null))
                .collect(),
        );
    }
    (out, truncated)
}

fn nullable<'a, T: tokio_postgres::types::FromSql<'a>>(
    row: &'a Row,
    i: usize,
) -> Result<Option<T>, String> {
    row.try_get::<_, Option<T>>(i).map_err(err)
}

fn typed_value(row: &Row, i: usize) -> Result<DbValue, String> {
    let ty = row.columns()[i].type_().clone();
    Ok(match ty {
        Type::BOOL => nullable::<bool>(row, i)?.map_or(DbValue::Null, DbValue::Bool),
        Type::INT2 => nullable::<i16>(row, i)?.map_or(DbValue::Null, |v| DbValue::Integer(v as i64)),
        Type::INT4 => nullable::<i32>(row, i)?.map_or(DbValue::Null, |v| DbValue::Integer(v as i64)),
        Type::INT8 | Type::OID => nullable::<i64>(row, i)?.map_or(DbValue::Null, DbValue::Integer),
        Type::FLOAT4 => nullable::<f32>(row, i)?.map_or(DbValue::Null, |v| DbValue::Real(v as f64)),
        Type::FLOAT8 => nullable::<f64>(row, i)?.map_or(DbValue::Null, DbValue::Real),
        Type::BYTEA => nullable::<Vec<u8>>(row, i)?.map_or(DbValue::Null, DbValue::Blob),
        Type::TIMESTAMPTZ | Type::TIMESTAMP => nullable::<std::time::SystemTime>(row, i)?
            .map_or(DbValue::Null, |t| DbValue::Text(civil_timestamp(t))),
        _ => nullable::<String>(row, i)?.map_or(DbValue::Null, DbValue::Text),
    })
}

fn civil_timestamp(t: std::time::SystemTime) -> String {
    let d = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let days = secs / 86_400;
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let da = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let yy = if mo <= 2 { y + 1 } else { y };
    let rem = secs % 86_400;
    format!("{yy:04}-{mo:02}-{da:02} {:02}:{:02}:{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

fn cast_wrapper(cols: &[tokio_postgres::Column], sql: &str) -> String {
    let proj: Vec<String> = cols
        .iter()
        .map(|c| format!("{}::text AS {}", quote(c.name()), quote(c.name())))
        .collect();
    format!("SELECT {} FROM ({sql}) AS _magma_cast", proj.join(", "))
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
        "select" | "values" | "table" | "show" | "explain" | "with"
    )
}

async fn column_names(
    client: &Client,
    table: &str,
    attnums: &[i16],
) -> Result<Vec<String>, String> {
    if attnums.is_empty() {
        return Ok(Vec::new());
    }
    let rows = client
        .query(
            "SELECT attnum, attname FROM pg_attribute \
             WHERE attrelid = $1::text::regclass AND attnum = ANY($2)",
            &[&table, &attnums],
        )
        .await
        .map_err(err)?;
    let mut named: Vec<(i16, String)> = rows
        .into_iter()
        .map(|r| Ok((r.try_get(0).map_err(err)?, r.try_get(1).map_err(err)?)))
        .collect::<Result<_, String>>()?;
    named.sort_by_key(|(n, _)| attnums.iter().position(|a| a == n).unwrap_or(usize::MAX));
    Ok(named.into_iter().map(|(_, name)| name).collect())
}

mod tls {
    use std::sync::Arc;

    /// A verifier that accepts anything — the explicit "trust server
    /// certificate" escape hatch for self-signed setups.
    #[derive(Debug)]
    pub struct AcceptAll;

    impl rustls::client::danger::ServerCertVerifier for AcceptAll {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    pub fn client_config(trust_cert: bool) -> rustls::ClientConfig {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let builder = rustls::ClientConfig::builder();
        if trust_cert {
            builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AcceptAll))
                .with_no_client_auth()
        } else {
            let mut roots = rustls::RootCertStore::empty();
            for cert in rustls_native_certs::load_native_certs().certs {
                logging::LogErr::log_err(roots.add(cert));
            }
            builder.with_root_certificates(roots).with_no_client_auth()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataSource, DriverKind, SslMode};

    fn blank_pg_source() -> DataSource {
        DataSource {
            id: 1,
            name: "pg".into(),
            kind: DriverKind::Postgres,
            host: "localhost".into(),
            port: 0,
            database: String::new(),
            user: String::new(),
            ssl_mode: SslMode::Prefer,
            trust_cert: false,
            save_password: true,
            url: None,
        }
    }

    #[test]
    fn build_pg_config_defaults_port_user_and_database() {
        let src = blank_pg_source();
        let (cfg, target) = build_pg_config(&src, Some("secret")).unwrap();
        assert_eq!(cfg.get_ports(), &[5432]);
        assert_eq!(cfg.get_user(), Some("postgres"));
        assert_eq!(cfg.get_dbname(), Some("postgres"));
        assert_eq!(target, "postgres @ localhost:5432");
    }

    #[test]
    fn build_pg_config_supports_url_and_host_port_shorthand() {
        let mut url_src = blank_pg_source();
        url_src.host.clear();
        url_src.url = Some("postgres://alice@db.example.com:5433/analytics".into());
        let (cfg, target) = build_pg_config(&url_src, None).unwrap();
        assert_eq!(cfg.get_ports(), &[5433]);
        assert_eq!(cfg.get_user(), Some("alice"));
        assert_eq!(cfg.get_dbname(), Some("analytics"));
        assert_eq!(target, "analytics @ db.example.com:5433");

        let mut host_port_src = blank_pg_source();
        host_port_src.host = "127.0.0.1:15432".into();
        let (cfg2, target2) = build_pg_config(&host_port_src, None).unwrap();
        assert_eq!(cfg2.get_ports(), &[15432]);
        assert_eq!(target2, "postgres @ 127.0.0.1:15432");
    }
}
