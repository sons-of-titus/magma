//! Install offers — the modal that asks before the editor installs a
//! server or enables a plugin.  The kernel holds the pending offer; this
//! module pops it and routes the answer to Janet, which owns what
//! acceptance does.

use kernel::command::execute_command_line;

use crate::sync;
use crate::{AppState, AppWindow};

/// Dialog button routing: `offer:accept` / `offer:decline`.
pub(crate) fn resolve_command(
    weak: &slint::Weak<AppWindow>,
    s: &mut AppState,
    command: &str,
) {
    let accepted = command == "offer:accept";
    // One offer per modal: pop what is showing, then answer it.
    let offer = s.editor.offers.take();
    if let Some(offer) = offer {
        let answer = if accepted { "yes" } else { "no" };
        let line = format!("offer-resolved {} {} {answer}", offer.kind, offer.id);
        if let Err(e) = execute_command_line(&mut s.editor, &line) {
            s.editor.show_message(kernel::state::MessageKind::Error, e);
        }
    }
    s.editor.frame_dirty = true;
    sync::refresh(weak, s);
}
