//! Loads a source's completion catalog off the foreground thread.

use std::sync::Arc;

use crate::runtime::BackgroundEvent;
use crate::state::Editor;

use super::DbEvent;

pub(crate) fn dispatch(editor: &mut Editor, source_id: u64) {
    let Some(conn) = editor.db.connections.get(&source_id).cloned() else {
        return;
    };
    match editor.background.as_ref() {
        Some(bg) => {
            let sender = bg.sender.clone();
            bg.spawn_blocking(move || {
                let _ = sender.send(BackgroundEvent::Db(DbEvent::Catalog {
                    source_id,
                    catalog: conn.catalog(),
                }));
            });
        }
        None => {
            if let Ok(catalog) = conn.catalog() {
                editor.db.catalogs.insert(source_id, Arc::new(catalog));
            }
        }
    }
}
