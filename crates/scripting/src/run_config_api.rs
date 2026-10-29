//! Janet API — run-config contribution.
//!
//! Language plugins register their build/test/run launch definitions here
//! instead of the editor hard-coding per-language commands.

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::run_config::RunConfig;

/// `(run-config/register name command &opt args cwd debuggable build-before)` → `:ok`
///
/// Add or replace a named run configuration.  `args` is an array of
/// strings; `cwd` may be nil (project root); `build-before` names another
/// config to complete first.
unsafe extern "C-unwind" fn c_run_config_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 2 {
            conv::signal_err("run-config/register: name and command required");
        }
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("run-config/register: name must be a string"),
        };
        let command = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("run-config/register: command must be a string"),
        };
        let cfg = RunConfig {
            name,
            command,
            args: conv::get_str_array(argc, argv, 2).unwrap_or_default(),
            cwd: conv::get_opt_str(argc, argv, 3).flatten(),
            env: std::collections::HashMap::new(),
            build_before_run: conv::get_opt_str(argc, argv, 5).flatten(),
            debuggable: conv::get_bool(argc, argv, 4).unwrap_or(false),
        };
        ed.project_manager.run_configs.upsert(cfg);
        conv::keyword("ok")
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![JanetReg {
        name: c"run-config/register".as_ptr() as *const _,
        cfun: Some(
            c_run_config_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
        ),
        documentation: c"Register a named run configuration: name command &opt args cwd debuggable build-before"
            .as_ptr() as *const _,
    }]
}
