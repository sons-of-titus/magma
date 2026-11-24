//! Janet API for managed downloads of language servers and toolchains.
//!
//! Registered C functions:
//!   (install/fetch name kind id spec) → work-id
//!   (install/find kind id bin)        → path string or nil
//!
//! `spec` pins the payload: `{:url :sha256 :format :bin}`.  Fetch, checksum,
//! and unpack run on the Work scheduler (progress events come from there);
//! the outcome arrives as `install-completed`/`install-failed` events.

use super::conv;
use super::with_editor;
use kernel::extension::Capability;
use evil_janet::*;

/// `(install/fetch name kind id spec)` → work-id
///
/// Download and install one pinned payload.  `kind` selects the landing
/// area ("servers" or "toolchains"), `id` the per-language / per-toolchain
/// slot; the version directory is derived from the checksum so identical
/// pins never re-download.
unsafe extern "C-unwind" fn c_install_fetch(argc: i32, argv: *mut Janet) -> Janet {
    if !super::extension_api::check_capability(&Capability::Network) {
        conv::signal_err("install/fetch requires the Network capability");
    }
    let name = unsafe { conv::get_str(argc, argv, 0) }
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "download".to_string());
    let kind = unsafe { conv::get_str(argc, argv, 1) }
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "servers".to_string());
    let id = match unsafe { conv::get_str(argc, argv, 2) } {
        Some(s) if !s.is_empty() => s,
        _ => conv::signal_err("install/fetch: id (language or toolchain) is required"),
    };
    let spec = unsafe { *argv.add(3) };
    let spec = match unsafe { parse_spec(spec) } {
        Some(s) => s,
        None => conv::signal_err("install/fetch: spec needs :url :sha256 :format :bin"),
    };

    with_editor(|ed| {
        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("install/fetch: no background runtime available"),
        };
        let downloads = kernel::state::persist::state_dir().join("downloads");
        let work_name = format!("Downloading {name}");
        let bg_event = bg.clone();
        let work_id = ed.scheduler.submit_fn(work_name, &bg, move |progress, cancel| {
            if cancel.is_cancelled() {
                return Err("cancelled".into());
            }
            let mut report = |done: u64, total: u64| progress.report(done, total.max(1));
            match install::install(&spec, &downloads, &kind, &id, &mut report) {
                Ok(path) => {
                    bg_event.send_event(kernel::runtime::BackgroundEvent::InstallFinished {
                        name: name.clone(),
                        kind: kind.clone(),
                        id: id.clone(),
                        path: path.to_string_lossy().into_owned(),
                        error: None,
                    });
                    Ok(())
                }
                Err(e) => {
                    bg_event.send_event(kernel::runtime::BackgroundEvent::InstallFinished {
                        name: name.clone(),
                        kind: kind.clone(),
                        id: id.clone(),
                        path: String::new(),
                        error: Some(e),
                    });
                    Err("install failed (see the install-failed event)".into())
                }
            }
        });
        conv::integer(work_id as i32)
    })
}

/// `spec[:url :sha256 :format :bin]` → [`install::InstallSpec`].
unsafe fn parse_spec(spec: Janet) -> Option<install::InstallSpec> {
    unsafe {
        if !super::jtable::is_spec(spec) {
            return None;
        }
        let url = super::jtable::get_str(spec, "url")?;
        let sha256 = super::jtable::get_str(spec, "sha256")?;
        let bin = super::jtable::get_str(spec, "bin")?;
        let format =
            install::Format::parse(&super::jtable::get_str(spec, "format")?).ok()?;
        Some(install::InstallSpec {
            url,
            sha256,
            format,
            bin,
        })
    }
}

/// `(install/find kind id bin)` → path string or nil
///
/// Locate a previously installed executable under its download slot.  The
/// newest version directory wins.
unsafe extern "C-unwind" fn c_install_find(_argc: i32, argv: *mut Janet) -> Janet {
    let kind = unsafe { conv::get_str(_argc, argv, 0) }.unwrap_or_default();
    let id = unsafe { conv::get_str(_argc, argv, 1) }.unwrap_or_default();
    let bin = unsafe { conv::get_str(_argc, argv, 2) }.unwrap_or_default();
    if kind.is_empty() || id.is_empty() || bin.is_empty() {
        return conv::nil();
    }
    let slot = kernel::state::persist::state_dir()
        .join("downloads")
        .join(&kind)
        .join(&id);
    match install::find_in_slot(&slot, &bin) {
        Some(path) => conv::string(&path.to_string_lossy()),
        None => conv::nil(),
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"install/fetch".as_ptr() as *const _,
            cfun: Some(c_install_fetch as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Download, verify, and unpack a pinned payload; returns work-id, result arrives as install-completed/install-failed".as_ptr() as *const _,
        },
        JanetReg {
            name: c"install/find".as_ptr() as *const _,
            cfun: Some(c_install_find as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"(install/find kind id bin) → path of a previously installed executable, or nil".as_ptr() as *const _,
        },
    ]
}
