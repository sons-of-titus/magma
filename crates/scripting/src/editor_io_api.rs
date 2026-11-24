//! Janet API — editor I/O: filesystem, shell, load-file, run-command.

use super::conv;
use super::with_editor;
use evil_janet::*;

/// Stringify a command-line argument: strings and keywords pass through,
/// numbers render in decimal form — Janet callers pass buffer ids as
/// numbers, and `get_str` silently drops them.
unsafe fn arg_to_string(argc: i32, argv: *mut Janet, i: i32) -> Option<String> {
    if i >= argc {
        return None;
    }
    if let Some(s) = unsafe { conv::get_str(argc, argv, i) } {
        return Some(s);
    }
    let v = unsafe { *argv.add(i as usize) };
    if unsafe { janet_checktype(v, JanetType_JANET_NUMBER) } != 0 {
        return Some(format!("{}", unsafe { janet_unwrap_number(v) }));
    }
    None
}

unsafe extern "C-unwind" fn c_editor_fs_write(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(path) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/fs-write requires a path")
        };
        let Some(content) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("editor/fs-write requires content")
        };
        if let Some(bg) = &ed.background {
            let path = path.clone();
            let content = content.clone();
            let result: Result<(), String> = bg.block_on(move || {
                (|| -> Result<(), String> {
                    if let Some(parent) = std::path::Path::new(&path).parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(&path, &content).map_err(|e| e.to_string())?;
                    Ok(())
                })()
            });
            result.unwrap_or_else(|e| conv::signal_err(&e));
            conv::nil()
        } else {
            ed.fs
                .write(&path, &content)
                .unwrap_or_else(|e| conv::signal_err(&e.to_string()));
            conv::nil()
        }
    })
}

unsafe extern "C-unwind" fn c_editor_fs_read(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(path) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/fs-read requires a path")
        };
        if let Some(bg) = &ed.background {
            let path = path.clone();
            let content = bg.block_on(move || std::fs::read_to_string(&path));
            match content {
                Ok(text) => conv::string(&text),
                Err(e) => conv::signal_err(&e.to_string()),
            }
        } else {
            match ed.fs.read(&path) {
                Ok(content) => conv::string(&content),
                Err(e) => conv::signal_err(&e.to_string()),
            }
        }
    })
}

unsafe extern "C-unwind" fn c_editor_shell(argc: i32, argv: *mut Janet) -> Janet {
    unsafe {
        with_editor(|_ed| {
            let Some(cmd) = conv::get_str(argc, argv, 0) else {
                conv::signal_err("editor/shell requires a command string")
            };
            match kernel::util::exec::shell_command(&cmd).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let result = if stderr.is_empty() {
                        stdout.into_owned()
                    } else if stdout.is_empty() {
                        stderr.into_owned()
                    } else {
                        format!("{}\n--- stderr ---\n{}", stdout, stderr)
                    };
                    conv::string(&result)
                }
                Err(e) => conv::signal_err(&format!("editor/shell exec error: {e}")),
            }
        })
    }
}

unsafe extern "C-unwind" fn c_editor_run_command(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(n) => n,
            None => conv::signal_err("editor/run-command requires a command name"),
        };
        // The command line is positional: every extra argument joins the
        // line so multi-argument commands get all their values (dropping
        // the tail here silently lost `buffer` ids and the like).
        let rest = (1..argc)
            .filter_map(|i| arg_to_string(argc, argv, i))
            .collect::<Vec<_>>()
            .join(" ");
        let line = if rest.is_empty() {
            name.clone()
        } else {
            format!("{name} {rest}")
        };
        match kernel::command::execute_command_line(ed, &line) {
            Ok(()) => conv::nil(),
            Err(e) => conv::signal_err(&e),
        }
    })
}

unsafe extern "C-unwind" fn c_editor_fs_exists(argc: i32, argv: *mut Janet) -> Janet {
    let path = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
    conv::boolean(std::path::Path::new(&path).exists())
}

unsafe extern "C-unwind" fn c_editor_load_file(argc: i32, argv: *mut Janet) -> Janet {
    unsafe {
        with_editor(|_ed| {
            let Some(path) = conv::get_str(argc, argv, 0) else {
                conv::signal_err("editor/load-file requires a path")
            };
            match crate::load_file(&path) {
                Ok(()) => conv::nil(),
                Err(e) => conv::signal_err(&e),
            }
        })
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/fs-write".as_ptr() as *const _,
            cfun: Some(c_editor_fs_write as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Write content string to a file path".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/fs-read".as_ptr() as *const _,
            cfun: Some(c_editor_fs_read as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Read file content as string from a path".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/shell".as_ptr() as *const _,
            cfun: Some(c_editor_shell as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Run a shell command and return combined stdout+stderr as a string"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/run-command".as_ptr() as *const _,
            cfun: Some(
                c_editor_run_command as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Run a named command with positional string arguments".as_ptr()
                as *const _,
        },
        JanetReg {
            name: c"editor/fs-exists?".as_ptr() as *const _,
            cfun: Some(c_editor_fs_exists as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return true if a path exists on the filesystem".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/load-file".as_ptr() as *const _,
            cfun: Some(c_editor_load_file as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Evaluate a Janet file from disk".as_ptr() as *const _,
        },
    ]
}
