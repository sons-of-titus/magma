//! Managed-download completion — the foreground half of `install/fetch`.

use crate::event::keys;
use crate::event::payload::{InstallCompletedPayload, InstallFailedPayload};
use crate::runtime::BackgroundEvent;
use crate::state::Editor;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::InstallFinished {
            name,
            kind,
            id,
            path,
            error,
        } => match error {
            None => ed.events.emit_typed(
                keys::events::INSTALL_COMPLETED,
                InstallCompletedPayload {
                    name,
                    kind,
                    id,
                    path,
                },
            ),
            Some(error) => ed.events.emit_typed(
                keys::events::INSTALL_FAILED,
                InstallFailedPayload { name, kind, id, error },
            ),
        },
        _ => {}
    }
}
