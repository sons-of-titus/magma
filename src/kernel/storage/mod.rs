//! File system abstraction — sync and async traits plus a disk-backed implementation.

#[derive(Debug)]
pub enum FsError {
    NotFound(String),
    IoError(String),
    PermissionDenied(String),
}

impl std::fmt::Display for FsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsError::NotFound(p) => write!(f, "Not found: {}", p),
            FsError::IoError(e) => write!(f, "IO error: {}", e),
            FsError::PermissionDenied(p) => write!(f, "Permission denied: {}", p),
        }
    }
}

impl std::error::Error for FsError {}

impl From<std::io::Error> for FsError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => FsError::NotFound(e.to_string()),
            std::io::ErrorKind::PermissionDenied => FsError::PermissionDenied(e.to_string()),
            _ => FsError::IoError(e.to_string()),
        }
    }
}

pub trait FileSystem: Send + Sync {
    fn read(&self, path: &str) -> Result<String, FsError>;
    fn write(&self, path: &str, content: &str) -> Result<(), FsError>;
    fn exists(&self, path: &str) -> bool;
    fn dir(&self, path: &str) -> Result<Vec<String>, FsError>;
}

/// Async versions of file system operations, using tokio::fs.
/// Spawned on the tokio runtime to avoid blocking the main thread.
pub trait AsyncFileSystem: Send + Sync {
    fn read_async(&self, path: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, FsError>> + Send>>;
    fn write_async(&self, path: &str, content: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), FsError>> + Send>>;
    fn exists_async(&self, path: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>;
    fn dir_async(&self, path: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>, FsError>> + Send>>;
}

pub mod disk;
pub mod watcher;
pub mod workspace;

pub use disk::DiskFileSystem;
pub use workspace::{WorkspaceManager, WorkspaceState, PersistedBuffer};
