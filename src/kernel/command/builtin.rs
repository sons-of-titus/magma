//! Built-in commands registered by the Rust core.
//!
//! This file is a thin coordinator that delegates to focused submodules in
//! `src/command/builtin/`.

mod buffer;
mod navigation;
mod motion;
mod editing;
mod visual;
mod search;
mod registers;
mod window;
mod file;
mod vcs;
mod completion;
mod misc;
mod colon;
pub(crate) mod helpers;

use crate::kernel::state::Editor;

/// Register all built-in commands.
pub fn register_builtin_commands(editor: &mut Editor) {
    buffer::register(editor);
    navigation::register(editor);
    motion::register(editor);
    editing::register(editor);
    visual::register(editor);
    search::register(editor);
    registers::register(editor);
    window::register(editor);
    file::register(editor);
    vcs::register(editor);
    completion::register(editor);
    misc::register(editor);
}
