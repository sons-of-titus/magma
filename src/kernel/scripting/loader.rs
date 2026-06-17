//! Janet VM initialisation: API registration, builtin script loading, user init.

use std::cell::Cell;
use std::ffi::CString;
use crate::kernel::state::Editor;
use evil_janet::*;
use super::EDITOR_PTR;

thread_local! {
    /// Whether janet_init() has been called on this OS thread.
    /// Each test thread starts with false; janet_init() is called exactly once per thread.
    static JANET_THREAD_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn eval_string(name: &str, source: &str) -> Result<(), String> {
    unsafe {
        let c_name = CString::new(name).map_err(|e| e.to_string())?;
        let c_source = CString::new(source).map_err(|e| e.to_string())?;
        let mut result = std::mem::MaybeUninit::<Janet>::zeroed();
        let status = janet_dostring(
            janet_core_env(std::ptr::null_mut()),
            c_source.as_ptr(),
            c_name.as_ptr(),
            result.as_mut_ptr(),
        );
        if status != 0 {
            Err(format!("Janet error in {name}"))
        } else {
            Ok(())
        }
    }
}

pub(super) fn load_builtin(name: &str, source: &str) {
    unsafe {
        let c_name = CString::new(name).expect("builtin script name contains null byte");
        let c_source = CString::new(source).expect("builtin script source contains null byte");
        let mut result = std::mem::MaybeUninit::<Janet>::zeroed();
        let status = janet_dostring(
            janet_core_env(std::ptr::null_mut()),
            c_source.as_ptr(),
            c_name.as_ptr(),
            result.as_mut_ptr(),
        );
        if status != 0 {
            debug!("Janet: error loading {name}");
        }
    }
}

/// Initialise the Janet VM with EDITOR_PTR set.
///
/// Clears any stale Janet event handlers from previous sessions before
/// re-registering C APIs and reloading builtins.  New code should call
/// `init_vm()` after setting EDITOR_PTR directly.
pub fn init(editor: &mut Editor) {
    EDITOR_PTR.with(|cell| cell.set(Some(editor as *mut Editor)));
    // Discard Janet values stored by previous test sessions — they belong to
    // GC heaps on other threads and would be dangling pointers now.
    super::event_api::reset_handlers();
    super::process_api::reset_task_functions();
    super::task_api::reset_task_complete_callbacks();
    super::debug_api::reset_dap_clients();
    super::extension_api::reset_current_extension();
    init_vm();
    #[cfg(feature = "janet")]
    {
        editor.runtime = Some(Box::new(super::runtime::JanetRuntime));
    }
}

///
/// Does NOT set EDITOR_PTR — the caller must ensure it is set before calling
/// this function (e.g. via `JanetRuntime::init` or directly).
pub fn init_vm() {
    JANET_THREAD_INITIALIZED.with(|initialized| {
        if !initialized.get() {
            unsafe { janet_init(); }
            initialized.set(true);
        }
    });

    let env = unsafe { janet_core_env(std::ptr::null_mut()) };

    let all_regs = {
        let mut regs = Vec::new();
        regs.extend(super::keymap_api::register());
        regs.extend(super::buffer_text_api::register());
        regs.extend(super::buffer_query_api::register());
        regs.extend(super::buffer_api::register());
        regs.extend(super::special_buffer_api::register());
        regs.extend(super::command_api::register());
        regs.extend(super::event_api::register());
        regs.extend(super::window_api::register());
        regs.extend(super::editor_state_api::register());
        regs.extend(super::editor_io_api::register());
        regs.extend(super::editor_view_api::register());
        regs.extend(super::messages_api::register());
        regs.extend(super::lsp_api::register());
        regs.extend(super::lsp_edit_api::register());
        regs.extend(super::vc_api::register());
        regs.extend(super::process_api::register());
        regs.extend(super::project_api::register());
        regs.extend(super::project_registry_api::register());
        regs.extend(super::workspace_api::register());
        regs.extend(super::face_api::register());
        regs.extend(super::treesitter_api::register());
        regs.extend(super::semantic_api::register());
        regs.extend(super::display_api::register());
        regs.extend(super::layout_api::register());
        regs.extend(super::mode_api::register());
        regs.extend(super::ecosystem_api::register());
        regs.extend(super::font_api::register());
        regs.extend(super::ui_api::register());
        regs.extend(super::overlay_api::register());
        regs.extend(super::gutter_api::register());
        regs.extend(super::decoration_api::register());
        regs.extend(super::modality_api::register());
        regs.extend(super::eval_api::register());
        regs.extend(super::fs_api::register());
        regs.extend(super::net_http_api::register());
        regs.extend(super::net_tcp_api::register());
        regs.extend(super::minibuffer_api::register());
        regs.extend(super::selection_api::register());
        regs.extend(super::register_api::register());
        regs.extend(super::option_api::register());
        regs.extend(super::plugin_state_api::register());
        regs.extend(super::search_api::register());
        regs.extend(super::clipboard_api::register());
        regs.extend(super::task_api::register());
        regs.extend(super::debug_api::register());
        regs.extend(super::scheduler_api::register());
        regs.extend(super::extension_api::register());
        regs.push(evil_janet::JanetReg {
            name: std::ptr::null(),
            cfun: None,
            documentation: std::ptr::null(),
        });
        regs
    };

    unsafe {
        janet_cfuns(env, c"".as_ptr() as *const _, all_regs.as_ptr());
    }

    load_builtin("builtins/vim.janet",             include_str!("../../../builtins/vim.janet"));
    load_builtin("builtins/vim_keybindings.janet", include_str!("../../../builtins/vim_keybindings.janet"));
    load_builtin("builtins/init.janet",            include_str!("../../../builtins/init.janet"));
    load_builtin("builtins/magma_utils.janet",     include_str!("../../../builtins/magma_utils.janet"));
    load_builtin("builtins/scroll_commands.janet", include_str!("../../../builtins/scroll_commands.janet"));
    load_builtin("builtins/indent_commands.janet", include_str!("../../../builtins/indent_commands.janet"));
    load_builtin("builtins/colon_mode.janet",      include_str!("../../../builtins/colon_mode.janet"));
    load_builtin("builtins/completion_mode.janet", include_str!("../../../builtins/completion_mode.janet"));
    load_builtin("builtins/multi_cursor.janet",    include_str!("../../../builtins/multi_cursor.janet"));
    load_builtin("builtins/major_modes.janet",     include_str!("../../../builtins/major_modes.janet"));
    load_builtin("builtins/plugin_loader.janet",   include_str!("../../../builtins/plugin_loader.janet"));
    load_builtin("builtins/syntax.janet",          include_str!("../../../builtins/syntax.janet"));
    load_builtin("builtins/special_buffers.janet", include_str!("../../../builtins/special_buffers.janet"));
    load_builtin("builtins/help.janet",            include_str!("../../../builtins/help.janet"));
    load_builtin("builtins/plugins/lsp.janet",          include_str!("../../../builtins/plugins/lsp.janet"));
    load_builtin("builtins/plugins/tags_gen.janet",     include_str!("../../../builtins/plugins/tags_gen.janet"));
    load_builtin("builtins/plugins/tags_nav.janet",     include_str!("../../../builtins/plugins/tags_nav.janet"));
    load_builtin("builtins/project.janet",         include_str!("../../../builtins/project.janet"));
    load_builtin("builtins/treesitter.janet",      include_str!("../../../builtins/treesitter.janet"));
    load_builtin("builtins/window.janet",          include_str!("../../../builtins/window.janet"));
    load_builtin("builtins/ecosystem.janet",       include_str!("../../../builtins/ecosystem.janet"));
    load_builtin("builtins/which_key.janet",       include_str!("../../../builtins/which_key.janet"));
    load_builtin("builtins/grep.janet",            include_str!("../../../builtins/grep.janet"));
    load_builtin("builtins/fonts.janet",           include_str!("../../../builtins/fonts.janet"));
    load_builtin("builtins/modeline.janet",        include_str!("../../../builtins/modeline.janet"));
    load_builtin("builtins/tab_bar.janet",         include_str!("../../../builtins/tab_bar.janet"));
    load_builtin("builtins/scroll_policy.janet",   include_str!("../../../builtins/scroll_policy.janet"));
    load_builtin("builtins/ui.janet",              include_str!("../../../builtins/ui.janet"));
    load_builtin("builtins/dired_ui.janet",        include_str!("../../../builtins/dired_ui.janet"));
    load_builtin("builtins/lsp_inlay.janet",       include_str!("../../../builtins/lsp_inlay.janet"));
    load_builtin("builtins/git_blame.janet",       include_str!("../../../builtins/git_blame.janet"));
    load_builtin("builtins/lsp_diag_eol.janet",   include_str!("../../../builtins/lsp_diag_eol.janet"));
    load_builtin("builtins/gutter.janet",          include_str!("../../../builtins/gutter.janet"));
    load_builtin("builtins/plugins/breakpoints.janet",  include_str!("../../../builtins/plugins/breakpoints.janet"));
    load_builtin("builtins/plugins/vcs_gutter.janet",   include_str!("../../../builtins/plugins/vcs_gutter.janet"));
    load_builtin("builtins/janet_eval.janet",      include_str!("../../../builtins/janet_eval.janet"));
    load_builtin("builtins/mshell.janet",          include_str!("../../../builtins/mshell.janet"));
    load_builtin("builtins/net.janet",             include_str!("../../../builtins/net.janet"));
    load_builtin("builtins/collab.janet",          include_str!("../../../builtins/collab.janet"));
    load_builtin("builtins/ssh_fs.janet",          include_str!("../../../builtins/ssh_fs.janet"));
    load_builtin("builtins/task.janet",            include_str!("../../../builtins/task.janet"));
    load_builtin("builtins/debug.janet",           include_str!("../../../builtins/debug.janet"));
    load_builtin("builtins/workspace.janet",        include_str!("../../../builtins/workspace.janet"));
    load_builtin("builtins/scheduler.janet",        include_str!("../../../builtins/scheduler.janet"));
    load_builtin("builtins/extension.janet",        include_str!("../../../builtins/extension.janet"));

    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() {
        let user_init = format!("{}/.config/magma/init.janet", home);
        if std::path::Path::new(&user_init).exists() {
            match std::fs::read_to_string(&user_init) {
                Ok(source) => load_builtin(&user_init, &source),
                Err(e) => debug!("Janet: could not read user init {user_init}: {e}"),
            }
        }
    }

    debug!("Janet runtime ready");
}
