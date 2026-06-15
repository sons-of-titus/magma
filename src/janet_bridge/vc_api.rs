//! Janet API for registering custom VCS backends via shell-command templates.
//!
//! Example from Janet:
//! ```janet
//! (vc/register-backend
//!   {:name          "fossil"
//!    :detect-marker ".fslckout"
//!    :root-cmd      "fossil info 2>/dev/null | awk '/local-root:/{print $2}'"
//!    :status-cmd    "fossil status 2>/dev/null"
//!    :diff-cmd      "fossil diff {file} 2>/dev/null"
//!    :log-cmd       "fossil timeline --limit {limit} 2>/dev/null"
//!    :stage-cmd     "fossil add -- {file} 2>/dev/null"
//!    :commit-cmd    "fossil commit -m {msg} 2>/dev/null"
//!    :push-cmd      "fossil sync 2>/dev/null"
//!    :blame-cmd     "fossil annotate {file} 2>/dev/null"
//!    :has-staging   false})
//! ```
//!
//! All `:*-cmd` keys are optional. A missing command returns an error when called.
//! `{file}`, `{dir}`, `{msg}`, `{limit}` are substituted at call time.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::vc::backends::ShellTemplateBackend;

/// Read a string value by keyword key from a table or struct Janet value.
unsafe fn dict_str(val: Janet, key: &str) -> Option<String> { unsafe {
    let k = conv::keyword(key);
    let v = janet_get(val, k);
    if janet_checktype(v, JanetType_JANET_STRING) == 0
        && janet_checktype(v, JanetType_JANET_KEYWORD) == 0
    {
        return None;
    }
    let ptr = janet_unwrap_string(v);
    if ptr.is_null() { return None; }
    Some(std::ffi::CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned())
}}

/// Read a boolean value by keyword key from a table or struct.
unsafe fn dict_bool(val: Janet, key: &str, default: bool) -> bool { unsafe {
    let k = conv::keyword(key);
    let v = janet_get(val, k);
    if janet_checktype(v, JanetType_JANET_BOOLEAN) != 0 {
        janet_unwrap_boolean(v) != 0
    } else {
        default
    }
}}

/// (vc/register-backend {:name … :detect-marker … :status-cmd … …})
///
/// Accepts both structs (`{…}`) and tables (`@{…}`).
pub unsafe extern "C-unwind" fn c_vc_register_backend(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    janet_arity(argc, 1, 1);
    let val = *argv;
    let is_table  = janet_checktype(val, JanetType_JANET_TABLE) != 0;
    let is_struct = janet_checktype(val, JanetType_JANET_STRUCT) != 0;
    if !is_table && !is_struct {
        conv::signal_err("vc/register-backend requires a table or struct argument");
    }

    let name = match dict_str(val, "name") {
        Some(n) => n,
        None => conv::signal_err("vc/register-backend: :name is required"),
    };
    let detect_marker = match dict_str(val, "detect-marker") {
        Some(m) => m,
        None => conv::signal_err("vc/register-backend: :detect-marker is required"),
    };

    let backend = ShellTemplateBackend {
        name,
        detect_marker,
        root_cmd:    dict_str(val, "root-cmd"),
        status_cmd:  dict_str(val, "status-cmd"),
        diff_cmd:    dict_str(val, "diff-cmd"),
        log_cmd:     dict_str(val, "log-cmd"),
        stage_cmd:   dict_str(val, "stage-cmd"),
        unstage_cmd: dict_str(val, "unstage-cmd"),
        commit_cmd:  dict_str(val, "commit-cmd"),
        push_cmd:    dict_str(val, "push-cmd"),
        blame_cmd:   dict_str(val, "blame-cmd"),
        has_staging: dict_bool(val, "has-staging", false),
    };

    with_editor(|ed| {
        ed.vc.backends.push(Box::new(backend));
        conv::nil()
    })
}}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"vc/register-backend".as_ptr() as *const _,
            cfun: Some(c_vc_register_backend as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a custom VCS backend via shell-command templates".as_ptr() as *const _,
        },
    ]
}
