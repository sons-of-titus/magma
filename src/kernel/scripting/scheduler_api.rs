//! Janet API for the Work Scheduler (Phase 9 — Concurrency Model).
//!
//! Registered C functions:
//!   (scheduler/submit name cmd)   → work-id
//!   (scheduler/cancel id)         → nil
//!   (scheduler/progress id)       → {:done N :total M} or nil
//!   (scheduler/status id)         → keyword
//!   (scheduler/list)              → array of {:id :name :status :done :total}

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (scheduler/submit name cmd) → work-id
///
/// Submit a named shell command as a tracked work item.  The command runs on
/// the blocking thread pool; completion arrives as `scheduler-work-completed`
/// and failure as `scheduler-work-failed`.  Returns a stable work-id.
unsafe extern "C-unwind" fn c_scheduler_submit(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 2 {
            conv::signal_err("scheduler/submit requires name and cmd");
        }
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("scheduler/submit: name must be a string"),
        };
        let cmd = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("scheduler/submit: cmd must be a string"),
        };
        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("scheduler/submit: no background runtime available"),
        };
        let work_id = ed.scheduler.submit_fn(name, &bg, move |progress, cancel| {
            use std::io::Read;
            progress.report(0, 1);
            if cancel.is_cancelled() {
                return Err("cancelled".into());
            }
            let mut command = std::process::Command::new("sh");
            command.arg("-c").arg(&cmd);
            command.stdout(std::process::Stdio::piped());
            command.stderr(std::process::Stdio::piped());
            match command.spawn() {
                Ok(mut child) => {
                    let mut out = String::new();
                    let mut err_str = String::new();
                    if let Some(s) = child.stdout.take() {
                        let _ = std::io::BufReader::new(s).read_to_string(&mut out);
                    }
                    if let Some(s) = child.stderr.take() {
                        let _ = std::io::BufReader::new(s).read_to_string(&mut err_str);
                    }
                    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                    progress.report(1, 1);
                    if code == 0 {
                        Ok(())
                    } else {
                        Err(format!("exit code {code}: {err_str}"))
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        });
        conv::integer(work_id as i32)
    })
}

/// (scheduler/cancel id) → nil
///
/// Cancel a running work item.  Directly-submitted work checks the
/// cancellation token in its loop; externally-tracked work (tasks, processes)
/// is not forcibly interrupted — only its status is updated.
unsafe extern "C-unwind" fn c_scheduler_cancel(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(-1) as u64;
        ed.scheduler.cancel(id);
        conv::nil()
    })
}

/// (scheduler/progress id) → {:done N :total M} or nil
///
/// Return the progress of work item `id`, or nil if the id is not found.
unsafe extern "C-unwind" fn c_scheduler_progress(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        match ed.scheduler.progress(id) {
            None => conv::nil(),
            Some(p) => {
                let tbl = janet_wrap_table(janet_table(2));
                let t = janet_unwrap_table(tbl);
                janet_table_put(t, conv::keyword("done"),  conv::integer(p.done as i32));
                janet_table_put(t, conv::keyword("total"), conv::integer(p.total as i32));
                tbl
            }
        }
    })
}

/// (scheduler/status id) → keyword
///
/// Return a keyword describing the status of work item `id`:
/// `:running` `:completed` `:failed` `:cancelled` `:not-found`
unsafe extern "C-unwind" fn c_scheduler_status(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(-1) as u64;
        use crate::kernel::scheduler::WorkStatus;
        let kw = match ed.scheduler.status(id) {
            None                              => "not-found",
            Some(WorkStatus::Running)         => "running",
            Some(WorkStatus::Completed)       => "completed",
            Some(WorkStatus::Failed(_))       => "failed",
            Some(WorkStatus::Cancelled)       => "cancelled",
        };
        conv::keyword(kw)
    })
}

/// (scheduler/list) → array of {:id :name :status :done :total}
///
/// Return all tracked work items.
unsafe extern "C-unwind" fn c_scheduler_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        use crate::kernel::scheduler::WorkStatus;
        let items = ed.scheduler.list();
        let arr = janet_wrap_array(janet_array(items.len() as i32));
        for (id, name, status, progress) in items {
            let status_kw = match status {
                WorkStatus::Running    => "running",
                WorkStatus::Completed  => "completed",
                WorkStatus::Failed(_)  => "failed",
                WorkStatus::Cancelled  => "cancelled",
            };
            let tbl = janet_wrap_table(janet_table(5));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("id"),     conv::integer(id as i32));
            janet_table_put(t, conv::keyword("name"),   conv::string(name));
            janet_table_put(t, conv::keyword("status"), conv::keyword(status_kw));
            janet_table_put(t, conv::keyword("done"),   conv::integer(progress.done as i32));
            janet_table_put(t, conv::keyword("total"),  conv::integer(progress.total as i32));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"scheduler/submit".as_ptr() as *const _,
            cfun: Some(c_scheduler_submit as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Submit a shell command as a tracked work item; returns work-id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"scheduler/cancel".as_ptr() as *const _,
            cfun: Some(c_scheduler_cancel as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Cancel a work item by work-id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"scheduler/progress".as_ptr() as *const _,
            cfun: Some(c_scheduler_progress as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return {:done N :total M} for a work item, or nil if not found".as_ptr() as *const _,
        },
        JanetReg {
            name: c"scheduler/status".as_ptr() as *const _,
            cfun: Some(c_scheduler_status as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a keyword status for a work item: :running :completed :failed :cancelled :not-found".as_ptr() as *const _,
        },
        JanetReg {
            name: c"scheduler/list".as_ptr() as *const _,
            cfun: Some(c_scheduler_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return an array of {:id :name :status :done :total} for all tracked work items".as_ptr() as *const _,
        },
    ]
}
