//! File system abstraction — sync and async traits plus a disk-backed
//! implementation.
//!
//! The traits and the disk implementation live in the `fs` crate.  This
//! module keeps the `storage::…` paths stable and hosts the editor-facing
//! storage subsystems that build on them.

pub use fs::{AsyncFileSystem, FileSystem, FsError};

pub mod crash_recovery;
pub mod disk;
pub mod reload;
pub mod watcher;
pub mod workspace;

pub use disk::DiskFileSystem;
pub use workspace::{WorkspaceManager, WorkspaceState, PersistedBuffer};
