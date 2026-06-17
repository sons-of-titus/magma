//! Janet scripting runtime bridge — wraps evil-janet FFI for Magma.

use std::cell::Cell;
use std::ffi::CString;

use crate::kernel::state::Editor;
#[cfg(feature = "janet")]
use evil_janet::*;

/// Global lock to serialise all access to the Janet VM.
///
/// The Janet runtime is a single global instance with no thread-safety
/// guarantees.  Tests and production code must acquire this lock before
/// any call into Janet.
/// Exposed to tests via `janet_bridge::JANET_VM_LOCK`; not called from production code paths.
#[allow(dead_code)]
#[cfg(feature = "janet")]
pub(crate) static JANET_VM_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(feature = "janet")]
thread_local! {
    /// Per-thread pointer to the current Editor.
    ///
    /// # Safety
    /// - Always accessed from the thread that owns the Editor.
    /// - Janet signals use `longjmp` which can skip past Rust destructors, so we
    ///   must not hold a `Mutex` guard (or any other RAII guard) across a call
    ///   into Janet.
    /// - Re-entrant calls (Janet → Rust → Janet → Rust) are safe because we
    ///   don't track borrows — each entry creates a fresh `&mut` from the raw
    ///   pointer.  The outer function frame is opaque to the compiler across the
    ///   generic closure boundary, so no aliasing UB arises in practice.
    pub(crate) static EDITOR_PTR: Cell<Option<*mut Editor>> = const { Cell::new(None) };

    /// Per-thread pointer to the current render Surface.
    ///
    /// Set by `set_surface_ptr` in the TUI/GUI loop immediately before the
    /// `render-frame` event is dispatched.  Cleared by `clear_surface_ptr`
    /// after the dispatch returns.  Janet C functions that write to the surface
    /// (e.g. `editor/surface-set-cell`) use `with_surface` to access it.
    pub(crate) static SURFACE_PTR: Cell<Option<*mut crate::kernel::render::surface::Surface>> = const { Cell::new(None) };
}
// ── Janet argument conversion helpers ─────────────────────────────────────

#[cfg(feature = "janet")]
pub(crate) mod conv {
    use evil_janet::*;

    pub unsafe fn get_str(argc: i32, argv: *mut Janet, i: i32) -> Option<String> { unsafe {
        if i >= argc {
            return None;
        }
        let v = *argv.add(i as usize);
        if janet_checktype(v, JanetType_JANET_STRING) == 0
            && janet_checktype(v, JanetType_JANET_KEYWORD) == 0
        {
            return None;
        }
        let ptr = janet_unwrap_string(v);
        if ptr.is_null() {
            return None;
        }
        let cstr = std::ffi::CStr::from_ptr(ptr as *const i8);
        Some(cstr.to_string_lossy().into_owned())
    }}

    pub unsafe fn get_opt_str(argc: i32, argv: *mut Janet, i: i32) -> Option<Option<String>> { unsafe {
        if i >= argc {
            return Some(None);
        }
        let v = *argv.add(i as usize);
        if janet_checktype(v, JanetType_JANET_NIL) != 0 {
            return Some(None);
        }
        get_str(argc, argv, i).map(Some)
    }}

    pub unsafe fn get_int(argc: i32, argv: *mut Janet, i: i32) -> Option<i32> { unsafe {
        if i >= argc {
            return None;
        }
        let v = *argv.add(i as usize);
        if janet_checktype(v, JanetType_JANET_NUMBER) == 0 {
            return None;
        }
        Some(janet_unwrap_integer(v))
    }}

    pub unsafe fn get_bool(argc: i32, argv: *mut Janet, i: i32) -> Option<bool> { unsafe {
        if i >= argc {
            return None;
        }
        let v = *argv.add(i as usize);
        if janet_checktype(v, JanetType_JANET_BOOLEAN) == 0 {
            return None;
        }
        Some(janet_unwrap_boolean(v) != 0)
    }}

    pub fn nil() -> Janet {
        unsafe { janet_wrap_nil() }
    }

    pub fn boolean(b: bool) -> Janet {
        unsafe { janet_wrap_boolean(b as i32) }
    }

    pub fn integer(i: i32) -> Janet {
        // janet_wrap_integer is static inline in janet.h — not exported.
        // Use janet_wrap_number (f64) instead — the unwrap functions handle both encodings.
        unsafe { janet_wrap_number(i as f64) }
    }

    pub fn string(s: &str) -> Janet {
        unsafe { janet_wrap_string(janet_string(s.as_ptr(), s.len() as i32)) }
    }

    pub fn keyword(s: &str) -> Janet {
        // janet_symbol creates an INTERNED string (same as `janet_keyword` in C,
        // which is #define'd to janet_symbol). Keywords must be interned so that
        // hash- and equality-based lookups (janet_table_get / janet_equals) work
        // correctly with keywords created by the Janet reader and (keyword ...).
        unsafe { janet_wrap_keyword(janet_symbol(s.as_ptr(), s.len() as i32)) }
    }

    /// Signal a Janet error — diverges (longjmp back to VM).
    pub fn signal_err(msg: &str) -> ! {
        unsafe { janet_signalv(JanetSignal_JANET_SIGNAL_ERROR, string(msg)) }
    }
}

/// Access the current thread's Editor from within a Janet C function callback.
///
/// Re-entrant-safe: if a Janet function calls back into Rust, the inner call
/// creates a new `&mut` from the same raw pointer.
#[cfg(feature = "janet")]
pub(crate) fn with_editor<F: FnOnce(&mut crate::kernel::state::Editor) -> evil_janet::Janet>(
    f: F,
) -> evil_janet::Janet {
    EDITOR_PTR.with(|cell| match cell.get() {
        Some(ptr) => unsafe { f(&mut *ptr) },
        None => conv::signal_err("editor not initialised"),
    })
}

/// Update the thread-local editor pointer.
///
/// Called from command closures when the editor address may have changed
/// (e.g. the RwLock guard was dropped and re-acquired).
#[cfg(feature = "janet")]
pub(crate) fn set_editor_ptr(editor: *mut Editor) {
    EDITOR_PTR.with(|cell| cell.set(Some(editor)));
}

/// Point the thread-local surface pointer at `surface` for the duration of a
/// `render-frame` event dispatch.  The pointer is valid until `clear_surface_ptr`
/// is called — the caller must ensure the surface outlives the dispatch.
#[cfg(feature = "janet")]
pub fn set_surface_ptr(surface: *mut crate::kernel::render::surface::Surface) {
    SURFACE_PTR.with(|cell| cell.set(Some(surface)));
}

/// Clear the thread-local surface pointer after a `render-frame` dispatch.
#[cfg(feature = "janet")]
pub fn clear_surface_ptr() {
    SURFACE_PTR.with(|cell| cell.set(None));
}

#[cfg(feature = "janet")]
mod keymap_api;
#[cfg(feature = "janet")]
mod buffer_api;
#[cfg(feature = "janet")]
mod buffer_text_api;
#[cfg(feature = "janet")]
mod buffer_query_api;
#[cfg(feature = "janet")]
mod special_buffer_api;
#[cfg(feature = "janet")]
mod command_api;
#[cfg(feature = "janet")]
mod event_api;
#[cfg(feature = "janet")]
mod window_api;
#[cfg(feature = "janet")]
mod editor_state_api;
#[cfg(feature = "janet")]
mod editor_io_api;
#[cfg(feature = "janet")]
mod editor_view_api;
#[cfg(feature = "janet")]
mod messages_api;
#[cfg(feature = "janet")]
mod lsp_api;
#[cfg(feature = "janet")]
mod lsp_edit_api;
#[cfg(feature = "janet")]
mod semantic_api;
#[cfg(feature = "janet")]
mod vc_api;
#[cfg(feature = "janet")]
mod process_api;
#[cfg(feature = "janet")]
mod project_api;
#[cfg(feature = "janet")]
mod project_registry_api;
#[cfg(feature = "janet")]
mod workspace_api;
#[cfg(feature = "janet")]
mod face_api;
#[cfg(feature = "janet")]
mod treesitter_api;
#[cfg(feature = "janet")]
mod display_api;
#[cfg(feature = "janet")]
mod layout_api;
#[cfg(feature = "janet")]
mod mode_api;
#[cfg(feature = "janet")]
mod ecosystem_api;
#[cfg(feature = "janet")]
mod font_api;
#[cfg(feature = "janet")]
mod ui_api;
#[cfg(feature = "janet")]
mod overlay_api;
#[cfg(feature = "janet")]
mod gutter_api;
#[cfg(feature = "janet")]
mod decoration_api;
#[cfg(feature = "janet")]
mod modality_api;
#[cfg(feature = "janet")]
mod eval_api;
#[cfg(feature = "janet")]
mod fs_api;
#[cfg(feature = "janet")]
mod net_http_api;
#[cfg(feature = "janet")]
mod net_tcp_api;
#[cfg(feature = "janet")]
mod minibuffer_api;
#[cfg(feature = "janet")]
mod selection_api;
#[cfg(feature = "janet")]
mod register_api;
#[cfg(feature = "janet")]
mod option_api;
#[cfg(feature = "janet")]
mod plugin_state_api;
#[cfg(feature = "janet")]
mod search_api;
#[cfg(feature = "janet")]
mod clipboard_api;
#[cfg(feature = "janet")]
mod task_api;
#[cfg(feature = "janet")]
mod debug_api;

#[cfg(feature = "janet")]
pub(crate) use process_api::execute_stored_task;
#[cfg(feature = "janet")]
pub(crate) use task_api::execute_task_complete_callback;

#[cfg(feature = "janet")]
mod loader;

#[cfg(feature = "janet")]
mod runtime;

#[cfg(feature = "janet")]
pub use loader::init;

#[cfg(feature = "janet")]
pub use runtime::JanetRuntime;

/// Evaluate a Janet expression.
///
/// The caller must ensure `EDITOR_PTR` is set (it is set at `init` time and
/// remains valid for the lifetime of the Editor).
#[cfg(feature = "janet")]
pub fn eval(expr: &str) -> String {
    match loader::eval_string("eval", expr) {
        Ok(()) => "ok".to_string(),
        Err(e) => e,
    }
}

/// Evaluate a Janet expression and return its printed result.
///
/// Returns `Ok(result_string)` on success where `result_string` is the
/// Janet `description` of the returned value, or `Err(msg)` on failure.
///
/// The caller must ensure `EDITOR_PTR` is set.
#[cfg(feature = "janet")]
pub fn eval_result(expr: &str) -> Result<String, String> {
    unsafe {
        let c_name = CString::new("eval").map_err(|e| e.to_string())?;
        let c_source = CString::new(expr).map_err(|e| e.to_string())?;
        let mut result = std::mem::MaybeUninit::<evil_janet::Janet>::zeroed();
        let status = janet_dostring(
            janet_core_env(std::ptr::null_mut()),
            c_source.as_ptr(),
            c_name.as_ptr(),
            result.as_mut_ptr(),
        );
        if status != 0 {
            Err("Janet error in eval".to_string())
        } else {
            let v = result.assume_init();
            let s = janet_description(v);
            if s.is_null() {
                Ok("nil".to_string())
            } else {
                let cstr = std::ffi::CStr::from_ptr(s as *const i8);
                Ok(cstr.to_string_lossy().into_owned())
            }
        }
    }
}

/// Look up a command name in `*janet-commands*` and call the stored function.
///
/// Called from `execute_command` as the fallthrough path when no Rust command
/// matches.  Returns `Err` if the name is not found in the Janet table or if
/// the function call fails.
#[cfg(feature = "janet")]
pub fn call_janet_command(
    editor: &mut Editor,
    name: &str,
    _args: &std::collections::HashMap<String, crate::kernel::command::args::ArgValue>,
) -> crate::kernel::command::CommandResult {
    EDITOR_PTR.with(|cell| cell.set(Some(editor as *mut Editor)));
    unsafe {
        let env = janet_core_env(std::ptr::null_mut());
        let mut root: Janet = std::mem::zeroed();
        let sym_bytes = b"*janet-commands*";
        let cmd_sym = janet_symbol(sym_bytes.as_ptr(), sym_bytes.len() as i32);
        if janet_resolve(env, cmd_sym, &mut root) == JanetBindingType_JANET_BINDING_NONE {
            return Err(format!("Unknown command: {name}"));
        }
        let table = janet_unwrap_table(root);
        let key_name = CString::new(name).map_err(|e| e.to_string())?;
        let key = janet_string(key_name.as_ptr() as *const u8, name.len() as i32);
        let func_val = janet_table_get(table, janet_wrap_string(key));
        if janet_checktype(func_val, JanetType_JANET_FUNCTION) == 0 {
            return Err(format!("Unknown command: {name}"));
        }
        let func = janet_unwrap_function(func_val);
        let mut out: Janet = std::mem::zeroed();
        let sig = janet_pcall(func, 0, std::ptr::null(), &mut out, std::ptr::null_mut());
        if sig != JanetSignal_JANET_SIGNAL_OK {
            return Err(format!("Janet command '{name}' signaled error"));
        }
    }
    Ok(())
}

/// Load a Janet file from disk.
///
/// The caller must ensure `EDITOR_PTR` is set so that the background handle
/// (if any) can be accessed through the editor.
#[cfg(feature = "janet")]
pub fn load_file(path: &str) -> Result<(), String> {
    let path_owned = path.to_string();
    let content = match EDITOR_PTR.with(|cell| cell.get()) {
        Some(ptr) => {
            let ed = unsafe { &*ptr };
            match &ed.background {
                Some(bg) => bg.block_on(move || std::fs::read_to_string(path_owned))
                    .map_err(|e| format!("{e}"))?,
                None => std::fs::read_to_string(path).map_err(|e| format!("{e}"))?,
            }
        }
        None => std::fs::read_to_string(path).map_err(|e| format!("{e}"))?,
    };
    loader::eval_string(path, &content)
}

// ── Stubs (janet feature disabled) ──────────────────────────────────────────

#[cfg(not(feature = "janet"))]
pub fn init(_editor: &mut Editor) {
    println!("Janet runtime not available (compile with --features janet)");
}

#[cfg(not(feature = "janet"))]
pub fn eval(expr: &str) -> String {
    format!("Janet eval not available: {expr}")
}

#[cfg(not(feature = "janet"))]
pub fn load_file(_path: &str) -> Result<(), String> {
    Err("Janet runtime not available".to_string())
}

// ── ScriptRuntime trait (merged from scripting/mod.rs) ─────────────────────
pub mod null_runtime;

use std::collections::HashMap;
use crate::kernel::command::args::ArgValue;
use crate::kernel::command::CommandResult;

pub trait ScriptRuntime: Send + Sync {
    fn init(&mut self, editor: *mut crate::kernel::state::Editor);
    fn eval(&mut self, expr: &str) -> String;
    fn eval_result(&mut self, expr: &str) -> Result<String, String>;
    fn load_file(&mut self, path: &str) -> Result<(), String>;
    fn call_command(&mut self, name: &str, args: &HashMap<String, ArgValue>) -> CommandResult;
    fn execute_stored_task(&mut self, _task_id: u64) {}
    fn execute_task_complete_callback(&mut self, _task_id: u64) {}
}
