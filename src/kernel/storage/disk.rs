//! Disk-backed implementation of `FileSystem` and `AsyncFileSystem`.
//!
//! The implementation lives in the `fs` crate; this module keeps the
//! `storage::disk::…` path stable.

pub use fs::{DiskFileSystem, atomic_write};
