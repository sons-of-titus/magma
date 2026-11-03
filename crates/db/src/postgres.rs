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
        if source.host.trim().is_empty() {
            return Err("PostgreSQL needs a host".to_string());
        }
        let database = if source.database.trim().is_empty() {
            source.user.clone()
        } else {
            source.database.clone()
        };
        let mut tls_config = Some(tls::client_config(source.trust_cert));
        let mut last_err = String::new();
        for attempt in 0..2 {
            let result = block_on(connect_once(source, &database, password, tls_config.clone()));
            match result {
                Ok((client, cancel)) => {
                    let version = block_on(query_version(&client));
                    let label = format!("PostgreSQL {version}");
                    let interrupt_token = cancel.clone();
                    return Ok(DbConnection::new(
                        Box::new(PostgresEngine { client, version }),
                        Box::new(move || {
                            let _ = block_on(tokio::time::timeout(
                                CANCEL_TIMEOUT,
                                interrupt_token.cancel_query(NoTls),
                            ));
                        }),
                        label,
                    ));
                }
                Err(e) => {
                    last_err = e;
                    // `Prefer` degrades to plaintext after one TLS failure.
                    if attempt == 0 && source.ssl_mode == crate::SslMode::Prefer {
                        tls_config = None;
                        continue;
                    }
                    break;
                }
            }
        }
        Err(format!(
            "cannot connect to {database} @ {}:{}: {last_err}",
            source.host, source.port
        ))
    }
}

async fn connect_once(
    source: &DataSource,
    database: &str,
    password: Option<&str>,
    tls_config: Option<rustls::ClientConfig>,
) -> Result<(Client, tokio_postgres::CancelToken), String> {
    let mut config = tokio_postgres::Config::new();
    config
        .host(&source.host)
        .port(source.port)
        .user(&source.user)
        .dbname(database);
    if let Some(pw) = password {
        config.password(pw);
    }
    config.connect_timeout(CONNECT_TIMEOUT);
    match source.ssl_mode {
        crate::SslMode::Disabled => config.ssl_mode(tokio_postgres::config::SslMode::Disable),
        crate::SslMode::Prefer => config.ssl_mode(tokio_postgres::config::SslMode::Prefer),
        crate::SslMode::Require => config.ssl_mode(tokio_postgres::config::SslMode::Require),
    };

    let client = match tls_config {
        Some(cfg) => {
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(cfg);
            let (client, conn) = tokio::time::timeout(CONNECT_TIMEOUT, config.connect(tls))
                .await
                .map_err(|_| "connection timed out".to_string())?
                .map_err(|e| e.to_string())?;
            tokio::spawn(async move {
                let _ = conn.await;
            });
            client
        }
        None => {
            let (client, conn) = tokio::time::timeout(CONNECT_TIMEOUT, config.connect(NoTls))
                .await
                .map_err(|_| "connection timed out".to_string())?
                .map_err(|e| e.to_string())?;
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
                    let _ = tx.rollback().await;
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
                    name: row.get::<_, String>(0),
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
                    .map(|r| Child::Schema { name: r.get(0) })
                    .collect()
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
                            "SELECT p.proname, pg_get_function_identity_arguments(p.oid), p.prokind \
                             FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
                             WHERE n.nspname = $1 ORDER BY p.proname",
                            &[schema],
                        )
                        .await
                        .map_err(err)?;
                    rows.into_iter()
                        .map(|r| {
                            let name: String = r.get(0);
                            let args: String = r.get(1);
                            let prokind: String = r.get(2);
                            Child::Function {
                                meta: FunctionMeta {
                                    schema: schema.clone(),
                                    signature: format!("{name}({args})"),
                                    is_procedure: prokind == "p",
                                    name,
                                },
                            }
                        })
                        .collect()
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
                        .map(|r| Child::Sequence {
                            meta: crate::SequenceMeta {
                                schema: schema.clone(),
                                name: r.get(0),
                            },
                        })
                        .collect()
                } else {
                    let sql = format!(
                        "SELECT c.relname, c.relkind FROM pg_class c \
                         JOIN pg_namespace n ON n.oid = c.relnamespace \
                         WHERE n.nspname = $1 AND c.relkind IN ({relkinds}) ORDER BY c.relname"
                    );
                    let rows = client.query(&sql, &[schema]).await.map_err(err)?;
                    rows.into_iter()
                        .map(|r| {
                            let name: String = r.get(0);
                            let relkind: String = r.get(1);
                            let tkind = match relkind.as_str() {
                                "v" => TableKind::View,
                                "m" => TableKind::MaterializedView,
                                _ => TableKind::Table,
                            };
                            Child::Table { name, kind: tkind }
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
        let qualified = format!("{}.{}", quote(schema), quote(name));
        let client = &self.client;

        let col_rows = client
            .query(
                "SELECT a.attname, format_type(a.atttypid, a.atttypmod), NOT a.attnotnull, \
                        pg_get_expr(d.adbin, d.adrelid), a.attidentity <> '' \
                 FROM pg_attribute a \
                 LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
                 WHERE a.attrelid = $1::regclass AND a.attnum > 0 AND NOT a.attisdropped \
                 ORDER BY a.attnum",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        let columns: Vec<ColumnMeta> = col_rows
            .into_iter()
            .map(|r| ColumnMeta {
                name: r.get(0),
                data_type: r.get(1),
                nullable: r.get(2),
                default: r.get(3),
                primary_key: false,
                auto_increment: r.get(4),
            })
            .collect();

        let pk_rows = client
            .query(
                "SELECT a.attname FROM pg_index i \
                 JOIN pg_attribute a ON a.attrelid = i.indrelid \
                     AND a.attnum = ANY(i.indkey) AND a.attnum > 0 \
                 WHERE i.indrelid = $1::regclass AND i.indisprimary",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        // ANY() loses key order; key order for multi-column pks is a known
        // limitation of this listing.
        let primary_key: Vec<String> = pk_rows.into_iter().map(|r| r.get(0)).collect();

        let fk_rows = client
            .query(
                "SELECT con.conname, con.conkey, con.confkey, nn.nspname, rc.relname \
                 FROM pg_constraint con \
                 JOIN pg_class rc ON rc.oid = con.confrelid \
                 JOIN pg_namespace nn ON nn.oid = rc.relnamespace \
                 WHERE con.contype = 'f' AND con.conrelid = $1::regclass ORDER BY con.conname",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        let mut foreign_keys = Vec::with_capacity(fk_rows.len());
        for r in &fk_rows {
            let cname: String = r.get(0);
            let conkey: Vec<i16> = r.get(1);
            let confkey: Vec<i16> = r.get(2);
            let ref_schema: String = r.get(3);
            let ref_table: String = r.get(4);
            foreign_keys.push(ForeignKeyMeta {
                name: cname,
                columns: column_names(client, &qualified, &conkey).await?,
                ref_schema,
                ref_table,
                ref_columns: column_names(client, &qualified, &confkey).await?,
            });
        }

        let idx_rows = client
            .query(
                "SELECT ic.relname, i.indisunique, i.indkey \
                 FROM pg_index i \
                 JOIN pg_class t ON t.oid = i.indrelid \
                 JOIN pg_class ic ON ic.oid = i.indexrelid \
                 WHERE i.indrelid = $1::regclass AND NOT i.indisprimary ORDER BY ic.relname",
                &[&qualified],
            )
            .await
            .map_err(err)?;
        let mut indexes = Vec::with_capacity(idx_rows.len());
        for r in &idx_rows {
            let name: String = r.get(0);
            let unique: bool = r.get(1);
            let indkey: Vec<i16> = r.get(2);
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
                        out.push(
                            (0..columns.len())
                                .map(|i| DbValue::Text(row.get::<_, String>(i)))
                                .collect(),
                        );
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

fn err(e: tokio_postgres::Error) -> String {
    if let Some(db) = e.as_db_error() {
        return db.message().to_string();
    }
    e.to_string()
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
                .map(|i| typed_value(row, i))
                .collect::<Result<Vec<_>, String>>()
                .unwrap_or_else(|_| vec![DbValue::Null; width]),
        );
    }
    (out, truncated)
}

fn typed_value(row: &Row, i: usize) -> Result<DbValue, String> {
    let ty = row.columns()[i].type_().clone();
    Ok(match ty {
        Type::BOOL => DbValue::Bool(row.try_get::<_, bool>(i).map_err(err)?),
        Type::INT2 => DbValue::Integer(row.try_get::<_, i16>(i).map_err(err)? as i64),
        Type::INT4 => DbValue::Integer(row.try_get::<_, i32>(i).map_err(err)? as i64),
        Type::INT8 | Type::OID => DbValue::Integer(row.try_get::<_, i64>(i).map_err(err)?),
        Type::FLOAT4 => DbValue::Real(row.try_get::<_, f32>(i).map_err(err)? as f64),
        Type::FLOAT8 => DbValue::Real(row.try_get::<_, f64>(i).map_err(err)?),
        Type::BYTEA => DbValue::Blob(row.try_get::<_, Vec<u8>>(i).map_err(err)?),
        Type::TIMESTAMPTZ | Type::TIMESTAMP => {
            let t = row.try_get::<_, std::time::SystemTime>(i).map_err(err)?;
            DbValue::Text(civil_timestamp(t))
        }
        _ => DbValue::Text(row.try_get::<_, String>(i).map_err(err)?),
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
             WHERE attrelid = $1::regclass AND attnum = ANY($2)",
            &[&table, &attnums],
        )
        .await
        .map_err(err)?;
    let mut named: Vec<(i16, String)> = rows.into_iter().map(|r| (r.get(0), r.get(1))).collect();
    named.sort_by_key(|(n, _)| *n);
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
                let _ = roots.add(cert);
            }
            builder.with_root_certificates(roots).with_no_client_auth()
        }
    }
}
