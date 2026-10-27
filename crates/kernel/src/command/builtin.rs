//! Built-in commands registered by the Rust core.
//!
//! This file is a thin coordinator that delegates to focused submodules in
//! `src/command/builtin/`.

pub mod buffer;
mod navigation;
mod motion;
mod editing;
mod visual;
mod search;
pub mod search_query;
mod registers;
mod window;
mod file;
pub mod vcs;
pub(crate) mod vc_ops;
pub(crate) mod vc_actions;
pub mod vc_diff;
pub(crate) mod vc_hunks;
mod completion;
mod misc;
mod project_cmds;
mod run_config_cmds;
pub(crate) mod helpers;

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
    misc::register(editor);
    project_cmds::register(editor);
    run_config_cmds::register(editor);
}
