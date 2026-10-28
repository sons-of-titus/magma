//! Database tool state: configured data sources, live connections, schema
//! metadata, and the latest query result.  Slow work (connect, metadata,
//! queries) runs on the background pool and lands here through
//! [`DbEvent`], mirroring the VC ops pattern.

use std::collections::HashMap;
use std::sync::Arc;

use db::{DataSource, DbConnection, DriverKind, QueryOutcome, TableMeta};

/// Rows fetched per query before the result is flagged truncated.
pub const FETCH_LIMIT: usize = 200;
const HISTORY_CAP: usize = 50;

/// Background work landing on the foreground.
pub enum DbEvent {
    // Manual Debug/Clone below: the connection handle is neither Derive-able.
    /// A connect attempt finished; on success the connection is live and
    /// its schema came along.
    Connected {
        source_id: u64,
        connection: Arc<DbConnection>,
        tables: Result<Vec<TableMeta>, String>,
    },
    TablesDone {
        source_id: u64,
        tables: Result<Vec<TableMeta>, String>,
    },
    QueryDone {
        job: u64,
        result: Result<QueryOutcome, String>,
    },
}

impl std::fmt::Debug for DbEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbEvent::Connected { source_id, .. } => {
                write!(f, "DbEvent::Connected({source_id})")
            }
            DbEvent::TablesDone { source_id, .. } => {
                write!(f, "DbEvent::TablesDone({source_id})")
            }
            DbEvent::QueryDone { job, .. } => write!(f, "DbEvent::QueryDone({job})"),
        }
    }
}

impl Clone for DbEvent {
    fn clone(&self) -> Self {
        match self {
            DbEvent::Connected {
                source_id,
                connection,
                tables,
            } => DbEvent::Connected {
                source_id: *source_id,
                connection: Arc::clone(connection),
                tables: tables.clone(),
            },
            DbEvent::TablesDone { source_id, tables } => DbEvent::TablesDone {
                source_id: *source_id,
                tables: tables.clone(),
            },
            DbEvent::QueryDone { job, result } => DbEvent::QueryDone {
                job: *job,
                result: result.clone(),
            },
        }
    }
}

#[derive(Default)]
pub struct DbState {
    pub sources: Vec<DataSource>,
    next_source_id: u64,
    /// Live connections keyed by source id.
    pub connections: HashMap<u64, Arc<DbConnection>>,
    /// Schema of each connected source, from the latest refresh.
    pub tables: HashMap<u64, Vec<TableMeta>>,
    /// The source the console and grid target.
    pub active: Option<u64>,
    /// Incremented per dispatched query; late results with stale jobs drop.
    pub query_job: u64,
    pub last_result: Option<QueryOutcome>,
    pub last_error: Option<String>,
    /// Recent successful queries, most recent first.
    pub history: Vec<String>,
    pub busy_metadata: bool,
    pub busy_query: bool,
}

impl DbState {
    pub fn add_source(&mut self, name: &str, path: &str) -> u64 {
        self.next_source_id += 1;
        let id = self.next_source_id;
        self.sources.push(DataSource {
            id,
            name: name.to_string(),
            kind: DriverKind::Sqlite,
            path: path.to_string(),
        });
        if self.active.is_none() {
            self.active = Some(id);
        }
        id
    }

    /// After a workspace restore, resume id assignment past the highest
    /// restored id.
    pub fn next_id_after_restore(&mut self) {
        self.next_source_id = self.sources.iter().map(|s| s.id).max().unwrap_or(0);
        if self.active.is_none() {
            self.active = self.sources.first().map(|s| s.id);
        }
    }

    pub fn next_source_id(&self) -> u64 {
        self.next_source_id
    }

    pub fn remove_source(&mut self, id: u64) {
        self.sources.retain(|s| s.id != id);
        self.connections.remove(&id);
        self.tables.remove(&id);
        if self.active == Some(id) {
            self.active = self.sources.first().map(|s| s.id);
            self.last_result = None;
        }
    }

    pub fn source(&self, id: u64) -> Option<&DataSource> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// The data source the panel currently targets.
    pub fn active_source(&self) -> Option<&DataSource> {
        let id = self.active?;
        self.source(id)
    }

    pub fn is_connected(&self, id: u64) -> bool {
        self.connections.contains_key(&id)
    }

    pub fn take_next_job(&mut self) -> u64 {
        self.query_job += 1;
        self.query_job
    }

    /// Apply finished metadata; keeps stale refreshes for dead sources out.
    pub fn apply_tables(&mut self, source_id: u64, tables: Result<Vec<TableMeta>, String>) {
        self.busy_metadata = false;
        match tables {
            Ok(t) if self.connections.contains_key(&source_id) => {
                self.tables.insert(source_id, t);
                self.last_error = None;
            }
            Ok(_) => {}
            Err(e) => self.last_error = Some(e),
        }
    }

    pub fn apply_connected(
        &mut self,
        source_id: u64,
        connection: Arc<DbConnection>,
        tables: Result<Vec<TableMeta>, String>,
    ) {
        self.busy_metadata = false;
        self.connections.insert(source_id, connection);
        match tables {
            Ok(t) => {
                self.tables.insert(source_id, t);
                self.last_error = None;
            }
            Err(e) => {
                self.tables.remove(&source_id);
                self.last_error = Some(e);
            }
        }
    }

    /// Apply a finished query if its job is still current.
    pub fn apply_query(&mut self, job: u64, result: Result<QueryOutcome, String>) {
        if job != self.query_job {
            return;
        }
        self.busy_query = false;
        match result {
            Ok(outcome) => {
                self.last_result = Some(outcome);
                self.last_error = None;
            }
            Err(e) => self.last_error = Some(e),
        }
    }

    pub fn note_history(&mut self, sql: &str) {
        let sql = sql.trim();
        if sql.is_empty() {
            return;
        }
        self.history.retain(|h| h != sql);
        self.history.insert(0, sql.to_string());
        self.history.truncate(HISTORY_CAP);
    }

    /// Interrupt the active source's running statement.
    pub fn interrupt_active(&self) -> bool {
        match self.active.and_then(|id| self.connections.get(&id)) {
            Some(conn) => {
                conn.interrupt();
                true
            }
            None => false,
        }
    }
}
