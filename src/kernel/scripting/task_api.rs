//! Janet API for the Task System (Phase 4).
//!
//! Registered C functions:
//!   (task/define name command &opt deps)   → task-id
//!   (task/run name-or-id)                  → task-id
//!   (task/cancel id)                       → nil
//!   (task/on-complete id callback)         → nil
//!   (task/status id)                       → keyword
//!   (task/list)                            → array of {:id :name :status :command}
//!   (task/output id)                       → {:stdout :stderr :exit-code :duration-ms} or nil
//!   (task/detect-project root)             → nil

use std::sync::{LazyLock, Mutex};
use std::collections::HashMap;

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

/// Send+Sync wrapper for Janet function references.
#[derive(Clone, Copy)]
struct JanetSend(Janet);
unsafe impl Send for JanetSend {}
unsafe impl Sync for JanetSend {}

/// Stored on-complete callbacks keyed by task ID.
static TASK_COMPLETE_CALLBACKS: LazyLock<Mutex<HashMap<u64, JanetSend>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Drop all stored callbacks — called at the start of `janet_bridge::init()`.
pub fn reset_task_complete_callbacks() {
    if let Ok(mut cbs) = TASK_COMPLETE_CALLBACKS.lock() {
        for JanetSend(fn_val) in cbs.values() {
            unsafe { janet_gcunroot(*fn_val); }
        }
        cbs.clear();
    }
}

// ── C functions ──────────────────────────────────────────────────────────────

/// (task/define name command &opt deps) → task-id
unsafe extern "C-unwind" fn c_task_define(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 2 {
            conv::signal_err("task/define requires name and command");
        }
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("task/define: name must be a string"),
        };
        let command = conv::get_str(argc, argv, 1).unwrap_or_default();

        let mut deps: Vec<u64> = Vec::new();
        if argc >= 3 {
            let deps_val = *argv.add(2);
            if janet_checktype(deps_val, JanetType_JANET_ARRAY) != 0 {
                let arr = janet_unwrap_array(deps_val);
                let count = (*arr).count as usize;
                for i in 0..count {
                    let v = *(*arr).data.add(i);
                    if janet_checktype(v, JanetType_JANET_NUMBER) != 0 {
                        deps.push(janet_unwrap_number(v) as u64);
                    }
                }
            }
        }

        let id = ed.task_scheduler.define(name, command, HashMap::new(), deps);
        conv::integer(id as i32)
    })
}

/// (task/run name-or-id) → task-id
unsafe extern "C-unwind" fn c_task_run(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            conv::signal_err("task/run requires a task name or id");
        }
        let bg = match ed.background.clone() {
            Some(bg) => bg,
            None => conv::signal_err("task/run: no background runtime available"),
        };

        let first = *argv.add(0);
        let result = if janet_checktype(first, JanetType_JANET_STRING) != 0
            || janet_checktype(first, JanetType_JANET_KEYWORD) != 0
        {
            let name = conv::get_str(argc, argv, 0).unwrap_or_default();
            ed.task_scheduler.run_by_name(&name, &bg).map(|id| id)
        } else if janet_checktype(first, JanetType_JANET_NUMBER) != 0 {
            let id = janet_unwrap_number(first) as u64;
            ed.task_scheduler.run_id(id, &bg).map(|_| id)
        } else {
            conv::signal_err("task/run: argument must be a task name (string) or id (number)");
        };

        let id = match result {
            Ok(id) => id,
            Err(e) => conv::signal_err(&e),
        };

        let task_name = ed.task_scheduler.tasks.get(&id)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        ed.events.emit_typed(keys::events::TASK_STARTED, TaskStartedPayload {
            id: id.to_string(),
            name: task_name,
        });

        conv::integer(id as i32)
    })
}

/// (task/cancel id) → nil
///
/// Unified cancel: handles both TaskScheduler tasks (new system) and
/// legacy `task/spawn` tasks stored in `io.tasks`.
unsafe extern "C-unwind" fn c_task_cancel(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        // Cancel new-system task (no-op if id not found)
        ed.task_scheduler.cancel(id);
        // Cancel legacy task/spawn task
        ed.io.tasks.remove(&id);
        // Clean up on-complete callback (new system)
        if let Some(JanetSend(fn_val)) = TASK_COMPLETE_CALLBACKS.lock().unwrap().remove(&id) {
            janet_gcunroot(fn_val);
        }
        // Clean up task/spawn stored function (legacy)
        super::process_api::remove_task_function(id);
        conv::nil()
    })
}

/// (task/on-complete id callback) → nil
///
/// Registers `callback` to be called on the main thread when task `id`
/// completes (success or failure).  The callback receives one argument:
/// `{:stdout :stderr :exit-code :duration-ms}`.
unsafe extern "C-unwind" fn c_task_on_complete(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|_ed| unsafe {
        if argc < 2 {
            conv::signal_err("task/on-complete requires id and callback");
        }
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        let fn_val = *argv.add(1);
        if janet_checktype(fn_val, JanetType_JANET_FUNCTION) == 0 {
            conv::signal_err("task/on-complete: second argument must be a function");
        }
        janet_gcroot(fn_val);
        TASK_COMPLETE_CALLBACKS.lock().unwrap().insert(id, JanetSend(fn_val));
        conv::nil()
    })
}

/// (task/status id) → keyword
unsafe extern "C-unwind" fn c_task_status(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        let kw = match ed.task_scheduler.status(id) {
            None => "not-found",
            Some(crate::kernel::task::TaskStatus::Pending)    => "pending",
            Some(crate::kernel::task::TaskStatus::Running)    => "running",
            Some(crate::kernel::task::TaskStatus::Completed)  => "completed",
            Some(crate::kernel::task::TaskStatus::Failed(_))  => "failed",
            Some(crate::kernel::task::TaskStatus::Cancelled)  => "cancelled",
        };
        conv::keyword(kw)
    })
}

/// (task/list) → array of {:id :name :status :command}
unsafe extern "C-unwind" fn c_task_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let count = ed.task_scheduler.tasks.len();
        let arr = janet_wrap_array(janet_array(count as i32));
        for task in ed.task_scheduler.tasks.values() {
            let status_kw = match &task.status {
                crate::kernel::task::TaskStatus::Pending   => "pending",
                crate::kernel::task::TaskStatus::Running   => "running",
                crate::kernel::task::TaskStatus::Completed => "completed",
                crate::kernel::task::TaskStatus::Failed(_) => "failed",
                crate::kernel::task::TaskStatus::Cancelled => "cancelled",
            };
            let tbl = janet_wrap_table(janet_table(4));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("id"),      conv::integer(task.id as i32));
            janet_table_put(t, conv::keyword("name"),    conv::string(&task.name));
            janet_table_put(t, conv::keyword("status"),  conv::keyword(status_kw));
            janet_table_put(t, conv::keyword("command"), conv::string(&task.command));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

/// (task/output id) → {:stdout :stderr :exit-code :duration-ms} or nil
unsafe extern "C-unwind" fn c_task_output(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        match ed.task_scheduler.output(id) {
            None => conv::nil(),
            Some(out) => {
                let tbl = janet_wrap_table(janet_table(4));
                let t = janet_unwrap_table(tbl);
                janet_table_put(t, conv::keyword("stdout"),      conv::string(&out.stdout));
                janet_table_put(t, conv::keyword("stderr"),      conv::string(&out.stderr));
                janet_table_put(t, conv::keyword("exit-code"),   conv::integer(out.exit_code));
                janet_table_put(t, conv::keyword("duration-ms"), conv::integer(out.duration_ms as i32));
                tbl
            }
        }
    })
}

/// (task/detect-project root) → nil
///
/// Auto-detects project type from marker files in `root` (Cargo.toml, package.json, …),
/// defines `build`/`test`/`run` tasks, and populates `project.language` and
/// `project.build_targets` via the Project Model layer (Phase 5).
unsafe extern "C-unwind" fn c_task_detect_project(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let root = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        if !root.is_empty() {
            ed.project_manager.detect(&root, &mut ed.task_scheduler);
        }
        conv::nil()
    })
}

// ── Main-thread callback execution ───────────────────────────────────────────

/// Called from `JanetRuntime::execute_task_complete_callback` when a task finishes.
/// Fires the stored callback with the task output as argument.
pub(crate) fn execute_task_complete_callback(
    ed: &mut crate::kernel::state::Editor,
    task_id: u64,
) {
    let fn_val = {
        let mut cbs = TASK_COMPLETE_CALLBACKS.lock().unwrap();
        cbs.remove(&task_id)
    };
    let Some(JanetSend(fn_ref)) = fn_val else { return };

    unsafe {
        janet_gcunroot(fn_ref);
        let arg = match ed.task_scheduler.output(task_id) {
            Some(out) => {
                let tbl = janet_wrap_table(janet_table(4));
                let t = janet_unwrap_table(tbl);
                janet_table_put(t, conv::keyword("stdout"),      conv::string(&out.stdout));
                janet_table_put(t, conv::keyword("stderr"),      conv::string(&out.stderr));
                janet_table_put(t, conv::keyword("exit-code"),   conv::integer(out.exit_code));
                janet_table_put(t, conv::keyword("duration-ms"), conv::integer(out.duration_ms as i32));
                tbl
            }
            None => conv::nil(),
        };
        let func = janet_unwrap_function(fn_ref);
        let args = [arg];
        let fiber = janet_fiber(func, 64, 1, args.as_ptr());
        if fiber.is_null() { return; }
        let mut out: Janet = std::mem::zeroed();
        janet_continue(fiber, janet_wrap_nil(), &mut out);
    }
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"task/define".as_ptr() as *const _,
            cfun: Some(c_task_define as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Define a named task with a shell command and optional dependency ids".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/run".as_ptr() as *const _,
            cfun: Some(c_task_run as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Run a task by name (string) or id (number); emits task-started event; returns task id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/cancel".as_ptr() as *const _,
            cfun: Some(c_task_cancel as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Cancel a pending or running task by id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/on-complete".as_ptr() as *const _,
            cfun: Some(c_task_on_complete as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a callback fired on the main thread when a task completes; receives {:stdout :stderr :exit-code :duration-ms}".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/status".as_ptr() as *const _,
            cfun: Some(c_task_status as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a keyword describing the task status: :pending :running :completed :failed :cancelled :not-found".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/list".as_ptr() as *const _,
            cfun: Some(c_task_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return an array of {:id :name :status :command} for all defined tasks".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/output".as_ptr() as *const _,
            cfun: Some(c_task_output as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return {:stdout :stderr :exit-code :duration-ms} for a completed task, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/detect-project".as_ptr() as *const _,
            cfun: Some(c_task_detect_project as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Auto-define build/test/run tasks from project markers at the given root path".as_ptr() as *const _,
        },
    ]
}
