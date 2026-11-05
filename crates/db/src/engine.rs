//! The engine contract and the shared connection handle.  Introspection is
//! lazy and node-scoped: the UI asks for one node's children at a time, so
//! a large catalog never loads wholesale.

use std::sync::Mutex;

use crate::{Dialect, PendingEdits, QueryOutcome, TableMeta};

/// Which typed folder a schema shows.  Engines return folders so they can
/// hide empty or unsupported categories per family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderKind {
    Tables,
    Views,
    Sequences,
    Functions,
}

/// A node address inside one connection's tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeRef {
    Root,
    Database(String),
    Schema { database: String, schema: String },
    Folder { database: String, schema: String, kind: FolderKind },
    Table { database: String, schema: String, name: String, is_view: bool },
}

impl NodeRef {
    /// Stable string key for caches and UI wiring.
    pub fn key(&self) -> String {
        fn folder(k: FolderKind) -> &'static str {
            match k {
                FolderKind::Tables => "tables",
                FolderKind::Views => "views",
                FolderKind::Sequences => "sequences",
                FolderKind::Functions => "functions",
            }
        }
        match self {
            NodeRef::Root => "/".to_string(),
            NodeRef::Database(d) => format!("/{d}"),
            NodeRef::Schema { database, schema } => format!("/{database}/{schema}"),
            NodeRef::Folder { database, schema, kind } => {
                format!("/{database}/{schema}/{}", folder(*kind))
            }
            NodeRef::Table { database, schema, name, is_view } => {
                format!("/{database}/{schema}/{}/{name}", if *is_view { "views" } else { "tables" })
            }
        }
    }

    pub fn parse(key: &str) -> Option<NodeRef> {
        let parts: Vec<&str> = key.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            [] => Some(NodeRef::Root),
            [d] => Some(NodeRef::Database(d.to_string())),
            [d, s] => Some(NodeRef::Schema { database: d.to_string(), schema: s.to_string() }),
            [d, s, k] => Some(NodeRef::Folder {
                database: d.to_string(),
                schema: s.to_string(),
                kind: match *k {
                    "tables" => FolderKind::Tables,
                    "views" => FolderKind::Views,
                    "sequences" => FolderKind::Sequences,
                    "functions" => FolderKind::Functions,
                    _ => return None,
                },
            }),
            [d, s, k, n] => Some(NodeRef::Table {
                database: d.to_string(),
                schema: s.to_string(),
                name: n.to_string(),
                is_view: *k == "views",
            }),
            _ => None,
        }
    }
}

/// One child node returned by an introspection step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Database { name: String, label: String },
    Schema { name: String },
    Folder { kind: FolderKind },
    Table { name: String, kind: crate::TableKind },
    Column { meta: crate::ColumnMeta },
    Index { meta: crate::IndexMeta },
    ForeignKey { meta: crate::ForeignKeyMeta },
    Sequence { meta: crate::SequenceMeta },
    Function { meta: crate::FunctionMeta },
}

/// One live connection.  Engines are internally synchronized so the kernel
/// can hold them across threads; the interrupt path stays lock-free
/// relative to a running statement — cancelling must never wait behind the
/// query it is cancelling.
pub struct Connection {
    engine: Mutex<Box<dyn DbEngine>>,
    interruptor: Box<dyn Fn() + Send + Sync>,
    label: String,
}

pub trait DbEngine: Send {
    /// Children of one tree node; the connection drives lazily per node.
    fn introspect(&mut self, node: &NodeRef) -> Result<Vec<Child>, String>;
    /// Full detail (columns, keys, indexes) for one table or view.
    fn table_detail(&mut self, node: &NodeRef) -> Result<TableMeta, String>;
    /// Run a script statement-by-statement, one outcome per statement;
    /// stops at the first error.  SELECT-shaped statements fetch at most
    /// `max_rows` and flag `truncated` when more existed.
    fn execute_script(&mut self, sql: &str, max_rows: usize) -> Result<Vec<QueryOutcome>, String>;
    /// Apply pending grid edits in one transaction against `table`.
    fn apply_edits(&mut self, table: &TableMeta, edits: &PendingEdits)
        -> Result<crate::EditResult, String>;
    /// Server version string for the connection status line.
    fn server_version(&self) -> String;
    fn dialect(&self) -> Dialect;
    /// Every table and view with its columns.  Engines with a cheap bulk
    /// query override this; the default walks the lazy tree.
    fn catalog(&mut self) -> Result<crate::Catalog, String> {
        crate::catalog::walk(self)
    }
}

impl Connection {
    pub fn new(
        engine: Box<dyn DbEngine>,
        interruptor: Box<dyn Fn() + Send + Sync>,
        label: String,
    ) -> Self {
        Connection { engine: Mutex::new(engine), interruptor, label }
    }

    pub fn introspect(&self, node: &NodeRef) -> Result<Vec<Child>, String> {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .introspect(node)
    }

    pub fn table_detail(&self, node: &NodeRef) -> Result<TableMeta, String> {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .table_detail(node)
    }

    pub fn catalog(&self) -> Result<crate::Catalog, String> {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .catalog()
    }

    pub fn execute_script(&self, sql: &str, max_rows: usize) -> Result<Vec<QueryOutcome>, String> {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .execute_script(sql, max_rows)
    }

    pub fn apply_edits(&self, table: &TableMeta, edits: &PendingEdits) -> Result<crate::EditResult, String> {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .apply_edits(table, edits)
    }

    pub fn interrupt(&self) {
        (self.interruptor)();
    }

    /// "PostgreSQL 16.2" style label for the status line.
    pub fn server_label(&self) -> String {
        self.label.clone()
    }

    pub fn dialect(&self) -> Dialect {
        self.engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dialect()
    }
}
