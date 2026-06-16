//! Janet API functions for process and async task management (Sprint 3).
//! Registered as `extern "C-unwind"` functions via evil-janet.

use std::sync::{LazyLock, Mutex};

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::event::payload::*;
use crate::event::keys;
use std::collections::HashMap;

/// Send+Sync wrapper for Janet function references stored for background tasks.
#[derive(Clone, Copy)]
struct JanetSend(Janet);
unsafe impl Send for JanetSend {}
unsafe impl Sync for JanetSend {}

/// GC-rooted function references for pending background tasks.
static TASK_FUNCTIONS: LazyLock<Mutex<HashMap<u64, JanetSend>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// (process/spawn cmd &opt args cwd) → process-id
///
/// Spawn a subprocess running `cmd` with the given space-separated `args`.
/// The optional `cwd` string sets the working directory for the child process.
/// Returns an opaque integer process handle.  The process runs asynchronously
/// via the tokio runtime.  stdout and stderr lines are streamed as
/// `process-output` events; process exit emits `process-exit`.
unsafe extern "C-unwind" fn c_process_spawn(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let cmd = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("process/spawn requires a command string"),
        };
        let args_str = conv::get_str(argc, argv, 1).unwrap_or_default();
        let cwd_opt = conv::get_str(argc, argv, 2);

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("no background runtime available"),
        };

        let mut command = std::process::Command::new("sh");
        command.arg("-c");
        let full_cmd = if args_str.is_empty() {
            cmd.clone()
        } else {
            format!("{} {}", cmd, args_str)
        };
        command.arg(&full_cmd);
        if let Some(ref cwd) = cwd_opt {
            command.current_dir(cwd);
        }
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        command.stdin(std::process::Stdio::piped());

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => conv::signal_err(&format!("process/spawn exec error: {e}")),
        };

        let pid = child.id();
        let id = ed.io.next_process_id;
        ed.io.next_process_id += 1;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Spawn stdout reader on the blocking thread pool
        let sender_out = bg.sender.clone();
        let sender_err = bg.sender.clone();
        let sender_exit = bg.sender.clone();
        let exit_cmd = full_cmd.clone();
        let exit_id = id;

        bg.spawn_blocking(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let _ = sender_out.send(crate::runtime::BackgroundEvent::ProcessOutput {
                    id, line, stream: "stdout".to_string(),
                });
            }
        });

        bg.spawn_blocking(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = sender_err.send(crate::runtime::BackgroundEvent::ProcessOutput {
                    id: exit_id, line, stream: "stderr".to_string(),
                });
            }
        });

        // Spawn a wait task to detect process exit
        bg.spawn_blocking(move || {
            let exit_code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            let _ = sender_exit.send(crate::runtime::BackgroundEvent::ProcessExited {
                id: exit_id,
                exit_code,
                cmd: exit_cmd,
            });
        });

        ed.io.processes.insert(id, crate::runtime::ProcessState {
            cmd: full_cmd,
            running: true,
            pid,
            stdin,
        });

        conv::integer(id as i32)
    })
}

/// (process/kill id)
///
/// Send SIGTERM to the process behind the handle.  The reader tasks will
/// detect EOF and the `process-exit` event will fire when the process dies.
unsafe extern "C-unwind" fn c_process_kill(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        if let Some(state) = ed.io.processes.get_mut(&id) {
            state.running = false;
            // Send SIGTERM via the kill command
            let _ = std::process::Command::new("kill")
                .arg(state.pid.to_string())
                .spawn();
        }
        conv::nil()
    })
}

/// (process/stdin id text)
///
/// Write `text` to the process's stdin.  The process must have been spawned
/// by `process/spawn` and still be running.
unsafe extern "C-unwind" fn c_process_stdin(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        let text = conv::get_str(argc, argv, 1).unwrap_or_default();
        if let Some(state) = ed.io.processes.get_mut(&id)
            && let Some(stdin) = state.stdin.as_mut() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
                let _ = stdin.flush();
            }
        conv::nil()
    })
}

/// (process/list) → [{:id n :cmd "…" :running bool} …]
///
/// Return a Janet array of tables describing all active processes.
unsafe extern "C-unwind" fn c_process_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_wrap_array(janet_array(ed.io.processes.len() as i32));
        for (id, state) in &ed.io.processes {
            let tbl = janet_wrap_table(janet_table(3));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("id"), conv::integer(*id as i32));
            janet_table_put(t, conv::keyword("cmd"), conv::string(&state.cmd));
            janet_table_put(t, conv::keyword("running"), conv::boolean(state.running));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

/// Execute a stored Janet function on the main thread.
///
/// Called from `process_background_event` when a `TaskReady` event fires.
pub(crate) fn execute_stored_task(ed: &mut crate::state::Editor, task_id: u64) {
    let fn_val = {
        let mut tasks = TASK_FUNCTIONS.lock().unwrap();
        tasks.remove(&task_id)
    };
    if let Some(JanetSend(fn_ref)) = fn_val {
        unsafe {
            janet_gcunroot(fn_ref);
            let func = janet_unwrap_function(fn_ref);
            let args = [janet_wrap_nil()];
            let fiber = janet_fiber(func, 64, 1, args.as_ptr());
            if fiber.is_null() {
                ed.events.emit_typed(keys::events::TASK_RESULT, TaskResultPayload {
                    id: task_id.to_string(),
                    value: String::new(),
                    error: "fiber creation failed".to_string(),
                });
                return;
            }
            let mut out: Janet = std::mem::zeroed();
            let sig = janet_continue(fiber, janet_wrap_nil(), &mut out);
            if sig == JanetSignal_JANET_SIGNAL_OK {
                let result = if janet_checktype(out, JanetType_JANET_STRING) != 0 {
                    let ptr = janet_unwrap_string(out);
                    if ptr.is_null() {
                        "nil".to_string()
                    } else {
                        std::ffi::CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned()
                    }
                } else if janet_checktype(out, JanetType_JANET_NUMBER) != 0 {
                    format!("{}", janet_unwrap_number(out))
                } else if janet_checktype(out, JanetType_JANET_BOOLEAN) != 0 {
                    format!("{}", janet_unwrap_boolean(out) != 0)
                } else {
                    "ok".to_string()
                };
                ed.events.emit_typed(keys::events::TASK_RESULT, TaskResultPayload {
                    id: task_id.to_string(),
                    value: result,
                    error: String::new(),
                });
            } else {
                ed.events.emit_typed(keys::events::TASK_RESULT, TaskResultPayload {
                    id: task_id.to_string(),
                    value: String::new(),
                    error: format!("signal {}", sig),
                });
            }
        }
    }
}

/// (task/spawn janet-fn) → task-id
///
/// Run a zero-arg Janet function on the tokio thread pool.  The function is
/// called on the main thread when the background task signals readiness.
/// The result (or error) is delivered via the `task-result` event with
/// `{:id n :value "…" :error "…"}`.
unsafe extern "C-unwind" fn c_task_spawn(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            conv::signal_err("task/spawn requires a function argument");
        }
        let fn_val = *argv.add(0);
        if janet_checktype(fn_val, JanetType_JANET_FUNCTION) == 0 {
            conv::signal_err("task/spawn requires a function");
        }

        let id = ed.io.next_task_id;
        ed.io.next_task_id += 1;

        // GC-root the function
        janet_gcroot(fn_val);
        TASK_FUNCTIONS.lock().unwrap().insert(id, JanetSend(fn_val));

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => {
                TASK_FUNCTIONS.lock().unwrap().remove(&id);
                janet_gcunroot(fn_val);
                conv::signal_err("no background runtime available");
            }
        };

        // Spawn a minimal tokio task that yields once and signals back
        let sender = bg.sender.clone();
        bg.spawn(async move {
            tokio::task::yield_now().await;
            let _ = sender.send(crate::runtime::BackgroundEvent::TaskReady { id });
        });

        ed.io.tasks.insert(id, crate::runtime::TaskState {
            running: true,
        });

        conv::integer(id as i32)
    })
}

/// (task/cancel id)
///
/// Abort a background task.  The `task-result` event will not fire for the
/// cancelled task.
unsafe extern "C-unwind" fn c_task_cancel(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        ed.io.tasks.remove(&id);
        // Remove the stored function so task-result won't fire
        if let Some(JanetSend(fn_val)) = TASK_FUNCTIONS.lock().unwrap().remove(&id) {
            janet_gcunroot(fn_val);
        }
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"process/spawn".as_ptr() as *const _,
            cfun: Some(c_process_spawn as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Spawn an async subprocess with optional args and cwd; streams output via events".as_ptr() as *const _,
        },
        JanetReg {
            name: c"process/kill".as_ptr() as *const _,
            cfun: Some(c_process_kill as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send SIGTERM to a managed subprocess".as_ptr() as *const _,
        },
        JanetReg {
            name: c"process/stdin".as_ptr() as *const _,
            cfun: Some(c_process_stdin as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Write text to a running subprocess's stdin".as_ptr() as *const _,
        },
        JanetReg {
            name: c"process/list".as_ptr() as *const _,
            cfun: Some(c_process_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List all active subprocesses".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/spawn".as_ptr() as *const _,
            cfun: Some(c_task_spawn as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Run a Janet function as a background task".as_ptr() as *const _,
        },
        JanetReg {
            name: c"task/cancel".as_ptr() as *const _,
            cfun: Some(c_task_cancel as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Cancel a background task".as_ptr() as *const _,
        },
    ]
}
