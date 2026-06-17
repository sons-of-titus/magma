mod helpers;

macro_rules! janet_test { ($ed:ident, $body:block) => {{
    let _lock = $crate::tests::helpers::acquire_janet_lock();
    let mut $ed = $crate::tests::helpers::make_editor();
    $crate::kernel::scripting::init(&mut $ed);
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
mod event_tests;
mod mode_tests;
mod insertable_tests;
mod terminal_tests;
mod vc_backend_tests;
mod janet_eval_tests;

test_pairs!(
    extension_foundation, special_buffers, process_async, project_api,
    vc, keymap, lsp_feature, syntax_highlighting, treesitter,
    fold, display, window_management, command_completion, command_popup,
    ecosystem, render, font_rendering, ui_customization,
    buffer_decoration, gutter_provider, unified_input, modality,
    namespace_refactor, net_api, magma_utils, view_tree,
    semantic_engine, task_system, project_model, debug_system,
    workspace_persist, concurrency_model, extension_capability
);

#[cfg(feature = "janet")] mod janet_tests;
#[cfg(feature = "janet")] mod render_integration_tests;
#[cfg(feature = "janet")] mod dispatch_operator_janet_tests;
