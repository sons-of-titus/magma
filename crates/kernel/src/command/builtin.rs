//! Built-in commands registered by the Rust core.
//!
//! This file is a thin coordinator that delegates to focused submodules in
//! `src/command/builtin/`.

pub mod buffer;
mod completion;
mod db_console;
mod db_grid;
mod db_navigate;
mod db_sources;
mod db_tree;
mod debug_cmds;
mod diagnostics_cmds;
mod editing;
mod file;
pub(crate) mod gui_cmds;
pub(crate) mod helpers;
mod indent_cmds;
mod lsp_cmds;
mod misc;
mod motion;
mod navigation;
mod project_cmds;
mod registers;
mod run_config_cmds;
mod scroll_cmds;
mod search;
pub mod search_query;
mod template_cmds;
pub(crate) mod vc_actions;
pub mod vc_blame;
pub mod vc_diff;
pub mod vc_hunks;
pub(crate) mod vc_ops;
pub mod vcs;
mod visual;
mod window;

use crate::state::Editor;

/// Register all built-in commands.
pub fn register_builtin_commands(editor: &mut Editor) {
    buffer::register(editor);
    buffer::find_files_register(&mut editor.commands);
    buffer::close_buffer_register(&mut editor.commands);
    navigation::register(editor);
    motion::register(editor);
    editing::register(editor);
    visual::register(editor);
    search::register(editor);
    registers::register(editor);
    window::register(editor);
    file::register(editor);
    vcs::register(editor);
    vc_actions::register(editor);
    vc_diff::register(editor);
    completion::register(editor);
    debug_cmds::register(editor);
    db_sources::register(editor);
    db_tree::register(editor);
    db_grid::register(editor);
    db_navigate::register(editor);
    db_console::register(editor);
    misc::register(editor);
    gui_cmds::register(editor);
    project_cmds::register(editor);
    run_config_cmds::register(editor);
    diagnostics_cmds::register(editor);
    lsp_cmds::register(editor);
    indent_cmds::register(editor);
    scroll_cmds::register(editor);
    template_cmds::register(editor);
}
