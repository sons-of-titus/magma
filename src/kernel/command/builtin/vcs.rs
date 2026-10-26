//! Shell, make, quickfix, and version-control commands.
//!
//! Version control is a GUI surface: `:vc` reveals the Commit tool window and
//! refreshes the status it renders.  Nothing here opens a text buffer for VC
//! status, and no keymap layer is pushed — the Commit panel's buttons drive
//! every mutation through the path-parameterized commands in `vc_actions`.

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::{Editor, MessageKind};

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("shell-command", "Run a shell command and show output in a buffer",
        vec![ArgSpec::new("command", ArgType::String)],
        |editor, args| {
            let cmd = args.get("command").and_then(|a| a.as_string())
                .ok_or_else(|| "shell-command requires a command string".to_string())?;
            crate::kernel::task::run_shell_command(editor, cmd)?;
            Ok(())
        },
    );

    cmds.register_fn("make", "Run make (with optional target) and show output",
        vec![ArgSpec::optional("target", ArgType::String, ArgValue::String("".to_string()))],
        |editor, args| {
            let target = args.get("target").and_then(|a| a.as_string())
                .unwrap_or("");
            crate::kernel::task::run_make(editor, target)?;
            Ok(())
        },
    );

    cmds.register_fn("quickfix-next", "Jump to the next quickfix entry (:cnext/:cn)",
        vec![],
        |editor, _args| {
            crate::kernel::task::quickfix_next(editor)
                .map_err(|e| e.to_string())?;
            Ok(())
        },
    );

    cmds.register_fn("quickfix-prev", "Jump to the previous quickfix entry (:cprev/:cp)",
        vec![],
        |editor, _args| {
            crate::kernel::task::quickfix_prev(editor)
                .map_err(|e| e.to_string())?;
            Ok(())
        },
    );

    cmds.register_fn("vc",
        "Open the Version Control tool window for the current buffer's repository",
        vec![],
        |editor, _args| vc_open(editor));

    cmds.register_fn("vc-refresh",
        "Refresh Version Control status, branches, and stashes",
        vec![],
        |editor, _args| vc_refresh(editor));

    cmds.register_fn("vc-log",
        "Show the recent commit log in a read-only buffer",
        vec![],
        |editor, _args| vc_show(editor, VcShow::Log));

    cmds.register_fn("vc-push",
        "Push to the remote",
        vec![],
        |editor, _args| vc_show(editor, VcShow::Push));

    cmds.register_fn("vc-diff",
        "Show the working-tree diff in a read-only buffer (optionally one file)",
        vec![ArgSpec::optional("file", ArgType::String, ArgValue::String(String::new()))],
        |editor, args| {
            let file = args.get("file").and_then(|a| a.as_string())
                .filter(|s| !s.is_empty()).map(|s| s.to_string());
            vc_show(editor, VcShow::Diff(file))
        });

    cmds.register_fn("vc-blame",
        "Show blame/annotate for the current file",
        vec![],
        |editor, _args| {
            let path = {
                let buf_id = get_current_buffer_id(editor);
                editor.buffers.get(buf_id)
                    .and_then(|a| a.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
                    .ok_or_else(|| "current buffer has no file path".to_string())?
            };
            vc_show(editor, VcShow::Blame(path))
        });

    super::vc_ops::register(editor);
}

enum VcShow { Diff(Option<String>), Log, Push, Blame(String) }

/// Determine the working directory from the focused buffer's path or cwd.
pub(crate) fn vc_working_dir(editor: &Editor) -> std::path::PathBuf {
    let buf_id = get_current_buffer_id(editor);
    editor.buffers.get(buf_id)
        .and_then(|a| a.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
        .as_deref()
        .and_then(|p| std::path::Path::new(p).parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Detect the repository for the current buffer and open the Commit panel on
/// it.  The GUI consumes `vc.request_open` to reveal the tool window.
fn vc_open(editor: &mut Editor) -> CommandResult {
    let dir = vc_working_dir(editor);
    let (backend_idx, root) = editor.vc.detect_for(&dir)
        .ok_or_else(|| "No version-control repository found".to_string())?;

    editor.vc.active_backend = Some(backend_idx);
    editor.vc.active_backend_name = Some(editor.vc.backends[backend_idx].name().to_string());
    editor.vc.repo_root = Some(root);
    editor.vc.request_open = true;
    // Derived state belongs to the previous repository; the diff view and the
    // branch/stash lists are cleared so nothing stale is ever rendered.
    super::vc_diff::close_diff(editor);
    editor.vc.branches.clear();
    editor.vc.stashes.clear();
    vc_refresh(editor)
}

/// Re-fetch status, branches, and stashes off the interactive path; the
/// `Vc*` events repopulate the Commit panel when the shell-outs complete.
pub(crate) fn vc_refresh(editor: &mut Editor) -> CommandResult {
    let (backend_idx, root) = match (editor.vc.active_backend, editor.vc.repo_root.clone()) {
        (Some(i), Some(r)) => (i, r),
        _ => return vc_open(editor),
    };
    // The backend registry is not clonable (trait objects); the background
    // job re-detects from the repo root — backends are stateless CLI
    // wrappers, so detection is exact and cheap.
    if let Some(bg) = editor.background.clone() {
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let result = crate::kernel::vc::status_for_root(&root);
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::Vc(
                crate::kernel::vc::Event::StatusFetched {
                    backend_idx, root, result,
                }));
        });
    } else {
        match crate::kernel::vc::status_for_root(&root) {
            Ok(status) => editor.vc.last_status = Some(status),
            Err(e) => editor.show_message(MessageKind::Error, e),
        }
    }
    super::vc_ops::spawn_branch_list(editor)?;
    super::vc_ops::spawn_stash_list(editor)?;
    Ok(())
}

/// Show output (diff, log, push result, blame) in a read-only buffer.  A
/// single file's diff opens the side-by-side view (per-hunk actions); the
/// whole-tree diff stays a plain read-only view.
fn vc_show(editor: &mut Editor, show: VcShow) -> CommandResult {
    let (name, remote) = match show {
        VcShow::Diff(Some(file)) => return super::vc_diff::spawn_diff(editor, file),
        VcShow::Diff(None) => ("*vc-diff-all*", crate::kernel::vc::VcRemoteOp::Diff(None)),
        VcShow::Log => ("*vc-log*", crate::kernel::vc::VcRemoteOp::Log),
        VcShow::Push => ("*vc-push*", crate::kernel::vc::VcRemoteOp::Push),
        VcShow::Blame(path) => ("*vc-blame*", crate::kernel::vc::VcRemoteOp::Blame(path)),
    };
    vc_spawn_op(editor, remote, Some(name), false)
}

/// Dispatch a repository operation on the maintenance lane; the `Vc(OpDone)`
/// event writes output and refreshes the panel from the foreground.
pub(crate) fn vc_spawn_op(
    editor: &mut Editor,
    op: crate::kernel::vc::VcRemoteOp,
    scratch: Option<&str>,
    then_refresh: bool,
) -> CommandResult {
    let (backend, root) = match (editor.vc.active_backend_name.clone(), editor.vc.repo_root.clone()) {
        (Some(b), Some(r)) => (b, r),
        _ => return Err("No active VC backend".to_string()),
    };
    let Some(bg) = editor.background.clone() else {
        // No background pool (tests, embedded): synchronous.
        let result = crate::kernel::vc::run_op(&root, &backend, &op);
        apply_vc_op(editor, scratch, then_refresh, result)?;
        return Ok(());
    };
    let sender = bg.sender.clone();
    let scratch_name = scratch.map(String::from);
    bg.spawn_blocking(move || {
        let result = crate::kernel::vc::run_op(&root, &backend, &op);
        let _ = sender.send(crate::kernel::runtime::BackgroundEvent::Vc(
            crate::kernel::vc::Event::OpDone {
                scratch: scratch_name,
                then_refresh,
                result,
            }));
    });
    Ok(())
}

/// Apply a finished operation: scratch output, follow-up refresh.
pub(crate) fn apply_vc_op(
    editor: &mut Editor,
    scratch: Option<&str>,
    then_refresh: bool,
    result: Result<String, String>,
) -> CommandResult {
    match result {
        Ok(output) => {
            if let Some(name) = scratch {
                vc_output_buf(editor, name, &output)?;
            }
            if then_refresh {
                vc_refresh(editor)?;
                // A stage/discard/commit rewrites the worktree: reload the
                // diff view so its hunks are never stale.
                super::vc_diff::reload_diff(editor);
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Write `text` into a named read-only output buffer and display it.
pub(crate) fn vc_output_buf(editor: &mut Editor, name: &str, text: &str) -> CommandResult {
    let key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).name == name)
        .map(|(k, _)| k)
        .unwrap_or_else(|| editor.create_buffer(name));
    {
        let arc = editor.buffers.get_mut(key).unwrap();
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, text);
        buf.read_only = true;
        buf.ephemeral = true;
    }
    if let Some(view) = editor.view_of_buffer_mut(key) {
        view.set_cursor(0);
    }
    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    Ok(())
}
