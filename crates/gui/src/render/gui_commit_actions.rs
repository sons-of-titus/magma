//! Apply Commit-tool-window requests at the shell's write-lock boundary.
//!
//! Split from `gui_shell` so that file stays focused; every failure lands in
//! the status message rather than being discarded.

use std::collections::HashMap;

use magma::kernel::command::{execute_command, args::ArgValue};
use magma::kernel::state::{Editor, MessageKind};

use super::gui_commit_tool::CommitAction;
use super::gui_dialog::DialogAction;

pub(crate) fn apply_commit_actions(ed: &mut Editor, actions: Vec<CommitAction>) {
    for action in actions {
        match action {
            CommitAction::Stage(path) => run(ed, "vc-stage-file", "path", path),
            CommitAction::Unstage(path) => run(ed, "vc-unstage-file", "path", path),
            CommitAction::Discard(path) => run(ed, "vc-discard-file", "path", path),
            CommitAction::ShowDiff(path) => run(ed, "vc-diff-file", "path", path),
            CommitAction::StashPush(message) => run(ed, "vc-stash", "message", message),
            CommitAction::StashPop(index) => run_int(ed, "vc-stash-pop", "index", index as i64),
            CommitAction::StashApply(index) => run_int(ed, "vc-stash-apply", "index", index as i64),
            CommitAction::StashDrop(index) => run_int(ed, "vc-stash-drop", "index", index as i64),
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
            CommitAction::Fetch => run0(ed, "vc-fetch"),
            CommitAction::Pull => run0(ed, "vc-pull"),
            CommitAction::Push => run0(ed, "vc-push"),
            CommitAction::Log => run0(ed, "vc-log"),
            CommitAction::Blame => run0(ed, "vc-blame"),
            CommitAction::TakeOurs(path) => run(ed, "vc-take-ours", "path", path),
            CommitAction::TakeTheirs(path) => run(ed, "vc-take-theirs", "path", path),
        }
    }
}

fn run0(ed: &mut Editor, cmd: &str) {
    if let Err(e) = execute_command(ed, cmd, &HashMap::new()) {
        ed.show_message(MessageKind::Error, e);
    }
}

fn run(ed: &mut Editor, cmd: &str, arg: &str, value: String) {
    let mut args = HashMap::new();
    args.insert(arg.to_string(), ArgValue::String(value));
    if let Err(e) = execute_command(ed, cmd, &args) {
        ed.show_message(MessageKind::Error, e);
    }
}

fn run_int(ed: &mut Editor, cmd: &str, arg: &str, value: i64) {
    let mut args = HashMap::new();
    args.insert(arg.to_string(), ArgValue::Integer(value));
    if let Err(e) = execute_command(ed, cmd, &args) {
        ed.show_message(MessageKind::Error, e);
    }
}
