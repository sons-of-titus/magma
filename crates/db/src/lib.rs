//! Database tool domain model: data sources, normalized metadata, lazy
//! introspection, bounded query execution, and transactional table edits.
//! Engines implement [`DbEngine`]; the kernel and UI speak only in the
//! types here, so a new driver slots in without touching either.

mod catalog;
mod edit;
mod engine;
mod mssql;
mod mysql;
mod postgres;
mod postgres_catalog;
mod query;
mod sqlite;
mod sqlscript;

pub use catalog::{Catalog, CatalogColumn, CatalogTable};
pub use edit::{
    EditOp, EditResult, PendingEdits, TableRef, generate_ops, insert_template, quote_table,
};
pub use engine::{Child, Connection, DbEngine, FolderKind, NodeRef};
pub use query::page_query;
pub use sqlscript::{split_statements, statement_at};

use std::sync::{LazyLock, Mutex};

/// How a connection treats TLS.  `Prefer` falls back to plaintext when the
/// handshake fails; only `Require` (optionally with `trust_cert`) upgrades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SslMode {
    Disabled,
    #[default]
    Prefer,
    Require,
}

impl SslMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SslMode::Disabled => "disabled",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "disabled" => Some(SslMode::Disabled),
            "prefer" => Some(SslMode::Prefer),
            "require" => Some(SslMode::Require),
            _ => None,
        }
    }
}

/// Supported database engines.  Each maps to one connection factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKind {
    Sqlite,
    Postgres,
    MySql,
    MariaDb,
    SqlServer,
}

impl DriverKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriverKind::Sqlite => "sqlite",
            DriverKind::Postgres => "postgres",
            DriverKind::MySql => "mysql",
            DriverKind::MariaDb => "mariadb",
            DriverKind::SqlServer => "sqlserver",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DriverKind::Sqlite => "SQLite",
            DriverKind::Postgres => "PostgreSQL",
            DriverKind::MySql => "MySQL",
            DriverKind::MariaDb => "MariaDB",
            DriverKind::SqlServer => "SQL Server",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sqlite" => Some(DriverKind::Sqlite),
            "postgres" | "postgresql" => Some(DriverKind::Postgres),
            "mysql" => Some(DriverKind::MySql),
            "mariadb" => Some(DriverKind::MariaDb),
            "sqlserver" | "mssql" => Some(DriverKind::SqlServer),
            _ => None,
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            DriverKind::Sqlite => 0,
            DriverKind::Postgres => 5432,
            DriverKind::MySql | DriverKind::MariaDb => 3306,
            DriverKind::SqlServer => 1433,
        }
    }

    /// Networked engines keep one connection per data source and expose its
    /// whole catalog tree; SQLite is file-local.
    pub fn is_embedded(&self) -> bool {
        matches!(self, DriverKind::Sqlite)
    }
}

/// A configured data source: non-secret connection facts only.  Passwords
/// live in the secrets file keyed by the source id, never here and never
/// in the settings file.
#[derive(Debug, Clone)]
pub struct DataSource {
    pub id: u64,
    pub name: String,
    pub kind: DriverKind,
    /// Host or, for SQLite, the database file path.
    pub host: String,
    pub port: u16,
    /// Service name; for SQLite the file path lives in `host`.
    pub database: String,
    pub user: String,
    pub ssl_mode: SslMode,
    /// Accept a server certificate that does not validate (self-signed).
    pub trust_cert: bool,
    /// The password is persisted in the secrets file, not session-only.
    pub save_password: bool,
    /// Full connection URL; when set it drives the connection instead of
    /// the individual fields.
    pub url: Option<String>,
}

impl DataSource {
    /// The host label the explorer shows for the connection node.
    pub fn endpoint_label(&self) -> String {
        if self.kind.is_embedded() {
            self.host.clone()
        } else {
            format!("{}@{}:{}", self.user, self.host, self.port)
        }
    }
}

/// One cell value, preserving the SQL NULL / empty-text distinction.
#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
    Null,
    Bool(bool),
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl DbValue {
    /// Display form for the results grid.  Long texts truncate — the full
    /// value stays reachable through the value viewer and queries.
    pub fn render(&self) -> String {
        const LIMIT: usize = 256;
        match self {
            DbValue::Null => "NULL".to_string(),
            DbValue::Bool(b) => b.to_string(),
            DbValue::Integer(i) => i.to_string(),
            DbValue::Real(f) => f.to_string(),
            DbValue::Text(t) => {
                if t.chars().count() <= LIMIT {
                    t.clone()
                } else {
                    let cut: String = t.chars().take(LIMIT).collect();
                    format!("{cut}…")
                }
            }
            DbValue::Blob(b) => format!("<blob {} bytes>", b.len()),
        }
    }

    /// Full lossless text for the value viewer.
    pub fn full_text(&self) -> String {
        match self {
            DbValue::Text(t) => t.clone(),
            other => other.render(),
        }
    }

    /// Grid styling hint: null | bool | number | text | blob.
    pub fn kind(&self) -> &'static str {
        match self {
            DbValue::Null => "null",
            DbValue::Bool(_) => "bool",
            DbValue::Integer(_) | DbValue::Real(_) => "number",
            DbValue::Text(_) => "text",
            DbValue::Blob(_) => "blob",
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, DbValue::Null)
    }

    /// Human-parseable value from grid text: typed NULL, bool, number,
    /// otherwise text.  Cell editing feeds this.
    pub fn parse(text: &str) -> DbValue {
        let t = text.trim();
        if t.eq_ignore_ascii_case("null") {
            return DbValue::Null;
        }
        if t.eq_ignore_ascii_case("true") {
            return DbValue::Bool(true);
        }
        if t.eq_ignore_ascii_case("false") {
            return DbValue::Bool(false);
        }
        if let Ok(i) = t.parse::<i64>() {
            return DbValue::Integer(i);
        }
        if let Ok(f) = t.parse::<f64>() {
            return DbValue::Real(f);
        }
        DbValue::Text(text.to_string())
    }
}

/// One column of a table or view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
    pub auto_increment: bool,
}

/// A foreign-key mapping on a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKeyMeta {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

/// An index on a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexMeta {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableKind {
    Table,
    View,
    MaterializedView,
}

/// A table or view with its full detail (columns, keys, indexes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMeta {
    pub database: String,
    pub schema: String,
    pub name: String,
    pub kind: TableKind,
    pub columns: Vec<ColumnMeta>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyMeta>,
    pub indexes: Vec<IndexMeta>,
}

impl TableMeta {
    /// Column index of each primary-key part.  Tables without a
    /// discoverable key cannot support safe editing.
    pub fn pk_columns(&self) -> Option<Vec<usize>> {
        if self.primary_key.is_empty() {
            return None;
        }
        let mut idx = Vec::with_capacity(self.primary_key.len());
        for pk in &self.primary_key {
            idx.push(self.columns.iter().position(|c| &c.name == pk)?);
        }
        Some(idx)
    }

    pub fn is_view(&self) -> bool {
        !matches!(self.kind, TableKind::Table)
    }

    /// Foreign key covering `column`, together with the referenced column name.
    pub fn fk_for_column(&self, column: &str) -> Option<(&ForeignKeyMeta, &str)> {
        for fk in &self.foreign_keys {
            if let Some(pos) = fk.columns.iter().position(|c| c == column) {
                let ref_col = fk.ref_columns.get(pos).map(String::as_str).unwrap_or("");
                return Some((fk, ref_col));
            }
        }
        None
    }
}

/// A sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceMeta {
    pub schema: String,
    pub name: String,
}

/// A function or procedure, shown with its argument signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionMeta {
    pub schema: String,
    pub name: String,
    pub signature: String,
    pub is_procedure: bool,
}

/// Result of executing one statement.  Rows are bounded by the fetch limit;
/// `truncated` says whether more existed.
#[derive(Debug, Clone, Default)]
pub struct QueryOutcome {
    pub statement: String,
    pub columns: Vec<ColumnOut>,
    pub rows: Vec<Vec<DbValue>>,
    pub truncated: bool,
    pub rows_affected: i64,
    pub is_query: bool,
    pub elapsed_ms: u64,
}

/// One output column of a result set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnOut {
    pub name: String,
    pub type_name: String,
}

/// The quoting and null-safe-comparison conventions of one dialect.  Edit
/// generation uses it; the UI never hardcodes engine syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Ansi,
    Postgres,
    MySql,
    SqlServer,
    Sqlite,
}

impl Dialect {
    /// Quote an identifier.
    pub fn quote_ident(&self, name: &str) -> String {
        match self {
            Dialect::MySql => format!("`{}`", name.replace('`', "``")),
            Dialect::SqlServer => format!("[{}]", name.replace(']', "]]")),
            _ => format!("\"{}\"", name.replace('"', "\"\"")),
        }
    }

    /// Render a value as a SQL literal for edit statements.
    pub fn literal(&self, v: &DbValue) -> String {
        match v {
            DbValue::Null => "NULL".to_string(),
            DbValue::Bool(b) => match self {
                Dialect::Sqlite => format!("{}", *b as i32),
                _ => b.to_string(),
            },
            DbValue::Integer(i) => i.to_string(),
            DbValue::Real(f) => {
                if f.is_finite() {
                    format!("{f}")
                } else {
                    "NULL".to_string()
                }
            }
            DbValue::Text(t) => format!("'{}'", t.replace('\'', "''")),
            DbValue::Blob(b) => {
                let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
                match self {
                    Dialect::SqlServer => format!("0x{hex}"),
                    _ => format!("X'{hex}'"),
                }
            }
        }
    }

    /// Null-safe equality for optimistic-concurrency predicates.
    pub fn null_safe_eq(&self, col: &str, lit: &str) -> String {
        let q = self.quote_ident(col);
        if lit == "NULL" {
            return format!("{q} IS NULL");
        }
        match self {
            Dialect::MySql => format!("{q} <=> {lit}"),
            Dialect::Postgres => format!("{q} IS NOT DISTINCT FROM {lit}"),
            _ => format!("{q} = {lit}"),
        }
    }
}

/// One shared tokio runtime for all networked drivers — connect, query, and
/// cancel requests all block on it from the background pool's threads.
static RUNTIME: LazyLock<Mutex<Option<tokio::runtime::Runtime>>> =
    LazyLock::new(|| Mutex::new(Some(tokio::runtime::Runtime::new().expect("db tokio runtime"))));

pub(crate) fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let guard = RUNTIME.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .expect("db runtime")
        .block_on(fut)
}

/// Open a connection for `source`, asking the caller (which resolves
/// stored passwords) for the password when the driver needs one.  The one
/// place that maps a data source kind to its engine.
pub fn connect(source: &DataSource, password: Option<&str>) -> Result<Connection, String> {
    match source.kind {
        DriverKind::Sqlite => sqlite::SqliteConnection::open(&source.host),
        DriverKind::Postgres => postgres::PostgresConnection::open(source, password),
        DriverKind::MySql | DriverKind::MariaDb => mysql::MySqlConnection::open(source, password),
        DriverKind::SqlServer => mssql::SqlServerConnection::open(source, password),
    }
}

/// Cheap validity probe used by the connection form's Test button; returns
/// the server label on success.
pub fn test_connection(source: &DataSource, password: Option<&str>) -> Result<String, String> {
    let conn = connect(source, password)?;
    Ok(conn.server_label())
}

/// Quote an identifier with the SQLite/ANSI convention.
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
