//! Version-control background-event handlers — finished repo operations and
//! async status fetches feeding the Commit tool window.

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
                Ok(status) => ed.vc.last_status = Some(status),
                Err(e) => ed.show_message(crate::kernel::state::MessageKind::Error, e),
            }
            let _ = (backend_idx, root);
        }
        BackgroundEvent::VcBranchesReady { branches } => {
            ed.vc.branches = branches;
        }
        BackgroundEvent::VcStashListed { result } => {
            match result {
                Ok(stashes) => ed.vc.stashes = stashes,
                Err(e) => ed.show_message(crate::kernel::state::MessageKind::Error, e),
            }
        }
        BackgroundEvent::VcDiffReady { path, result } => {
            match result {
                Ok(data) => crate::kernel::command::builtin::vc_diff::open_diff(ed, &path, data),
                Err(e) => ed.show_message(crate::kernel::state::MessageKind::Error, e),
            }
        }
        _ => {}
    }
}
