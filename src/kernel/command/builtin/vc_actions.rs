//! Path-parameterized version-control commands for the Commit tool window.
//!
//! The Commit window has no cursor, so these take an explicit path (or
//! message).  All repository work dispatches through the background op lane —
//! nothing shells out on the foreground.

use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;

use super::vcs::vc_spawn_op;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("vc-stage-file", "Stage the given path (Commit window)",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| {
            let path = path_arg(args)?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Stage(path), None, true)
        });

    cmds.register_fn("vc-unstage-file", "Unstage the given path (Commit window)",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| {
            let path = path_arg(args)?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Unstage(path), None, true)
        });

    cmds.register_fn("vc-discard-file", "Discard worktree changes to the given path",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| {
            let path = path_arg(args)?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::DiscardFile(path), None, true)
        });

    cmds.register_fn("vc-diff-file", "Open the given path's side-by-side diff view",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| {
            let path = path_arg(args)?;
            super::vc_diff::spawn_diff(editor, path)
        });

    cmds.register_fn("vc-commit-message", "Commit staged changes with the given message",
        vec![ArgSpec::new("message", ArgType::String)],
        |editor, args| {
            let msg = args.get("message").and_then(|a| a.as_string())
                .map(str::to_string).unwrap_or_default();
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Commit(msg), None, true)
        });

    cmds.register_fn("vc-amend-message",
        "Amend the last commit with the given message (empty keeps the previous one)",
        vec![ArgSpec::optional("message", ArgType::String, ArgValue::String(String::new()))],
        |editor, args| {
            let msg = args.get("message").and_then(|a| a.as_string())
                .map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Amend(msg), None, true)
        });

    cmds.register_fn("vc-branch-list", "Refresh the branch list for the Commit window",
        vec![],
        |editor, _args| super::vc_ops::spawn_branch_list(editor));

    cmds.register_fn("vc-checkout-branch", "Check out an existing branch",
        vec![ArgSpec::new("branch", ArgType::String)],
        |editor, args| {
            let branch = args.get("branch").and_then(|a| a.as_string())
                .map(str::to_string).filter(|s| !s.is_empty())
                .ok_or_else(|| "a branch name is required".to_string())?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Checkout { branch, create: false },
                None, true)
        });
}

fn path_arg(args: &std::collections::HashMap<String, ArgValue>) -> Result<String, String> {
    args.get("path").and_then(|a| a.as_string())
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "a path is required".to_string())
}
