//! Janet API functions for the command system — registered as `extern "C"` via evil-janet.

use std::collections::HashMap;
use evil_janet::*;
use super::conv;
use super::with_editor;

/// (command/run name & args) — execute a Rust command
unsafe extern "C-unwind" fn c_command_run(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = (unsafe { conv::get_str(argc, argv, 0) }) else {
            return conv::nil();
        };
        let args = HashMap::new();
        match crate::command::execute_command(ed, &name, &args) {
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

/// (command/define name fn &opt metadata) — register a Janet function as a command
///
/// Stores `fn` in the global `*janet-commands*` table and registers a Rust
/// command entry that retrieves and calls the function via `janet_call`.
unsafe extern "C-unwind" fn c_command_define(argc: i32, argv: *mut Janet) -> Janet {
    let name = match unsafe { conv::get_str(argc, argv, 0) } {
        Some(n) => n,
        None => conv::signal_err("command/define requires a command name"),
    };
    if argc < 2 {
        conv::signal_err("command/define requires a function value");
    }
    let fn_val = unsafe { *argv.add(1) };

    // Store in Janet's global *janet-commands* table
    unsafe {
        let env = janet_core_env(std::ptr::null_mut());

        // Resolve *janet-commands*; create it if missing
        let commands_sym = b"*janet-commands*\0";
        let sym = janet_symbol(commands_sym.as_ptr(), commands_sym.len() as i32 - 1); // exclude NUL
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

        let key = conv::string(&name);
        janet_table_put(commands_table, key, fn_val);
    }

    // Store the command name for the closure to use for table lookup.
    let lookup_name = name.clone();
    with_editor(|ed| {
        ed.commands.register_fn(&name, "Janet-defined command", vec![], move |ed, _args| {
            crate::janet_bridge::set_editor_ptr(ed as *mut crate::state::Editor);
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
                let mut out: Janet = std::mem::zeroed();
                janet_pcall(func, 0, std::ptr::null(), &mut out, std::ptr::null_mut());
            }
            Ok(())
        });
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
            documentation: c"Define a new command from a Janet function".as_ptr() as *const _,
        },
    ]
}
