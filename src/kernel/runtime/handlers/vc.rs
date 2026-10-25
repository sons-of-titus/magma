//! Version-control background-event handlers — finished repo operations and
//! async status fetches.

use crate::kernel::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::VcOpDone { scratch, then_refresh, result } => {
            if let Err(e) = crate::kernel::command::builtin::vcs::apply_vc_op(
                ed, scratch.as_deref(), then_refresh, result,
            ) {
                ed.show_message(crate::kernel::state::MessageKind::Error, e);
            }
        }
        BackgroundEvent::VcStatusFetched { backend_idx, root, result } => {
            match result {
                Ok(status) => {
                    ed.vc.last_status = Some(status);
                    crate::kernel::command::builtin::vcs::redraw_vc_buffer(ed);
                }
                Err(e) => ed.show_message(crate::kernel::state::MessageKind::Error, e),
            }
            let _ = (backend_idx, root);
        }
        BackgroundEvent::VcPreviewReady { path, text } => {
            ed.vc.preview_path = Some(path);
            ed.vc.preview = Some(text);
        }
        BackgroundEvent::VcBranchesReady { branches } => {
            ed.vc.branches = branches;
        }
        _ => {}
    }
}
