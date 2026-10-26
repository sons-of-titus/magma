//! The project symbol index — persistent, incremental, revision-of-truth
//! by content hash (rework.md §5).
//!
//! - **Persistent**: records survive restarts under the Magma state dir,
//!   one file per project root.
//! - **Incremental**: a file re-indexes only when its content hash (or its
//!   size/mtime fast path) changes.
//! - **Crash-safe**: JSON lines written via temp+rename; corrupted lines
//!   and foreign schema versions are discarded on load.
//! - **Observable**: `stats()` exposes files/symbols/queue depth.
//!
//! The extractor, the record shape, and the JSON-lines format live in the
//! `index` crate.  The store lives on the foreground `Editor`; background
//! walk/extract jobs send typed `IndexRecord` events that are merged here.

pub use index::{Event, IndexRecord, IndexStats, extract, record_from_text, store};

use std::collections::HashMap;

use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::state::Editor;
use extract::IndexedSymbol;

/// Reconcile the persistent symbol index with a fresh project walk: keep
/// only surviving paths, then re-extract the files whose size/mtime stamps
/// no longer match their record.  Extraction runs chunked on the blocking
/// pool; records land as `index::Event::RecordArrived` and merge on the foreground.
pub fn reconcile_project_index(ed: &mut Editor, files: &[String]) {
    use std::time::UNIX_EPOCH;
    let Some(root) = ed.project_manager.project.root.clone() else { return };
    ed.index.open_project(&root.to_string_lossy());
    let keep: std::collections::HashSet<String> = files.iter().cloned().collect();
    ed.index.retain_paths(&keep);

    let stale: Vec<String> = files.iter().filter(|rel| {
        let abs = root.join(rel.as_str());
        match std::fs::metadata(&abs) {
            Ok(meta) => {
                let mtime = meta.modified().ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                !ed.index.is_current(rel, meta.len(), mtime)
            }
            Err(_) => false,
        }
    }).cloned().collect();

    if stale.is_empty() {
        return;
    }
    let Some(bg) = ed.background.clone() else { return };
    ed.index.note_pending(stale.len());
    for chunk in stale.chunks(64) {
        let chunk = chunk.to_vec();
        let root = root.clone();
        let sender = bg.sender.clone();
        // Lane P2, coalesced per reconcile round: a newer walk supersedes
        // extraction jobs still queued from the previous one.
        bg.spawn_on(crate::kernel::runtime::Lane::Indexing, Some("index-reconcile"), move || {
            for rel in chunk {
                let abs = root.join(&rel);
                let Ok(meta) = std::fs::metadata(&abs) else { continue };
                // Non-UTF8 files (binaries, exotic encodings) are skipped:
                // the index is a source index.
                let Ok(text) = std::fs::read_to_string(&abs) else { continue };
                let mtime = meta.modified().ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let rec = crate::kernel::index::record_from_text(&rel, mtime, &text);
                let _ = sender.send(BackgroundEvent::Index(
                    crate::kernel::index::Event::RecordArrived(rec)));
            }
        });
    }
}

#[derive(Debug, Default)]
pub struct IndexStore {
    records: HashMap<String, store::FileRecord>,
    /// Files discovered but not yet indexed (walk → extract pipeline).
    pending: usize,
    dirty: bool,
    loaded_root: Option<String>,
}

impl IndexStore {
    /// Load the persisted index for a project root (no-op when the same
    /// root is already loaded).
    pub fn open_project(&mut self, root: &str) {
        if self.loaded_root.as_deref() == Some(root) {
            return;
        }
        self.records = store::load(&store::index_path_for_root(root, &crate::kernel::state::persist::state_dir()));
        self.loaded_root = Some(root.to_string());
        self.dirty = false;
        self.pending = 0;
    }

    /// Merge a fresh per-file record.  Returns true when the content was
    /// new (not a no-op re-stamp).
    pub fn apply(&mut self, rec: IndexRecord) -> bool {
        self.pending = self.pending.saturating_sub(1);
        let unchanged = self.records.get(&rec.path)
            .map(|existing| existing.hash == rec.hash)
            .unwrap_or(false);
        if unchanged {
            return false;
        }
        self.records.insert(rec.path.clone(), store::FileRecord {
            path: rec.path,
            mtime_secs: rec.mtime_secs,
            size: rec.size,
            hash: rec.hash,
            language: rec.language,
            symbols: rec.symbols,
        });
        self.dirty = true;
        true
    }

    /// Drop records for paths no longer present in the project walk.
    pub fn retain_paths(&mut self, keep: &std::collections::HashSet<String>) {
        let before = self.records.len();
        self.records.retain(|path, _| keep.contains(path));
        if self.records.len() != before {
            self.dirty = true;
        }
    }

    pub fn note_pending(&mut self, n: usize) {
        self.pending += n;
    }

    /// All symbols whose name starts with `prefix` — workspace symbol
    /// search / completion augmentation.
    pub fn symbols_with_prefix(&self, prefix: &str) -> Vec<(String, &IndexedSymbol)> {
        let mut out = Vec::new();
        if prefix.is_empty() {
            return out;
        }
        for rec in self.records.values() {
            for sym in &rec.symbols {
                if sym.name.starts_with(prefix) {
                    out.push((rec.path.clone(), sym));
                    if out.len() >= 200 {
                        return out;
                    }
                }
            }
        }
        out.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        out
    }

    /// Symbols defined in one file.
    pub fn symbols_in_file(&self, path: &str) -> &[IndexedSymbol] {
        static EMPTY: [IndexedSymbol; 0] = [];
        self.records.get(path).map(|r| r.symbols.as_slice()).unwrap_or(&EMPTY)
    }

    pub fn stats(&self) -> IndexStats {
        IndexStats {
            files: self.records.len(),
            symbols: self.records.values().map(|r| r.symbols.len()).sum(),
            pending: self.pending,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Persist when dirty; clears the flag.
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        let Some(root) = &self.loaded_root else { return Ok(()) };
        let path = store::index_path_for_root(root, &crate::kernel::state::persist::state_dir());
        let result = store::save(&path, &self.records);
        if result.is_ok() {
            self.dirty = false;
        }
        result
    }

    /// Whether a file's stamps say its record is current.
    pub fn is_current(&self, path: &str, size: u64, mtime_secs: u64) -> bool {
        self.records.get(path)
            .map(|r| r.size == size && r.mtime_secs == mtime_secs)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(path: &str, name: &str, hash: u64) -> IndexRecord {
        IndexRecord {
            path: path.into(),
            mtime_secs: 1,
            size: 10,
            hash,
            language: "rust".into(),
            symbols: vec![IndexedSymbol { name: name.into(), kind: "fn".into(), line: 0 }],
        }
    }

    #[test]
    fn apply_merges_and_dedupes_by_hash() {
        let mut idx = IndexStore::default();
        assert!(idx.apply(rec("a.rs", "alpha", 1)), "first record is new");
        assert!(!idx.apply(rec("a.rs", "alpha", 1)), "same hash is a no-op re-stamp");
        assert!(idx.apply(rec("a.rs", "alpha", 2)), "changed hash re-indexes");
        assert_eq!(idx.stats().files, 1);
        assert_eq!(idx.stats().symbols, 1);
    }

    #[test]
    fn prefix_query_and_retention() {
        let mut idx = IndexStore::default();
        idx.apply(rec("a.rs", "alpha", 1));
        idx.apply(rec("b.rs", "alphabet", 1));
        let hits = idx.symbols_with_prefix("alph");
        assert_eq!(hits.len(), 2);

        let mut keep = std::collections::HashSet::new();
        keep.insert("a.rs".to_string());
        idx.retain_paths(&keep);
        assert_eq!(idx.stats().files, 1);
        assert!(idx.is_dirty());
    }

    #[test]
    fn flush_without_project_is_a_clean_noop() {
        let mut idx = IndexStore::default();
        idx.apply(rec("a.rs", "alpha", 1));
        idx.flush().unwrap();
    }
}
