//! Index background-event handlers — project-index completion and per-file
//! symbol records.

use crate::event::keys;
use crate::event::payload::*;
use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Index(event) => handle_index_event(ed, event),
        _ => {}
    }
}

fn handle_index_event(ed: &mut Editor, event: crate::index::Event) {
    use crate::index::Event;
    match event {
        Event::ProjectIndexed {
            project_name,
            generation,
            files,
            ignored_files,
            done,
        } => {
            // A batch from a superseded open or walk describes a tree that
            // no longer exists — drop it instead of overwriting the current
            // project's list.
            if ed.project_manager.current_project.as_deref() != Some(&project_name)
                || generation != ed.project_manager.project.generation
            {
                return;
            }
            if let Some(proj) = ed.project_manager.projects.get_mut(&project_name) {
                proj.files = files.clone();
                proj.files_hash = files.iter().cloned().collect();
                proj.ignored_files = ignored_files.iter().cloned().collect();
                proj.files_epoch += 1;
                proj.file_index_dirty = false;
                proj.indexing = !done;
            }
            {
                let proj = &mut ed.project_manager.project;
                proj.files = files.clone();
                proj.files_hash = files.iter().cloned().collect();
                proj.ignored_files = ignored_files.iter().cloned().collect();
                proj.files_epoch += 1;
                proj.file_index_dirty = false;
                proj.indexing = !done;
            }
            if done {
                ed.project_manager
                    .sync_to_project_graph(&mut ed.semantic.project_graph);
                crate::index::reconcile_project_index(ed, &files, generation);
                ed.events.emit_typed(
                    keys::events::FILE_INDEXED,
                    FileIndexedPayload {
                        project_name,
                        count: files.len().to_string(),
                    },
                );
            }
        }
        Event::RecordArrived { generation, record } => {
            if generation != ed.project_manager.project.generation {
                return;
            }
            ed.index.apply(record);
        }
        Event::RecordSkipped { generation } => {
            if generation != ed.project_manager.project.generation {
                return;
            }
            ed.index.note_skipped();
        }
        Event::StampScan { generation, stamps } => {
            crate::index::apply_stamp_scan(ed, generation, stamps);
        }
    }
}
