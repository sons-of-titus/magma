//! Database tool domain model: data sources, metadata discovery, and
//! bounded query execution.  Engines implement [`DbConnection`]; the UI
//! speaks only in [`DataSource`], [`TableMeta`], and [`QueryOutcome`], so
//! new engines slot in without touching the panel.

mod sqlite;

pub use sqlite::SqliteConnection;

use std::sync::Mutex;

/// Supported database engines.  Each maps to one connection factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKind {
    Sqlite,
}

impl DriverKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriverKind::Sqlite => "sqlite",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DriverKind::Sqlite => "SQLite",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sqlite" => Some(DriverKind::Sqlite),
            _ => None,
        }
    }
}

/// A configured data source.  SQLite sources are file paths — no secrets —
/// and future networked engines hang their credentials off the source id in
/// platform credential storage, never in ordinary workspace configuration.
#[derive(Debug, Clone)]
pub struct DataSource {
    pub id: u64,
    pub name: String,
    pub kind: DriverKind,
    pub path: String,
}

/// One cell value, preserving the SQL NULL / empty-text distinction.
#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl DbValue {
    /// Display form for the results grid.  Blobs and long texts truncate —
    /// the full bytes stay reachable through queries, not the grid.
    pub fn render(&self) -> String {
        const LIMIT: usize = 256;
        match self {
            DbValue::Null => "NULL".to_string(),
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

    /// Grid styling hint: null | number | text | blob.
    pub fn kind(&self) -> &'static str {
        match self {
            DbValue::Null => "null",
            DbValue::Integer(_) | DbValue::Real(_) => "number",
            DbValue::Text(_) => "text",
            DbValue::Blob(_) => "blob",
        }
    }
}

/// One column of a table or view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub declared_type: String,
    pub primary_key: bool,
    pub not_null: bool,
}

/// An index on a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexMeta {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

/// A table or view in the connected database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMeta {
    pub name: String,
    pub is_view: bool,
    pub columns: Vec<ColumnMeta>,
    pub indexes: Vec<IndexMeta>,
}

/// Result of executing one statement.  Rows are bounded by the fetch limit;
/// `truncated` says whether more existed.
#[derive(Debug, Clone, Default)]
pub struct QueryOutcome {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<DbValue>>,
    pub truncated: bool,
    pub rows_affected: i64,
    pub is_query: bool,
    pub elapsed_ms: u64,
}

/// A live connection to one data source.  Engines are internally
/// synchronized so the kernel can hold them across threads; the interrupt
/// path stays lock-free relative to a running statement — cancelling must
/// never wait behind the query it is cancelling.
pub struct DbConnection {
    engine: Mutex<Box<dyn DbEngine>>,
    interruptor: Box<dyn Fn() + Send + Sync>,
}

pub trait DbEngine: Send {
    /// List tables and views with their columns and indexes.
    fn tables(&mut self) -> Result<Vec<TableMeta>, String>;
    /// Execute one statement (or the first statement of a script), fetching
    /// at most `max_rows` for SELECT-shaped results.
    fn execute(&mut self, sql: &str, max_rows: usize) -> Result<QueryOutcome, String>;
    /// Human-facing dialect name for status display.
    fn label(&self) -> &'static str;
}

impl DbConnection {
    pub fn new(engine: Box<dyn DbEngine>, interruptor: Box<dyn Fn() + Send + Sync>) -> Self {
        DbConnection {
            engine: Mutex::new(engine),
            interruptor,
        }
    }

    pub fn tables(&self) -> Result<Vec<TableMeta>, String> {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .tables()
    }

    pub fn execute(&self, sql: &str, max_rows: usize) -> Result<QueryOutcome, String> {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .execute(sql, max_rows)
    }

    pub fn interrupt(&self) {
        (self.interruptor)();
    }

    pub fn label(&self) -> &'static str {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .label()
    }
}

/// Open a connection for `source`.  The one place that maps a data source
/// kind to its engine.
pub fn connect(source: &DataSource) -> Result<DbConnection, String> {
    match source.kind {
        DriverKind::Sqlite => SqliteConnection::open(&source.path),
    }
}

/// Cheap validity probe used by the connection form's Test button.
pub fn test_connection(source: &DataSource) -> Result<(), String> {
    connect(source).map(|_| ())
}

/// Quote an identifier for the dialect.  SQLite and friends double-quote;
/// engines with backtick conventions override in their metadata helpers.
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A ready-made SELECT for the table browser.
pub fn select_all(table: &str, limit: usize) -> String {
    format!("SELECT * FROM {} LIMIT {limit}", quote_ident(table))
}
