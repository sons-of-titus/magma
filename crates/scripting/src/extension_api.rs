//! Janet API for the Plugin Capability System (Phase 10).
//!
//! Registered C functions:
//!   (extension/declare name version capabilities)   → :ok or error
//!   (extension/undeclare)                           → nil
//!   (extension/manifest)                            → table or nil
//!   (extension/list)                                → array of tables
//!   (extension/capabilities name)                   → array of keywords

use std::cell::RefCell;

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::event::keys;
use kernel::event::payload::{ExtensionDeclaredPayload, ExtensionUndeclaredPayload};
use kernel::extension::{Capability, ExtensionManifest};

thread_local! {
    /// Execution contexts, outermost first.  Core (empty stack) is
    /// trusted; extension contexts are checked against the registry; a
    /// revoked context fails every check.  Handlers and commands capture
    /// the context at registration and re-enter it at invocation, so the
    /// gate holds at runtime, not just load time.
    static CONTEXT_STACK: RefCell<Vec<ExecContext>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug, Clone)]
pub(crate) enum ExecContext {
    Core,
    Extension(String),
    /// The payload name is kept for debugging/inspection; the variant
    /// itself is what fails every capability check.
    Revoked(#[allow(dead_code)] String),
}

pub(crate) fn current_context() -> ExecContext {
    CONTEXT_STACK.with(|s| s.borrow().last().cloned().unwrap_or(ExecContext::Core))
}

pub(crate) fn push_context(ctx: ExecContext) {
    CONTEXT_STACK.with(|s| s.borrow_mut().push(ctx));
}

pub(crate) fn pop_context() {
    CONTEXT_STACK.with(|s| {
        s.borrow_mut().pop();
    });
}

/// Run `f` inside `ctx`, restoring the previous context afterwards.
pub(crate) fn with_context<R>(ctx: ExecContext, f: impl FnOnce() -> R) -> R {
    push_context(ctx);
    let out = f();
    pop_context();
    out
}

/// Attribute a script-level registration to the current plugin.  Core
/// registrations record nothing — the journal only ever holds what a
/// plugin's entry script registered, which is exactly what `plugin/unload`
/// may remove.
pub(crate) fn record_if_plugin(
    ed: &mut kernel::state::Editor,
    contribution: kernel::state::contributions::Contribution,
) {
    if let ExecContext::Extension(plugin) = current_context() {
        ed.plugin_contributions.record(&plugin, contribution);
    }
}

/// The capability verdict for the calling context.
pub(super) fn check_capability(cap: &Capability) -> bool {
    match current_context() {
        ExecContext::Core => true,
        ExecContext::Revoked(_) => false,
        ExecContext::Extension(ref name) => super::EDITOR_PTR.with(|cell| match cell.get() {
            Some(ptr) => unsafe { (&*ptr).extension_registry.has_capability(name, cap) },
            None => false,
        }),
    }
}

/// Reset the thread-local at the start of each Janet VM session.
pub fn reset_current_extension() {
    CONTEXT_STACK.with(|c| c.borrow_mut().clear());
}

/// `(extension/declare name version capabilities)` → `:ok`
///
/// Declare the manifest for the currently-loading extension.  `capabilities`
/// is a Janet array of keywords: `:filesystem`, `:network`,
/// `:process-execution`, `:editor-access`.
///
/// This both registers the manifest in the `ExtensionRegistry` and sets the
/// thread-local context so that subsequent C function calls in this loading
/// session are capability-checked.
unsafe extern "C-unwind" fn c_extension_declare(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 3 {
            conv::signal_err("extension/declare: name version capabilities required");
        }
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("extension/declare: name must be a string"),
        };
        let version = conv::get_str(argc, argv, 1).unwrap_or_else(|| "0.0.0".to_string());
        let entry_point = conv::get_str(argc, argv, 2).unwrap_or_default();

        // Argument 3 (optional): capability array
        let mut capabilities: Vec<Capability> = Vec::new();
        if argc >= 4 {
            let cap_val = *argv.add(3);
            if janet_checktype(cap_val, JanetType_JANET_ARRAY) != 0 {
                let arr = janet_unwrap_array(cap_val);
                let len = (*arr).count as usize;
                for i in 0..len {
                    let elem = *(*arr).data.add(i);
                    if janet_checktype(elem, JanetType_JANET_KEYWORD) != 0 {
                        let ptr = janet_unwrap_keyword(elem);
                        if !ptr.is_null() {
                            let s = std::ffi::CStr::from_ptr(ptr as *const std::ffi::c_char).to_string_lossy();
                            if let Some(cap) = Capability::from_str(&s) {
                                capabilities.push(cap);
                            }
                        }
                    }
                }
            }
        }

        let manifest =
            ExtensionManifest::new(name.clone(), version.clone(), capabilities, entry_point);
        ed.extension_registry.register(manifest);

        // Activate the extension context for this loading session
        crate::extension_api::push_context(ExecContext::Extension(name.clone()));

        ed.events.emit_typed(
            keys::events::EXTENSION_DECLARED,
            ExtensionDeclaredPayload { name, version },
        );

        conv::keyword("ok")
    })
}

/// `(extension/undeclare)` → nil
///
/// Revoke the calling extension's context: every capability check fails
/// from here on.  An extension cannot undeclare itself back into core
/// trust — that was a one-call self-escalation.  Core code calling this
/// is a no-op.
unsafe extern "C-unwind" fn c_extension_undeclare(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let revoked = CONTEXT_STACK.with(|s| {
            let mut stack = s.borrow_mut();
            match stack.last() {
                Some(ExecContext::Extension(name)) => {
                    let name = name.clone();
                    *stack.last_mut().unwrap() = ExecContext::Revoked(name.clone());
                    Some(name)
                }
                _ => None,
            }
        });
        if let Some(name) = revoked {
            ed.events.emit_typed(
                keys::events::EXTENSION_UNDECLARED,
                ExtensionUndeclaredPayload { name },
            );
        }
        conv::nil()
    })
}

/// `(extension/manifest)` → `{:name :version :capabilities :entry-point}` or nil
///
/// Return the manifest table for the currently-active extension, or nil
/// if no extension is active.
unsafe extern "C-unwind" fn c_extension_manifest(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        match current_context() {
            ExecContext::Extension(ref name) => match ed.extension_registry.get(name) {
                None => conv::nil(),
                Some(manifest) => manifest_to_janet(manifest),
            },
            _ => conv::nil(),
        }
    })
}

/// `(extension/list)` → array of manifest tables
///
/// Return all registered extension manifests.
unsafe extern "C-unwind" fn c_extension_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let manifests = ed.extension_registry.list();
        let arr = janet_wrap_array(janet_array(manifests.len() as i32));
        for manifest in manifests {
            janet_array_push(janet_unwrap_array(arr), manifest_to_janet(manifest));
        }
        arr
    })
}

/// `(extension/capabilities name)` → array of capability keywords or nil
///
/// Return the capabilities declared by extension `name`, or nil if the
/// extension is not registered.
unsafe extern "C-unwind" fn c_extension_capabilities(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("extension/capabilities: name string required"),
        };
        match ed.extension_registry.get(&name) {
            None => conv::nil(),
            Some(manifest) => {
                let arr = janet_wrap_array(janet_array(manifest.capabilities.len() as i32));
                for cap in &manifest.capabilities {
                    janet_array_push(janet_unwrap_array(arr), conv::keyword(cap.as_str()));
                }
                arr
            }
        }
    })
}

/// Build a Janet table from a manifest.
unsafe fn manifest_to_janet(manifest: &kernel::extension::ExtensionManifest) -> Janet {
    unsafe {
        let tbl = janet_wrap_table(janet_table(4));
        let t = janet_unwrap_table(tbl);
        janet_table_put(t, conv::keyword("name"), conv::string(&manifest.name));
        janet_table_put(t, conv::keyword("version"), conv::string(&manifest.version));
        janet_table_put(
            t,
            conv::keyword("entry-point"),
            conv::string(&manifest.entry_point),
        );

        let cap_arr = janet_wrap_array(janet_array(manifest.capabilities.len() as i32));
        for cap in &manifest.capabilities {
            janet_array_push(janet_unwrap_array(cap_arr), conv::keyword(cap.as_str()));
        }
        janet_table_put(t, conv::keyword("capabilities"), cap_arr);
        tbl
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"extension/declare".as_ptr() as *const _,
            cfun: Some(
                c_extension_declare as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Register an extension manifest and activate its capability context"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"extension/undeclare".as_ptr() as *const _,
            cfun: Some(
                c_extension_undeclare as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Clear the active extension context after loading".as_ptr() as *const _,
        },
        JanetReg {
            name: c"extension/manifest".as_ptr() as *const _,
            cfun: Some(
                c_extension_manifest as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Return the manifest table for the currently-active extension, or nil"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"extension/list".as_ptr() as *const _,
            cfun: Some(c_extension_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return an array of all registered extension manifests".as_ptr()
                as *const _,
        },
        JanetReg {
            name: c"extension/capabilities".as_ptr() as *const _,
            cfun: Some(
                c_extension_capabilities as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Return the capability keywords declared by the named extension"
                .as_ptr() as *const _,
        },
    ]
}
