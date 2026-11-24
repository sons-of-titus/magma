//! Agent tool execution and environment inspection primitives.

use std::collections::HashMap;
use std::path::Path;

use agent::llm::{self, ToolCall};
use serde_json::{Value, json};

use crate::command::args::ArgValue;
use crate::command::execute_command;
use crate::state::Editor;

/// Editor commands that never mutate files or spawn processes.  Every name
/// here must be a registered command: an exempt command that is not
/// registered falls through execute_tool's shell fallback and would run
/// without approval.
pub const APPROVAL_EXEMPT_COMMANDS: &[&str] = &["find-files", "search-buffer"];

pub fn requires_approval(call: &ToolCall) -> bool {
    match call.name.as_str() {
        "read_active_buffer"
        | "get_diagnostics"
        | "list_editor_commands"
        | "read_file"
        | "list_directory" => false,
        "write_file" | "edit_file" => true,
        "run_command" | "shell_command" | "bash" | "terminal" | "exec" => true,
        "run_editor_command" => {
            let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
            match args.get("command").and_then(Value::as_str) {
                Some(cmd) => !APPROVAL_EXEMPT_COMMANDS.contains(&cmd),
                None => true,
            }
        }
        _ => true,
    }
}

/// Track observable side effects of an executed call for the session's file list.
pub fn record_side_effects(ed: &mut Editor, session_id: u64, call: &ToolCall) {
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
    match call.name.as_str() {
        "read_file" | "write_file" | "edit_file" => {
            if let Some(path) = args.get("path").and_then(Value::as_str) {
                ed.agent.record_file(session_id, path);
            }
        }
        "run_editor_command" => {
            let command = args.get("command").and_then(Value::as_str).unwrap_or("");
            if command == "open-file" {
                if let Some(path) = args
                    .get("args")
                    .and_then(|a| a.get("path"))
                    .and_then(Value::as_str)
                {
                    ed.agent.record_file(session_id, path);
                }
            }
        }
        _ => {}
    }
}

/// Human-readable one-liner for a tool call.
pub fn tool_summary(call: &ToolCall) -> String {
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
    match call.name.as_str() {
        "run_command" | "shell_command" | "bash" | "terminal" | "exec" => {
            let cmd = args
                .get("command")
                .or_else(|| args.get("cmd"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            format!("$ {cmd}")
        }
        "read_file" => {
            let p = args.get("path").and_then(Value::as_str).unwrap_or("?");
            format!("read {p}")
        }
        "write_file" => {
            let p = args.get("path").and_then(Value::as_str).unwrap_or("?");
            format!("write {p}")
        }
        "edit_file" => {
            let p = args.get("path").and_then(Value::as_str).unwrap_or("?");
            format!("edit {p}")
        }
        "list_directory" => {
            let p = args.get("path").and_then(Value::as_str).unwrap_or(".");
            format!("ls {p}")
        }
        "run_editor_command" => {
            let cmd = args.get("command").and_then(Value::as_str).unwrap_or("?");
            let rest = args.get("args").map(|a| a.to_string()).unwrap_or_default();
            if rest == "{}" || rest.is_empty() {
                cmd.to_string()
            } else {
                format!("{cmd} {rest}")
            }
        }
        _ => call.name.clone(),
    }
}

/// Run a shell command synchronously in the editor workspace root, capturing stdout and stderr.
pub fn run_shell_command_sync(ed: &Editor, cmd_str: &str) -> String {
    let root = ed
        .project_manager
        .project
        .root
        .as_deref()
        .unwrap_or(Path::new("."));

    #[cfg(target_os = "windows")]
    let res = std::process::Command::new("cmd")
        .args(["/C", cmd_str])
        .current_dir(root)
        .output();

    #[cfg(not(target_os = "windows"))]
    let res = std::process::Command::new("sh")
        .args(["-c", cmd_str])
        .current_dir(root)
        .output();

    match res {
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut result = format!("exit code: {code}\n");
            if !stdout.trim().is_empty() {
                result.push_str(&format!("stdout:\n{stdout}\n"));
            }
            if !stderr.trim().is_empty() {
                result.push_str(&format!("stderr:\n{stderr}\n"));
            }
            if stdout.trim().is_empty() && stderr.trim().is_empty() {
                result.push_str("(no output)\n");
            }
            llm::truncate_for_context(&result)
        }
        Err(e) => format!("error executing '{cmd_str}': {e}"),
    }
}

/// If a buffer for `path` is currently open in the editor, update its content.
pub fn reload_buffer_if_open(ed: &mut Editor, path_str: &str, new_content: &str) {
    for (_, arc) in ed.buffers.iter() {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        if buf.path.as_deref() == Some(path_str) {
            let len = buf.len();
            buf.replace(0, len, new_content);
        }
    }
}

/// Execute one tool call on the foreground.
pub fn execute_tool(ed: &mut Editor, call: &ToolCall) -> String {
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
    match call.name.as_str() {
        "run_command" | "shell_command" | "bash" | "terminal" | "exec" => {
            let cmd = args
                .get("command")
                .or_else(|| args.get("cmd"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if cmd.trim().is_empty() {
                return "error: run_command requires a non-empty command".to_string();
            }
            run_shell_command_sync(ed, cmd)
        }
        "read_file" => {
            let Some(path_str) = args.get("path").and_then(Value::as_str) else {
                return "error: read_file requires a path".to_string();
            };
            let resolved = crate::project::resolve_path(
                ed.project_manager.project.root.as_deref(),
                path_str,
            );
            match std::fs::read_to_string(&resolved) {
                Ok(content) => {
                    let start_line = args
                        .get("start_line")
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        .max(1) as usize;
                    let line_count = args
                        .get("line_count")
                        .and_then(Value::as_u64)
                        .map(|n| n as usize);
                    let lines: Vec<&str> = content.lines().collect();
                    let total = lines.len();
                    let end = match line_count {
                        Some(cnt) => (start_line + cnt - 1).min(total),
                        None => total,
                    };
                    let formatted = lines
                        .iter()
                        .enumerate()
                        .skip(start_line - 1)
                        .take(end.saturating_sub(start_line - 1))
                        .map(|(idx, line)| format!("{:4} | {}", idx + 1, line))
                        .collect::<Vec<_>>()
                        .join("\n");
                    llm::truncate_for_context(&format!(
                        "file: {path_str} (lines {start_line}..{end} of {total})\n{formatted}"
                    ))
                }
                Err(e) => format!("error reading '{path_str}': {e}"),
            }
        }
        "write_file" => {
            let Some(path_str) = args.get("path").and_then(Value::as_str) else {
                return "error: write_file requires a path".to_string();
            };
            let Some(content) = args.get("content").and_then(Value::as_str) else {
                return "error: write_file requires content".to_string();
            };
            let resolved = crate::project::resolve_path(
                ed.project_manager.project.root.as_deref(),
                path_str,
            );
            let path_obj = Path::new(&resolved);
            if let Some(parent) = path_obj.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(path_obj, content) {
                Ok(()) => {
                    reload_buffer_if_open(ed, &resolved, content);
                    format!("ok: wrote {} bytes to {path_str}", content.len())
                }
                Err(e) => format!("error writing '{path_str}': {e}"),
            }
        }
        "edit_file" => {
            let Some(path_str) = args.get("path").and_then(Value::as_str) else {
                return "error: edit_file requires a path".to_string();
            };
            let Some(target) = args.get("target").and_then(Value::as_str) else {
                return "error: edit_file requires a target string to replace".to_string();
            };
            let Some(replacement) = args.get("replacement").and_then(Value::as_str) else {
                return "error: edit_file requires replacement content".to_string();
            };
            let resolved = crate::project::resolve_path(
                ed.project_manager.project.root.as_deref(),
                path_str,
            );
            let path_obj = Path::new(&resolved);
            match std::fs::read_to_string(path_obj) {
                Ok(content) => {
                    if !content.contains(target) {
                        return format!("error: target string not found in '{path_str}'");
                    }
                    let occurrences = content.matches(target).count();
                    let updated = content.replacen(target, replacement, 1);
                    match std::fs::write(path_obj, &updated) {
                        Ok(()) => {
                            reload_buffer_if_open(ed, &resolved, &updated);
                            format!("ok: replaced target in '{path_str}' (target appeared {occurrences} time(s))")
                        }
                        Err(e) => format!("error saving edited file '{path_str}': {e}"),
                    }
                }
                Err(e) => format!("error reading '{path_str}': {e}"),
            }
        }
        "list_directory" => {
            let path_str = args
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(".");
            let resolved = crate::project::resolve_path(
                ed.project_manager.project.root.as_deref(),
                path_str,
            );
            match std::fs::read_dir(&resolved) {
                Ok(entries) => {
                    let mut items = Vec::new();
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        if is_dir {
                            items.push(format!("{name}/"));
                        } else {
                            items.push(name);
                        }
                    }
                    items.sort();
                    if items.is_empty() {
                        format!("directory {path_str} is empty")
                    } else {
                        items.join("\n")
                    }
                }
                Err(e) => format!("error listing directory '{path_str}': {e}"),
            }
        }
        "read_active_buffer" => {
            let key = ed.focused_buffer_key();
            ed.buffers
                .get(key)
                .map(|arc| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    let text = buf.slice(0, buf.len().min(65_536));
                    format!("buffer: {}\n{}", buf.name, llm::truncate_for_context(&text))
                })
                .unwrap_or_else(|| "no active buffer".to_string())
        }
        "get_diagnostics" => {
            let key = ed.focused_buffer_key();
            ed.buffers
                .get(key)
                .map(|arc| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    let diags: Vec<String> = buf.diagnostic_strings();
                    if diags.is_empty() {
                        "no diagnostics".to_string()
                    } else {
                        diags.join("\n")
                    }
                })
                .unwrap_or_else(|| "no active buffer".to_string())
        }
        "list_editor_commands" => {
            let mut lines: Vec<String> = ed
                .commands
                .list_detailed()
                .into_iter()
                .map(|(name, desc, _has_args)| format!("{name}: {desc}"))
                .collect();
            lines.sort();
            lines.join("\n")
        }
        "run_editor_command" => {
            let Some(command) = args.get("command").and_then(Value::as_str) else {
                return "error: run_editor_command requires a command".to_string();
            };
            if command == "shell-command" {
                let cmd = args
                    .get("args")
                    .and_then(|a| a.get("command"))
                    .or_else(|| args.get("command_str"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                return run_shell_command_sync(ed, cmd);
            }
            if ed.commands.get_entry(command).is_none() && !command.starts_with(':') {
                return run_shell_command_sync(ed, command);
            }
            let mut cmd_args: HashMap<String, ArgValue> = HashMap::new();
            if let Some(obj) = args.get("args").and_then(Value::as_object) {
                for (k, v) in obj {
                    let val = match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        other => other.to_string(),
                    };
                    cmd_args.insert(k.clone(), ArgValue::String(val));
                }
            }
            match execute_command(ed, command, &cmd_args) {
                Ok(()) => format!("ok: executed {command}"),
                Err(e) => format!("error: {e}"),
            }
        }
        other => format!("error: unknown tool {other}"),
    }
}
