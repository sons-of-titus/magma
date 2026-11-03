//! SQL console commands: per-source console buffers, execution of the
//! whole buffer / current statement / selection, and cancellation.

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::db::DbEvent;
use crate::runtime::BackgroundEvent;
use crate::state::Editor;
use db::{split_statements, statement_at};

use super::db_sources::source_id;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "db-console",
        "Open (or focus) the SQL console buffer for a data source",
        vec![ArgSpec::optional("source", ArgType::Integer, ArgValue::Integer(0))],
        |editor, args| {
            let id = source_id(editor, args)?;
            let key = open_console_buffer(editor, id)?;
            if let Some(win) = editor.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            editor.ui_requests.push("database".to_string());
            editor.frame_dirty = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "db-execute-buffer",
        "Run every statement in the focused SQL console against its source",
        vec![],
        |editor, _args| execute_text(editor, whole_buffer, "buffer"),
    );

    cmds.register_fn(
        "db-execute-current",
        "Run the statement under the cursor",
        vec![],
        |editor, _args| execute_text(editor, statement_under_cursor, "current"),
    );

    cmds.register_fn(
        "db-execute-selection",
        "Run the selected SQL text",
        vec![],
        |editor, _args| execute_text(editor, selected_text, "selection"),
    );

    cmds.register_fn(
        "db-cancel",
        "Interrupt the running query on the active source",
        vec![],
        |editor, _args| {
            if editor.db.interrupt_active() {
                editor.show_message(
                    crate::state::MessageKind::Info,
                    "interrupt sent to the database",
                );
                Ok(())
            } else {
                Err("no connected data source to interrupt".to_string())
            }
        },
    );
}

fn whole_buffer(editor: &mut Editor) -> Option<String> {
    let key = focused_buffer(editor)?;
    let arc = editor.buffers.get(key)?;
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    Some(buf.slice(0, buf.len()))
}

fn statement_under_cursor(editor: &mut Editor) -> Option<String> {
    let key = focused_buffer(editor)?;
    let cursor = editor.focused_view()?.cursor_offset();
    let arc = editor.buffers.get(key)?;
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let text = buf.slice(0, buf.len());
    statement_at(&text, cursor).map(|s| s.text)
}

fn selected_text(editor: &mut Editor) -> Option<String> {
    let key = focused_buffer(editor)?;
    let cursor = editor.focused_view()?.cursor_offset();
    let (a, b) = editor.selection_range(cursor)?;
    let arc = editor.buffers.get(key)?;
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    Some(buf.slice(a.min(b), a.max(b)))
}

/// `focused_buffer_key` answers 0 for "none"; slab keys start at zero, so
/// the honest check is whether the key exists in the buffer slab.
fn focused_buffer(editor: &Editor) -> Option<usize> {
    let key = editor.focused_buffer_key();
    editor.buffers.contains(key).then_some(key)
}

/// Shared tail of the execute commands: resolve the console's source,
/// require a connection, then dispatch the script to a fresh query tab.
fn execute_text(
    editor: &mut Editor,
    extract: fn(&mut Editor) -> Option<String>,
    what: &str,
) -> Result<(), String> {
    let Some(sql) = extract(editor) else {
        return Err(format!("nothing to run ({what})"));
    };
    let sql = sql.trim().to_string();
    if sql.is_empty() {
        return Err("the SQL text is empty".to_string());
    }
    let source_id = focused_buffer(editor)
        .and_then(|k| editor.db.console_source(k))
        .or(editor.db.active);
    let source_id = source_id.ok_or_else(|| "no data source for this console".to_string())?;
    let conn = editor
        .db
        .connections
        .get(&source_id)
        .cloned()
        .ok_or_else(|| "source is not connected".to_string())?;

    editor.db.note_history(&sql);
    let job = editor.db.take_next_job();
    let tab_id = editor.db.take_next_tab_id();
    let statement_count = split_statements(&sql).len().max(1);
    editor.db.tabs.push(crate::db::DataTab {
        id: tab_id,
        source_id,
        kind: crate::db::DataTabKind::Query,
        results: Vec::new(),
        active_result: 0,
        title: tab_title(&sql, statement_count),
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

    match editor.background.as_ref() {
        Some(bg) => {
            let sender = bg.sender.clone();
            bg.spawn_blocking(move || {
                let results = conn.execute_script(&sql, crate::db::FETCH_LIMIT);
                let _ = sender.send(BackgroundEvent::Db(DbEvent::QueryDone {
                    job,
                    tab_id,
                    results,
                }));
            });
        }
        None => {
            let results = conn.execute_script(&sql, crate::db::FETCH_LIMIT);
            editor.db.apply_query(job, tab_id, results);
            editor.frame_dirty = true;
        }
    }
    editor.ui_requests.push("database".to_string());
    editor.frame_dirty = true;
    Ok(())
}

fn tab_title(sql: &str, count: usize) -> String {
    let first = sql.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut title: String = first.trim().chars().take(24).collect();
    if title.len() < first.trim().len() {
        title.push('…');
    }
    if count > 1 {
        title.push_str(&format!("  ({count} statements)"));
    }
    title
}

/// The console buffer is an editor buffer named `*sql: <source>*` — it
/// renders in the EditorView with the SQL major mode, participates in
/// crash recovery, and persists with the workspace.
fn open_console_buffer(editor: &mut Editor, source_id: u64) -> Result<usize, String> {
    if let Some(&key) = editor.db.consoles.get(&source_id)
        && editor.buffers.contains(key)
    {
        return Ok(key);
    }
    let name = editor
        .db
        .source(source_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let buf_name = format!("*sql: {name}*");
    if let Some((key, _)) = editor
        .buffers
        .iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).name == buf_name)
    {
        editor.db.consoles.insert(source_id, key);
        return Ok(key);
    }
    let id = editor.allocate_buffer_id();
    let buf = crate::text_engine::Buffer::from_string(
        crate::state::id::BufferId(id),
        &buf_name,
        "-- SQL console: put a statement here; Cmd+Enter runs the statement under the cursor.\n",
    );
    let arc = std::sync::Arc::new(std::sync::Mutex::new(buf));
    let entry = editor.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(arc.clone());
    editor
        .views
        .insert(key, crate::text_engine::BufferView::new(arc));
    if let Some(a) = editor.buffers.get(key) {
        a.lock().unwrap_or_else(|e| e.into_inner()).major_mode =
            crate::text_engine::MajorMode::from_name("sql");
    }
    editor.db.consoles.insert(source_id, key);
    Ok(key)
}
