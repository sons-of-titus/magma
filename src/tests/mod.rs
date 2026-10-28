mod helpers;

macro_rules! test_pairs { ($($base:ident),+ $(,)?) => {
    $(
        paste::paste! {
            mod [<$base _tests>];
        }
    )+
}; }

mod anchor_tests;
mod command_tests;
mod completion_tests;
mod db_tests;
mod diag_version_tests;
mod document_property_tests;
mod file_reload_tests;
mod insertable_tests;
mod line_index_tests;
mod mode_tests;
mod project_tree_tests;
mod project_walk_tests;
mod terminal_manager_tests;
mod terminal_palette_tests;
mod terminal_tests;
mod text_layout_tests;
mod transaction_core_tests;
mod transaction_tests;
mod vc_backend_tests;
mod vc_diff_tests;

test_pairs!(
    extension_foundation,
    special_buffers,
    process_async,
    project_api,
    vc,
    lsp_feature,
    syntax_highlighting,
    treesitter,
    fold,
    display,
    window_management,
    ecosystem,
    render,
    font_rendering,
    ui_customization,
    buffer_decoration,
    gutter_provider,
    unified_input,
    namespace_refactor,
    net_api,
    magma_utils,
    view_tree,
    semantic_engine,
    task_system,
    project_model,
    debug_system,
    workspace_persist,
    concurrency_model,
    extension_capability,
    agent_system,
    language_ecosystem,
    mouse,
    quit,
    status_message,
    command_surface,
    run_config,
    session,
    e2e_scenarios
);

mod dap_event_tests;
mod editor_overlay_tests;
mod index_event_tests;
mod lsp_event_tests;
mod memory_budget_tests;
mod project_event_tests;
mod render_host_tests;
mod task_event_tests;
mod vc_event_tests;
mod viewport_scroll_tests;
