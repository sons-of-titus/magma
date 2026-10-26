//! The project symbol index — the persistent, incremental record store and
//! the text-level extractor that fills it.
//!
//! - **Persistent**: records survive restarts, one file per project root
//!   under the editor's state directory.
//! - **Incremental**: a file re-indexes only when its content hash (or its
//!   size/mtime fast path) changes.
//! - **Crash-safe**: JSON lines written via temp+rename; corrupted lines
//!   and foreign schema versions are discarded on load.
//!
//! The crate owns the format and the extraction rules only.  Driving the
//! background walk, deciding when a file is stale, and merging results onto
//! the foreground live in the editor.

#[cfg(test)]
mod tests;

pub mod extract;
pub mod store;

pub use extract::IndexedSymbol;

/// A completed per-file index result (also the runtime event payload shape).
#[derive(Debug, Clone)]
pub struct IndexRecord {
    pub path: String,
    pub mtime_secs: u64,
    pub size: u64,
    pub hash: u64,
    pub language: String,
    pub symbols: Vec<IndexedSymbol>,
}

/// Extract an IndexRecord from file content.
pub fn record_from_text(path: &str, mtime_secs: u64, text: &str) -> IndexRecord {
    let language = path.rsplit('.').next().unwrap_or("").to_string();
    IndexRecord {
        path: path.to_string(),
        mtime_secs,
        size: text.len() as u64,
        hash: store::content_hash(text),
        language,
        symbols: extract::extract(path, text),
    }
}

/// Query statistics for the status surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexStats {
    pub files: usize,
    pub symbols: usize,
    pub pending: usize,
}
