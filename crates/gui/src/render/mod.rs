//! The graphical shell: chrome widgets, input translation, and the editor
//! pane paint path.
//!
//! The kernel's render modules are re-exported here so sibling modules keep
//! their original `super::frame` / `super::surface` / `super::gpu_atlas`
//! paths, exactly as they did inside the kernel.

pub use magma::kernel::render::{
    decorations, editor_view, frame, gpu_atlas, gutter, gutter_providers, host, row_layout,
    surface, tabs, view, view_tree,
};

pub use magma::kernel::render::CursorPaint;

pub mod gpu_paint_callback;
pub mod gui;
pub mod gui_agent_tool;
pub mod gui_breadcrumb;
pub mod gui_command_line;
pub mod gui_commit_actions;
pub mod gui_commit_sections;
pub mod gui_commit_tool;
pub mod gui_debug_tool;
pub mod gui_dialog;
pub mod gui_dock;
pub mod gui_editor_pane;
pub mod gui_editor_window;
pub mod gui_find_tool;
pub mod gui_fonts;
pub mod gui_fuzzy;
pub mod gui_goto_dialogs;
pub mod gui_icons;
pub mod gui_input;
pub mod gui_layout;
pub mod gui_menu;
pub mod gui_minimap;
pub mod gui_output_tool;
pub mod gui_render;
pub mod gui_run_config_dialog;
pub mod gui_run_tool;
pub mod gui_search_dialog;
pub mod gui_settings_dialog;
pub mod gui_shell;
pub mod gui_sidebar;
pub mod gui_sidebar_state;
pub mod gui_sidebar_tree;
pub mod gui_status;
pub mod gui_tabs;
pub mod gui_terminal_tool;
pub mod gui_theme;
pub mod gui_welcome;
pub mod gui_welcome_panes;
pub mod tool_window;
pub mod tool_window_view;
