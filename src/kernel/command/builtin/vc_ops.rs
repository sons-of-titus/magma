//! Branch and stash commands behind the Commit tool window (F-23).
//!
//! Index-addressed surfaces for the GUI panel: stashes are addressed by the
//! index the panel rendered, never by a cursor line.  Hunk staging lives in
//! `vc_diff` (the side-by-side diff view).  Repository work still dispatches through
//! the background op lane, so nothing shells out on the foreground.

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::state::Editor;
use crate::kernel::vc::VcRemoteOp;
use crate::kernel::vc::backends::{parse_branch_lines, parse_stash_lines};

use super::vcs::vc_spawn_op;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("vc-branch-create",
        "Create and check out a new branch",
        vec![ArgSpec::new("name", ArgType::String)],
        |editor, args| {
            let name = args.get("name").and_then(|a| a.as_string())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "vc-branch-create requires a branch name".to_string())?;
            vc_spawn_op(editor, VcRemoteOp::Checkout { branch: name, create: true }, None, true)
        });

    cmds.register_fn("vc-stash",
        "Stash the working tree changes (vc-stash [message])",
        vec![ArgSpec::optional("message", ArgType::String, ArgValue::String(String::new()))],
        |editor, args| {
            let msg = args.get("message").and_then(|a| a.as_string())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            vc_spawn_op(editor, VcRemoteOp::StashPush(msg), None, true)
        });

    cmds.register_fn("vc-stash-pop",
        "Pop the stash at the given index",
        vec![ArgSpec::new("index", ArgType::Integer)],
        |editor, args| stash_op(editor, index_arg(args)?, StashOp::Pop));

    cmds.register_fn("vc-stash-apply",
        "Apply (keep) the stash at the given index",
        vec![ArgSpec::new("index", ArgType::Integer)],
        |editor, args| stash_op(editor, index_arg(args)?, StashOp::Apply));

    cmds.register_fn("vc-stash-drop",
        "Drop the stash at the given index",
        vec![ArgSpec::new("index", ArgType::Integer)],
        |editor, args| stash_op(editor, index_arg(args)?, StashOp::Drop));
}

// ── Branch list ──────────────────────────────────────────────────────────────

/// Refresh `vc.branches` for the Commit panel's branch widget.
pub(crate) fn spawn_branch_list(editor: &mut Editor) -> CommandResult {
    let (backend, root) = active(editor)?;
    if let Some(bg) = editor.background.clone() {
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let branches = crate::kernel::vc::run_op(&root, &backend, &VcRemoteOp::ListBranches)
                .map(|out| parse_branch_lines(&out).into_iter().map(|b| b.name).collect())
                .unwrap_or_default();
            let _ = sender.send(BackgroundEvent::Vc(
                crate::kernel::vc::Event::BranchesReady { branches }));
        });
    } else {
        editor.vc.branches = crate::kernel::vc::run_op(&root, &backend, &VcRemoteOp::ListBranches)
            .map(|out| parse_branch_lines(&out).into_iter().map(|b| b.name).collect())
            .unwrap_or_default();
    }
    Ok(())
}

// ── Stash list ───────────────────────────────────────────────────────────────

/// Refresh `vc.stashes` for the Commit panel's stash section.
pub(crate) fn spawn_stash_list(editor: &mut Editor) -> CommandResult {
    let (backend, root) = active(editor)?;
    if let Some(bg) = editor.background.clone() {
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            // A backend without stash support reports "not supported"; that is
            // an empty list to the panel, not an error.
            let stashes = crate::kernel::vc::run_op(&root, &backend, &VcRemoteOp::StashList)
                .map(|out| parse_stash_lines(&out))
                .unwrap_or_default();
            let _ = sender.send(BackgroundEvent::Vc(
                crate::kernel::vc::Event::StashListed { result: Ok(stashes) }));
        });
    } else {
        editor.vc.stashes = crate::kernel::vc::run_op(&root, &backend, &VcRemoteOp::StashList)
            .map(|out| parse_stash_lines(&out))
            .unwrap_or_default();
    }
    Ok(())
}

// ── Stash ops ────────────────────────────────────────────────────────────────

enum StashOp { Pop, Apply, Drop }

fn stash_op(editor: &mut Editor, index: usize, op: StashOp) -> CommandResult {
    let remote = match op {
        StashOp::Pop => VcRemoteOp::StashApply { index, pop: true },
        StashOp::Apply => VcRemoteOp::StashApply { index, pop: false },
        StashOp::Drop => VcRemoteOp::StashDrop(index),
    };
    vc_spawn_op(editor, remote, None, true)
}

// ── shared ───────────────────────────────────────────────────────────────────

fn active(editor: &Editor) -> Result<(String, std::path::PathBuf), String> {
    match (editor.vc.active_backend_name.clone(), editor.vc.repo_root.clone()) {
        (Some(b), Some(r)) => Ok((b, r)),
        _ => Err("No active VC backend".to_string()),
    }
}

fn index_arg(args: &std::collections::HashMap<String, ArgValue>) -> Result<usize, String> {
    args.get("index").and_then(|a| a.as_integer())
        .filter(|&i| i >= 0)
        .map(|i| i as usize)
        .ok_or_else(|| "an index is required".to_string())
}
