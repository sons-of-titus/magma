//! Index background-event handlers — project-index completion and per-file
//! symbol records.

use crate::kernel::event::keys;
use crate::kernel::event::payload::*;
use crate::kernel::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Index(event) => handle_index_event(ed, event),
        _ => {}
    }
}

fn handle_index_event(ed: &mut Editor, event: crate::kernel::index::Event) {
    use crate::kernel::index::Event;
    match event {
        Event::ProjectIndexed { project_name, files } => {
            if let Some(proj) = ed.project_manager.projects.get_mut(&project_name) {
                proj.files = files.clone();
                proj.files_epoch += 1;
                proj.file_index_dirty = false;
            }
            if ed.project_manager.current_project.as_deref() == Some(&project_name) {
                ed.project_manager.project.files = files.clone();
                ed.project_manager.project.files_epoch += 1;
                ed.project_manager.project.file_index_dirty = false;
                ed.project_manager.sync_to_project_graph(&mut ed.semantic.project_graph);
                crate::kernel::index::reconcile_project_index(ed, &files);
            }
            ed.events.emit_typed(keys::events::FILE_INDEXED, FileIndexedPayload {
                project_name,
                count: files.len().to_string(),
            });
        }
        Event::RecordArrived(rec) => {
            ed.index.apply(rec);
        }
    }
}
