/// Integration-level tests: command execution, modal editing, event bus.
/// dispatch_tests exercises the *full* key→resolve→execute→handle_text_input path,
/// which is the path that was broken when the vim layer leaked into insert mode.

mod command_tests;
mod event_tests;
mod mode_tests;
mod insertable_tests;
mod extension_foundation_tests;

#[cfg(feature = "janet")]
mod extension_foundation_janet_tests;

#[cfg(feature = "janet")]
mod janet_tests;

mod special_buffers_tests;

#[cfg(feature = "janet")]
mod special_buffers_janet_tests;

mod process_async_tests;

#[cfg(feature = "janet")]
mod process_async_janet_tests;

mod project_api_tests;

#[cfg(feature = "janet")]
mod project_api_janet_tests;

mod terminal_tests;

mod vc_tests;
mod vc_backend_tests;

#[cfg(feature = "janet")]
mod vc_janet_tests;

mod keymap_tests;

#[cfg(feature = "janet")]
mod keymap_janet_tests;

mod lsp_feature_tests;

#[cfg(feature = "janet")]
mod lsp_feature_janet_tests;

mod syntax_highlighting_tests;

#[cfg(feature = "janet")]
mod syntax_highlighting_janet_tests;

mod treesitter_tests;

#[cfg(feature = "janet")]
mod treesitter_janet_tests;

mod fold_tests;

#[cfg(feature = "janet")]
mod fold_janet_tests;

mod display_tests;

#[cfg(feature = "janet")]
mod display_janet_tests;

mod window_management_tests;

#[cfg(feature = "janet")]
mod window_management_janet_tests;

mod command_completion_tests;

#[cfg(feature = "janet")]
mod command_completion_janet_tests;

mod command_popup_tests;

#[cfg(feature = "janet")]
mod command_popup_janet_tests;

mod ecosystem_tests;

#[cfg(feature = "janet")]
mod ecosystem_janet_tests;

mod render_tests;

#[cfg(feature = "janet")]
mod render_janet_tests;

mod font_rendering_tests;

#[cfg(feature = "janet")]
mod font_rendering_janet_tests;

mod ui_customization_tests;

#[cfg(feature = "janet")]
mod ui_customization_janet_tests;

mod buffer_decoration_tests;

#[cfg(feature = "janet")]
mod buffer_decoration_janet_tests;

#[cfg(feature = "janet")]
mod render_integration_tests;

mod gutter_api_tests;

#[cfg(feature = "janet")]
mod gutter_api_janet_tests;

mod unified_input_tests;

#[cfg(feature = "janet")]
mod unified_input_janet_tests;

mod modality_tests;

#[cfg(feature = "janet")]
mod modality_janet_tests;

#[cfg(feature = "janet")]
mod dispatch_operator_janet_tests;

mod janet_eval_tests;

#[cfg(feature = "janet")]
mod janet_eval_janet_tests;

mod net_api_tests;

#[cfg(feature = "janet")]
mod net_api_janet_tests;

mod namespace_refactor_tests;

#[cfg(feature = "janet")]
mod namespace_refactor_janet_tests;
