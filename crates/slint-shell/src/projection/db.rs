//! Database panel projections: schema tree rows, result grid, status line.

use std::collections::HashSet;

use slint::{ModelRc, VecModel};

use crate::{DbCell, DbColumn, DbRow, DbTreeRow};
use kernel::state::Editor;

/// Flatten the connected source's schema: tables and views with their
/// columns (primary keys marked) and indexes under expanded tables.
pub fn db_tree(ed: &Editor, expanded: &HashSet<usize>) -> Vec<DbTreeRow> {
    let Some(tables) = ed.db.active.and_then(|id| ed.db.tables.get(&id)) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for (i, table) in tables.iter().enumerate() {
        let kind = if table.is_view { "view" } else { "table" };
        let open = expanded.contains(&i);
        rows.push(DbTreeRow {
            depth: 0,
            kind: kind.into(),
            label: table.name.clone().into(),
            detail: format!("{} col", table.columns.len()).into(),
            expanded: open,
            idx: i as i32,
        });
        if !open {
            continue;
        }
        for col in &table.columns {
            rows.push(DbTreeRow {
                depth: 1,
                kind: if col.primary_key { "pk" } else { "column" }.into(),
                label: col.name.clone().into(),
                detail: if col.declared_type.is_empty() {
                    "".into()
                } else {
                    col.declared_type.to_ascii_lowercase().into()
                },
                expanded: false,
                idx: -1,
            });
        }
        for index in &table.indexes {
            rows.push(DbTreeRow {
                depth: 1,
                kind: "index".into(),
                label: index.name.clone().into(),
                detail: if index.unique {
                    format!("unique · {}", index.columns.join(", ")).into()
                } else {
                    index.columns.join(", ").into()
                },
                expanded: false,
                idx: -1,
            });
        }
    }
    rows
}

pub fn db_columns(ed: &Editor) -> Vec<DbColumn> {
    ed.db
        .last_result
        .as_ref()
        .map(|r| {
            r.columns
                .iter()
                .map(|c| DbColumn {
                    name: c.clone().into(),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn db_rows(ed: &Editor) -> Vec<DbRow> {
    let Some(result) = ed.db.last_result.as_ref() else {
        return Vec::new();
    };
    result
        .rows
        .iter()
        .map(|cells| DbRow {
            cells: ModelRc::new(VecModel::from(
                cells
                    .iter()
                    .map(|v| DbCell {
                        text: v.render().into(),
                        kind: v.kind().into(),
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect()
}

/// (status text, is_error).  Query errors outrank metadata ones.
pub fn db_status(ed: &Editor) -> (String, bool) {
    if let Some(e) = &ed.db.last_error {
        return (e.clone(), true);
    }
    match ed.db.last_result.as_ref() {
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
        None => (String::new(), false),
    }
}

pub fn db_row_count(ed: &Editor) -> String {
    ed.db
        .last_result
        .as_ref()
        .filter(|r| r.is_query)
        .map(|r| format!("{} rows", r.rows.len()))
        .unwrap_or_default()
}

pub fn db_elapsed(ed: &Editor) -> String {
    ed.db
        .last_result
        .as_ref()
        .map(|r| format!("{:.1} ms", r.elapsed_ms))
        .unwrap_or_default()
}

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
    if let Some(conn) = ed.db.connections.get(&id) {
        let tables = ed.db.tables.get(&id).map(|t| t.len()).unwrap_or(0);
        return format!("{} · {tables} tables", conn.label());
    }
    String::new()
}
