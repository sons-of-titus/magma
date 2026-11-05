//! The database tool's live state: data sources, connections, the lazy
//! introspection cache per source, and the open data tabs (table browsers
//! and query results) with their pending edits.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use db::{Child, Connection, DataSource, NodeRef, PendingEdits, QueryOutcome, TableMeta};

const HISTORY_CAP: usize = 50;

/// Load state of one tree node's children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeStatus {
    /// Never expanded — the UI shows the disclosure arrow dimmed.
    NotLoaded,
    Loading,
    Loaded,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct NodeState {
    pub status: NodeStatus,
    pub children: Vec<Child>,
    /// UI expansion, tracked here so refreshes survive the projection.
    pub expanded: bool,
}

/// Per-source cache of the introspected tree, keyed by [`NodeRef::key`].
/// The root node always exists.
#[derive(Debug, Default)]
pub struct IntrospectTree {
    pub nodes: HashMap<String, NodeState>,
}

impl IntrospectTree {
    /// Empty cache.  The root materializes on first expand/refresh — an
    /// absent node reads as collapsed and unloaded everywhere.
    pub fn new() -> Self {
        IntrospectTree {
            nodes: HashMap::new(),
        }
    }

    pub fn node(&self, key: &str) -> Option<&NodeState> {
        self.nodes.get(key)
    }

    /// Mark a node loading; creating it if unseen (a refresh can target a
    /// failed node the cache dropped).
    pub fn mark_loading(&mut self, key: &str) {
        self.nodes
            .entry(key.to_string())
            .or_insert_with(|| NodeState {
                status: NodeStatus::NotLoaded,
                children: Vec::new(),
                expanded: true,
            })
            .status = NodeStatus::Loading;
        if let Some(node) = self.nodes.get_mut(key) {
            node.expanded = true;
        }
    }

    /// Store fetched children and drop every descendant cache — a refresh
    /// of a mid-tree node must not serve stale grandchildren.  `error`
    /// marks the node failed while keeping it addressable for a retry.
    pub fn apply_children(&mut self, key: &str, children: Vec<Child>, error: Option<String>) {
        let status = match error {
            Some(e) => NodeStatus::Failed(e),
            None => NodeStatus::Loaded,
        };
        let entry = self
            .nodes
            .entry(key.to_string())
            .or_insert_with(|| NodeState {
                status: NodeStatus::NotLoaded,
                children: Vec::new(),
                expanded: true,
            });
        entry.status = status;
        entry.children = children;
        entry.expanded = true;
        let prefix = format!("{key}/");
        self.nodes.retain(|k, _| !k.starts_with(&prefix) || k == key);
    }

    /// Expand a node whose children are already cached.
    pub fn mark_expanded(&mut self, key: &str) {
        if let Some(node) = self.nodes.get_mut(key) {
            node.expanded = true;
        }
    }

    /// Collapse a node (does not drop cached children).
    pub fn collapse(&mut self, key: &str) {
        if let Some(node) = self.nodes.get_mut(key) {
            node.expanded = false;
        }
    }

    /// Forget the whole cache (disconnect / full synchronize).
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// Collapse every node at once.
    pub fn collapse_all(&mut self) {
        for node in self.nodes.values_mut() {
            node.expanded = false;
        }
    }
}

/// One open tab in the results area: a table browser or a query result.
#[derive(Debug, Clone)]
pub struct DataTab {
    pub id: u64,
    pub source_id: u64,
    pub kind: DataTabKind,
    /// One outcome per executed statement; the grid shows `active_result`.
    pub results: Vec<QueryOutcome>,
    pub active_result: usize,
    pub title: String,
    /// Table-browser paging/sorting/filtering; query tabs ignore these.
    pub page: usize,
    pub sort: Option<(usize, bool)>,
    pub filter: String,
    /// Pending (unapplied) grid edits for table tabs.
    pub pending: PendingEdits,
    /// Grid cell selection for copy: (row, column).
    pub selected: Option<(usize, usize)>,
    /// A script is running against this tab; late results with stale jobs drop.
    pub busy: bool,
    pub job: u64,
    /// Last error surfaced for this tab (query or edit).
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DataTabKind {
    /// A table browser: lazy page fetches plus editable rows.
    Table {
        node: NodeRef,
        detail: Option<TableMeta>,
    },
    /// A query console run: read-only results.
    Query,
}

impl DataTab {
    pub fn table_detail(&self) -> Option<&TableMeta> {
        match &self.kind {
            DataTabKind::Table { detail, .. } => detail.as_ref(),
            DataTabKind::Query => None,
        }
    }

    pub fn table_node(&self) -> Option<&NodeRef> {
        match &self.kind {
            DataTabKind::Table { node, .. } => Some(node),
            DataTabKind::Query => None,
        }
    }

    /// The active result set, if any.
    pub fn result(&self) -> Option<&QueryOutcome> {
        self.results.get(self.active_result)
    }

    pub fn is_view(&self) -> bool {
        self.table_detail().map(|t| t.is_view()).unwrap_or(true)
    }
}

#[derive(Default)]
pub struct DbState {
    pub sources: Vec<DataSource>,
    next_source_id: u64,
    /// Live connections keyed by source id.
    pub connections: HashMap<u64, Arc<Connection>>,
    /// Introspection cache per source.
    pub trees: HashMap<u64, IntrospectTree>,
    /// Whole-schema snapshot per source, feeding console completion.
    pub catalogs: HashMap<u64, Arc<db::Catalog>>,
    /// The source the explorer, console, and new tabs target.
    pub active: Option<u64>,
    /// Nodes with introspection in flight: (source, node key).
    pub loading: HashSet<(u64, String)>,
    /// Open data tabs (table browsers and query results).
    pub tabs: Vec<DataTab>,
    pub active_tab: Option<u64>,
    next_tab_id: u64,
    /// SQL console buffer per source (slab key of the *sql: …* buffer).
    pub consoles: HashMap<u64, usize>,
    /// Passwords typed this session that were not saved to the keychain.
    /// Never persisted, never projected to the UI.
    pub session_passwords: HashMap<u64, String>,
    /// Incremented per dispatched query; late results with stale jobs drop.
    pub query_job: u64,
    /// Incremented per connection-form probe; the open form matches on it.
    pub test_token: u64,
    pub last_test: Option<Result<String, String>>,
    /// Recent successful statements, most recent first.
    pub history: Vec<String>,
    /// Last global connection error (connect form / connect command).
    pub last_error: Option<String>,
}

impl DbState {
    pub fn add_source(&mut self, source: DataSource) -> u64 {
        self.next_source_id += 1;
        let id = self.next_source_id;
        let mut source = source;
        source.id = id;
        self.sources.push(source);
        if self.active.is_none() {
            self.active = Some(id);
        }
        id
    }

    /// After a workspace restore, resume id assignment past the highest
    /// restored id.
    pub fn next_id_after_restore(&mut self) {
        self.next_source_id = self.sources.iter().map(|s| s.id).max().unwrap_or(0);
        self.next_tab_id = self.tabs.iter().map(|t| t.id).max().unwrap_or(0);
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
        self.trees.remove(&id);
        self.catalogs.remove(&id);
        self.consoles.remove(&id);
        self.session_passwords.remove(&id);
        self.loading.retain(|(s, _)| *s != id);
        self.tabs.retain(|t| t.source_id != id);
        if self.active_tab.is_some_and(|t| !self.tabs.iter().any(|x| x.id == t)) {
            self.active_tab = self.tabs.last().map(|t| t.id);
        }
        if self.active == Some(id) {
            self.active = self.sources.first().map(|s| s.id);
        }
    }

    pub fn source(&self, id: u64) -> Option<&DataSource> {
        self.sources.iter().find(|s| s.id == id)
    }

    pub fn source_mut(&mut self, id: u64) -> Option<&mut DataSource> {
        self.sources.iter_mut().find(|s| s.id == id)
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

    pub fn take_next_tab_id(&mut self) -> u64 {
        self.next_tab_id += 1;
        self.next_tab_id
    }

    pub fn take_next_test_token(&mut self) -> u64 {
        self.test_token += 1;
        self.test_token
    }

    /// The password a connect may use: what the user typed this session
    /// (unsaved) wins, then the keychain.  The caller resolves keychain
    /// storage; this only orchestrates.
    pub fn password_for(&self, id: u64, keychain: impl Fn() -> Option<String>) -> Option<String> {
        if let Some(pw) = self.session_passwords.get(&id) {
            return Some(pw.clone());
        }
        keychain()
    }

    // ── Tree ────────────────────────────────────────────────────────────

    pub fn tree(&self, id: u64) -> &IntrospectTree {
        static EMPTY: std::sync::LazyLock<IntrospectTree> =
            std::sync::LazyLock::new(IntrospectTree::new);
        self.trees.get(&id).unwrap_or(&EMPTY)
    }

    pub fn tree_mut(&mut self, id: u64) -> &mut IntrospectTree {
        self.trees.entry(id).or_default()
    }

    /// Apply a finished introspection; stale results for dead sources or
    /// superseded loads drop.
    pub fn apply_introspected(&mut self, source_id: u64, path: &str, children: Result<Vec<Child>, String>) {
        self.loading.remove(&(source_id, path.to_string()));
        let Some(tree) = self.trees.get_mut(&source_id) else {
            return;
        };
        match children {
            Ok(children) => {
                if self.connections.contains_key(&source_id) {
                    tree.apply_children(path, children, None);
                }
            }
            Err(e) => tree.apply_children(path, Vec::new(), Some(e)),
        }
    }

    pub fn is_loading(&self, source_id: u64, path: &str) -> bool {
        self.loading.contains(&(source_id, path.to_string()))
    }

    // ── Tabs ────────────────────────────────────────────────────────────

    pub fn tab(&self, id: u64) -> Option<&DataTab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn tab_mut(&mut self, id: u64) -> Option<&mut DataTab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn active_tab(&self) -> Option<&DataTab> {
        let id = self.active_tab?;
        self.tab(id)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut DataTab> {
        let id = self.active_tab?;
        self.tab_mut(id)
    }

    /// The source a console buffer belongs to, by buffer slab key.
    pub fn console_source(&self, buffer_key: usize) -> Option<u64> {
        self.consoles
            .iter()
            .find(|(_, k)| **k == buffer_key)
            .map(|(s, _)| *s)
    }

    pub fn close_tab(&mut self, id: u64) {
        self.tabs.retain(|t| t.id != id);
        if self.active_tab == Some(id) {
            self.active_tab = self.tabs.last().map(|t| t.id);
        }
    }

    /// Apply a finished script to its tab if the job is still current.
    pub fn apply_query(&mut self, job: u64, tab_id: u64, results: Result<Vec<QueryOutcome>, String>) {
        let Some(tab) = self.tab_mut(tab_id) else {
            return;
        };
        if tab.job != job {
            return;
        }
        tab.busy = false;
        match results {
            Ok(results) => {
                tab.results = results;
                tab.active_result = 0;
                tab.error = None;
            }
            Err(e) => tab.error = Some(e),
        }
    }

    /// Apply finished edits; success clears the pending set.
    pub fn apply_edits(&mut self, tab_id: u64, result: Result<db::EditResult, String>) {
        let Some(tab) = self.tab_mut(tab_id) else {
            return;
        };
        match result {
            Ok(_) => {
                tab.pending.clear();
                tab.error = None;
            }
            Err(e) => tab.error = Some(e),
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

#[cfg(test)]
mod tests {
    use super::*;
    use db::DriverKind;

    fn source(name: &str) -> DataSource {
        DataSource {
            id: 0,
            name: name.to_string(),
            kind: DriverKind::Sqlite,
            host: ":memory:".to_string(),
            port: 0,
            database: String::new(),
            user: String::new(),
            ssl_mode: Default::default(),
            trust_cert: false,
            save_password: false,
            url: None,
        }
    }

    fn tab(id: u64) -> DataTab {
        DataTab {
            id,
            source_id: 1,
            kind: DataTabKind::Query,
            results: Vec::new(),
            active_result: 0,
            title: "query".into(),
            page: 0,
            sort: None,
            filter: String::new(),
            pending: PendingEdits::default(),
            selected: None,
            busy: false,
            job: 0,
            error: None,
        }
    }

    #[test]
    fn sources_get_sequential_ids_and_active_default() {
        let mut st = DbState::default();
        let a = st.add_source(source("a"));
        let b = st.add_source(source("b"));
        assert_eq!((a, b), (1, 2));
        assert_eq!(st.active, Some(a));
        assert_eq!(st.active_source().unwrap().name, "a");
        st.remove_source(a);
        assert_eq!(st.active, Some(b));
    }

    #[test]
    fn tree_refresh_invalidates_descendants_only() {
        let mut st = DbState::default();
        let id = st.add_source(source("t"));
        st.tree_mut(id).apply_children("/", vec![Child::Database {
            name: "main".into(),
            label: "main".into(),
        }], None);
        st.tree_mut(id)
            .apply_children("/main", vec![Child::Schema { name: "public".into() }], None);
        st.tree_mut(id).apply_children(
            "/main/public",
            vec![Child::Folder {
                kind: db::FolderKind::Tables,
            }],
            None,
        );
        // Refreshing /main drops /main/public but keeps the root.
        let mut tree = IntrospectTree::new();
        tree.apply_children("/", vec![Child::Database {
            name: "main".into(),
            label: "main".into(),
        }], None);
        tree.apply_children("/main", vec![Child::Schema { name: "public".into() }], None);
        tree.apply_children(
            "/main/public",
            vec![Child::Folder {
                kind: db::FolderKind::Tables,
            }],
            None,
        );
        tree.apply_children("/main", vec![Child::Schema { name: "public".into() }], None);
        assert!(tree.node("/main/public").is_none(), "descendants dropped on refresh");
        assert!(tree.node("/").is_some());
    }

    #[test]
    fn stale_query_results_and_tab_removals_are_safe() {
        let mut st = DbState::default();
        st.add_source(source("t"));
        let tab_id = st.take_next_tab_id();
        st.tabs.push(tab(tab_id));
        st.active_tab = Some(tab_id);
        let job = st.take_next_job();
        st.tab_mut(tab_id).unwrap().job = job;
        st.tab_mut(tab_id).unwrap().busy = true;
        // A stale job id must not touch the tab.
        st.apply_query(job + 5, tab_id, Ok(vec![]));
        assert!(st.tab(tab_id).unwrap().busy);
        st.apply_query(job, tab_id, Ok(vec![]));
        assert!(!st.tab(tab_id).unwrap().busy);
        // Removing the source removes its tabs.
        st.remove_source(1);
        assert!(st.tabs.is_empty());
    }

    #[test]
    fn successful_edits_clear_pending_changes() {
        let mut st = DbState::default();
        st.add_source(source("t"));
        let tab_id = st.take_next_tab_id();
        let mut t = tab(tab_id);
        t.pending.ops.push(db::EditOp::InsertRow { values: vec![] });
        st.tabs.push(t);
        st.apply_edits(tab_id, Ok(db::EditResult::default()));
        assert!(st.tab(tab_id).unwrap().pending.is_empty());
        st.tab_mut(tab_id).unwrap().pending.ops.push(db::EditOp::InsertRow { values: vec![] });
        st.apply_edits(tab_id, Err("boom".into()));
        assert!(!st.tab(tab_id).unwrap().pending.is_empty());
        assert_eq!(st.tab(tab_id).unwrap().error.as_deref(), Some("boom"));
    }

    #[test]
    fn session_password_wins_over_keychain() {
        let mut st = DbState::default();
        let id = st.add_source(source("t"));
        st.session_passwords.insert(id, "typed".into());
        assert_eq!(
            st.password_for(id, || Some("chain".into())).as_deref(),
            Some("typed")
        );
        st.session_passwords.clear();
        assert_eq!(st.password_for(id, || Some("chain".into())).as_deref(), Some("chain"));
        assert_eq!(st.password_for(id, || None), None);
    }

    #[test]
    fn node_parse_round_trips() {
        for key in [
            NodeRef::Root.key().as_str(),
            &NodeRef::Database("main".into()).key(),
            &NodeRef::Schema {
                database: "db".into(),
                schema: "public".into(),
            }
            .key(),
            &NodeRef::Folder {
                database: "db".into(),
                schema: "public".into(),
                kind: db::FolderKind::Tables,
            }
            .key(),
            &NodeRef::Table {
                database: "db".into(),
                schema: "public".into(),
                name: "users".into(),
                is_view: true,
            }
            .key(),
        ] {
            let parsed = NodeRef::parse(key).expect(key);
            assert_eq!(parsed.key(), key);
        }
    }
}
