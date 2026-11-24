//! Janet API — toolchain discovery and plugin-registered toolchains.
//!
//! Registered C functions:
//!   (toolchain/find name) → path string or nil
//!   (toolchain/find-extended name) → path string or nil
//!   (toolchain/register spec) → :ok
//!   (toolchain/registered)    → array of tables

use super::conv;
use super::jtable;
use super::with_editor;
use evil_janet::*;
use kernel::toolchains::{ToolchainAction, ToolchainInfo, ToolchainInstall};

/// (toolchain/find name) → path or nil
/// Resolve an executable against $PATH so scripts can give a clear
/// "not installed" answer instead of a spawn failure.
unsafe extern "C-unwind" fn c_toolchain_find(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|_ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match kernel::project::toolchain::find(&name) {
            Some(path) => conv::string(&path.to_string_lossy()),
            None => conv::nil(),
        }
    })
}

/// (toolchain/find-extended name) → path or nil
/// Resolve an executable against `$PATH`, then the well-known install roots
/// that packaging hides from it (keg-only Homebrew, MacPorts, VS Code
/// extensions; macOS adds `xcrun --find`) — so a server installed outside
/// `$PATH` is found instead of reported missing.
unsafe extern "C-unwind" fn c_toolchain_find_extended(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|_ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match kernel::project::toolchain::find_extended(&name) {
            Some(path) => conv::string(&path.to_string_lossy()),
            None => conv::nil(),
        }
    })
}

/// `(toolchain/register spec)` → `:ok`
///
/// Register a toolchain from a spec table:
/// `{:id "rust.cargo" :language "rust" :detect ["Cargo.toml"]
///   :actions {"build" {:program "cargo" :args ["build"] :debuggable false}
///             "run"    {:program "cargo" :args ["run"] :debuggable true}}}`
///
/// Marker files recognize the toolchain's project type; actions become run
/// configurations when a project is detected (saved configs win).  Actions
/// use structured program/argument lists — no shell strings.
unsafe extern "C-unwind" fn c_toolchain_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            conv::signal_err("toolchain/register: spec table required");
        }
        let spec = *argv.add(0);
        if !jtable::is_spec(spec) {
            conv::signal_err("toolchain/register: spec must be a table or struct");
        };
        let Some(id) = jtable::get_str(spec, "id") else {
            conv::signal_err("toolchain/register: :id is required");
        };
        let Some(language) = jtable::get_str(spec, "language") else {
            conv::signal_err("toolchain/register: :language is required");
        };
        let Some(detect) = jtable::get_str_array(spec, "detect") else {
            conv::signal_err("toolchain/register: :detect (marker files) is required");
        };
        let Some(actions_spec) = jtable::get_spec(spec, "actions") else {
            conv::signal_err("toolchain/register: :actions table is required");
        };
        let mut actions = std::collections::BTreeMap::new();
        for (name, action_value) in jtable::string_spec_entries(actions_spec) {
            if !jtable::is_spec(action_value) {
                conv::signal_err(&format!(
                    "toolchain/register: action {name:?} has an invalid spec"
                ));
            }
            let Some(program) = jtable::get_str(action_value, "program") else {
                conv::signal_err(&format!(
                    "toolchain/register: action {name:?} needs :program"
                ));
            };
            let env = env_table(action_value).unwrap_or_default();
            actions.insert(
                name,
                ToolchainAction {
                    program,
                    args: jtable::get_str_array(action_value, "args").unwrap_or_default(),
                    cwd: jtable::get_str(action_value, "cwd"),
                    env,
                    debuggable: jtable::get_bool(action_value, "debuggable").unwrap_or(false),
                },
            );
        }
        let debug_adapter =
            jtable::get_spec(spec, "debug").and_then(|d| jtable::get_str(d, "adapter"));
        let install = jtable::get_spec(spec, "install").and_then(|i| {
            Some(ToolchainInstall {
                url: jtable::get_str(i, "url")?,
                sha256: jtable::get_str(i, "sha256")?,
                format: jtable::get_str(i, "format")?,
                bin: jtable::get_str(i, "bin")?,
            })
        });
        let info = ToolchainInfo {
            id,
            language,
            detect_markers: detect,
            actions,
            debug_adapter,
            install,
            owner: jtable::get_str(spec, "plugin").unwrap_or_default(),
        };
        match ed.toolchains.register(info) {
            Ok(()) => conv::keyword("ok"),
            Err(e) => conv::signal_err(&e),
        }
    })
}

/// `spec[:env]` as string→string pairs; None when absent or not a spec.
unsafe fn env_table(spec: Janet) -> Option<std::collections::HashMap<String, String>> {
    unsafe {
        let env_spec = jtable::get_spec(spec, "env")?;
        let mut out = std::collections::HashMap::new();
        for (k, v) in jtable::string_spec_entries(env_spec) {
            let mut val_argv = v;
            if let Some(val) = conv::get_str(1, &mut val_argv, 0) {
                out.insert(k, val);
            }
        }
        Some(out)
    }
}

/// `(toolchain/registered)` → array of
/// `{:id :language :detect :actions :plugin}` tables
unsafe extern "C-unwind" fn c_toolchain_registered(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let list = ed.toolchains.list();
        let arr = janet_wrap_array(janet_array(list.len().max(1) as i32));
        for t in list {
            let tbl = janet_wrap_table(janet_table(6));
            let t_tbl = janet_unwrap_table(tbl);
            janet_table_put(t_tbl, conv::keyword("id"), conv::string(&t.id));
            janet_table_put(t_tbl, conv::keyword("language"), conv::string(&t.language));
            janet_table_put(
                t_tbl,
                conv::keyword("debug-adapter"),
                t.debug_adapter
                    .as_deref()
                    .map(conv::string)
                    .unwrap_or_else(conv::nil),
            );
            let markers = janet_array(t.detect_markers.len().max(1) as i32);
            for m in &t.detect_markers {
                janet_array_push(markers, conv::string(m));
            }
            janet_table_put(t_tbl, conv::keyword("detect"), janet_wrap_array(markers));
            let actions = janet_array(t.actions.len().max(1) as i32);
            for (name, action) in &t.actions {
                let at = janet_wrap_table(janet_table(5));
                let a_tbl = janet_unwrap_table(at);
                janet_table_put(a_tbl, conv::keyword("name"), conv::string(name));
                janet_table_put(
                    a_tbl,
                    conv::keyword("program"),
                    conv::string(&action.program),
                );
                let args = janet_array(action.args.len().max(1) as i32);
                for a in &action.args {
                    janet_array_push(args, conv::string(a));
                }
                janet_table_put(a_tbl, conv::keyword("args"), janet_wrap_array(args));
                janet_table_put(
                    a_tbl,
                    conv::keyword("debuggable"),
                    conv::boolean(action.debuggable),
                );
                janet_array_push(actions, at);
            }
            janet_table_put(t_tbl, conv::keyword("actions"), janet_wrap_array(actions));
            janet_table_put(t_tbl, conv::keyword("plugin"), conv::string(&t.owner));
            match &t.install {
                Some(i) => {
                    let it = janet_wrap_table(janet_table(4));
                    let i_tbl = janet_unwrap_table(it);
                    janet_table_put(i_tbl, conv::keyword("url"), conv::string(&i.url));
                    janet_table_put(i_tbl, conv::keyword("sha256"), conv::string(&i.sha256));
                    janet_table_put(i_tbl, conv::keyword("format"), conv::string(&i.format));
                    janet_table_put(i_tbl, conv::keyword("bin"), conv::string(&i.bin));
                    janet_table_put(t_tbl, conv::keyword("install"), it);
                }
                None => janet_table_put(t_tbl, conv::keyword("install"), conv::nil()),
            }
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"toolchain/find".as_ptr() as *const _,
            cfun: Some(c_toolchain_find as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"(toolchain/find name) → path string or nil — resolve an executable against $PATH".as_ptr() as *const _,
        },
        JanetReg {
            name: c"toolchain/find-extended".as_ptr() as *const _,
            cfun: Some(c_toolchain_find_extended as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"(toolchain/find-extended name) → path string or nil — resolve against $PATH and the well-known install roots".as_ptr() as *const _,
        },
        JanetReg {
            name: c"toolchain/register".as_ptr() as *const _,
            cfun: Some(c_toolchain_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a toolchain from a spec table with detection markers and structured actions".as_ptr() as *const _,
        },
        JanetReg {
            name: c"toolchain/registered".as_ptr() as *const _,
            cfun: Some(c_toolchain_registered as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return an array of {:id :language :detect :actions :plugin} tables for all registered toolchains".as_ptr() as *const _,
        },
    ]
}
