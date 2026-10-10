//! Shell, make, quickfix, and version-control commands.

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;

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
        "Open the VC status buffer for the current buffer's repository",
        vec![],
        |editor, _args| vc_open(editor),
    );

    cmds.register_fn("vc-refresh",
        "Refresh the VC status buffer",
        vec![],
        |editor, _args| vc_refresh(editor),
    );

    cmds.register_fn("vc-stage",
        "Stage the file under the cursor in the VC buffer",
        vec![],
        |editor, _args| vc_file_op(editor, VcOp::Stage),
    );

    cmds.register_fn("vc-unstage",
        "Unstage the file under the cursor in the VC buffer",
        vec![],
        |editor, _args| vc_file_op(editor, VcOp::Unstage),
    );

    cmds.register_fn("vc-diff",
        "Show diff for the file under the cursor (or all changes)",
        vec![ArgSpec::optional("file", ArgType::String, ArgValue::String(String::new()))],
        |editor, args| {
            let file_arg = args.get("file").and_then(|a| a.as_string())
                .filter(|s| !s.is_empty()).map(|s| s.to_string());
            vc_show(editor, VcShow::Diff(file_arg))
        },
    );

    cmds.register_fn("vc-diff-cursor",
        "Show diff for the file under the cursor in the VC buffer",
        vec![],
        |editor, _args| vc_file_op(editor, VcOp::Diff),
    );

    cmds.register_fn("vc-log",
        "Show the recent commit log",
        vec![],
        |editor, _args| vc_show(editor, VcShow::Log),
    );

    cmds.register_fn("vc-commit",
        "Commit with the message in the *vc-commit* buffer, or open it if empty",
        vec![ArgSpec::optional("message", ArgType::String, ArgValue::String(String::new()))],
        |editor, args| {
            let msg = args.get("message").and_then(|a| a.as_string())
                .filter(|s| !s.is_empty()).map(|s| s.to_string());
            if let Some(m) = msg {
                vc_commit(editor, &m)
            } else {
                vc_open_commit_buf(editor)
            }
        },
    );

    cmds.register_fn("vc-commit-save",
        "Commit using the message written in the *vc-commit* buffer",
        vec![],
        |editor, _args| {
            let msg = vc_read_commit_buf(editor)?;
            vc_commit(editor, &msg)
        },
    );

    cmds.register_fn("vc-push",
        "Push to the remote",
        vec![],
        |editor, _args| vc_show(editor, VcShow::Push),
    );

    cmds.register_fn("vc-blame",
        "Show blame/annotate for the current file",
        vec![],
        |editor, _args| {
            let path = {
                let buf_id = get_current_buffer_id(editor);
                editor.buffers.get(buf_id)
                    .and_then(|a| a.lock().unwrap().path.clone())
                    .ok_or_else(|| "current buffer has no file path".to_string())?
            };
            vc_show(editor, VcShow::Blame(path))
        },
    );

    cmds.register_fn("vc-close",
        "Close the VC buffer and pop the vc keymap layer",
        vec![],
        |editor, _args| {
            editor.keymaps.pop_layer("vc");
            editor.vc.buf_key = None;
            editor.vc.active_backend = None;
            Ok(())
        },
    );

    // VC keymap layer
    editor.keymaps.set_layer("vc", "g",      "vc-refresh");
    editor.keymaps.set_layer("vc", "r",      "vc-refresh");
    editor.keymaps.set_layer("vc", "s",      "vc-stage");
    editor.keymaps.set_layer("vc", "u",      "vc-unstage");
    editor.keymaps.set_layer("vc", "d",      "vc-diff-cursor");
    editor.keymaps.set_layer("vc", "D",      "vc-diff");
    editor.keymaps.set_layer("vc", "c",      "vc-commit");
    editor.keymaps.set_layer("vc", "p",      "vc-push");
    editor.keymaps.set_layer("vc", "l",      "vc-log");
    editor.keymaps.set_layer("vc", "q",      "vc-close");
}

enum VcOp { Stage, Unstage, Diff }
enum VcShow { Diff(Option<String>), Log, Push, Blame(String) }

/// Determine the working directory from the focused buffer's path or cwd.
fn vc_working_dir(editor: &Editor) -> std::path::PathBuf {
    let buf_id = get_current_buffer_id(editor);
    editor.buffers.get(buf_id)
        .and_then(|a| a.lock().unwrap().path.clone())
        .as_deref()
        .and_then(|p| std::path::Path::new(p).parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Return the 0-indexed line the VC buffer cursor is on.
fn vc_cursor_line(editor: &Editor) -> usize {
    let Some(key) = editor.vc.buf_key else { return 0 };
    let cursor = editor.view_of_buffer(key).map(|v| v.cursor_offset()).unwrap_or(0);
    let Some(arc) = editor.buffers.get(key) else { return 0 };
    let buf = arc.lock().unwrap();
    buf.slice(0, cursor).chars().filter(|&c| c == '\n').count()
}

/// Write `text` into the VC buffer, preserving cursor line.
fn vc_write_buf(editor: &mut Editor, text: &str) -> CommandResult {
    let Some(key) = editor.vc.buf_key else { return Ok(()); };
    if !editor.buffers.contains(key) { return Ok(()); }
    let saved_line = {
        let cursor = editor.view_of_buffer(key).map(|v| v.cursor_offset()).unwrap_or(0);
        let buf = editor.buffers.get(key).unwrap().lock().unwrap();
        buf.slice(0, cursor).chars().filter(|&c| c == '\n').count()
    };
    {
        let arc = editor.buffers.get_mut(key).unwrap();
        let mut buf = arc.lock().unwrap();
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, text);
    }
    let offset = editor.buffers.get(key).unwrap().lock().unwrap().line_start_offset(saved_line);
    if let Some(off) = offset {
        if let Some(view) = editor.view_of_buffer_mut(key) {
            view.set_cursor(off);
        }
    }
    Ok(())
}

/// Open or refresh the `*vc*` status buffer for the current buffer's repo.
fn vc_open(editor: &mut Editor) -> CommandResult {
    let dir = vc_working_dir(editor);

    let (backend_idx, root) = editor.vc.detect_for(&dir)
        .ok_or_else(|| "No version-control repository found".to_string())?;

    editor.vc.active_backend = Some(backend_idx);
    editor.vc.active_backend_name = Some(editor.vc.backends[backend_idx].name().to_string());
    editor.vc.repo_root = Some(root.clone());

    let status = editor.vc.backends[backend_idx].status(&root)?;
    let text = crate::kernel::vc::build_display(&root, &status);
    editor.vc.last_status = Some(status);

    // Find or create *vc* buffer
    let buf_key = editor.vc.buf_key
        .and_then(|k| editor.buffers.get(k).map(|_| k))
        .unwrap_or_else(|| {
            editor.create_buffer("*vc*")
        });
    editor.vc.buf_key = Some(buf_key);

    {
        let arc = editor.buffers.get_mut(buf_key).unwrap();
        let mut buf = arc.lock().unwrap();
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, &text);
    }
    let header_offset = editor.buffers.get(buf_key).unwrap().lock().unwrap()
        .line_start_offset(crate::kernel::vc::VC_HEADER_LINES);
    if let Some(off) = header_offset {
        if let Some(view) = editor.view_of_buffer_mut(buf_key) {
            view.set_cursor(off);
        }
    }

    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(buf_key);
    }
    editor.keymaps.push_layer("vc");
    Ok(())
}

/// Re-fetch status off the foreground thread; the `VcStatusFetched` event
/// redraws the *vc* buffer when the shell-out completes (rework.md §1: no
/// blocking IO on the interactive path).
fn vc_refresh(editor: &mut Editor) -> CommandResult {
    let (backend_idx, root) = match (editor.vc.active_backend, editor.vc.repo_root.clone()) {
        (Some(i), Some(r)) => (i, r),
        _ => return vc_open(editor),
    };
    let Some(bg) = editor.background.clone() else {
        // No background pool (tests, server-less embed): synchronous.
        let status = editor.vc.backends[backend_idx].status(&root)?;
        let text = crate::kernel::vc::build_display(&root, &status);
        editor.vc.last_status = Some(status);
        vc_write_buf(editor, &text)?;
        return Ok(());
    };
    // The backend registry is not clonable (trait objects); the background
    // job re-detects from the repo root — backends are stateless CLI
    // wrappers, so detection is exact and cheap.
    let sender = bg.sender.clone();
    bg.spawn_blocking(move || {
        let result = crate::kernel::vc::status_for_root(&root);
        let _ = sender.send(crate::kernel::runtime::BackgroundEvent::VcStatusFetched {
            backend_idx, root, result,
        });
    });
    Ok(())
}

/// Redraw the *vc* buffer from `vc.last_status` (the async status path).
pub(crate) fn redraw_vc_buffer(editor: &mut Editor) {
    let Some(root) = editor.vc.repo_root.clone() else { return };
    let Some(status) = editor.vc.last_status.clone() else { return };
    let text = crate::kernel::vc::build_display(&root, &status);
    if let Err(e) = vc_write_buf(editor, &text) {
        editor.show_message(crate::kernel::state::MessageKind::Error, e);
    }
}

/// Apply a file operation (stage / unstage / diff) to the entry under the cursor.
fn vc_file_op(editor: &mut Editor, op: VcOp) -> CommandResult {
    let line = vc_cursor_line(editor);
    let path = editor.vc.last_status.as_ref()
        .and_then(|s| crate::kernel::vc::path_at_line(s, line))
        .ok_or_else(|| "No file on this line".to_string())?;

    let remote = match op {
        VcOp::Stage => crate::kernel::vc::VcRemoteOp::Stage(path),
        VcOp::Unstage => crate::kernel::vc::VcRemoteOp::Unstage(path),
        VcOp::Diff => crate::kernel::vc::VcRemoteOp::Diff(Some(path)),
    };
    vc_spawn_op(editor, remote, Some("*vc-diff*"), matches!(op, VcOp::Stage | VcOp::Unstage))
}

/// Dispatch a repository operation on the maintenance lane; the
/// `VcOpDone` event writes the scratch buffer and refreshes status from
/// the foreground (rework.md §1: no blocking IO on the interactive path).
fn vc_spawn_op(
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
        let _ = sender.send(crate::kernel::runtime::BackgroundEvent::VcOpDone {
            scratch: scratch_name,
            then_refresh,
            result,
        });
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
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Show output (diff, log, push result, blame) in a scratch buffer.
fn vc_show(editor: &mut Editor, show: VcShow) -> CommandResult {
    let (scratch, remote) = match show {
        VcShow::Diff(file) => ("*vc-diff*", crate::kernel::vc::VcRemoteOp::Diff(file)),
        VcShow::Log => ("*vc-log*", crate::kernel::vc::VcRemoteOp::Log),
        VcShow::Push => ("*vc-push*", crate::kernel::vc::VcRemoteOp::Push),
        VcShow::Blame(path) => ("*vc-blame*", crate::kernel::vc::VcRemoteOp::Blame(path)),
    };
    vc_spawn_op(editor, remote, Some(scratch), false)
}

/// Write `text` into a named scratch buffer and display it.
fn vc_output_buf(editor: &mut Editor, name: &str, text: &str) -> CommandResult {
    let key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap().name == name)
        .map(|(k, _)| k)
        .unwrap_or_else(|| {
            editor.create_buffer(name)
        });
    {
        let arc = editor.buffers.get_mut(key).unwrap();
        let mut buf = arc.lock().unwrap();
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, text);
    }
    if let Some(view) = editor.view_of_buffer_mut(key) {
        view.set_cursor(0);
    }
    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    Ok(())
}

/// Open a `*vc-commit*` buffer and drop to insert mode.
fn vc_open_commit_buf(editor: &mut Editor) -> CommandResult {
    vc_output_buf(editor, "*vc-commit*",
        "# Enter commit message above. Run :VCcommit-save when done.\n")?;
    editor.editor_mode = crate::kernel::state::mode::EditorMode::new("insert", true);
    editor.keymaps.pop_layer("editor");
    editor.keymaps.push_layer("insert");
    Ok(())
}

/// Read and return the trimmed commit message from `*vc-commit*`, stripping `#` lines.
fn vc_read_commit_buf(editor: &mut Editor) -> Result<String, String> {
    let key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap().name == "*vc-commit*")
        .map(|(k, _)| k)
        .ok_or_else(|| "No *vc-commit* buffer open".to_string())?;
    let text = {
        let arc = editor.buffers.get(key).unwrap();
        let buf = arc.lock().unwrap();
        buf.slice(0, buf.len())
    };
    let msg: String = text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let msg = msg.trim().to_string();
    if msg.is_empty() { return Err("Commit message is empty".to_string()); }
    Ok(msg)
}

/// Commit with the given message and refresh the VC buffer.
fn vc_commit(editor: &mut Editor, message: &str) -> CommandResult {
    vc_spawn_op(editor, crate::kernel::vc::VcRemoteOp::Commit(message.to_string()), None, true)
}
