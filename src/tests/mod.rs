mod helpers;

macro_rules! janet_test { ($ed:ident, $body:block) => {{
    let _lock = $crate::tests::helpers::acquire_janet_lock();
    let mut $ed = $crate::tests::helpers::make_editor();
    $crate::kernel::scripting::init(&mut $ed);
    // Every Janet C call in the body resolves the editor through the
    // scoped bridge (docs/core-redesign.md §11) — no stashed pointers.
    let _editor_scope = $crate::kernel::scripting::EditorScope::new(&mut $ed);
    $body
}}; }

macro_rules! test_pairs { ($($base:ident),+ $(,)?) => {
    $(
        paste::paste! {
            mod [<$base _tests>];
            #[cfg(feature = "janet")]
            mod [<$base _janet_tests>];
        }
    )+
}; }

mod command_tests;
mod completion_tests;
mod event_tests;
mod file_reload_tests;
mod mode_tests;
mod insertable_tests;
mod terminal_tests;
mod terminal_manager_tests;
mod terminal_palette_tests;
mod vc_backend_tests;
mod vc_diff_tests;
mod janet_eval_tests;
mod anchor_tests;
mod transaction_tests;
mod line_index_tests;
mod document_property_tests;
mod transaction_core_tests;
mod text_layout_tests;
mod correctness_gates_tests;
mod project_open_tests;
mod diag_version_tests;
#[cfg(feature = "gui")]
mod quickfix_tests;
#[cfg(feature = "gui")]
mod view_layout_tests;
#[cfg(feature = "gui")]
mod gui_repaint_tests;
#[cfg(feature = "gui")]
mod gui_welcome_tests;
#[cfg(feature = "gui")]
mod gui_chrome_tests;
#[cfg(feature = "gui")]
mod ime_tests;

test_pairs!(
    extension_foundation, special_buffers, process_async, project_api,
    vc, keymap, lsp_feature, syntax_highlighting, treesitter,
    fold, display, window_management,
    ecosystem, render, font_rendering, ui_customization,
    buffer_decoration, gutter_provider, unified_input,
    namespace_refactor, net_api, magma_utils, view_tree,
    semantic_engine, task_system, project_model, debug_system,
    workspace_persist, concurrency_model, extension_capability,
    agent_system, language_ecosystem, mouse, gui_overhaul, quit,
    status_message, command_surface, run_config, icon_pack, session, e2e_scenarios
);

#[cfg(feature = "janet")] mod janet_tests;
#[cfg(feature = "janet")] mod render_integration_tests;
#[cfg(feature = "janet")] mod keymap_guard_tests;
#[cfg(feature = "janet")] mod builtin_parse_tests;
mod scratch_probe_tests;
#[cfg(feature = "janet")]
mod perf_probe_tests;
mod memory_budget_tests;
mod viewport_scroll_tests;
