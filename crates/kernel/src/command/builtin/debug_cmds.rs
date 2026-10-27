//! Debugger commands for the Debug tool window.
//!
//! The lifecycle verbs (continue, step, pause) stay in Janet; these are the
//! primitives the redesigned Debug panel needs that had no command surface:
//! expanding a variables node, selecting a stack frame, and per-breakpoint
//! enable/remove from the breakpoints list.

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::state::Editor;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "dbg-expand",
        "Fetch the child variables of a variables reference (Debug panel)",
        vec![ArgSpec::new("reference", ArgType::Integer)],
        |editor, args| {
            let r = args
                .get("reference")
                .and_then(|a| a.as_integer())
                .ok_or_else(|| "a variables reference is required".to_string())?;
            let sid = active_session(editor)?;
            crate::runtime_dap::request_variables(editor, sid, r);
            Ok(())
        },
    );

    cmds.register_fn(
        "dbg-frame",
        "Select the stack frame whose scopes the Debug panel inspects",
        vec![ArgSpec::new("frame", ArgType::Integer)],
        |editor, args| {
            let frame_id = args
                .get("frame")
                .and_then(|a| a.as_integer())
                .filter(|&i| i >= 0)
                .map(|i| i as u64)
                .ok_or_else(|| "a frame id is required".to_string())?;
            let sid = active_session(editor)?;
            let session = editor
                .debug
                .sessions
                .get_mut(&sid)
                .ok_or_else(|| "no active debug session".to_string())?;
            if !session.frames.iter().any(|f| f.id == frame_id) {
                return Err("no such frame in the current stack".to_string());
            }
            session.selected_frame = Some(frame_id);
            crate::script_runtime::send_dap_command(
                editor,
                sid,
                "scopes",
                &format!(r#"{{"frameId":{frame_id}}}"#),
            );
            crate::runtime_dap::evaluate_watches_for_frame(editor, sid, frame_id);
            Ok(())
        },
    );

    cmds.register_fn(
        "dbg-bp-enable",
        "Enable or disable the breakpoints at a file line (Debug panel)",
        vec![
            ArgSpec::new("path", ArgType::Path),
            ArgSpec::new("line", ArgType::Integer),
            ArgSpec::new("enabled", ArgType::Boolean),
        ],
        |editor, args| {
            let (path, line, enabled) = bp_args(args)?;
            crate::debug::set_breakpoint_enabled(editor, &path, line, enabled);
            Ok(())
        },
    );

    cmds.register_fn(
        "dbg-bp-remove",
        "Remove the breakpoints at a file line (Debug panel)",
        vec![
            ArgSpec::new("path", ArgType::Path),
            ArgSpec::new("line", ArgType::Integer),
        ],
        |editor, args| {
            let (path, line, _) = bp_args(args)?;
            crate::debug::remove_breakpoints_at(editor, &path, line);
            Ok(())
        },
    );
}

fn active_session(editor: &Editor) -> Result<u64, String> {
    editor
        .debug
        .active_session
        .ok_or_else(|| "no active debug session".to_string())
}

/// The breakpoint commands share (path, 1-based line) plus an optional
/// enabled flag consumed only by `dbg-bp-enable`.
fn bp_args(
    args: &std::collections::HashMap<String, ArgValue>,
) -> Result<(String, usize, bool), String> {
    let path = args
        .get("path")
        .and_then(|a| a.as_string())
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "a path is required".to_string())?;
    let line = args
        .get("line")
        .and_then(|a| a.as_integer())
        .filter(|&l| l > 0)
        .map(|l| l as usize)
        .ok_or_else(|| "a 1-based line is required".to_string())?;
    let enabled = args
        .get("enabled")
        .and_then(|a| a.as_bool())
        .unwrap_or(true);
    Ok((path, line, enabled))
}
