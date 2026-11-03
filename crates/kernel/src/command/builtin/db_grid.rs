//! Data-editor grid commands: selection, copy, cell edits, row
//! insert/delete, Apply (transactional), and revert.  Edits stay pending
//! until Apply; Apply runs the generated statements in one transaction and
//! refetches the page.

use std::collections::HashMap;

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::db::DbEvent;
use crate::runtime::BackgroundEvent;
use crate::state::Editor;
use db::EditOp;

use super::db_tree::{fetch_table_page, tab_id};

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "db-grid-select",
        "Select a grid cell",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("row", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("column", ArgType::Integer, ArgValue::Integer(0)),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let (row, col) = row_col(args)?;
            if let Some(tab) = editor.db.tab_mut(tab_id) {
                tab.selected = Some((row, col));
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-copy",
        "Copy the selected cell (or its whole row with 'row')",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("scope", ArgType::String, ArgValue::String("cell".into())),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let scope = args
                .get("scope")
                .and_then(|a| a.as_string())
                .unwrap_or("cell");
            let Some(tab) = editor.db.tab(tab_id) else {
                return Err("no such tab".to_string());
            };
            let Some(result) = tab.result() else {
                return Err("no result set in this tab".to_string());
            };
            let Some((row, col)) = tab.selected else {
                return Err("no cell selected".to_string());
            };
            let text = if scope == "row" {
                result
                    .rows
                    .get(row)
                    .map(|cells| {
                        cells
                            .iter()
                            .map(|v| v.render())
                            .collect::<Vec<_>>()
                            .join("\t")
                    })
                    .unwrap_or_default()
            } else {
                result
                    .rows
                    .get(row)
                    .and_then(|cells| cells.get(col))
                    .map(|v| v.full_text())
                    .unwrap_or_default()
            };
            if text.is_empty() {
                return Ok(());
            }
            editor.clipboard.set_text(&text);
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-edit-cell",
        "Stage a cell edit against the fetched page (pending until Apply)",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("row", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("column", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("value", ArgType::String, ArgValue::String(String::new())),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let (row, col) = row_col(args)?;
            let value = args
                .get("value")
                .and_then(|a| a.as_string())
                .unwrap_or_default()
                .to_string();
            {
                let Some(tab) = editor.db.tab(tab_id) else {
                    return Err("no such tab".to_string());
                };
                let Some(detail) = tab.table_detail() else {
                    return Err("table metadata is not loaded".to_string());
                };
                if detail.is_view() {
                    return Err("views are read-only".to_string());
                }
                if detail.pk_columns().is_none() {
                    return Err("table has no primary key; rows cannot be edited".to_string());
                }
                let Some(result) = tab.result() else {
                    return Err("no result set in this tab".to_string());
                };
                if col >= detail.columns.len() {
                    return Err("column out of range".to_string());
                }
                if !result.rows.get(row).is_some() {
                    return Err("row out of range".to_string());
                }
            }
            let original_row = editor
                .db
                .tab(tab_id)
                .and_then(|t| t.result())
                .and_then(|r| r.rows.get(row).cloned());
            let Some(original_row) = original_row else {
                return Err("row out of range".to_string());
            };
            let Some(tab) = editor.db.tab_mut(tab_id) else {
                return Ok(());
            };
            // One pending edit per cell: a second edit replaces the first.
            tab.pending
                .ops
                .retain(|o| !matches!(o, EditOp::UpdateCell { row: r, col: c, .. } if *r == row && *c == col));
            tab.pending.ops.push(EditOp::UpdateCell {
                row,
                col,
                value: db::DbValue::parse(&value),
                original_row,
            });
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-new-row",
        "Stage an empty insert row (pending until Apply)",
        vec![ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let column_count = {
                let Some(tab) = editor.db.tab(tab_id) else {
                    return Err("no such tab".to_string());
                };
                let Some(detail) = tab.table_detail() else {
                    return Err("table metadata is not loaded".to_string());
                };
                if detail.pk_columns().is_none() {
                    return Err("table has no primary key; rows cannot be inserted".to_string());
                }
                if detail.is_view() {
                    return Err("views are read-only".to_string());
                }
                detail.columns.len()
            };
            let Some(tab) = editor.db.tab_mut(tab_id) else {
                return Ok(());
            };
            // A single staged insert row at a time keeps the UI honest.
            tab.pending.ops.retain(|o| !matches!(o, EditOp::InsertRow { .. }));
            tab.pending.ops.push(EditOp::InsertRow {
                values: vec![db::DbValue::Null; column_count],
            });
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-new-cell",
        "Set a column of the staged insert row (pending until Apply)",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("column", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("value", ArgType::String, ArgValue::String(String::new())),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let col = args
                .get("column")
                .and_then(|a| a.as_integer())
                .unwrap_or(0)
                .max(0) as usize;
            let value = args
                .get("value")
                .and_then(|a| a.as_string())
                .unwrap_or_default()
                .to_string();
            let Some(tab) = editor.db.tab_mut(tab_id) else {
                return Err("no such tab".to_string());
            };
            let Some(op) = tab
                .pending
                .ops
                .iter_mut()
                .find(|o| matches!(o, EditOp::InsertRow { .. }))
            else {
                return Err("no staged insert row; run db-grid-new-row first".to_string());
            };
            if let EditOp::InsertRow { values } = op {
                if col >= values.len() {
                    return Err("column out of range".to_string());
                }
                values[col] = db::DbValue::parse(&value);
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-delete-row",
        "Stage a row delete (pending until Apply)",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("row", ArgType::Integer, ArgValue::Integer(0)),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let (row, _) = row_col(args)?;
            let (original_row, already) = {
                let Some(tab) = editor.db.tab(tab_id) else {
                    return Err("no such tab".to_string());
                };
                let Some(detail) = tab.table_detail() else {
                    return Err("table metadata is not loaded".to_string());
                };
                if detail.is_view() {
                    return Err("views are read-only".to_string());
                }
                if detail.pk_columns().is_none() {
                    return Err("table has no primary key; rows cannot be deleted".to_string());
                }
                let Some(result) = tab.result() else {
                    return Err("no result set in this tab".to_string());
                };
                let Some(original_row) = result.rows.get(row).cloned() else {
                    return Err("row out of range".to_string());
                };
                let already = tab
                    .pending
                    .ops
                    .iter()
                    .any(|o| matches!(o, EditOp::DeleteRow { row: r, .. } if *r == row));
                (original_row, already)
            };
            if already {
                return Ok(());
            }
            let Some(tab) = editor.db.tab_mut(tab_id) else {
                return Ok(());
            };
            // Editing and deleting the same row collapse to the delete.
            tab.pending.ops.retain(|o| !matches!(o, EditOp::UpdateCell { row: r, .. } if *r == row));
            tab.pending.ops.push(EditOp::DeleteRow {
                row,
                original_row,
            });
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-revert",
        "Drop pending edits (one row, or everything without a row)",
        vec![
            ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0)),
            ArgSpec::optional("row", ArgType::Integer, ArgValue::Integer(0)),
        ],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let Some(tab) = editor.db.tab_mut(tab_id) else {
                return Err("no such tab".to_string());
            };
            match args.get("row").and_then(|a| a.as_integer()) {
                Some(row) => tab.pending.revert_row(row.max(0) as usize),
                None => tab.pending.clear(),
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-grid-apply",
        "Apply pending edits in one transaction, then refetch the page",
        vec![ArgSpec::optional("tab", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let tab_id = tab_id(editor, args)?;
            let (pending, detail, conn) = {
                let Some(tab) = editor.db.tab(tab_id) else {
                    return Err("no such tab".to_string());
                };
                if tab.pending.is_empty() {
                    return Ok(());
                }
                let Some(detail) = tab.table_detail().cloned() else {
                    return Err("table metadata is not loaded".to_string());
                };
                let conn = editor
                    .db
                    .connections
                    .get(&tab.source_id)
                    .cloned()
                    .ok_or_else(|| "source is not connected".to_string())?;
                (tab.pending.clone(), detail, conn)
            };
            match editor.background.as_ref() {
                Some(bg) => {
                    let sender = bg.sender.clone();
                    bg.spawn_blocking(move || {
                        let result = conn.apply_edits(&detail, &pending);
                        let _ = sender.send(BackgroundEvent::Db(DbEvent::EditsApplied {
                            tab_id,
                            result,
                        }));
                    });
                }
                None => {
                    let result = conn.apply_edits(&detail, &pending);
                    let failed = result.as_ref().err().cloned();
                    editor.db.apply_edits(tab_id, result);
                    let job = editor.db.take_next_job();
                    if let Some(tab) = editor.db.tab_mut(tab_id) {
                        tab.busy = true;
                        tab.job = job;
                    }
                    fetch_table_page(editor, tab_id);
                    if let Some(e) = failed {
                        return Err(e);
                    }
                }
            }
            editor.frame_dirty = true;
            Ok(())
        },
    );
}

fn row_col(args: &HashMap<String, ArgValue>) -> Result<(usize, usize), String> {
    let row = args
        .get("row")
        .and_then(|a| a.as_integer())
        .unwrap_or(0)
        .max(0) as usize;
    let col = args
        .get("column")
        .and_then(|a| a.as_integer())
        .unwrap_or(0)
        .max(0) as usize;
    Ok((row, col))
}
