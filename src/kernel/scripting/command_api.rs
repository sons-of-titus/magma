//! Janet API functions for the command system — registered as `extern "C"` via evil-janet.

use std::collections::HashMap;
use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::command::args::ArgValue;

/// (command/run name &opt kvs*) — execute a command with `:key value` pairs.
unsafe extern "C-unwind" fn c_command_run(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        let mut args = HashMap::new();
        let mut i = 1;
        while i + 1 < argc {
            let (Some(key), Some(value)) = (conv::get_str(argc, argv, i), conv::get_str(argc, argv, i + 1)) else {
                break;
            };
            args.insert(key, ArgValue::String(value));
            i += 2;
        }
        match crate::kernel::command::execute_command(ed, &name, &args) {
            Ok(()) => conv::nil(),
            Err(e) => conv::signal_err(&e),
        }
    })
}

/// (command/list) → [name ...]
unsafe extern "C-unwind" fn c_command_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let names = ed.commands.list();
        let arr = janet_wrap_array(janet_array(names.len() as i32));
        for name in names {
            janet_array_push(janet_unwrap_array(arr), conv::string(name));
        }
        arr
    })
}

/// (command/exists? name) → bool
unsafe extern "C-unwind" fn c_command_exists(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::boolean(false);
        };
        conv::boolean(ed.commands.exists(&name))
    })
}

fn store_janet_command(name: &str, fn_val: Janet) {
    unsafe {
        let env = janet_core_env(std::ptr::null_mut());
        let commands_sym = b"*janet-commands*\0";
        let sym = janet_symbol(commands_sym.as_ptr(), commands_sym.len() as i32 - 1);
        let mut resolved: Janet = std::mem::zeroed();
        let bt = janet_resolve(env, sym, &mut resolved);

        let commands_table = if bt == JanetBindingType_JANET_BINDING_DEF || bt == JanetBindingType_JANET_BINDING_VAR {
            janet_unwrap_table(resolved)
        } else {
            let mut result: Janet = std::mem::zeroed();
            janet_dostring(
                env,
                c"(def *janet-commands* (table/new 32))".as_ptr(),
                c"init".as_ptr(),
                &mut result,
            );
            let mut resolved2: Janet = std::mem::zeroed();
            janet_resolve(env, sym, &mut resolved2);
            janet_unwrap_table(resolved2)
        };

        let key = conv::string(name);
        janet_table_put(commands_table, key, fn_val);
    }
}

/// Call a stored Janet command function with the argument count its arity
/// accepts: a function that can take an argument receives the raw command
/// argument (empty when none was supplied); a zero-arity function is called
/// with none.  A signaling call is returned as `Err` — never swallowed.
pub(crate) fn invoke_janet_command(
    func: *mut JanetFunction,
    raw: Option<&str>,
) -> Result<(), String> {
    unsafe {
        // A command may declare zero arity (`(fn [] …)`) or one parameter
        // (`(fn [arg] …)`, `(fn [&] …)`); dispatch must match, or the call
        // fails at the arity check before any body runs.
        let max_arity = (*(*func).def).max_arity;
        let mut out: Janet = std::mem::zeroed();
        let sig = if max_arity >= 1 {
            let cargs = [conv::string(raw.unwrap_or(""))];
            janet_pcall(func, 1, cargs.as_ptr(), &mut out, std::ptr::null_mut())
        } else {
            janet_pcall(func, 0, std::ptr::null(), &mut out, std::ptr::null_mut())
        };
        if sig != JanetSignal_JANET_SIGNAL_OK {
            return Err("Janet command failed".to_string());
        }
    }
    Ok(())
}

fn register_janet_command_wrapper(ed: &mut crate::kernel::state::Editor, name: &str) {
    let lookup_name = name.to_string();
    // Commands run long after registration: re-enter the context that
    // defined them so capability gates hold at invocation time.
    let registration_ctx = super::extension_api::current_context();
    ed.commands.register(name, "Janet-defined command", vec![], Box::new(move |ed, args| {
        // The registry hands us a live exclusive borrow; the Janet call runs
        // inside its scope (docs/core-redesign.md §11).
        crate::kernel::scripting::scoped_editor(ed, || {
            super::extension_api::with_context(registration_ctx.clone(), || {
            unsafe {
                let env = janet_core_env(std::ptr::null_mut());
                let mut root: Janet = std::mem::zeroed();
                let sym_bytes = b"*janet-commands*";
                let cmd_sym = janet_symbol(sym_bytes.as_ptr(), sym_bytes.len() as i32);
                if janet_resolve(env, cmd_sym, &mut root) == JanetBindingType_JANET_BINDING_NONE {
                    return Ok(());
                }
                let table = janet_unwrap_table(root);
                let key = janet_string(lookup_name.as_ptr() as *const u8, lookup_name.len() as i32);
                let func_val = janet_table_get(table, janet_wrap_string(key));
                if janet_checktype(func_val, JanetType_JANET_FUNCTION) == 0 {
                    return Ok(());
                }
                let func = janet_unwrap_function(func_val);
                // execute_command_line delivers the raw remainder under "arg".
                let raw = args.get("arg").and_then(|v| v.as_string().map(|s| s.to_string()));
                invoke_janet_command(func, raw.as_deref())
            }
            })
        })
    }));
}

/// (command/define name fn &opt metadata) — register a Janet function as a command
///
/// Stores `fn` in the global `*janet-commands*` table and registers a Rust
/// command entry that retrieves and calls the function via `janet_call`.
///
/// If a Rust command with the same name already exists, emits a warning and
/// refuses to overwrite.  Use `command/redefine` to force-replace.
unsafe extern "C-unwind" fn c_command_define(argc: i32, argv: *mut Janet) -> Janet {
    let name = match unsafe { conv::get_str(argc, argv, 0) } {
        Some(n) => n,
        None => conv::signal_err("command/define requires a command name"),
    };
    if argc < 2 {
        conv::signal_err("command/define requires a function value");
    }
    let fn_val = unsafe { *argv.add(1) };

    // Check for name conflict with Rust-registered commands.
    with_editor(|ed| {
        if ed.commands.exists(&name) {
            let msg = format!("command/define: name conflict — '{name}' is already registered in the Rust command table. Use command/redefine to force-replace.");
            return conv::string(&msg);  // Return warning string instead of signalling
        }
        store_janet_command(&name, fn_val);
        register_janet_command_wrapper(ed, &name);
        conv::nil()
    })
}

/// (command/redefine name fn &opt metadata) — define a command, overwriting any existing entry
///
/// Like `command/define` but silently overwrites any existing Rust command
/// with the same name.  Use with care.
unsafe extern "C-unwind" fn c_command_redefine(argc: i32, argv: *mut Janet) -> Janet {
    let name = match unsafe { conv::get_str(argc, argv, 0) } {
        Some(n) => n,
        None => conv::signal_err("command/redefine requires a command name"),
    };
    if argc < 2 {
        conv::signal_err("command/redefine requires a function value");
    }
    let fn_val = unsafe { *argv.add(1) };

    store_janet_command(&name, fn_val);
    with_editor(|ed| {
        register_janet_command_wrapper(ed, &name);
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"command/run".as_ptr() as *const _,
            cfun: Some(c_command_run as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Execute a command by name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"command/list".as_ptr() as *const _,
            cfun: Some(c_command_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List registered command names".as_ptr() as *const _,
        },
        JanetReg {
            name: c"command/exists?".as_ptr() as *const _,
            cfun: Some(c_command_exists as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Check if a command exists".as_ptr() as *const _,
        },
        JanetReg {
            name: c"command/define".as_ptr() as *const _,
            cfun: Some(c_command_define as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Define a new command from a Janet function; refuses to overwrite Rust commands".as_ptr() as *const _,
        },
        JanetReg {
            name: c"command/redefine".as_ptr() as *const _,
            cfun: Some(c_command_redefine as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Define or redefine a command from a Janet function; overwrites existing Rust commands".as_ptr() as *const _,
        },
    ]
}
