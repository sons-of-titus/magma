//! Path-parameterized version-control commands for the Commit tool window.
//!
//! The `*vc*` buffer's commands act on the cursor line; the GUI Commit window
//! has no cursor, so these take an explicit path (or message).  All repository
//! work dispatches through the background op lane — nothing shells out on the
//! foreground.

use crate::kernel::command::CommandResult;
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

    cmds.register_fn("vc-diff-file", "Load the inline diff preview for the given path",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| {
            let path = path_arg(args)?;
            spawn_preview(editor, path)
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

    cmds.register_fn("vc-amend",
        "Amend the last commit (arms amend mode, opens *vc-commit*)",
        vec![],
        |editor, _args| {
            editor.vc.amend_pending = true;
            super::vcs::vc_open_commit_buf_amend(editor)
        });

    cmds.register_fn("vc-branch-list", "Refresh the branch list for the Commit window",
        vec![],
        |editor, _args| spawn_branch_list(editor));

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

fn active(editor: &Editor) -> Result<(String, std::path::PathBuf), String> {
    match (editor.vc.active_backend_name.clone(), editor.vc.repo_root.clone()) {
        (Some(b), Some(r)) => Ok((b, r)),
        _ => Err("No active VC backend".to_string()),
    }
}

fn spawn_preview(editor: &mut Editor, path: String) -> CommandResult {
    let (backend, root) = active(editor)?;
    editor.vc.preview_path = Some(path.clone());
    editor.vc.preview = Some("Loading diff…".to_string());
    let op = crate::kernel::vc::VcRemoteOp::PreviewDiff(path.clone());
    if let Some(bg) = editor.background.clone() {
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let text = crate::kernel::vc::run_op(&root, &backend, &op)
                .unwrap_or_else(|e| format!("(diff unavailable: {e})"));
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::VcPreviewReady { path, text });
        });
    } else {
        let text = crate::kernel::vc::run_op(&root, &backend, &op)
            .unwrap_or_else(|e| format!("(diff unavailable: {e})"));
        editor.vc.preview = Some(text);
    }
    Ok(())
}

fn spawn_branch_list(editor: &mut Editor) -> CommandResult {
    let (backend, root) = active(editor)?;
    let op = crate::kernel::vc::VcRemoteOp::ListBranches;
    if let Some(bg) = editor.background.clone() {
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let out = crate::kernel::vc::run_op(&root, &backend, &op).unwrap_or_default();
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::VcBranchesReady {
                branches: parse_branches(&out),
            });
        });
    } else {
        let out = crate::kernel::vc::run_op(&root, &backend, &op).unwrap_or_default();
        editor.vc.branches = parse_branches(&out);
    }
    Ok(())
}

fn parse_branches(out: &str) -> Vec<String> {
    out.lines()
        .map(|l| l.trim_start_matches(['*', ' ']).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
