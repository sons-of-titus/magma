//! The on-disk index format: schema-versioned JSON lines, one file record
//! per line, written atomically (temp + rename).  A corrupted line is
//! skipped on load — the index rebuilds that file instead of failing.

use std::collections::HashMap;
use std::path::PathBuf;

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

/// Where the index for a project root lives, under the Magma data dir.
pub fn index_path_for_root(root: &str) -> PathBuf {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in root.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    crate::kernel::state::persist::state_dir()
        .join("index")
        .join(format!("{hash:016x}"))
        .join("index.jsonl")
}

/// Load every well-formed record.  A wrong schema version discards
/// everything (the caller re-indexes); a corrupted line only skips itself.
pub fn load(path: &std::path::Path) -> HashMap<String, FileRecord> {
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
pub fn save(path: &std::path::Path, records: &HashMap<String, FileRecord>) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_round_trips_and_is_versioned() {
        let dir = std::env::temp_dir().join(format!("magma-index-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.jsonl");

        let mut records = HashMap::new();
        records.insert("a.rs".into(), FileRecord {
            path: "a.rs".into(), mtime_secs: 1, size: 2, hash: 3,
            language: "rust".into(),
            symbols: vec![IndexedSymbol { name: "f".into(), kind: "fn".into(), line: 0 }],
        });
        save(&path, &records).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded, records);

        // Schema bump discards the old world instead of misreading it.
        let mixed = format!("{HEADER} v99\n{}\n", serde_json::to_string(&records["a.rs"]).unwrap());
        std::fs::write(&path, mixed).unwrap();
        assert!(load(&path).is_empty(), "a foreign schema version must not load");

        // A corrupted line skips itself; good lines survive.
        let good = serde_json::to_string(&records["a.rs"]).unwrap();
        std::fs::write(&path, format!("{HEADER} v{SCHEMA_VERSION}\n{{corrupt\n{good}\n")).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_loads_empty() {
        assert!(load(std::path::Path::new("/nonexistent/index.jsonl")).is_empty());
    }

    #[test]
    fn content_hash_is_stable_and_discriminating() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
    }
}
