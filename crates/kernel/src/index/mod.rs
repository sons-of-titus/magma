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

mod query;

pub use index::{Event, IndexRecord, IndexStats, extract, record_from_text, store};

use std::collections::HashMap;

use crate::runtime::BackgroundEvent;
use crate::state::Editor;
use extract::IndexedSymbol;

/// Reconcile the persistent symbol index with a fresh project walk: keep
/// only surviving paths, then scan the walk's files for re-extraction.  The
/// size/mtime stamp scan runs on the blocking pool (one `stat` per file —
/// thousands on a large project); stamps come back as
/// `index::Event::StampScan` and [`apply_stamp_scan`] does the foreground
/// diff and chunked extraction.
pub fn reconcile_project_index(ed: &mut Editor, files: &[String], generation: u64) {
    let Some(root) = ed.project_manager.project.root.clone() else {
        return;
    };
    ed.index.open_project(&root.to_string_lossy());
    let keep: std::collections::HashSet<String> = files.iter().cloned().collect();
    ed.index.retain_paths(&keep);

    let Some(bg) = ed.background.clone() else {
        return;
    };
    let rels: Vec<String> = files.to_vec();
    let sender = bg.sender.clone();
    // Coalesced: a newer walk supersedes a scan still queued from the
    // previous one.
    bg.spawn_on(
        crate::runtime::Lane::Indexing,
        Some("index-scan"),
        move || {
            use std::time::UNIX_EPOCH;
            let stamps: Vec<(String, u64, u64)> = rels
                .iter()
                .filter_map(|rel| {
                    let meta = std::fs::metadata(root.join(rel)).ok()?;
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    Some((rel.clone(), meta.len(), mtime))
                })
                .collect();
            let _ = sender.send(BackgroundEvent::Index(crate::index::Event::StampScan {
                generation,
                stamps,
            }));
        },
    );
}

/// Foreground half of the reconcile: diff a scan's stamps against the store
/// and enqueue extraction for stale files.  Scans from a superseded project
/// generation are dropped.
pub fn apply_stamp_scan(ed: &mut Editor, generation: u64, stamps: Vec<(String, u64, u64)>) {
    use std::time::UNIX_EPOCH;

    if generation != ed.project_manager.project.generation {
        return;
    }
    let stale: Vec<String> = stamps
        .into_iter()
        .filter(|(rel, size, mtime)| !ed.index.is_current(rel, *size, *mtime))
        .map(|(rel, _, _)| rel)
        .collect();
    if stale.is_empty() {
        return;
    }
    let Some(bg) = ed.background.clone() else {
        return;
    };
    ed.index.note_pending(stale.len());
    let root = ed.project_manager.project.root.clone().unwrap_or_default();
    for chunk in stale.chunks(64) {
        let chunk = chunk.to_vec();
        let root = root.clone();
        let sender = bg.sender.clone();
        // Lane P2, deliberately uncoalesced: coalescing is keyed, so a key
        // shared by every chunk would let all but the newest skip itself and
        // strand its files' queue slots forever.  Repeat extraction is
        // idempotent — `apply` dedups by content hash — and a later
        // generation's handler drops the results.
        bg.spawn_on(crate::runtime::Lane::Indexing, None, move || {
            for rel in chunk {
                let abs = root.join(&rel);
                let Ok(meta) = std::fs::metadata(&abs) else {
                    let _ =
                        sender.send(BackgroundEvent::Index(crate::index::Event::RecordSkipped {
                            generation,
                        }));
                    continue;
                };
                // Non-UTF8 files (binaries, exotic encodings) carry no
                // symbols — the index is a source index — but their queue
                // slot must still drain.
                let Ok(text) = std::fs::read_to_string(&abs) else {
                    let _ =
                        sender.send(BackgroundEvent::Index(crate::index::Event::RecordSkipped {
                            generation,
                        }));
                    continue;
                };
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let rec = crate::index::record_from_text(&rel, mtime, &text);
                let _ = sender.send(BackgroundEvent::Index(crate::index::Event::RecordArrived {
                    generation,
                    record: rec,
                }));
            }
        });
    }
}

/// Re-extract one saved file's symbols off the save path — O(file), no
/// project walk.  The existing generation guards and hash-deduped `apply`
/// make repeat saves free; files outside the project are ignored (their
/// discovery still flows through the watcher-driven walk).
pub fn reindex_saved_file(ed: &mut Editor, path: &str) {
    let Some(root) = ed.project_manager.project.root.clone() else {
        return;
    };
    let Ok(root) = std::fs::canonicalize(&root) else {
        return;
    };
    let Ok(abs) = std::fs::canonicalize(path) else {
        return;
    };
    let Some(rel) = abs.strip_prefix(&root).ok().and_then(|p| p.to_str()) else {
        return;
    };
    let Some(bg) = ed.background.clone() else {
        return;
    };
    let generation = ed.project_manager.project.generation;
    ed.index.note_pending(1);
    let sender = bg.sender.clone();
    let abs = abs.to_string_lossy().into_owned();
    let rel = rel.to_string();
    // Coalesced: a burst of saves leaves only the newest extraction queued.
    bg.spawn_on(
        crate::runtime::Lane::Indexing,
        Some("index-save"),
        move || {
            let Ok(meta) = std::fs::metadata(&abs) else {
                return;
            };
            // Non-UTF8 files are skipped: the index is a source index.
            let Ok(text) = std::fs::read_to_string(&abs) else {
                return;
            };
            use std::time::UNIX_EPOCH;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let record = crate::index::record_from_text(&rel, mtime, &text);
            let _ = sender.send(BackgroundEvent::Index(crate::index::Event::RecordArrived {
                generation,
                record,
            }));
        },
    );
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
        self.records = store::load(&store::index_path_for_root(
            root,
            &crate::state::persist::state_dir(),
        ));
        self.loaded_root = Some(root.to_string());
        self.dirty = false;
        self.pending = 0;
    }

    /// Merge a fresh per-file record.  Returns true when the content was
    /// new (not a no-op re-stamp).
    pub fn apply(&mut self, rec: IndexRecord) -> bool {
        self.pending = self.pending.saturating_sub(1);
        let unchanged = self
            .records
            .get(&rec.path)
            .map(|existing| existing.hash == rec.hash)
            .unwrap_or(false);
        if unchanged {
            return false;
        }
        self.records.insert(
            rec.path.clone(),
            store::FileRecord {
                path: rec.path,
                mtime_secs: rec.mtime_secs,
                size: rec.size,
                hash: rec.hash,
                language: rec.language,
                symbols: rec.symbols,
            },
        );
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

    /// Drain one queue slot for a counted file that produced no record.
    pub fn note_skipped(&mut self) {
        self.pending = self.pending.saturating_sub(1);
    }

    /// Drop all outstanding extraction slots.  A generation bump invalidates
    /// them: their arrivals will be dropped as stale, so they must not keep
    /// the reported queue depth above zero.
    pub fn clear_pending(&mut self) {
        self.pending = 0;
    }

    /// Symbols defined in one file.
    pub fn symbols_in_file(&self, path: &str) -> &[IndexedSymbol] {
        static EMPTY: [IndexedSymbol; 0] = [];
        self.records
            .get(path)
            .map(|r| r.symbols.as_slice())
            .unwrap_or(&EMPTY)
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
        let Some(root) = &self.loaded_root else {
            return Ok(());
        };
        let path = store::index_path_for_root(root, &crate::state::persist::state_dir());
        let result = store::save(&path, &self.records);
        if result.is_ok() {
            self.dirty = false;
        }
        result
    }

    /// Whether a file's stamps say its record is current.
    pub fn is_current(&self, path: &str, size: u64, mtime_secs: u64) -> bool {
        self.records
            .get(path)
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
            symbols: vec![IndexedSymbol {
                name: name.into(),
                kind: "fn".into(),
                line: 0,
            }],
        }
    }

    #[test]
    fn apply_merges_and_dedupes_by_hash() {
        let mut idx = IndexStore::default();
        assert!(idx.apply(rec("a.rs", "alpha", 1)), "first record is new");
        assert!(
            !idx.apply(rec("a.rs", "alpha", 1)),
            "same hash is a no-op re-stamp"
        );
        assert!(
            idx.apply(rec("a.rs", "alpha", 2)),
            "changed hash re-indexes"
        );
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
