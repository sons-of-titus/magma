//! Branch, hunk, stash, and conflict commands (F-23).
//!
//! Each list-style surface is a scratch buffer plus a keymap layer: the
//! cursor line maps back to the entry it renders, exactly like the `*vc*`
//! status buffer.  Repository work dispatches through the background op
//! path — nothing shells out on the foreground.

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;

use super::vcs::{vc_spawn_op, VC_BRANCHES_BUF, VC_HUNKS_BUF, VC_STASH_BUF};

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("vc-branches",
        "List branches; return/c checks out the one under the cursor",
        vec![],
        |editor, _args| {
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::ListBranches,
                Some(VC_BRANCHES_BUF), false)
        },
    );
    cmds.register_fn("vc-checkout-cursor",
        "Check out the branch under the cursor in *vc-branches*",
        vec![],
        |editor, _args| {
            let branch = branch_at_cursor(editor)
                .ok_or_else(|| "No branch on this line".to_string())?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Checkout {
                branch, create: false,
            }, None, true)
        },
    );
    cmds.register_fn("vc-branch-create",
        "Create and check out a new branch (vc-branch-create <name>)",
        vec![ArgSpec::new("name", ArgType::String)],
        |editor, args| {
            let name = args.get("name").and_then(|a| a.as_string())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "vc-branch-create requires a branch name".to_string())?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Checkout {
                branch: name, create: true,
            }, None, true)
        },
    );

    cmds.register_fn("vc-hunks",
        "Split the unstaged diff of the file under the cursor into hunks; s stages, x discards one",
        vec![],
        |editor, _args| vc_hunks(editor),
    );
    cmds.register_fn("vc-stage-hunk",
        "Stage the hunk under the cursor in *vc-hunks*",
        vec![],
        |editor, _args| vc_hunk_op(editor, true),
    );
    cmds.register_fn("vc-discard-hunk",
        "Discard the hunk under the cursor in *vc-hunks* (rewrites the worktree)",
        vec![],
        |editor, _args| vc_hunk_op(editor, false),
    );

    cmds.register_fn("vc-stash",
        "Stash the working tree changes (vc-stash [message]) and refresh",
        vec![ArgSpec::optional("message", ArgType::String, ArgValue::String(String::new()))],
        |editor, args| {
            let msg = args.get("message").and_then(|a| a.as_string())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::StashPush(msg), None, true)
        },
    );
    cmds.register_fn("vc-stash-list",
        "List stashes; return pops, a applies, x drops the one under the cursor",
        vec![],
        |editor, _args| {
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::StashList,
                Some(VC_STASH_BUF), false)
        },
    );
    cmds.register_fn("vc-stash-pop-cursor",
        "Pop the stash under the cursor in *vc-stash*",
        vec![],
        |editor, _args| {
            let index = stash_index_at_cursor(editor)?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::StashApply { index, pop: true }, None, true)
        },
    );
    cmds.register_fn("vc-stash-apply-cursor",
        "Apply (keep) the stash under the cursor in *vc-stash*",
        vec![],
        |editor, _args| {
            let index = stash_index_at_cursor(editor)?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::StashApply { index, pop: false }, None, true)
        },
    );
    cmds.register_fn("vc-stash-drop-cursor",
        "Drop the stash under the cursor in *vc-stash*",
        vec![],
        |editor, _args| {
            let index = stash_index_at_cursor(editor)?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::StashDrop(index), None, true)
        },
    );

    cmds.register_fn("vc-resolve",
        "Mark the conflicted file under the cursor resolved by staging it",
        vec![],
        |editor, _args| {
            let line = super::vcs::vc_cursor_line(editor);
            let path = editor.vc.last_status.as_ref()
                .and_then(|s| crate::kernel::vc::path_at_line(s, line))
                .ok_or_else(|| "No file on this line".to_string())?;
            vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Stage(path), None, true)
        },
    );

    // Secondary keymap layers, pushed when their buffer opens.
    editor.keymaps.set_layer("vc-branches", "return", "vc-checkout-cursor");
    editor.keymaps.set_layer("vc-branches", "c",       "vc-checkout-cursor");
    editor.keymaps.set_layer("vc-hunks", "s",          "vc-stage-hunk");
    editor.keymaps.set_layer("vc-hunks", "x",          "vc-discard-hunk");
    editor.keymaps.set_layer("vc-stash", "return",     "vc-stash-pop-cursor");
    editor.keymaps.set_layer("vc-stash", "a",          "vc-stash-apply-cursor");
    editor.keymaps.set_layer("vc-stash", "x",          "vc-stash-drop-cursor");
}

// ── Branches ─────────────────────────────────────────────────────────────────

/// The branch on the cursor line of `*vc-branches*` (`* name` / `  name`).
fn branch_at_cursor(editor: &mut Editor) -> Option<String> {
    let key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).name == VC_BRANCHES_BUF)
        .map(|(k, _)| k)?;
    let line = cursor_line_in(editor, key);
    let arc = editor.buffers.get(key)?;
    let text = arc.lock().unwrap_or_else(|e| e.into_inner()).line(line)?;
    if text.starts_with('#') { return None; }
    let name = text.trim_start_matches(['*', ' ']).trim().to_string();
    (!name.is_empty()).then_some(name)
}

// ── Hunks ────────────────────────────────────────────────────────────────────

/// Fetch the unstaged diff of the file under the cursor on the maintenance
/// lane; the `vc-hunks-diff` event renders the `*vc-hunks*` buffer.
fn vc_hunks(editor: &mut Editor) -> CommandResult {
    let line = super::vcs::vc_cursor_line(editor);
    let path = editor.vc.last_status.as_ref()
        .and_then(|s| crate::kernel::vc::path_at_line(s, line))
        .ok_or_else(|| "No file on this line".to_string())?;

    let (backend, root) = match (editor.vc.active_backend_name.clone(), editor.vc.repo_root.clone()) {
        (Some(b), Some(r)) => (b, r),
        _ => return Err("No active VC backend".to_string()),
    };
    let Some(bg) = editor.background.clone() else {
        let diff = crate::kernel::vc::run_op(&root, &backend,
            &crate::kernel::vc::VcRemoteOp::Diff(Some(path)))?;
        return build_hunks_buf(editor, &diff);
    };
    let sender = bg.sender.clone();
    bg.spawn_on(crate::kernel::runtime::Lane::Maintenance, Some("vc-hunks"),
        move || {
            let diff = crate::kernel::vc::run_op(&root, &backend,
                &crate::kernel::vc::VcRemoteOp::HunkDiff(path))
                .unwrap_or_default();
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::Custom(
                "vc-hunks-diff".to_string(), diff,
            ));
        });
    Ok(())
}

/// Render `*vc-hunks*` from a raw diff: one marker line per hunk, the
/// single-hunk patches kept on `VcState` for stage/discard.
pub(crate) fn build_hunks_buf(editor: &mut Editor, diff: &str) -> CommandResult {
    let hunks = crate::kernel::vc::backends::split_hunks(diff);
    if hunks.is_empty() {
        editor.show_message(crate::kernel::state::MessageKind::Info, "no unstaged hunks");
        return Ok(());
    }
    let mut text = String::from("# s stage hunk · x discard hunk · q close\n\n");
    for (i, patch) in hunks.iter().enumerate() {
        let header = patch.lines().find(|l| l.starts_with("@@")).unwrap_or("");
        text.push_str(&format!("Hunk {} of {}: {header}\n", i + 1, hunks.len()));
        for body in patch.lines().skip_while(|l| !l.starts_with("@@")).skip(1) {
            text.push_str(body);
            text.push('\n');
        }
        text.push('\n');
    }
    editor.vc.last_hunks = hunks;
    super::vcs::vc_output_buf(editor, VC_HUNKS_BUF, &text)?;
    editor.keymaps.push_layer("vc-hunks");
    Ok(())
}

/// The hunk index (0-based) whose block contains the cursor line.
fn hunk_at_cursor(editor: &mut Editor) -> Option<usize> {
    let key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).name == VC_HUNKS_BUF)
        .map(|(k, _)| k)?;
    let line = cursor_line_in(editor, key);
    let arc = editor.buffers.get(key)?;
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let mut idx = None;
    for n in 0..=line {
        if let Some(text) = buf.line(n)
            && text.starts_with("Hunk ") {
                idx = text.split_whitespace().nth(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .map(|n| n - 1);
            }
    }
    idx
}

fn vc_hunk_op(editor: &mut Editor, stage: bool) -> CommandResult {
    let Some(idx) = hunk_at_cursor(editor) else {
        return Err("No hunk on this line".to_string());
    };
    let Some(patch) = editor.vc.last_hunks.get(idx).cloned() else {
        return Err("Hunk list is stale — reopen with :vc-hunks".to_string());
    };
    let op = if stage {
        crate::kernel::vc::VcRemoteOp::StagePatch(patch)
    } else {
        crate::kernel::vc::VcRemoteOp::DiscardPatch(patch)
    };
    vc_spawn_op(editor, op, None, true)?;
    if !stage {
        editor.show_message(crate::kernel::state::MessageKind::Info,
            "hunk discarded — reopen :vc-hunks for the fresh diff");
    }
    Ok(())
}

// ── Stash ────────────────────────────────────────────────────────────────────

/// The `stash@{N}` index on the cursor line of `*vc-stash*`.
fn stash_index_at_cursor(editor: &mut Editor) -> Result<usize, String> {
    let key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).name == VC_STASH_BUF)
        .map(|(k, _)| k)
        .ok_or_else(|| "No *vc-stash* buffer open".to_string())?;
    let line = cursor_line_in(editor, key);
    let Some(arc) = editor.buffers.get(key) else {
        return Err("No *vc-stash* buffer open".to_string());
    };
    let entry = arc.lock().unwrap_or_else(|e| e.into_inner())
        .line(line)
        .ok_or_else(|| "No stash on this line".to_string())?;
    entry.trim_start_matches("stash@{")
        .split('}').next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| "No stash on this line".to_string())
}

// ── shared ───────────────────────────────────────────────────────────────────

/// Cursor line within an arbitrary buffer key.
fn cursor_line_in(editor: &Editor, key: usize) -> usize {
    let cursor = editor.view_of_buffer(key).map(|v| v.cursor_offset()).unwrap_or(0);
    let Some(arc) = editor.buffers.get(key) else { return 0 };
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    buf.slice(0, cursor).chars().filter(|&c| c == '\n').count()
}
