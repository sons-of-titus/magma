//! Explorer tree commands: lazy expand/collapse, per-node refresh, full
//! synchronize, and opening a table in the data editor.

use std::collections::HashMap;

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::db::DbEvent;
use crate::runtime::BackgroundEvent;
use crate::state::Editor;

use super::db_sources::{introspect_dispatch, source_id};

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "db-expand",
        "Expand a tree node, introspecting it on first open",
        vec![ArgSpec::new("path", ArgType::String)],
        |editor, args| {
            let path = path_arg(args)?;
            let id = source_id(editor, args)?;
            let status = editor
                .db
                .tree(id)
                .node(&path)
                .map(|n| n.status.clone())
                .unwrap_or(crate::db::NodeStatus::NotLoaded);
            match status {
                crate::db::NodeStatus::Loading => {}
                crate::db::NodeStatus::Loaded => {
                    editor.db.tree_mut(id).mark_expanded(&path);
                }
                crate::db::NodeStatus::NotLoaded | crate::db::NodeStatus::Failed(_) => {
                    introspect_dispatch(editor, id, &path)?
                }
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-collapse",
        "Collapse a tree node",
        vec![ArgSpec::new("path", ArgType::String)],
        |editor, args| {
            let path = path_arg(args)?;
            let id = source_id(editor, args)?;
            editor.db.tree_mut(id).collapse(&path);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-refresh-node",
        "Re-introspect one tree node (its cached children drop)",
        vec![ArgSpec::new("path", ArgType::String)],
        |editor, args| {
            let path = path_arg(args)?;
            let id = source_id(editor, args)?;
            introspect_dispatch(editor, id, &path)?;
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-refresh-source",
        "Synchronize a data source: drop the tree cache and reload",
        vec![ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let id = source_id(editor, args)?;
            editor.db.tree_mut(id).clear();
            editor.db.tree_mut(id).mark_loading("/");
            introspect_dispatch(editor, id, "/")?;
            crate::db::dispatch_catalog(editor, id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-open-table",
        "Open a table or view in the data editor",
        vec![ArgSpec::new("path", ArgType::String)],
        |editor, args| {
            let path = path_arg(args)?;
            let id = source_id(editor, args)?;
            let node = db::NodeRef::parse(&path).ok_or("bad table path")?;
            let tab_id = editor.db.take_next_tab_id();
            let job = editor.db.take_next_job();
            editor.db.tabs.push(crate::db::DataTab {
                id: tab_id,
                source_id: id,
                kind: crate::db::DataTabKind::Table {
                    node: node.clone(),
                    detail: None,
                },
                results: Vec::new(),
                active_result: 0,
                title: table_title(&node),
                page: 0,
                sort: None,
                filter: String::new(),
                pending: Default::default(),
                selected: None,
                busy: true,
                job,
                error: None,
            });
            editor.db.active_tab = Some(tab_id);
            fetch_table_page(editor, tab_id);
            editor.ui_requests.push("database".to_string());
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-page",
        "Move the data editor's page by delta pages",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("delta", ArgType::Integer, ArgValue::Integer(0)),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let delta = args
                .get("delta")
                .and_then(|a| a.as_integer())
                .unwrap_or(0) as i64;
            let job = editor.db.take_next_job();
            {
                let Some(tab) = editor.db.tab_mut(tab_id) else {
                    return Err("no such tab".to_string());
                };
                tab.page = (tab.page as i64 + delta).max(0) as usize;
                tab.busy = true;
                tab.job = job;
            }
            fetch_table_page(editor, tab_id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-sort",
        "Cycle the data editor's sort column (asc → desc → none)",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("column", ArgType::Integer, ArgValue::Integer(0)),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let column = args
                .get("column")
                .and_then(|a| a.as_integer())
                .unwrap_or(0)
                .max(0) as usize;
            let job = editor.db.take_next_job();
            {
                let Some(tab) = editor.db.tab_mut(tab_id) else {
                    return Err("no such tab".to_string());
                };
                tab.sort = match tab.sort {
                    Some((c, true)) if c == column => Some((column, false)),
                    Some((c, false)) if c == column => None,
                    _ => Some((column, true)),
                };
                tab.page = 0;
                tab.busy = true;
                tab.job = job;
            }
            fetch_table_page(editor, tab_id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-filter",
        "Set the data editor's SQL filter and reload page 0",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("filter", ArgType::String, ArgValue::String(String::new())),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let filter = args
                .get("filter")
                .and_then(|a| a.as_string())
                .unwrap_or_default()
                .to_string();
            let job = editor.db.take_next_job();
            {
                let Some(tab) = editor.db.tab_mut(tab_id) else {
                    return Err("no such tab".to_string());
                };
                tab.filter = filter;
                tab.page = 0;
                tab.busy = true;
                tab.job = job;
            }
            fetch_table_page(editor, tab_id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-refresh",
        "Reload the data editor's current page",
        vec![ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let job = editor.db.take_next_job();
            {
                let Some(tab) = editor.db.tab_mut(tab_id) else {
                    return Err("no such tab".to_string());
                };
                tab.busy = true;
                tab.job = job;
            }
            fetch_table_page(editor, tab_id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-close-tab",
        "Close a data tab",
        vec![ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            editor.db.close_tab(tab_id);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-select-tab",
        "Focus a data tab",
        vec![ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            if editor.db.tab(tab_id).is_some() {
                editor.db.active_tab = Some(tab_id);
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );
}

/// The one place that (re)fetches a table tab's page: table detail always
/// refreshes first (the page query needs column metadata for ordering),
/// then the page query — one background task, two events.
pub(super) fn fetch_table_page(editor: &mut Editor, tab_id: u64) {
    let Some(tab) = editor.db.tab(tab_id) else {
        return;
    };
    let source_id = tab.source_id;
    let job = tab.job;
    let page = tab.page;
    let sort = tab.sort;
    let filter = tab.filter.clone();
    let node = tab.table_node().cloned();
    let Some(conn) = editor.db.connections.get(&source_id).cloned() else {
        if let Some(tab) = editor.db.tab_mut(tab_id) {
            tab.busy = false;
            tab.error = Some("source is not connected".to_string());
        }
        return;
    };

    match editor.background.as_ref() {
        Some(bg) => {
            let sender = bg.sender.clone();
            bg.spawn_blocking(move || {
                let Some(node) = node else {
                    let _ = sender.send(BackgroundEvent::Db(DbEvent::QueryDone {
                        job,
                        tab_id,
                        results: Err("not a table tab".to_string()),
                    }));
                    return;
                };
                let detail = conn.table_detail(&node);
                let _ = sender.send(BackgroundEvent::Db(DbEvent::TableDetail {
                    tab_id,
                    detail: detail.clone().map_err(|e| e),
                }));
                if let Ok(detail) = detail {
                    let sql = db::page_query(
                        conn.dialect(),
                        &detail,
                        sort,
                        &filter,
                        page,
                        crate::db::FETCH_LIMIT,
                    );
                    let results = conn.execute_script(&sql, crate::db::FETCH_LIMIT);
                    let _ = sender.send(BackgroundEvent::Db(DbEvent::QueryDone {
                        job,
                        tab_id,
                        results,
                    }));
                }
            });
        }
        None => {
            // Inline path for tests: detail then page, applied directly.
            let Some(node) = node else {
                editor.db.apply_query(job, tab_id, Err("not a table tab".into()));
                editor.frame_dirty = true;
                return;
            };
            match conn.table_detail(&node) {
                Ok(detail) => {
                    if let Some(tab) = editor.db.tab_mut(tab_id)
                        && let crate::db::DataTabKind::Table { detail: d, .. } = &mut tab.kind
                    {
                        *d = Some(detail.clone());
                    }
                    let sql = db::page_query(
                        conn.dialect(),
                        &detail,
                        sort,
                        &filter,
                        page,
                        crate::db::FETCH_LIMIT,
                    );
                    let results = conn.execute_script(&sql, crate::db::FETCH_LIMIT);
                    editor.db.apply_query(job, tab_id, results);
                }
                Err(e) => {
                    editor.db.apply_query(job, tab_id, Err(e));
                }
            }
            editor.frame_dirty = true;
        }
    }
}

fn table_title(node: &db::NodeRef) -> String {
    match node {
        db::NodeRef::Table { name, .. } => name.clone(),
        _ => "table".to_string(),
    }
}

fn path_arg(args: &HashMap<String, ArgValue>) -> Result<String, String> {
    args.get("path")
        .and_then(|a| a.as_string())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "a tree path is required".to_string())
}

pub(super) fn tab_id(editor: &mut Editor, args: &HashMap<String, ArgValue>) -> Result<u64, String> {
    // 0 is the optional arg's injected default; real tab ids start at 1.
    if let Some(v) = args.get("tab").and_then(|a| a.as_integer())
        && v > 0
    {
        return Ok(v as u64);
    }
    editor
        .db
        .active_tab
        .ok_or_else(|| "no active data tab".to_string())
}
