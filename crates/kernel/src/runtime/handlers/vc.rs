//! Version-control background-event handlers — finished repo operations and
//! async status fetches feeding the Commit tool window.

use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Vc(event) => handle_vc_event(ed, event),
        _ => {}
    }
}

fn handle_vc_event(ed: &mut Editor, event: crate::vc::Event) {
    use crate::vc::Event;
    match event {
        Event::OpDone {
            scratch,
            then_refresh,
            result,
        } => {
            if let Err(e) = crate::command::builtin::vcs::apply_vc_op(
                ed,
                scratch.as_deref(),
                then_refresh,
                result,
            ) {
                ed.show_message(crate::state::MessageKind::Error, e);
            }
        }
        Event::StatusFetched {
            backend_idx,
            root,
            result,
        } => {
            match result {
                Ok(status) => ed.vc.last_status = Some(status),
                Err(e) => ed.show_message(crate::state::MessageKind::Error, e),
            }
            let _ = (backend_idx, root);
        }
        Event::BranchesReady { branches } => {
            ed.vc.branches = branches;
        }
        Event::StashListed { result } => match result {
            Ok(stashes) => ed.vc.stashes = stashes,
            Err(e) => ed.show_message(crate::state::MessageKind::Error, e),
        },
        Event::DiffReady { path, result } => match result {
            Ok(data) => crate::command::builtin::vc_diff::open_diff(ed, &path, data),
            Err(e) => ed.show_message(crate::state::MessageKind::Error, e),
        },
    }
}
