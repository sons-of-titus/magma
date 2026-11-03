//! Database panel projections: explorer tree rows, grid model, tabs, and
//! the status line.  Everything reads the kernel's `DbState`; nothing here
//! mutates it.

use slint::{ModelRc, VecModel};

use crate::{DbCell, DbGridColumn, DbGridRow, DbTabInfo, DbTreeRow};
use db::{FolderKind};
use kernel::db::{DataTabKind, NodeState, NodeStatus};
use kernel::state::Editor;

/// Flatten the active source's tree, honoring expansion and the object
/// filter.  The root level collapses into the source row so the explorer
/// leads with "PostgreSQL @ host".
pub fn db_tree(ed: &Editor, filter: &str) -> Vec<DbTreeRow> {
    let Some(id) = ed.db.active else {
        return Vec::new();
    };
    let Some(source) = ed.db.source(id) else {
        return Vec::new();
    };
    let tree = ed.db.tree(id);
    let filter_lc = filter.to_ascii_lowercase();

    let mut rows = Vec::new();
    let root_open = tree.node("/").map(|n| n.expanded).unwrap_or(false);
    rows.push(DbTreeRow {
        key: "/".into(),
        depth: 0,
        kind: "source".into(),
        label: format!("{} @ {}", source.kind.label(), source.endpoint_label()).into(),
        detail: "".into(),
        status: status_of(tree, ed, id, "/").into(),
        expanded: root_open,
        has_children: true,
    });
    if !root_open || !ed.db.is_connected(id) {
        return rows;
    }

    let Some(root) = tree.node("/") else {
        return rows;
    };
    for child in &root.children {
        let db::Child::Database { name, .. } = child else {
            continue;
        };
        let db_key = format!("/{name}");
        push_container(
            &tree,
            ed,
            id,
            &db_key,
            1,
            "database",
            name,
            &filter_lc,
            &mut rows,
        );
        if expanded_now(&tree, &db_key) {
            walk(&tree, ed, id, &db_key, 2, &filter_lc, &mut rows);
        }
    }
    rows
}

fn walk(
    tree: &kernel::db::IntrospectTree,
    ed: &Editor,
    source_id: u64,
    parent_key: &str,
    depth: usize,
    filter: &str,
    rows: &mut Vec<DbTreeRow>,
) {
    let Some(node) = tree.node(parent_key) else {
        return;
    };
    let children: Vec<(String, String, String, String, bool)> = node
        .children
        .iter()
        .map(|child| identify(parent_key, child))
        .collect();
    for (key, kind, label, detail, has_children) in children {
        let matches = filter.is_empty()
            || label.to_ascii_lowercase().contains(filter)
            || (has_children && subtree_label_matches(tree, &key, filter));
        if !matches {
            continue;
        }
        let expanded = expanded_now(tree, &key);
        rows.push(DbTreeRow {
            key: key.clone().into(),
            depth: depth as i32,
            kind: kind.clone().into(),
            label: label.into(),
            detail: detail.into(),
            status: status_of(tree, ed, source_id, &key).into(),
            expanded,
            has_children,
        });
        if has_children && expanded {
            walk(tree, ed, source_id, &key, depth + 1, filter, rows);
        }
    }
}

/// (key, kind, label, detail, expandable) for one cached child.
fn identify(parent_key: &str, child: &db::Child) -> (String, String, String, String, bool) {
    match child {
        db::Child::Database { name, .. } => (
            format!("{parent_key}/{name}"),
            "database".into(),
            name.clone(),
            String::new(),
            true,
        ),
        db::Child::Schema { name } => (
            format!("{parent_key}/{name}"),
            "schema".into(),
            name.clone(),
            String::new(),
            true,
        ),
        db::Child::Folder { kind } => {
            let seg = folder_segment(*kind);
            (
                format!("{parent_key}/{seg}"),
                "folder".into(),
                folder_label(*kind).into(),
                String::new(),
                true,
            )
        }
        db::Child::Table { name, kind } => {
            let seg = match kind {
                db::TableKind::Table => "tables",
                _ => "views",
            };
            (
                format!("{parent_key}/{seg}/{name}"),
                match kind {
                    db::TableKind::Table => "table",
                    _ => "view",
                }
                .into(),
                name.clone(),
                String::new(),
                true,
            )
        }
        db::Child::Column { meta } => (
            format!("{parent_key}/columns/{}", meta.name),
            if meta.primary_key { "pk" } else { "column" }.into(),
            meta.name.clone(),
            meta.data_type.to_ascii_lowercase(),
            false,
        ),
        db::Child::Index { meta } => (
            format!("{parent_key}/indexes/{}", meta.name),
            "index".into(),
            meta.name.clone(),
            if meta.unique {
                format!("unique · {}", meta.columns.join(", "))
            } else {
                meta.columns.join(", ")
            },
            false,
        ),
        db::Child::ForeignKey { meta } => (
            format!("{parent_key}/fks/{}", meta.name),
            "fk".into(),
            meta.name.clone(),
            format!("{} → {}", meta.columns.join(", "), meta.ref_table),
            false,
        ),
        db::Child::Sequence { meta } => (
            format!("{parent_key}/sequences/{}", meta.name),
            "sequence".into(),
            meta.name.clone(),
            String::new(),
            false,
        ),
        db::Child::Function { meta } => (
            format!("{parent_key}/functions/{}", meta.name),
            "function".into(),
            meta.signature.clone(),
            String::new(),
            false,
        ),
    }
}

fn push_container(
    tree: &kernel::db::IntrospectTree,
    ed: &Editor,
    source_id: u64,
    key: &str,
    depth: usize,
    kind: &str,
    label: &str,
    filter: &str,
    rows: &mut Vec<DbTreeRow>,
) {
    let matches = filter.is_empty()
        || label.to_ascii_lowercase().contains(filter)
        || subtree_label_matches(tree, key, filter);
    if !matches {
        return;
    }
    rows.push(DbTreeRow {
        key: key.into(),
        depth: depth as i32,
        kind: kind.into(),
        label: label.into(),
        detail: "".into(),
        status: status_of(tree, ed, source_id, key).into(),
        expanded: expanded_now(tree, key),
        has_children: true,
    });
}

fn expanded_now(tree: &kernel::db::IntrospectTree, key: &str) -> bool {
    tree.node(key).map(|n| n.expanded).unwrap_or(false)
}

fn status_of(tree: &kernel::db::IntrospectTree, ed: &Editor, source_id: u64, key: &str) -> &'static str {
    match tree.node(key).map(|n: &NodeState| &n.status) {
        Some(NodeStatus::Loading) => "loading",
        Some(NodeStatus::Failed(_)) => "error",
        _ => {
            if ed.db.is_loading(source_id, key) {
                "loading"
            } else {
                "loaded"
            }
        }
    }
}

/// Does any cached descendant label match?  Keeps matching containers
/// visible under a filter even when their own label does not.
fn subtree_label_matches(tree: &kernel::db::IntrospectTree, key: &str, filter: &str) -> bool {
    let prefix = format!("{key}/");
    tree.nodes
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .any(|(_, state)| state.children.iter().any(|c| child_matches(c, filter)))
}

fn child_matches(child: &db::Child, filter: &str) -> bool {
    let label = match child {
        db::Child::Database { name, .. }
        | db::Child::Schema { name }
        | db::Child::Table { name, .. } => name,
        db::Child::Column { meta } => &meta.name,
        db::Child::Index { meta } => &meta.name,
        db::Child::ForeignKey { meta } => &meta.name,
        db::Child::Sequence { meta } => &meta.name,
        db::Child::Function { meta } => &meta.signature,
        db::Child::Folder { kind } => match kind {
            FolderKind::Tables => "tables",
            FolderKind::Views => "views",
            FolderKind::Sequences => "sequences",
            FolderKind::Functions => "functions",
        },
    };
    label.to_ascii_lowercase().contains(filter)
}

fn folder_segment(kind: FolderKind) -> &'static str {
    match kind {
        FolderKind::Tables => "tables",
        FolderKind::Views => "views",
        FolderKind::Sequences => "sequences",
        FolderKind::Functions => "functions",
    }
}

fn folder_label(kind: FolderKind) -> &'static str {
    folder_segment(kind)
}

// ── Tabs + grid ──────────────────────────────────────────────────────────────

pub fn db_tabs(ed: &Editor) -> Vec<DbTabInfo> {
    ed.db
        .tabs
        .iter()
        .map(|tab| DbTabInfo {
            id: tab.id as i32,
            title: tab.title.clone().into(),
            busy: tab.busy,
            pending: !tab.pending.is_empty(),
            active: ed.db.active_tab == Some(tab.id),
        })
        .collect()
}

pub fn db_grid_title(ed: &Editor) -> String {
    ed.db.active_tab().map(|t| t.title.clone()).unwrap_or_default()
}

/// Grid columns with widths (shell-owned resize state) and sort arrows.
pub fn db_columns(ed: &Editor, widths: &[f32]) -> Vec<DbGridColumn> {
    let Some(tab) = ed.db.active_tab() else {
        return Vec::new();
    };
    let Some(result) = tab.result() else {
        return Vec::new();
    };
    result
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| DbGridColumn {
            name: col.name.clone().into(),
            r#type: col.type_name.to_ascii_lowercase().into(),
            width: (widths.get(i).copied().unwrap_or(120.0)).max(40.0),
            sort: match tab.sort {
                Some((c, true)) if c == i => "asc".into(),
                Some((c, false)) if c == i => "desc".into(),
                _ => "".into(),
            },
        })
        .collect()
}

/// Grid rows with the pending-edit overlay applied: edited cells show the
/// new value and a marker; deleted rows render struck; the staged insert
/// row renders appended.
pub fn db_rows(ed: &Editor) -> Vec<DbGridRow> {
    let Some(tab) = ed.db.active_tab() else {
        return Vec::new();
    };
    let Some(result) = tab.result() else {
        return Vec::new();
    };
    let first_page_row = tab.page * kernel::db::FETCH_LIMIT;
    let mut rows: Vec<DbGridRow> = result
        .rows
        .iter()
        .enumerate()
        .map(|(row_no, cells)| {
            let deleted = tab
                .pending
                .ops
                .iter()
                .any(|o| matches!(o, db::EditOp::DeleteRow { row, .. } if *row == row_no));
            DbGridRow {
                cells: ModelRc::new(VecModel::from(
                    cells
                        .iter()
                        .enumerate()
                        .map(|(col, v)| {
                            let edited = tab
                                .pending
                                .ops
                                .iter()
                                .any(|o| matches!(o, db::EditOp::UpdateCell { row: r, col: c, .. } if *r == row_no && *c == col));
                            DbCell {
                                text: v.render().into(),
                                kind: v.kind().into(),
                                edited,
                            }
                        })
                        .collect::<Vec<_>>(),
                )),
                status: if deleted { "deleted".into() } else { "".into() },
                number: (first_page_row + row_no + 1).to_string().into(),
            }
        })
        .collect();

    // The staged insert row renders under the fetched page.
    if let Some(db::EditOp::InsertRow { values }) = tab
        .pending
        .ops
        .iter()
        .find(|o| matches!(o, db::EditOp::InsertRow { .. }))
    {
        rows.push(DbGridRow {
            cells: ModelRc::new(VecModel::from(
                values
                    .iter()
                    .map(|v| DbCell {
                        text: if v.is_null() { "".into() } else { v.render().into() },
                        kind: "inserted".into(),
                        edited: false,
                    })
                    .collect::<Vec<_>>(),
            )),
            status: "inserted".into(),
            number: "+".into(),
        });
    }
    rows
}

pub fn db_status(ed: &Editor) -> (String, bool) {
    let Some(tab) = ed.db.active_tab() else {
        return (String::new(), false);
    };
    if let Some(e) = &tab.error {
        return (e.clone(), true);
    }
    match tab.result() {
        Some(outcome) if outcome.is_query => {
            let n = outcome.rows.len();
            (
                if outcome.truncated {
                    format!("first {n} rows of the result set")
                } else {
                    format!("{n} rows")
                },
                false,
            )
        }
        Some(outcome) => (format!("{} rows affected", outcome.rows_affected), false),
        None => {
            if tab.busy {
                ("running…".to_string(), false)
            } else {
                (String::new(), false)
            }
        }
    }
}

pub fn db_row_count(ed: &Editor) -> String {
    ed.db
        .active_tab()
        .and_then(|t| t.result())
        .filter(|r| r.is_query)
        .map(|r| format!("{} rows", r.rows.len()))
        .unwrap_or_default()
}

pub fn db_elapsed(ed: &Editor) -> String {
    ed.db
        .active_tab()
        .and_then(|t| t.result())
        .map(|r| format!("{:.1} ms", r.elapsed_ms))
        .unwrap_or_default()
}

pub fn db_truncated(ed: &Editor) -> bool {
    ed.db
        .active_tab()
        .and_then(|t| t.result())
        .map(|r| r.truncated)
        .unwrap_or(false)
}

pub fn db_page_info(ed: &Editor) -> String {
    let Some(tab) = ed.db.active_tab() else {
        return String::new();
    };
    if !matches!(tab.kind, DataTabKind::Table { .. }) {
        return String::new();
    }
    let n = tab.result().map(|r| r.rows.len()).unwrap_or(0);
    if tab.page == 0 && n < kernel::db::FETCH_LIMIT && !tab.busy {
        return format!("rows 1–{n}");
    }
    let from = tab.page * kernel::db::FETCH_LIMIT + 1;
    format!("rows {from}–{} · page {}", from + n.saturating_sub(1), tab.page + 1)
}

pub fn db_can_edit(ed: &Editor) -> bool {
    match ed.db.active_tab().map(|t| &t.kind) {
        Some(DataTabKind::Table { detail, .. }) => detail
            .as_ref()
            .map(|d| !d.is_view() && d.pk_columns().is_some())
            .unwrap_or(false),
        _ => false,
    }
}

pub fn db_pending_count(ed: &Editor) -> i32 {
    ed.db
        .active_tab()
        .map(|t| t.pending.len() as i32)
        .unwrap_or(0)
}

pub fn db_has_tab(ed: &Editor) -> bool {
    ed.db.active_tab.is_some()
}

// ── Sources + connection header ──────────────────────────────────────────────

pub fn db_sources(ed: &Editor) -> Vec<String> {
    ed.db.sources.iter().map(|s| s.name.clone()).collect()
}

pub fn db_active_source_name(ed: &Editor) -> String {
    ed.db
        .active_source()
        .map(|s| s.name.clone())
        .unwrap_or_default()
}

pub fn db_connected_label(ed: &Editor) -> String {
    let id = match ed.db.active {
        Some(id) => id,
        None => return String::new(),
    };
    match ed.db.connections.get(&id) {
        Some(conn) => conn.server_label(),
        None => String::new(),
    }
}

pub fn db_busy(ed: &Editor) -> bool {
    !ed.db.loading.is_empty() || ed.db.tabs.iter().any(|t| t.busy)
}

pub fn db_querying(ed: &Editor) -> bool {
    ed.db
        .active
        .map(|id| ed.db.tabs.iter().any(|t| t.source_id == id && t.busy))
        .unwrap_or(false)
}
