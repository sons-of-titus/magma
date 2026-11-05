//! Database background-event handling: store connections, introspection
//! results, query outcomes, and edit results; publish bus events for
//! scripting.

use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    let BackgroundEvent::Db(db_event) = event else {
        return;
    };
    match db_event {
        crate::db::DbEvent::Connected {
            source_id,
            connection,
            error,
        } => {
            match (connection, error) {
                (Some(conn), _) => {
                    ed.db.connections.insert(source_id, conn);
                    ed.db.last_error = None;
                    crate::db::dispatch_catalog(ed, source_id);
                }
                (None, Some(e)) => {
                    // The root node carries the failure so the tree shows
                    // the error inline where the user clicked.
                    ed.db
                        .tree_mut(source_id)
                        .apply_children("/", Vec::new(), Some(e.clone()));
                    ed.db.last_error = Some(e);
                }
                (None, None) => {}
            }
            ed.frame_dirty = true;
            ed.events.emit(
                "db-connection-changed",
                [
                    ("source".to_string(), source_id.to_string()),
                    (
                        "connected".to_string(),
                        ed.db.is_connected(source_id).to_string(),
                    ),
                ]
                .into_iter()
                .collect(),
            );
        }
        crate::db::DbEvent::Introspected {
            source_id,
            path,
            children,
        } => {
            ed.db.apply_introspected(source_id, &path, children);
            ed.frame_dirty = true;
        }
        crate::db::DbEvent::TableDetail { tab_id, detail } => {
            if let Some(tab) = ed.db.tab_mut(tab_id) {
                match detail {
                    Ok(detail) => {
                        if let crate::db::DataTabKind::Table { detail: d, .. } = &mut tab.kind {
                            *d = Some(detail);
                        }
                    }
                    Err(e) => {
                        tab.busy = false;
                        tab.error = Some(e);
                    }
                }
            }
            ed.frame_dirty = true;
        }
        crate::db::DbEvent::QueryDone {
            job,
            tab_id,
            results,
        } => {
            let summary = match &results {
                Ok(outcomes) => format!(
                    "{} statement(s), {} rows total",
                    outcomes.len(),
                    outcomes.iter().map(|o| o.rows.len()).sum::<usize>()
                ),
                Err(e) => e.clone(),
            };
            ed.db.apply_query(job, tab_id, results);
            ed.frame_dirty = true;
            ed.events.emit(
                "db-query-done",
                [
                    ("job".to_string(), job.to_string()),
                    ("tab".to_string(), tab_id.to_string()),
                    ("summary".to_string(), summary),
                ]
                .into_iter()
                .collect(),
            );
        }
        crate::db::DbEvent::EditsApplied { tab_id, result } => {
            let summary = match &result {
                Ok(r) => format!("{} statement(s) applied", r.statements_run),
                Err(e) => e.clone(),
            };
            ed.db.apply_edits(tab_id, result);
            ed.frame_dirty = true;
            ed.events.emit(
                "db-edits-applied",
                [
                    ("tab".to_string(), tab_id.to_string()),
                    ("summary".to_string(), summary),
                ]
                .into_iter()
                .collect(),
            );
        }
        crate::db::DbEvent::TestDone { token, result } => {
            ed.db.last_test = Some(result);
            ed.frame_dirty = true;
            let _ = token;
        }
        crate::db::DbEvent::Catalog { source_id, catalog } => {
            if let Ok(catalog) = catalog
                && ed.db.connections.contains_key(&source_id)
            {
                ed.db.catalogs.insert(source_id, std::sync::Arc::new(catalog));
            }
        }
    }
}
