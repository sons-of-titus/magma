//! Project-level checks that do not need an editor: language detection from
//! marker files, `.gitignore` matching for the project walk, and toolchain
//! binary discovery.
//!
//! `Project`, `ProjectManager`, and the workspace walk itself live in the
//! editor — they hold state and run Janet handlers.

pub mod detect;
pub mod gitignore;
pub mod toolchain;
