//! Database background-event handling: store connect/metadata/query
//! results and publish bus events for scripting.

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
            tables,
        } => {
            ed.db.apply_connected(source_id, connection, tables);
            ed.frame_dirty = true;
        }
        crate::db::DbEvent::TablesDone { source_id, tables } => {
            ed.db.apply_tables(source_id, tables);
            ed.frame_dirty = true;
        }
        crate::db::DbEvent::QueryDone { job, result } => {
            let summary = match &result {
                Ok(outcome) => format!(
                    "{} rows{}",
                    outcome.rows.len(),
                    if outcome.truncated {
                        "+ (truncated)"
                    } else {
                        ""
                    }
                ),
                Err(e) => e.clone(),
            };
            ed.db.apply_query(job, result);
            ed.frame_dirty = true;
            ed.events.emit(
                "db-query-done",
                [
                    ("job".to_string(), job.to_string()),
                    ("summary".to_string(), summary),
                ]
                .into_iter()
                .collect(),
            );
        }
    }
}
