//! The on-disk index format: schema-versioned JSON lines, one file record
//! per line, written atomically (temp + rename).  A corrupted line is
//! skipped on load — the index rebuilds that file instead of failing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::extract::IndexedSymbol;

pub const SCHEMA_VERSION: u32 = 1;
const HEADER: &str = "MAGMA-INDEX";

/// One indexed file: staleness stamps plus its symbols.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub mtime_secs: u64,
    pub size: u64,
    /// FNV-1a 64 of the content — the authoritative staleness stamp.
    pub hash: u64,
    pub language: String,
    pub symbols: Vec<IndexedSymbol>,
}

/// Where the index for a project root lives, under the given state dir.
pub fn index_path_for_root(root: &str, state_dir: &Path) -> PathBuf {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in root.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    state_dir
        .join("index")
        .join(format!("{hash:016x}"))
        .join("index.jsonl")
}

/// Load every well-formed record.  A wrong schema version discards
/// everything (the caller re-indexes); a corrupted line only skips itself.
pub fn load(path: &Path) -> HashMap<String, FileRecord> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut lines = text.lines();
    let header_ok = lines.next()
        .and_then(|l| l.strip_prefix(HEADER))
        .and_then(|rest| rest.trim().strip_prefix("v"))
        .and_then(|v| v.parse::<u32>().ok())
        .map(|v| v == SCHEMA_VERSION)
        .unwrap_or(false);
    if !header_ok {
        return HashMap::new();
    }
    let mut out = HashMap::new();
    for line in lines {
        match serde_json::from_str::<FileRecord>(line) {
            Ok(rec) => {
                out.insert(rec.path.clone(), rec);
            }
            Err(_) => continue,
        }
    }
    out
}

/// Persist atomically.  A failed write leaves the previous index intact.
pub fn save(path: &Path, records: &HashMap<String, FileRecord>) -> Result<(), String> {
    let mut text = format!("{HEADER} v{SCHEMA_VERSION}\n");
    let mut recs: Vec<&FileRecord> = records.values().collect();
    recs.sort_by(|a, b| a.path.cmp(&b.path));
    for rec in recs {
        let line = serde_json::to_string(rec).map_err(|e| e.to_string())?;
        text.push_str(&line);
        text.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("jsonl.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// FNV-1a 64 content hash — the file stamp.
pub fn content_hash(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
