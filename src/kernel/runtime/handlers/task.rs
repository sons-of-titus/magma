//! Task/index/search background-event handlers — task completion with tool
//! diagnostics, grep batches, scheduler work, completion word scans, the
//! symbol index, and tree-sitter parse readiness.

use crate::kernel::state::Editor;
use crate::kernel::event::keys;
use crate::kernel::event::payload::*;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::FileIndexed { project_name, files } => {
            // Update the project state in the registry with the file list
            if let Some(proj) = ed.project_manager.projects.get_mut(&project_name) {
                proj.files = files.clone();
                proj.files_epoch += 1;
                proj.file_index_dirty = false;
            }
            if ed.project_manager.current_project.as_deref() == Some(&project_name) {
                ed.project_manager.project.files = files.clone();
                ed.project_manager.project.files_epoch += 1;
                ed.project_manager.project.file_index_dirty = false;
                // Phase 5: wire indexed files into the Semantic Engine's ProjectGraph
                ed.project_manager.sync_to_project_graph(&mut ed.semantic.project_graph);
                crate::kernel::index::reconcile_project_index(ed, &files);
            }
            ed.events.emit_typed(keys::events::FILE_INDEXED, FileIndexedPayload {
                project_name,
                count: files.len().to_string(),
            });
        }
        BackgroundEvent::GrepResultsBatch { generation, entries } => {
            if generation == ed.search_generation {
                ed.quickfix_list.extend(entries);
                ed.quickfix_list.truncate(crate::kernel::task::QUICKFIX_MAX_RESULTS);
            }
        }
        BackgroundEvent::GrepDone { generation } => {
            if generation == ed.search_generation {
                ed.quickfix_index = 0;
            }
        }
        BackgroundEvent::WorkProgress { id, done, total } => {
            ed.scheduler.update_progress(id, done, total);
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.events.emit_typed(keys::events::SCHEDULER_WORK_PROGRESS, SchedulerWorkProgressPayload {
                id: id.to_string(), name, done: done.to_string(), total: total.to_string(),
            });
        }
        BackgroundEvent::WorkCompleted { id } => {
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.scheduler.mark_completed(id);
            ed.events.emit_typed(keys::events::SCHEDULER_WORK_COMPLETED, SchedulerWorkCompletedPayload {
                id: id.to_string(), name,
            });
        }
        BackgroundEvent::WorkFailed { id, error } => {
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.scheduler.mark_failed(id, error.clone());
            ed.events.emit_typed(keys::events::SCHEDULER_WORK_FAILED, SchedulerWorkFailedPayload {
                id: id.to_string(), name, error,
            });
        }
        BackgroundEvent::BufferWords { buf_key, revision, words } => {
            let current = ed.buffers.get(buf_key).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision());
            if current == Some(revision) {
                ed.completion.word_cache.insert(buf_key, crate::kernel::state::completion::WordCache {
                    revision,
                    words: std::sync::Arc::new(words.into_iter().collect()),
                });
            }
            // Clear the in-flight marker only for the request that landed —
            // a newer revision's scan must keep its slot.
            if ed.completion.words_requested == Some((buf_key, revision)) {
                ed.completion.words_requested = None;
            }
        }
        BackgroundEvent::IndexRecordArrived(rec) => {
            ed.index.apply(rec);
        }
        BackgroundEvent::TsTreeReady { buf_key, revision } => {
            crate::kernel::semantic::ts_schedule::take_ready_tree(ed, buf_key, revision);
        }
        BackgroundEvent::TaskOutputLine { id, stream, line } => {
            // Output-buffer tasks stream straight into their buffer so long
            // builds show progress as they run.
            if let Some(sink) = ed.task_scheduler.output_sinks.get(&id).cloned() {
                crate::kernel::task::append_output_line(ed, &sink, &line);
            }
            let mut data = std::collections::HashMap::new();
            data.insert("task-id".to_string(), id.to_string());
            data.insert("stream".to_string(), stream);
            data.insert("line".to_string(), line);
            ed.events.emit("task-output-line", data);
        }
        BackgroundEvent::TaskRunCompleted { id, exit_code, stdout, stderr, duration_ms } => {
            handle_task_run_completed(ed, id, exit_code, stdout, stderr, duration_ms);
        }
        _ => {}
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
    use crate::kernel::task::TaskOutput;
    let output = TaskOutput { stdout, stderr, exit_code, duration_ms };
    let handled = crate::kernel::task::finish_output_task(
        ed, id, exit_code, &output.stdout, &output.stderr);
    let (task_cmd, task_cwd) = ed.task_scheduler.tasks.get(&id)
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
        crate::kernel::task::apply_tool_diagnostics(
            ed, &task_cmd, &format!("{}{}", output.stdout, output.stderr), &cwd);
    }
    let task_name = ed.task_scheduler.tasks.get(&id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    ed.task_scheduler.running.remove(&id);
    ed.last_task = Some((task_name.clone(), format!(
        "$ {}\n{}{}",
        ed.task_scheduler.tasks.get(&id).map(|t| t.command.clone()).unwrap_or_default(),
        output.stdout,
        output.stderr,
    )));
    if exit_code == 0 {
        ed.task_scheduler.mark_completed(id, output);
        ed.scheduler.complete_task(id);
        ed.events.emit_typed(keys::events::TASK_COMPLETED, TaskCompletedPayload {
            id: id.to_string(),
            name: task_name,
            exit_code: exit_code.to_string(),
            duration_ms: duration_ms.to_string(),
        });
    } else {
        let err = format!("exit code {exit_code}");
        ed.task_scheduler.mark_failed(id, err.clone(), output);
        ed.scheduler.fail_task(id, err.clone());
        ed.events.emit_typed(keys::events::TASK_FAILED, TaskFailedPayload {
            id: id.to_string(),
            name: task_name,
            error: err,
        });
    }
    crate::kernel::run_config::continue_pending(ed, id, exit_code);
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
