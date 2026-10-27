pub mod diag_parse;
mod event;
pub mod scheduler;
pub use event::Event;
pub use scheduler::{OutputSink, Task, TaskId, TaskOutput, TaskScheduler, TaskStatus};

use crate::state::Editor;

pub use tool_diag::QuickfixEntry;

/// Upper bound on quickfix-list entries, applied wherever results stream in
/// (find-in-files, grep batches, keyword-context).
pub const QUICKFIX_MAX_RESULTS: usize = 500;

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
        command: cmd.to_string(),
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

/// Close out a finished run: exit-status line, quickfix parse, sink
/// removal.  Returns false when no sink existed (run-config launches
/// stream to the output pane instead) so the caller can parse those.
pub fn finish_output_task(ed: &mut Editor, id: u64, exit_code: i32, stdout: &str, stderr: &str) -> bool {
    let Some(sink) = ed.task_scheduler.output_sinks.remove(&id) else { return false };
    let status = format!("\n[exit {exit_code}]\n");
    if let Some(arc) = ed.buffers.get_mut(sink.buffer_key) {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let end = buf.len();
        buf.insert(end, &status);
    }
    if sink.parse_quickfix {
        let cwd = ed.task_scheduler.tasks.get(&id)
            .and_then(|t| t.cwd.clone())
            .or_else(|| ed.project_manager.project.root.clone())
            .unwrap_or_default();
        apply_tool_diagnostics(ed, &sink.command, &format!("{stdout}{stderr}"), &cwd);
    }
    true
}

/// Parse finished tool output into the quickfix list and typed diagnostics on
/// every open buffer.  One failed build becomes clickable errors and gutter
/// marks.  Only the tool-owned store is touched, so a clean build clears stale
/// build diagnostics without disturbing LSP-owned ones.
pub fn apply_tool_diagnostics(ed: &mut Editor, command: &str, output: &str, cwd: &std::path::Path) {
    let findings = diag_parse::parse_tool_output(command, output, cwd);
    ed.quickfix_list = findings.iter().map(|f| f.quickfix()).collect();
    ed.quickfix_index = 0;

    let mut by_path: std::collections::HashMap<String, Vec<crate::semantic::provider::Diagnostic>> =
        std::collections::HashMap::new();
    for f in &findings {
        by_path.entry(f.path.clone()).or_default().push(f.diag.clone());
    }
    for (_, arc) in ed.buffers.iter_mut() {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let diags = buf.path.as_deref()
            .and_then(|path| by_path.remove(path))
            .unwrap_or_default();
        buf.set_tool_diagnostics(diags);
    }
    ed.frame_dirty = true;
}

/// Which quickfix-style list a navigation command walks.  The GUI mirrors the
/// focused tool window into `Editor::quickfix_focus`; a command with no panel
/// in mind walks build problems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuickfixList {
    /// Build/compiler problems (the Output tool's PROBLEMS section).
    #[default]
    Problems,
    /// Find-in-files / grep hits (the Find in Files tool).
    Search,
}

impl QuickfixList {
    /// The entries and current cursor for this list.
    pub fn entries(self, ed: &Editor) -> (&[QuickfixEntry], usize) {
        match self {
            QuickfixList::Problems => (&ed.quickfix_list, ed.quickfix_index),
            QuickfixList::Search => (&ed.search.results, ed.search.index),
        }
    }

    fn set_index(self, ed: &mut Editor, index: usize) {
        match self {
            QuickfixList::Problems => ed.quickfix_index = index,
            QuickfixList::Search => ed.search.index = index,
        }
    }
}

/// Jump to the next entry in the focused list (`:cn`).
pub fn quickfix_next(editor: &mut Editor) -> Result<(), String> {
    navigate_quickfix(editor, 1)
}

/// Jump to the previous entry in the focused list (`:cp`).
pub fn quickfix_prev(editor: &mut Editor) -> Result<(), String> {
    navigate_quickfix(editor, -1)
}

fn navigate_quickfix(editor: &mut Editor, step: isize) -> Result<(), String> {
    let kind = editor.quickfix_focus;
    let (len, index) = {
        let (list, index) = kind.entries(editor);
        (list.len(), index)
    };
    if len == 0 {
        return Err("No quickfix entries".to_string());
    }
    let next = index as isize + step;
    if next < 0 {
        return Err("Already at first entry".to_string());
    }
    let next = next as usize;
    if next >= len {
        return Err("Already at last entry".to_string());
    }
    kind.set_index(editor, next);
    jump_to_quickfix(editor, next, kind);
    Ok(())
}

fn jump_to_quickfix(editor: &mut Editor, idx: usize, kind: QuickfixList) {
    let (list, qf) = match kind {
        QuickfixList::Problems => (&editor.quickfix_list, editor.quickfix_list[idx].clone()),
        QuickfixList::Search => (&editor.search.results, editor.search.results[idx].clone()),
    };
    let path = qf.filename.clone();

    // Only build problems seed the target file's tool diagnostics — a search
    // hit must not paint the file with build errors.
    let seed_tool_diags = kind == QuickfixList::Problems;
    let diags: Vec<crate::semantic::provider::Diagnostic> = if seed_tool_diags {
        list.iter()
            .filter(|e| e.filename == path)
            .map(|e| {
                let (line, col) = (e.line.saturating_sub(1), e.col.saturating_sub(1));
                crate::semantic::provider::Diagnostic {
                    message: e.message.clone(),
                    severity: crate::semantic::provider::DiagnosticSeverity::Error,
                    line, column: col, end_line: line, end_column: col,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // Try to find an existing buffer for this file
    let existing_key = editor.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(path.as_str()))
        .map(|(key, _)| key);

    if let Some(key) = existing_key {
        if let Some(arc) = editor.buffers.get_mut(key) {
            let (target, char_offset) = {
                let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                if seed_tool_diags { buf.set_tool_diagnostics(diags); }
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
            let mut buf = crate::text_engine::Buffer::from_string(
                crate::state::id::BufferId(id), &name, &content,
            );
            buf.path = Some(path.clone());
            if seed_tool_diags { buf.set_tool_diagnostics(diags); }
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
            let mut view = crate::text_engine::BufferView::new(arc);
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
    let buf = crate::text_engine::Buffer::from_string(
        crate::state::id::BufferId(id), &name, content,
    );
    let arc = std::sync::Arc::new(std::sync::Mutex::new(buf));
    let entry = editor.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(arc.clone());
    editor.views.insert(key, crate::text_engine::BufferView::new(arc));
    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    mod test_support {
        pub fn make_editor_with_buffer(text: &str) -> crate::state::Editor {
            let mut ed = crate::state::Editor::new(
                Box::new(crate::storage::disk::DiskFileSystem::new()));
            crate::command::builtin::register_builtin_commands(&mut ed);
            let key = ed.create_buffer_from_str("test", text);
            if let Some(win) = ed.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            ed
        }

        pub fn focused_key(ed: &crate::state::Editor) -> usize {
            ed.view_tree.focused_window()
                .and_then(|wid| ed.view_tree.buffer(wid))
                .unwrap_or(0)
        }
    }

    // The parser's own behavioral tests live in diag_parse.rs; these cover
    // the editor-level application of parsed output.

    fn ed_with_main() -> (crate::state::Editor, usize) {
        let mut ed = test_support::make_editor_with_buffer("fn main() {}\n");
        let key = test_support::focused_key(&ed);
        ed.buffers.get_mut(key).unwrap().lock().unwrap_or_else(|e| e.into_inner()).path =
            Some("/repo/src/main.rs".to_string());
        (ed, key)
    }

    #[test]
    fn apply_tool_diagnostics_fills_quickfix_and_buffer() {
        let (mut ed, key) = ed_with_main();
        apply_tool_diagnostics(&mut ed, "cargo build",
            "error[E0308]: mismatched types\n --> src/main.rs:4:9\n",
            std::path::Path::new("/repo"));

        assert_eq!(ed.quickfix_list.len(), 1);
        let qf = &ed.quickfix_list[0];
        assert_eq!(qf.filename, "/repo/src/main.rs");
        assert_eq!(qf.line, 4);
        assert_eq!(qf.col, 9);

        let buf = ed.buffers.get(key).unwrap().lock().unwrap_or_else(|e| e.into_inner());
        let ds = buf.get_diagnostics();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].message, "mismatched types");
        assert_eq!(ds[0].severity,
            crate::semantic::provider::DiagnosticSeverity::Error);
    }

    #[test]
    fn apply_tool_diagnostics_clean_run_clears_quickfix_and_tool_marks() {
        let (mut ed, key) = ed_with_main();
        apply_tool_diagnostics(&mut ed, "cargo build",
            "error: broken\n --> src/main.rs:4:9\n", std::path::Path::new("/repo"));
        assert_eq!(ed.quickfix_list.len(), 1);
        assert_eq!(ed.buffers.get(key).unwrap().lock().unwrap_or_else(|e| e.into_inner())
            .tool_diagnostics.len(), 1);

        apply_tool_diagnostics(&mut ed, "cargo build",
            "   Compiling repo v0.1.0\n    Finished dev profile", std::path::Path::new("/repo"));
        assert!(ed.quickfix_list.is_empty(), "a clean run clears the list");
        assert_eq!(ed.buffers.get(key).unwrap().lock().unwrap_or_else(|e| e.into_inner())
            .tool_diagnostics.len(), 0, "a clean run clears stale tool diagnostics");
    }

    #[test]
    fn clean_build_preserves_lsp_diagnostics() {
        use crate::semantic::provider::{Diagnostic, DiagnosticSeverity};
        let (mut ed, key) = ed_with_main();
        ed.buffers.get(key).unwrap().lock().unwrap_or_else(|e| e.into_inner())
            .set_lsp_diagnostics(vec![Diagnostic {
                message: "unused import".to_string(),
                severity: DiagnosticSeverity::Warning,
                line: 0, column: 0, end_line: 0, end_column: 1,
            }]);

        apply_tool_diagnostics(&mut ed, "cargo build",
            "    Finished dev profile", std::path::Path::new("/repo"));

        let buf = ed.buffers.get(key).unwrap().lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(buf.lsp_diagnostics.len(), 1,
            "a clean build must not clear LSP-owned diagnostics");
        assert!(buf.tool_diagnostics.is_empty());
    }

    #[test]
    fn generic_colon_formats_still_parse() {
        let d = diag_parse::parse_tool_output("make",
            "src/main.rs:42:10: unexpected token\nsrc/lib.rs:7: error: something",
            std::path::Path::new("/repo"));
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].quickfix().col, 10);
        assert_eq!(d[1].quickfix().col, 1, "file:line: message defaults to col 1");
        assert!(d[0].path.ends_with("src/main.rs"));
    }

    #[test]
    fn prose_lines_do_not_become_diagnostics() {
        let d = diag_parse::parse_tool_output("x",
            "Compilation finished successfully\n  hello world",
            std::path::Path::new("/repo"));
        assert!(d.is_empty());
    }
}