//! Janet scripting runtime bridge — wraps evil-janet FFI for Magma.

#[macro_use]
extern crate logging;

#[cfg(feature = "janet")]
use std::cell::Cell;
#[cfg(feature = "janet")]
use std::ffi::CString;

#[cfg(feature = "janet")]
use evil_janet::*;
use kernel::state::Editor;

/// Global lock to serialise all access to the Janet VM.
///
/// The Janet runtime is a single global instance with no thread-safety
/// guarantees.  Tests and production code must acquire this lock before
/// any call into Janet.
/// Exposed to tests via `janet_bridge::JANET_VM_LOCK`; not called from production code paths.
#[allow(dead_code)]
#[cfg(feature = "janet")]
pub static JANET_VM_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
}
// ── Janet argument conversion helpers ─────────────────────────────────────

#[cfg(feature = "janet")]
pub(crate) mod conv {
    use evil_janet::*;

    pub unsafe fn get_str(argc: i32, argv: *mut Janet, i: i32) -> Option<String> {
        unsafe {
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
        }
    }

    pub unsafe fn get_opt_str(argc: i32, argv: *mut Janet, i: i32) -> Option<Option<String>> {
        unsafe {
            if i >= argc {
                return Some(None);
            }
            let v = *argv.add(i as usize);
            if janet_checktype(v, JanetType_JANET_NIL) != 0 {
                return Some(None);
            }
            get_str(argc, argv, i).map(Some)
        }
    }

    pub unsafe fn get_int(argc: i32, argv: *mut Janet, i: i32) -> Option<i32> {
        unsafe {
            if i >= argc {
                return None;
            }
            let v = *argv.add(i as usize);
            if janet_checktype(v, JanetType_JANET_NUMBER) == 0 {
                return None;
            }
            Some(janet_unwrap_integer(v))
        }
    }

    pub unsafe fn get_bool(argc: i32, argv: *mut Janet, i: i32) -> Option<bool> {
        unsafe {
            if i >= argc {
                return None;
            }
            let v = *argv.add(i as usize);
            if janet_checktype(v, JanetType_JANET_BOOLEAN) == 0 {
                return None;
            }
            Some(janet_unwrap_boolean(v) != 0)
        }
    }

    /// Read a Janet array of strings/keywords into a Vec.  Non-string
    /// elements are skipped.
    pub unsafe fn get_str_array(argc: i32, argv: *mut Janet, i: i32) -> Option<Vec<String>> {
        unsafe {
            if i >= argc {
                return None;
            }
            let v = *argv.add(i as usize);
            if janet_checktype(v, JanetType_JANET_ARRAY) == 0 {
                return None;
            }
            let arr = janet_unwrap_array(v);
            let len = (*arr).count as usize;
            let mut out = Vec::with_capacity(len);
            for idx in 0..len {
                let elem = *(*arr).data.add(idx);
                let mut tmp_argv = elem;
                if let Some(s) = get_str(1, &mut tmp_argv, 0) {
                    out.push(s);
                }
            }
            Some(out)
        }
    }

    /// Read a table argument as `(string-key, string-value)` pairs.
    /// `None` when the argument is absent or not a table; non-string
    /// keys and values are skipped.
    pub unsafe fn get_table_strings(
        argc: i32,
        argv: *mut Janet,
        i: i32,
    ) -> Option<Vec<(String, String)>> {
        unsafe {
            if i >= argc {
                return None;
            }
            let v = *argv.add(i as usize);
            if janet_checktype(v, JanetType_JANET_TABLE) == 0 {
                return None;
            }
            let tbl = janet_unwrap_table(v);
            // Iterate the full backing-array capacity, not `count`: live
            // entries sit at their hash index, which can be anywhere in
            // [0, capacity).
            let mut out = Vec::new();
            for idx in 0..(*tbl).capacity {
                let kv = *(*tbl).data.add(idx as usize);
                if janet_checktype(kv.key, JanetType_JANET_NIL) != 0 {
                    continue;
                }
                let mut key_argv = kv.key;
                let mut val_argv = kv.value;
                if let (Some(k), Some(val)) = (
                    get_str(1, &mut key_argv, 0),
                    get_str(1, &mut val_argv, 0),
                ) {
                    out.push((k, val));
                }
            }
            Some(out)
        }
    }

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

/// The FFI soundness invariant (docs/core-redesign.md §11): a Janet C
/// function may only touch the editor inside a `scoped_editor` window — a
/// span where the caller's exclusive `&mut Editor` borrow is live and no
/// other Rust-side `&mut` is used.  The pointer is never kept across the
/// scope: a guard that has been dropped cannot be aliased from Janet.
///
/// Access the current thread's Editor from within a Janet C function
/// callback.  Signals a clean Janet error outside any scope.
#[cfg(feature = "janet")]
pub(crate) fn with_editor<F: FnOnce(&mut kernel::state::Editor) -> evil_janet::Janet>(
    f: F,
) -> evil_janet::Janet {
    EDITOR_PTR.with(|cell| match cell.get() {
        Some(ptr) => unsafe { f(&mut *ptr) },
        None => conv::signal_err("editor not initialised"),
    })
}

/// Run `f` with the editor pointer installed from a live `&mut` borrow.
/// Nesting restores the previous pointer on exit.  Within `f`, all editor
/// access from Janet goes through `with_editor`; Rust code must not use
/// its own borrow for the editor while Janet is running.  `f` must not
/// capture the editor — use `EditorScope` where it needs to.
#[cfg(feature = "janet")]
pub fn scoped_editor<R>(editor: &mut Editor, f: impl FnOnce() -> R) -> R {
    let _scope = EditorScope::new(editor);
    f()
}

/// Installs the editor pointer on creation and restores the previous value
/// on drop.  Nesting composes: the inner scope's drop reinstates the outer
/// pointer.
#[cfg(feature = "janet")]
pub struct EditorScope {
    prev: Option<*mut Editor>,
}

#[cfg(feature = "janet")]
impl EditorScope {
    pub fn new(editor: &mut Editor) -> Self {
        let prev = EDITOR_PTR.with(|cell| cell.replace(Some(editor as *mut Editor)));
        EditorScope { prev }
    }
}

#[cfg(feature = "janet")]
impl Drop for EditorScope {
    fn drop(&mut self) {
        EDITOR_PTR.with(|cell| cell.set(self.prev));
    }
}

#[cfg(not(feature = "janet"))]
pub fn scoped_editor<R>(editor: &mut Editor, f: impl FnOnce() -> R) -> R {
    let _ = editor;
    f()
}

/// Drop any editor pointer left installed by a scope (test teardown).
#[cfg(feature = "janet")]
pub fn clear_editor_scope() {
    EDITOR_PTR.with(|cell| cell.set(None));
}

#[cfg(not(feature = "janet"))]
pub fn clear_editor_scope() {}

#[cfg(feature = "janet")]
mod agent_api;
#[cfg(feature = "janet")]
mod buffer_api;
#[cfg(feature = "janet")]
mod buffer_query_api;
#[cfg(feature = "janet")]
mod buffer_text_api;
#[cfg(feature = "janet")]
mod buffer_word_api;
#[cfg(feature = "janet")]
mod syntax_api;
#[cfg(feature = "janet")]
mod clipboard_api;
#[cfg(feature = "janet")]
mod command_api;
#[cfg(feature = "janet")]
mod debug_api;
#[cfg(feature = "janet")]
mod decoration_api;
#[cfg(feature = "janet")]
mod display_api;
#[cfg(feature = "janet")]
mod ecosystem_api;
#[cfg(feature = "janet")]
mod editor_io_api;
#[cfg(feature = "janet")]
mod editor_state_api;
#[cfg(feature = "janet")]
mod editor_view_api;
#[cfg(feature = "janet")]
mod eval_api;
#[cfg(feature = "janet")]
mod event_api;
#[cfg(feature = "janet")]
mod extension_api;
#[cfg(feature = "janet")]
mod face_api;
#[cfg(feature = "janet")]
mod font_api;
#[cfg(feature = "janet")]
mod fs_api;
#[cfg(feature = "janet")]
mod gutter_api;
#[cfg(feature = "janet")]
mod icons_api;
#[cfg(feature = "janet")]
mod keymap_api;
#[cfg(feature = "janet")]
mod language_api;
#[cfg(feature = "janet")]
mod layout_api;
#[cfg(feature = "janet")]
mod lsp_api;
#[cfg(feature = "janet")]
mod lsp_edit_api;
#[cfg(feature = "janet")]
mod messages_api;
#[cfg(feature = "janet")]
mod modality_api;
#[cfg(feature = "janet")]
mod mode_api;
#[cfg(feature = "janet")]
mod net_http_api;
#[cfg(feature = "janet")]
mod net_tcp_api;
#[cfg(feature = "janet")]
mod option_api;
#[cfg(feature = "janet")]
mod overlay_api;
#[cfg(feature = "janet")]
mod plugin_api;
#[cfg(feature = "janet")]
mod plugin_state_api;
#[cfg(feature = "janet")]
mod process_api;
#[cfg(feature = "janet")]
mod project_api;
#[cfg(feature = "janet")]
mod project_registry_api;
#[cfg(feature = "janet")]
mod register_api;
#[cfg(feature = "janet")]
mod run_config_api;
#[cfg(feature = "janet")]
mod scheduler_api;
#[cfg(feature = "janet")]
mod search_api;
#[cfg(feature = "janet")]
mod selection_api;
#[cfg(feature = "janet")]
mod semantic_api;
#[cfg(feature = "janet")]
mod special_buffer_api;
#[cfg(feature = "janet")]
mod task_api;
#[cfg(feature = "janet")]
mod toolchain_api;
#[cfg(feature = "janet")]
mod treesitter_api;
#[cfg(feature = "janet")]
mod ui_api;
#[cfg(feature = "janet")]
mod vc_api;
#[cfg(feature = "janet")]
mod window_api;
#[cfg(feature = "janet")]
mod workspace_api;

#[cfg(feature = "janet")]
mod loader;

#[cfg(feature = "janet")]
mod runtime;

#[cfg(feature = "janet")]
pub use loader::init;

#[cfg(feature = "janet")]
pub use loader::take_builtin_load_errors;

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
    args: &std::collections::HashMap<String, kernel::command::args::ArgValue>,
) -> kernel::command::CommandResult {
    scoped_editor(editor, || call_janet_command_scoped(name, args))
}

/// Body of `call_janet_command`; requires an active `scoped_editor` window.
#[cfg(feature = "janet")]
pub fn call_janet_command_scoped(
    name: &str,
    args: &std::collections::HashMap<String, kernel::command::args::ArgValue>,
) -> kernel::command::CommandResult {
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
        let raw = args
            .get("arg")
            .and_then(|v| v.as_string().map(|s| s.to_string()));
        command_api::invoke_janet_command(func, raw.as_deref())
    }
}

/// Load a Janet file from disk.
///
/// Requires an active `scoped_editor` window; the background handle is read
/// through `with_editor`.
#[cfg(feature = "janet")]
pub fn load_file(path: &str) -> Result<(), String> {
    let mut content: Option<Result<String, String>> = None;
    with_editor(|ed| {
        let via_bg = ed.background.as_ref().map(|bg| {
            let p = path.to_string();
            bg.block_on(move || std::fs::read_to_string(p))
                .map_err(|e| format!("{e}"))
        });
        content = Some(
            via_bg.unwrap_or_else(|| std::fs::read_to_string(path).map_err(|e| format!("{e}"))),
        );
        conv::nil()
    });
    let content =
        content.unwrap_or_else(|| std::fs::read_to_string(path).map_err(|e| format!("{e}")))?;
    loader::eval_string(path, &content)
}

/// Run a stored background task's function inside the current scope.
#[cfg(feature = "janet")]
pub fn execute_stored_task_scoped(task_id: u64) {
    with_editor(|ed| {
        crate::process_api::execute_stored_task(ed, task_id);
        conv::nil()
    });
}

/// Run a task-completion callback inside the current scope.
#[cfg(feature = "janet")]
pub fn execute_task_complete_callback_scoped(task_id: u64) {
    with_editor(|ed| {
        crate::task_api::execute_task_complete_callback(ed, task_id);
        conv::nil()
    });
}

// ── Stubs (janet feature disabled) ──────────────────────────────────────────

#[cfg(not(feature = "janet"))]
pub fn init(_editor: &mut Editor) {
    warn!("Janet runtime not available (compile with --features janet)");
}

#[cfg(not(feature = "janet"))]
pub fn eval(expr: &str) -> String {
    format!("Janet eval not available: {expr}")
}

#[cfg(not(feature = "janet"))]
pub fn load_file(_path: &str) -> Result<(), String> {
    Err("Janet runtime not available".to_string())
}

pub mod null_runtime;

use kernel::script_runtime::ScriptRuntime;

// (re-exports already provided by the moved module declarations below)
