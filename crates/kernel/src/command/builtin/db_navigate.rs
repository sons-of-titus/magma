//! Visual database navigation commands: table structure view, foreign-key
//! jump (from data cells, structure rows, or explorer tree nodes), and
//! quick `SELECT TOP N` into a console.

use std::collections::HashMap;

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::state::Editor;
use db::{Dialect, ForeignKeyMeta, NodeRef, TableMeta, TableRef};

use super::db_console::{dispatch_script, open_console_buffer};
use super::db_sources::source_id;
use super::db_tree::{open_table_tab, tab_id};

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "db-open-structure",
        "Open a table's structure view (columns, keys, indexes)",
        vec![
            ArgSpec::optional("path", ArgType::String, ArgValue::String(String::new())),
            ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
        ],
        |editor, args| {
            let path = args
                .get("path")
                .and_then(|a| a.as_string())
                .unwrap_or_default();
            if !path.is_empty() {
                let id = source_id(editor, args)?;
                let node = NodeRef::parse(path).ok_or("bad table path")?;
                open_table_tab(editor, id, node, String::new(), true);
                return Ok(());
            }
            let tid = tab_id(editor, args)?;
            let Some(tab) = editor.db.tab_mut(tid) else {
                return Err("no such tab".to_string());
            };
            if tab.table_node().is_none() {
                return Err("structure view is only available on table tabs".to_string());
            }
            tab.structure = true;
            tab.selected = None;
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-toggle-structure",
        "Toggle the active table tab between Data and Structure views",
        vec![ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let tid = tab_id(editor, args)?;
            let Some(tab) = editor.db.tab_mut(tid) else {
                return Err("no such tab".to_string());
            };
            if tab.table_node().is_none() {
                return Err("structure view is only available on table tabs".to_string());
            }
            tab.structure = !tab.structure;
            tab.selected = None;
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-select-top",
        "Open the SQL console with a SELECT TOP/LIMIT for a table and run it",
        vec![
            ArgSpec::new("path", ArgType::String),
            ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("limit", ArgType::Integer, ArgValue::Integer(100)),
        ],
        |editor, args| {
            let path = args
                .get("path")
                .and_then(|a| a.as_string())
                .filter(|s| !s.is_empty())
                .ok_or("a table path is required")?;
            let NodeRef::Table {
                database,
                schema,
                name,
                ..
            } = NodeRef::parse(path).ok_or("not a table path")?
            else {
                return Err("not a table path".to_string());
            };
            let id = source_id(editor, args)?;
            let limit = args
                .get("limit")
                .and_then(|a| a.as_integer())
                .unwrap_or(100)
                .max(1);
            let dialect = editor
                .db
                .connections
                .get(&id)
                .map(|c| c.dialect())
                .unwrap_or(Dialect::Ansi);
            let target = db::quote_table(
                &TableRef {
                    database,
                    schema,
                    name,
                },
                dialect,
            );
            let sql = match dialect {
                Dialect::SqlServer => format!("SELECT TOP {limit} * FROM {target};"),
                _ => format!("SELECT * FROM {target} LIMIT {limit};"),
            };
            let key = open_console_buffer(editor, id)?;
            if let Some(arc) = editor.buffers.get(key) {
                let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let len = buf.len();
                let prefix = if len == 0 || buf.slice(0, len).ends_with('\n') {
                    ""
                } else {
                    "\n"
                };
                buf.insert(len, &format!("{prefix}{sql}\n"));
            }
            crate::semantic::fallback_syntax::on_buffer_created(editor, key);
            if let Some(win) = editor.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            dispatch_script(editor, id, sql)
        },
    );

    cmds.register_fn(
        "db-follow-fk",
        "Jump to the referenced table or row for a foreign key",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("row", ArgType::Integer, ArgValue::Integer(-1)),
            ArgSpec::optional("column", ArgType::Integer, ArgValue::Integer(-1)),
            ArgSpec::optional("path", ArgType::String, ArgValue::String(String::new())),
            ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0)),
        ],
        follow_fk,
    );
}

fn follow_fk(editor: &mut Editor, args: &HashMap<String, ArgValue>) -> Result<(), String> {
    let path = args
        .get("path")
        .and_then(|a| a.as_string())
        .unwrap_or_default();
    if !path.is_empty() {
        return follow_tree_fk(editor, args, path);
    }
    let tid = tab_id(editor, args)?;
    let (source_id, structure, selected, detail, result) = {
        let Some(tab) = editor.db.tab(tid) else {
            return Err("no such tab".to_string());
        };
        let detail = tab
            .table_detail()
            .cloned()
            .ok_or_else(|| "table metadata is not loaded".to_string())?;
        (
            tab.source_id,
            tab.structure,
            tab.selected,
            detail,
            tab.result().cloned(),
        )
    };
    let row_arg = args.get("row").and_then(|a| a.as_integer()).unwrap_or(-1);
    let col_arg = args.get("column").and_then(|a| a.as_integer()).unwrap_or(-1);

    if structure {
        let row = if row_arg >= 0 {
            Some(row_arg as usize)
        } else {
            selected.map(|(r, _)| r)
        }
        .ok_or_else(|| "select a foreign-key row first".to_string())?;
        let fk = if row < detail.columns.len() {
            detail
                .fk_for_column(&detail.columns[row].name)
                .map(|(fk, _)| fk.clone())
        } else {
            detail.foreign_keys.get(row - detail.columns.len()).cloned()
        }
        .ok_or_else(|| "row is not a foreign key".to_string())?;
        let target = target_node(&detail, &fk);
        open_table_tab(editor, source_id, target, String::new(), false);
        return Ok(());
    }

    let (row, col) = if row_arg >= 0 && col_arg >= 0 {
        (row_arg as usize, col_arg as usize)
    } else {
        selected.ok_or_else(|| "select a foreign-key cell first".to_string())?
    };
    let result = result.ok_or_else(|| "no result set in this tab".to_string())?;
    let col_name = &result
        .columns
        .get(col)
        .ok_or_else(|| "column out of range".to_string())?
        .name;
    let fk = detail
        .foreign_keys
        .iter()
        .find(|fk| fk.columns.iter().any(|c| c == col_name))
        .ok_or_else(|| format!("column {col_name} is not a foreign key"))?;
    let dialect = editor
        .db
        .connections
        .get(&source_id)
        .map(|c| c.dialect())
        .unwrap_or(Dialect::Ansi);
    let mut preds = Vec::with_capacity(fk.columns.len());
    for (local_col, ref_col) in fk.columns.iter().zip(fk.ref_columns.iter()) {
        if ref_col.is_empty() {
            return Err("referenced column is unknown".to_string());
        }
        let local_idx = result
            .columns
            .iter()
            .position(|c| &c.name == local_col)
            .ok_or_else(|| format!("column {local_col} is not in the result set"))?;
        let val = result
            .rows
            .get(row)
            .and_then(|r| r.get(local_idx))
            .ok_or_else(|| "row out of range".to_string())?;
        if val.is_null() {
            return Err("foreign key value is NULL".to_string());
        }
        preds.push(format!(
            "{} = {}",
            dialect.quote_ident(ref_col),
            dialect.literal(val)
        ));
    }
    let target = target_node(&detail, fk);
    open_table_tab(editor, source_id, target, preds.join(" AND "), false);
    Ok(())
}

fn follow_tree_fk(
    editor: &mut Editor,
    args: &HashMap<String, ArgValue>,
    path: &str,
) -> Result<(), String> {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() != 6 || parts[4] != "fks" {
        return Err("not a foreign-key tree node".to_string());
    }
    let id = source_id(editor, args)?;
    let parent_key = format!("/{}/{}/{}/{}", parts[0], parts[1], parts[2], parts[3]);
    let fk_name = parts[5];
    let fk = editor
        .db
        .tree(id)
        .node(&parent_key)
        .and_then(|n| {
            n.children.iter().find_map(|c| match c {
                db::Child::ForeignKey { meta } if meta.name == fk_name => Some(meta.clone()),
                _ => None,
            })
        })
        .ok_or_else(|| format!("foreign key {fk_name} not found"))?;
    let schema = if fk.ref_schema.is_empty() {
        parts[1].to_string()
    } else {
        fk.ref_schema
    };
    let target = NodeRef::Table {
        database: parts[0].to_string(),
        schema,
        name: fk.ref_table,
        is_view: false,
    };
    open_table_tab(editor, id, target, String::new(), false);
    Ok(())
}

fn target_node(detail: &TableMeta, fk: &ForeignKeyMeta) -> NodeRef {
    let schema = if fk.ref_schema.is_empty() {
        detail.schema.clone()
    } else {
        fk.ref_schema.clone()
    };
    NodeRef::Table {
        database: detail.database.clone(),
        schema,
        name: fk.ref_table.clone(),
        is_view: false,
    }
}
