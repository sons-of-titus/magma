//! Session background-event handlers — the buffers of an async session
//! restore streaming in from the pool.  Every event is stamped with the
//! project generation that requested the restore; events from a superseded
//! project are dropped, never opened.

use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::SessionBufferLoaded {
            generation,
            path,
            cursor,
            content,
        } => {
            if generation != ed.project_manager.project.generation {
                return;
            }
            ed.session_restore_pending = ed.session_restore_pending.saturating_sub(1);
            match content {
                Ok(text) => {
                    let already_open = ed.buffers.iter().any(|(_, arc)| {
                        arc.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref()
                            == Some(path.as_str())
                    });
                    if already_open {
                        // Dedupe + focus semantics without a second disk read.
                        crate::session::open_path(ed, &path);
                    } else {
                        crate::command::builtin::buffer::open_loaded_file(ed, &path, &text);
                    }
                }
                // Non-UTF8 content (image, PDF) needs the sync open path's
                // decoding; rare, so a foreground read is acceptable.
                Err(_) => {
                    crate::session::open_path(ed, &path);
                }
            }
            if let Some((key, _)) = ed.buffers.iter().find(|(_, arc)| {
                arc.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .path
                    .as_deref()
                    == Some(path.as_str())
            }) && let Some(view) = ed.views.get_mut(&key)
            {
                view.set_cursor(cursor);
            }
        }
        BackgroundEvent::SessionRestoreDone { generation, focused } => {
            if generation != ed.project_manager.project.generation {
                return;
            }
            ed.session_restore_pending = 0;
            if let Some(focused) = focused {
                crate::session::open_path(ed, &focused);
            }
            info!("session: background restore complete");
        }
        _ => {}
    }
}
