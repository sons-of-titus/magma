//! Background work landing on the foreground.

/// Slow database work finishing off the foreground thread.  Every variant
/// carries the identity the foreground needs to apply it safely (stale
/// results for removed sources or superseded jobs drop on the floor).
/// Manual Clone: `Connection` is not Clone-able but is shared by Arc.
#[derive(Clone)]
pub enum DbEvent {
    /// A connect attempt finished; `error` carries the failure.
    Connected {
        source_id: u64,
        connection: Option<std::sync::Arc<db::Connection>>,
        error: Option<String>,
    },
    /// One tree node's children finished loading.
    Introspected {
        source_id: u64,
        path: String,
        children: Result<Vec<db::Child>, String>,
    },
    /// A table's metadata finished loading for its data tab.
    TableDetail {
        tab_id: u64,
        detail: Result<db::TableMeta, String>,
    },
    /// A script finished against a tab; several outcomes, one per statement.
    QueryDone {
        job: u64,
        tab_id: u64,
        results: Result<Vec<db::QueryOutcome>, String>,
    },
    /// Pending grid edits applied (or refused).
    EditsApplied {
        tab_id: u64,
        result: Result<db::EditResult, String>,
    },
    /// A connection-form probe finished; `token` matches the open form.
    TestDone {
        token: u64,
        result: Result<String, String>,
    },
    /// A source's completion catalog finished loading.
    Catalog {
        source_id: u64,
        catalog: Result<db::Catalog, String>,
    },
}

impl std::fmt::Debug for DbEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbEvent::Connected { source_id, error, .. } => {
                write!(f, "DbEvent::Connected({source_id}, error={error:?})")
            }
            DbEvent::Introspected { source_id, path, .. } => {
                write!(f, "DbEvent::Introspected({source_id}, {path})")
            }
            DbEvent::TableDetail { tab_id, .. } => {
                write!(f, "DbEvent::TableDetail(tab={tab_id})")
            }
            DbEvent::QueryDone { job, tab_id, .. } => {
                write!(f, "DbEvent::QueryDone(job={job}, tab={tab_id})")
            }
            DbEvent::EditsApplied { tab_id, .. } => {
                write!(f, "DbEvent::EditsApplied(tab={tab_id})")
            }
            DbEvent::TestDone { token, .. } => write!(f, "DbEvent::TestDone({token})"),
            DbEvent::Catalog { source_id, .. } => write!(f, "DbEvent::Catalog({source_id})"),
        }
    }
}
