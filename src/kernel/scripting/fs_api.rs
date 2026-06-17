//! Janet C functions for filesystem primitives (Sprint 12).

use evil_janet::*;
use super::conv;
use crate::kernel::extension::Capability;

/// (fs/cwd) → string
///
/// Return the current working directory as an absolute path string.
unsafe extern "C-unwind" fn c_fs_cwd(_argc: i32, _argv: *mut Janet) -> Janet {
    if !super::extension_api::check_capability(&Capability::Filesystem) {
        conv::signal_err("fs/cwd requires the Filesystem capability");
    }
    match std::env::current_dir() {
        Ok(path) => conv::string(&path.to_string_lossy()),
        Err(e) => conv::signal_err(&format!("fs/cwd: {e}")),
    }
}

/// (fs/chdir path) → nil
///
/// Change the current working directory to `path`.  Signals a Janet error
/// if the directory does not exist or is not accessible.
unsafe extern "C-unwind" fn c_fs_chdir(argc: i32, argv: *mut Janet) -> Janet {
    unsafe {
        if !super::extension_api::check_capability(&Capability::Filesystem) {
            conv::signal_err("fs/chdir requires the Filesystem capability");
        }
        let path = match conv::get_str(argc, argv, 0) {
            Some(p) => p,
            None => conv::signal_err("fs/chdir requires a path"),
        };
        match std::env::set_current_dir(&path) {
            Ok(()) => conv::nil(),
            Err(e) => conv::signal_err(&format!("fs/chdir: {e}")),
        }
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"fs/cwd".as_ptr() as *const _,
            cfun: Some(c_fs_cwd as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the current working directory as a string".as_ptr() as *const _,
        },
        JanetReg {
            name: c"fs/chdir".as_ptr() as *const _,
            cfun: Some(c_fs_chdir as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Change the current working directory".as_ptr() as *const _,
        },
    ]
}
