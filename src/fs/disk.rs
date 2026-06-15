//! Disk-backed implementation of `FileSystem` and `AsyncFileSystem`.

use std::path::Path;
use super::{FileSystem, AsyncFileSystem, FsError};

#[derive(Debug, Clone)]
pub struct DiskFileSystem;

impl Default for DiskFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskFileSystem {
    pub fn new() -> Self {
        DiskFileSystem
    }
}

impl FileSystem for DiskFileSystem {
    fn read(&self, path: &str) -> Result<String, FsError> {
        Ok(std::fs::read_to_string(path)?)
    }

    fn write(&self, path: &str, content: &str) -> Result<(), FsError> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(std::fs::write(path, content)?)
    }

    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn dir(&self, path: &str) -> Result<Vec<String>, FsError> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            entries.push(entry.file_name().to_string_lossy().to_string());
        }
        entries.sort();
        Ok(entries)
    }
}

impl AsyncFileSystem for DiskFileSystem {
    fn read_async(&self, path: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, FsError>> + Send>> {
        let path = path.to_string();
        Box::pin(async move {
            Ok(tokio::fs::read_to_string(&path).await?)
        })
    }

    fn write_async(&self, path: &str, content: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), FsError>> + Send>> {
        let path = path.to_string();
        let content = content.to_string();
        Box::pin(async move {
            if let Some(parent) = Path::new(&path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            Ok(tokio::fs::write(&path, &content).await?)
        })
    }

    fn exists_async(&self, path: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let path = path.to_string();
        Box::pin(async move { tokio::fs::try_exists(&path).await.unwrap_or(false) })
    }

    fn dir_async(&self, path: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>, FsError>> + Send>> {
        let path = path.to_string();
        Box::pin(async move {
            let mut entries = Vec::new();
            let mut read_dir = tokio::fs::read_dir(&path).await?;
            while let Some(entry) = read_dir.next_entry().await? {
                entries.push(entry.file_name().to_string_lossy().to_string());
            }
            entries.sort();
            Ok(entries)
        })
    }
}
