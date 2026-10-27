//! Janet API — toolchain discovery.
//!
//! Registered C functions:
//!   (toolchain/find name) → path string or nil
//!   (toolchain/find-extended name) → path string or nil

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (toolchain/find name) → path or nil
/// Resolve an executable against $PATH so scripts can give a clear
/// "not installed" answer instead of a spawn failure.
unsafe extern "C-unwind" fn c_toolchain_find(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|_ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match magma::kernel::project::toolchain::find(&name) {
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
        match magma::kernel::project::toolchain::find_extended(&name) {
            Some(path) => conv::string(&path.to_string_lossy()),
            None => conv::nil(),
        }
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
    ]
}
