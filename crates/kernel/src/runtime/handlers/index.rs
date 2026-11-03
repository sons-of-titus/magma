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

fn apply_walk_batch(
    proj: &mut crate::project::Project,
    walk_id: u64,
    new_walk: bool,
    files: &[String],
    ignored_files: &[String],
    done: bool,
) {
    if new_walk {
        proj.applied_walk_id = walk_id;
        proj.files.clear();
        proj.files_hash.clear();
    }
    proj.files.extend(files.iter().cloned());
    proj.files_hash.extend(files.iter().cloned());
    // The Batcher accumulates the ignored set across the whole walk, so a
    // plain replace tracks it without reset bookkeeping.
    proj.ignored_files = ignored_files.iter().cloned().collect();
    proj.files_epoch += 1;
    proj.file_index_dirty = false;
    proj.indexing = !done;
}

fn handle_index_event(ed: &mut Editor, event: crate::index::Event) {
    use crate::index::Event;
    match event {
        Event::ProjectIndexed {
            project_name,
            generation,
            walk_id,
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
            // Batches are deltas: accumulate them per walk id, reset on the
            // first batch of a newer walk, and drop batches that interleave
            // from an older walk still draining.
            if walk_id < ed.project_manager.project.applied_walk_id {
                return;
            }
            let new_walk = walk_id != ed.project_manager.project.applied_walk_id;
            if let Some(proj) = ed.project_manager.projects.get_mut(&project_name) {
                apply_walk_batch(
                    proj,
                    walk_id,
                    new_walk,
                    &files,
                    &ignored_files,
                    done,
                );
            }
            {
                let proj = &mut ed.project_manager.project;
                apply_walk_batch(
                    proj,
                    walk_id,
                    new_walk,
                    &files,
                    &ignored_files,
                    done,
                );
            }
            if done {
                // The accumulated list is the full walk result; the batch
                // itself only holds its own tail.
                let full_files = ed.project_manager.project.files.clone();
                ed.project_manager
                    .sync_to_project_graph(&mut ed.semantic.project_graph);
                crate::index::reconcile_project_index(ed, &full_files, generation);
                ed.events.emit_typed(
                    keys::events::FILE_INDEXED,
                    FileIndexedPayload {
                        project_name,
                        count: full_files.len().to_string(),
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
