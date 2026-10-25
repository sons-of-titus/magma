//! Apply Commit-tool-window requests at the shell's write-lock boundary.
//!
//! Split from `gui_shell` so that file stays focused; every failure lands in
//! the status message rather than being discarded.

use std::collections::HashMap;

use crate::kernel::command::{execute_command, args::ArgValue};
use crate::kernel::state::{Editor, MessageKind};

use super::gui_commit_tool::CommitAction;
use super::gui_dialog::DialogAction;

pub(crate) fn apply_commit_actions(ed: &mut Editor, actions: Vec<CommitAction>) {
    for action in actions {
        match action {
            CommitAction::Stage(path) => run(ed, "vc-stage-file", "path", path),
            CommitAction::Unstage(path) => run(ed, "vc-unstage-file", "path", path),
            CommitAction::Discard(path) => run(ed, "vc-discard-file", "path", path),
            CommitAction::ShowDiff(path) => run(ed, "vc-diff-file", "path", path),
            CommitAction::Checkout(branch) => run(ed, "vc-checkout-branch", "branch", branch),
            CommitAction::NewBranch(name) => run(ed, "vc-branch-create", "name", name),
            CommitAction::Commit { message, amend } => {
                let cmd = if amend { "vc-amend-message" } else { "vc-commit-message" };
                run(ed, cmd, "message", message);
            }
            CommitAction::Refresh => {
                if let Err(e) = execute_command(ed, "vc-refresh", &HashMap::new())
                    .or_else(|_| execute_command(ed, "vc", &HashMap::new())) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            CommitAction::BranchList => {
                if let Err(e) = execute_command(ed, "vc-branch-list", &HashMap::new()) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            CommitAction::OpenFile(path) => DialogAction::OpenFile(path).apply(ed),
            CommitAction::CopyPath(path) => {
                ed.clipboard.set_text(&path);
                ed.show_message(MessageKind::Info, format!("Copied: {path}"));
            }
        }
    }
}

fn run(ed: &mut Editor, cmd: &str, arg: &str, value: String) {
    let mut args = HashMap::new();
    args.insert(arg.to_string(), ArgValue::String(value));
    if let Err(e) = execute_command(ed, cmd, &args) {
        ed.show_message(MessageKind::Error, e);
    }
}
