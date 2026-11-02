//! Task background-event handlers — task completion with tool diagnostics,
//! scheduler work, completion word scans, and tree-sitter parse readiness.

use crate::event::keys;
use crate::event::payload::*;
use crate::state::Editor;

use super::super::BackgroundEvent;

/// The Output pane keeps the tail of the task stream: a 100 MB build log
/// must not become a per-keystroke allocation in the frontend.  Tool
/// diagnostics parse the full output before this cap is applied.
const LAST_TASK_MAX_BYTES: usize = 512 * 1024;

fn set_last_task(ed: &mut Editor, name: String, mut output: String) {
    clamp_output_tail(&mut output);
    ed.last_task = Some((name, output));
    ed.last_task_generation += 1;
}

fn push_last_task_stream(ed: &mut Editor, task_name: &str, line: &str) {
    let line = format!("{line}\n");
    match &mut ed.last_task {
        Some((name, output)) if name == task_name => output.push_str(&line),
        _ => ed.last_task = Some((task_name.to_string(), line)),
    }
    if let Some((_, output)) = &mut ed.last_task {
        clamp_output_tail(output);
    }
    ed.last_task_generation += 1;
}

fn clamp_output_tail(output: &mut String) {
    if output.len() <= LAST_TASK_MAX_BYTES {
        return;
    }
    let mut cut = output.len() - LAST_TASK_MAX_BYTES;
    while !output.is_char_boundary(cut) {
        cut += 1;
    }
    // Keep whole lines: the pane renders line rows, so a half line at the
    // top only ever shows as a fragment.
    if let Some(nl) = output[cut..].find('\n') {
        cut += nl + 1;
    }
    output.drain(0..cut);
}

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::WorkProgress { id, done, total } => {
            ed.scheduler.update_progress(id, done, total);
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.events.emit_typed(
                keys::events::SCHEDULER_WORK_PROGRESS,
                SchedulerWorkProgressPayload {
                    id: id.to_string(),
                    name,
                    done: done.to_string(),
                    total: total.to_string(),
                },
            );
        }
        BackgroundEvent::WorkCompleted { id } => {
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.scheduler.mark_completed(id);
            ed.events.emit_typed(
                keys::events::SCHEDULER_WORK_COMPLETED,
                SchedulerWorkCompletedPayload {
                    id: id.to_string(),
                    name,
                },
            );
        }
        BackgroundEvent::WorkFailed { id, error } => {
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.scheduler.mark_failed(id, error.clone());
            ed.events.emit_typed(
                keys::events::SCHEDULER_WORK_FAILED,
                SchedulerWorkFailedPayload {
                    id: id.to_string(),
                    name,
                    error,
                },
            );
        }
        BackgroundEvent::BufferWords {
            buf_key,
            revision,
            words,
        } => {
            let current = ed
                .buffers
                .get(buf_key)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision());
            if current == Some(revision) {
                ed.completion.word_cache.insert(
                    buf_key,
                    crate::state::completion::WordCache {
                        revision,
                        words: std::sync::Arc::new(words.into_iter().collect()),
                    },
                );
            }
            // Clear the in-flight marker only for the request that landed —
            // a newer revision's scan must keep its slot.
            if ed.completion.words_requested == Some((buf_key, revision)) {
                ed.completion.words_requested = None;
            }
        }
        BackgroundEvent::TsTreeReady { buf_key, revision } => {
            crate::semantic::ts_schedule::take_ready_tree(ed, buf_key, revision);
        }
        BackgroundEvent::Task(event) => handle_task_event(ed, event),
        _ => {}
    }
}

fn handle_task_event(ed: &mut Editor, event: crate::task::Event) {
    use crate::task::Event;
    match event {
        Event::OutputLine { id, stream, line } => {
            // Output-buffer tasks stream straight into their buffer so long
            // builds show progress as they run.
            if let Some(sink) = ed.task_scheduler.output_sinks.get(&id).cloned() {
                crate::task::append_output_line(ed, &sink, &line);
            } else {
                // Run-config launches stream into the output pane, so a long
                // run shows progress as it runs instead of only at completion.
                let task_name = ed
                    .task_scheduler
                    .tasks
                    .get(&id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();
                push_last_task_stream(ed, &task_name, &line);
            }
            let mut data = std::collections::HashMap::new();
            data.insert("task-id".to_string(), id.to_string());
            data.insert("stream".to_string(), stream);
            data.insert("line".to_string(), line.trim_end().to_string());
            ed.events.emit("task-output-line", data);
        }
        Event::RunCompleted {
            id,
            exit_code,
            stdout,
            stderr,
            duration_ms,
        } => {
            handle_task_run_completed(ed, id, exit_code, stdout, stderr, duration_ms);
        }
    }
}

/// Task completion: parse tool output into diagnostics, update the
/// scheduler state, fire completion callbacks, and launch dependents.
fn handle_task_run_completed(
    ed: &mut Editor,
    id: u64,
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
) {
    use crate::task::TaskOutput;
    let output = TaskOutput {
        stdout,
        stderr,
        exit_code,
        duration_ms,
    };
    let handled =
        crate::task::finish_output_task(ed, id, exit_code, &output.stdout, &output.stderr);
    let (task_cmd, task_cwd) = ed
        .task_scheduler
        .tasks
        .get(&id)
        .map(|t| (t.command.clone(), t.cwd.clone()))
        .unwrap_or_default();
    // Run-config launches stream to the output pane, not a sink
    // buffer: parse their tool output here so build/test results —
    // errors, warnings, or a clean run clearing the list — always
    // reach the editor.
    if !handled {
        let cwd = task_cwd
            .or_else(|| ed.project_manager.project.root.clone())
            .unwrap_or_default();
        crate::task::apply_tool_diagnostics(
            ed,
            &task_cmd,
            &format!("{}{}", output.stdout, output.stderr),
            &cwd,
        );
    }
    let task_name = ed
        .task_scheduler
        .tasks
        .get(&id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let combined = format!(
        "$ {}\n{}{}",
        ed.task_scheduler
            .tasks
            .get(&id)
            .map(|t| t.command.clone())
            .unwrap_or_default(),
        output.stdout,
        output.stderr,
    );
    set_last_task(ed, task_name.clone(), combined);
    ed.task_scheduler.running.remove(&id);
    if exit_code == 0 {
        ed.task_scheduler.mark_completed(id, output);
        ed.scheduler.complete_task(id);
        ed.events.emit_typed(
            keys::events::TASK_COMPLETED,
            TaskCompletedPayload {
                id: id.to_string(),
                name: task_name,
                exit_code: exit_code.to_string(),
                duration_ms: duration_ms.to_string(),
            },
        );
    } else {
        let err = format!("exit code {exit_code}");
        ed.task_scheduler.mark_failed(id, err.clone(), output);
        ed.scheduler.fail_task(id, err.clone());
        ed.events.emit_typed(
            keys::events::TASK_FAILED,
            TaskFailedPayload {
                id: id.to_string(),
                name: task_name,
                error: err,
            },
        );
    }
    crate::run_config::continue_pending(ed, id, exit_code);
    let mut rt = ed.runtime.take();
    if let Some(rt) = &mut rt {
        rt.execute_task_complete_callback(ed, id);
    }
    ed.runtime = rt;
    if let Some(bg) = ed.background.clone() {
        let ready: Vec<u64> = ed.task_scheduler.ready_to_run();
        for rid in ready {
            let _ = ed.task_scheduler.launch(rid, &bg);
        }
    }
}
