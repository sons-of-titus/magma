pub mod scheduler;
pub use scheduler::{OutputSink, Task, TaskId, TaskOutput, TaskScheduler, TaskStatus};

use crate::kernel::state::Editor;

/// A parsed quickfix entry from compiler error output.
#[derive(Debug, Clone)]
pub struct QuickfixEntry {
    pub filename: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

/// Launch a shell command into the `*shell*` buffer, streaming its output
/// through the TaskScheduler.  Returns immediately; the buffer fills as the
/// command runs and ends with an exit-status line.
pub fn run_shell_command(editor: &mut Editor, cmd: &str) -> Result<usize, String> {
    launch_output_task(editor, &format!("# {}", cmd), cmd, "*shell*", false)
}

/// Launch `make` into the `*make*` buffer; on completion the output is
/// parsed into the quickfix list.
pub fn run_make(editor: &mut Editor, target: &str) -> Result<usize, String> {
    let cmd_str = if target.is_empty() {
        "make".to_string()
    } else {
        format!("make {}", target)
    };
    launch_output_task(editor, &format!("# {}", cmd_str), &cmd_str, "*make*", true)
}

/// Create the output buffer and launch the command as a background task.
fn launch_output_task(
    editor: &mut Editor,
    display: &str,
    cmd: &str,
    base_name: &str,
    parse_quickfix: bool,
) -> Result<usize, String> {
    let Some(bg) = editor.background.clone() else {
        return Err("no background runtime".to_string());
    };
    let buffer_key = create_output_buffer(editor, base_name, "", display);
    // Output buffers are ephemeral: they are regenerated on every run and
    // must never trip the quit guard or enter project sessions.
    if let Some(arc) = editor.buffers.get_mut(buffer_key) {
        arc.lock().unwrap_or_else(|e| e.into_inner()).ephemeral = true;
    }
    // Unique per run so concurrent commands never collide on a task name.
    let n = editor.task_scheduler.output_sinks.len() + editor.task_scheduler.next_id as usize;
    let name = format!("{base_name} #{}", n + 1);
    let id = editor.task_scheduler.define(name, cmd.to_string(), Default::default(), vec![]);
    editor.task_scheduler.output_sinks.insert(id, OutputSink {
        buffer_key,
        parse_quickfix,
    });
    editor.task_scheduler.launch(id, &bg)?;
    Ok(buffer_key)
}

/// Append a streamed line to a task's output buffer (foreground thread).
pub fn append_output_line(ed: &mut Editor, sink: &OutputSink, line: &str) {
    if let Some(arc) = ed.buffers.get_mut(sink.buffer_key) {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let end = buf.len();
        buf.insert(end, line);
        let end = buf.len();
        buf.insert(end, "\n");
    }
}

/// Close out a finished run: exit-status line, quickfix parse, sink removal.
pub fn finish_output_task(ed: &mut Editor, id: u64, exit_code: i32, stdout: &str, stderr: &str) {
    let Some(sink) = ed.task_scheduler.output_sinks.remove(&id) else { return };
    let status = format!("\n[exit {exit_code}]\n");
    if let Some(arc) = ed.buffers.get_mut(sink.buffer_key) {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let end = buf.len();
        buf.insert(end, &status);
    }
    if sink.parse_quickfix {
        let entries = parse_quickfix(&format!("{stdout}{stderr}"));
        ed.quickfix_list = entries;
        ed.quickfix_index = 0;
    }
}

/// Jump to the next quickfix entry.
pub fn quickfix_next(editor: &mut Editor) -> Result<(), String> {
    if editor.quickfix_list.is_empty() {
        return Err("No quickfix entries".to_string());
    }
    if editor.quickfix_index + 1 >= editor.quickfix_list.len() {
        return Err("Already at last entry".to_string());
    }
    editor.quickfix_index += 1;
    jump_to_quickfix(editor, editor.quickfix_index);
    Ok(())
}

/// Jump to the previous quickfix entry.
pub fn quickfix_prev(editor: &mut Editor) -> Result<(), String> {
    if editor.quickfix_list.is_empty() {
        return Err("No quickfix entries".to_string());
    }
    if editor.quickfix_index == 0 {
        return Err("Already at first entry".to_string());
    }
    editor.quickfix_index -= 1;
    jump_to_quickfix(editor, editor.quickfix_index);
    Ok(())
}

fn jump_to_quickfix(editor: &mut Editor, idx: usize) {
    let qf = &editor.quickfix_list[idx].clone();
    let path = qf.filename.clone();

    // Collect all diagnostics for this file from the quickfix list
    let diags: Vec<crate::kernel::semantic::provider::Diagnostic> =
        editor.quickfix_list.iter()
        .filter(|e| e.filename == path)
        .map(|e| {
            let (line, col) = (e.line.saturating_sub(1), e.col.saturating_sub(1));
            crate::kernel::semantic::provider::Diagnostic {
                message: e.message.clone(),
                severity: crate::kernel::semantic::provider::DiagnosticSeverity::Error,
                line, column: col, end_line: line, end_column: col,
            }
        })
        .collect();

    // Try to find an existing buffer for this file
    let existing_key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(path.as_str()))
        .map(|(key, _)| key);

    if let Some(key) = existing_key {
        if let Some(arc) = editor.buffers.get_mut(key) {
            let (target, char_offset) = {
                let mut buf = arc.lock().unwrap();
                buf.set_diagnostics(diags);
                let target = buf.line_start_offset(qf.line.saturating_sub(1)).unwrap_or(0);
                let line_text = buf.line(qf.line.saturating_sub(1)).unwrap_or_default();
                let char_offset: usize = line_text.chars()
                    .take(qf.col.saturating_sub(1))
                    .map(|c| c.len_utf8())
                    .sum();
                (target, char_offset)
            };
            if let Some(view) = editor.view_of_buffer_mut(key) {
                view.set_cursor(target + char_offset);
            }
        }
        if let Some(win) = editor.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
    } else {
        // Open the file and jump to the position
        if let Ok(content) = editor.fs.read(&path) {
            let name = std::path::Path::new(&path)
                .file_name().map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let id = editor.allocate_buffer_id();
            let mut buf = crate::kernel::text_engine::Buffer::from_string(
                crate::kernel::state::id::BufferId(id), &name, &content,
            );
            buf.path = Some(path.clone());
            buf.set_diagnostics(diags);
            let target = buf.line_start_offset(qf.line.saturating_sub(1)).unwrap_or(0);
            let line_text = buf.line(qf.line.saturating_sub(1)).unwrap_or_default();
            let char_offset: usize = line_text.chars()
                .take(qf.col.saturating_sub(1))
                .map(|c| c.len_utf8())
                .sum();
            let arc = std::sync::Arc::new(std::sync::Mutex::new(buf));
            let slab_entry = editor.buffers.vacant_entry();
            let key = slab_entry.key();
            slab_entry.insert(arc.clone());
            let mut view = crate::kernel::text_engine::BufferView::new(arc);
            view.set_cursor(target + char_offset);
            editor.views.insert(key, view);
            if let Some(win) = editor.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
        }
    }
}

/// Create a new buffer with output content and attach to focused window.
fn create_output_buffer(editor: &mut Editor, base_name: &str, content: &str, cmd: &str) -> usize {
    let name = format!("{}({})", base_name, cmd);
    let id = editor.allocate_buffer_id();
    let buf = crate::kernel::text_engine::Buffer::from_string(
        crate::kernel::state::id::BufferId(id), &name, content,
    );
    let arc = std::sync::Arc::new(std::sync::Mutex::new(buf));
    let entry = editor.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(arc.clone());
    editor.views.insert(key, crate::kernel::text_engine::BufferView::new(arc));
    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    key
}

/// Parse common compiler/linter error formats:
///   file:line:col: message
///   file:line: message
///   file(line,col): message
fn parse_quickfix(output: &str) -> Vec<QuickfixEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        if let Some(entry) = parse_quickfix_line(line) {
            entries.push(entry);
        }
    }
    entries
}

fn parse_quickfix_line(line: &str) -> Option<QuickfixEntry> {
    // Pattern 1: file:line:col: message
    // Pattern 2: file:line: message
    if let Some(rest) = line.strip_prefix('/') {
        // Absolute path starting with /
        let full_path = format!("/{}", rest);
        return parse_colon_line(&full_path, line);
    }
    parse_colon_line(line, line)
}

fn parse_colon_line(path_part: &str, _original: &str) -> Option<QuickfixEntry> {
    // Match: path:line:col: message  or  path:line: message
    let parts: Vec<&str> = path_part.splitn(4, ':').collect();
    if parts.len() >= 3 {
        let filename = parts[0].trim().to_string();
        let line: usize = parts[1].parse().ok()?;
        // Check if parts[2] looks like a column number (digits only)
        if let Ok(col) = parts[2].parse::<usize>() {
            let message = if parts.len() > 3 { parts[3].trim().to_string() } else { String::new() };
            if !message.is_empty() {
                return Some(QuickfixEntry { filename, line, col, message });
            }
        } else {
            // It's file:line: message format (no column)
            let message = parts[2..].join(":").trim().to_string();
            if !message.is_empty() {
                return Some(QuickfixEntry { filename, line, col: 1, message });
            }
        }
    }

    // Pattern: file(line,col): message
    if let Some(rest) = path_part.find('(') {
        let filename = path_part[..rest].trim().to_string();
        let rest = &path_part[rest + 1..];
        if let Some(close) = rest.find(')') {
            let loc = &rest[..close];
            if let Some((line_str, col_str)) = loc.split_once(',')
                && let (Ok(line), Ok(col)) = (line_str.trim().parse::<usize>(), col_str.trim().parse::<usize>()) {
                    let msg_start = rest[close + 1..].trim();
                    // Skip leading colon
                    let message = msg_start.strip_prefix(':').unwrap_or(msg_start).trim().to_string();
                    if !message.is_empty() {
                        return Some(QuickfixEntry { filename, line, col, message });
                    }
                }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_colon_format() {
        let e = parse_quickfix_line("src/main.rs:42:10: unexpected token").unwrap();
        assert_eq!(e.filename, "src/main.rs");
        assert_eq!(e.line, 42);
        assert_eq!(e.col, 10);
        assert_eq!(e.message, "unexpected token");
    }

    #[test]
    fn test_parse_no_col() {
        let e = parse_quickfix_line("src/main.rs:42: error: something").unwrap();
        assert_eq!(e.filename, "src/main.rs");
        assert_eq!(e.line, 42);
        assert_eq!(e.col, 1);
        assert!(e.message.contains("error: something"));
    }

    #[test]
    fn test_parse_paren_format() {
        let e = parse_quickfix_line("src/main.rs(42,10): expected `;`").unwrap();
        assert_eq!(e.filename, "src/main.rs");
        assert_eq!(e.line, 42);
        assert_eq!(e.col, 10);
        assert_eq!(e.message, "expected `;`");
    }

    #[test]
    fn test_parse_absolute_path() {
        let e = parse_quickfix_line("/home/user/proj/src/main.rs:42:10: error").unwrap();
        assert_eq!(e.filename, "/home/user/proj/src/main.rs");
    }

    #[test]
    fn test_non_matching_line() {
        assert!(parse_quickfix_line("Compilation finished successfully").is_none());
        assert!(parse_quickfix_line("  hello world").is_none());
    }
}
