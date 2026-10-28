use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("index-{tag}-{}-{nanos}", std::process::id()))
}

fn record(path: &str) -> store::FileRecord {
    store::FileRecord {
        path: path.into(),
        mtime_secs: 1,
        size: 2,
        hash: 3,
        language: "rust".into(),
        symbols: vec![IndexedSymbol {
            name: "f".into(),
            kind: "fn".into(),
            line: 0,
        }],
    }
}

// ── Extraction ───────────────────────────────────────────────────────────────

#[test]
fn rust_declarations_are_indexed() {
    let src = "mod inner;\nfn alpha() {}\npub fn beta(x: u32) -> u32 { x }\nstruct Gamma { a: u32 }\n// fn commented_out() {}\nlet fn_in_let = 1;";
    let syms = extract::extract("lib.rs", src);
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["inner", "alpha", "beta", "Gamma"]);
    assert_eq!(syms[0].kind, "mod");
    assert_eq!(syms[1].line, 1);
}

#[test]
fn python_and_janet_families() {
    let py = extract::extract(
        "app.py",
        "class Thing:\n    def method(self):\n        pass\n# def hidden()\n",
    );
    assert_eq!(
        py.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["Thing", "method"]
    );

    let jl = extract::extract(
        "plugin.janet",
        "(defn- helper [x] x)\n(def value 1)\n# (defn commented [])\n",
    );
    assert!(jl.iter().any(|s| s.name == "helper"));
    assert!(jl.iter().any(|s| s.name == "value"));
    assert!(!jl.iter().any(|s| s.name == "commented"));
}

#[test]
fn unknown_languages_index_nothing() {
    assert!(extract::extract("data.csv", "fn looks_like,a,b\n").is_empty());
}

#[test]
fn numbers_and_garbage_are_rejected() {
    let src = "fn 123() {}\nfn (weird) {}\nstruct ____ok {}";
    let syms = extract::extract("x.rs", src);
    assert_eq!(
        syms.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["weird", "____ok"]
    );
}

// ── Store ────────────────────────────────────────────────────────────────────

#[test]
fn save_then_load_round_trips_and_is_versioned() {
    let dir = temp_dir("roundtrip");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("index.jsonl");

    let mut records = HashMap::new();
    records.insert("a.rs".into(), record("a.rs"));
    store::save(&path, &records).unwrap();
    assert_eq!(store::load(&path), records);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_creates_missing_parent_directories_and_sorts_records() {
    let dir = temp_dir("nested");
    let path = dir.join("deep").join("index.jsonl");
    let mut records = HashMap::new();
    records.insert("b.rs".into(), record("b.rs"));
    records.insert("a.rs".into(), record("a.rs"));

    store::save(&path, &records).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = written.lines().collect();
    assert!(lines[1].contains("\"a.rs\""));
    assert!(
        lines[2].contains("\"b.rs\""),
        "records are written in path order"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_foreign_schema_version_discards_everything() {
    let dir = temp_dir("version");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("index.jsonl");
    let good = serde_json::to_string(&record("a.rs")).unwrap();
    std::fs::write(&path, format!("MAGMA-INDEX v99\n{good}\n")).unwrap();

    assert!(
        store::load(&path).is_empty(),
        "a foreign schema version must not load"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_corrupted_line_skips_itself_and_good_lines_survive() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("index.jsonl");
    let good = serde_json::to_string(&record("a.rs")).unwrap();
    std::fs::write(&path, format!("MAGMA-INDEX v1\n{{corrupt\n{good}\n")).unwrap();

    let loaded = store::load(&path);
    assert_eq!(loaded.len(), 1);
    assert!(loaded.contains_key("a.rs"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_without_a_header_loads_empty() {
    let dir = temp_dir("noheader");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("index.jsonl");
    std::fs::write(&path, serde_json::to_string(&record("a.rs")).unwrap()).unwrap();

    assert!(store::load(&path).is_empty());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_file_loads_empty() {
    assert!(store::load(Path::new("/nonexistent/index.jsonl")).is_empty());
}

#[test]
fn content_hash_is_stable_and_discriminating() {
    assert_eq!(store::content_hash("abc"), store::content_hash("abc"));
    assert_ne!(store::content_hash("abc"), store::content_hash("abd"));
}

#[test]
fn the_index_path_is_per_root_and_uses_the_given_state_dir() {
    let state = Path::new("/state");
    let a = store::index_path_for_root("/project/a", state);
    let b = store::index_path_for_root("/project/b", state);
    assert_ne!(a, b);
    assert!(a.starts_with("/state/index"));
    assert!(a.ends_with("index.jsonl"));

    let elsewhere = store::index_path_for_root("/project/a", Path::new("/other"));
    assert_ne!(a, elsewhere);
    assert_eq!(
        a.file_name(),
        elsewhere.file_name(),
        "root hash, not state dir, names the file"
    );
}

// ── Records and stats ────────────────────────────────────────────────────────

#[test]
fn record_from_text_stamps_size_hash_language_and_symbols() {
    let text = "fn alpha() {}\n";
    let rec = record_from_text("src/lib.rs", 42, text);
    assert_eq!(rec.path, "src/lib.rs");
    assert_eq!(rec.mtime_secs, 42);
    assert_eq!(rec.size, text.len() as u64);
    assert_eq!(rec.hash, store::content_hash(text));
    assert_eq!(rec.language, "rs");
    assert_eq!(
        rec.symbols,
        vec![IndexedSymbol {
            name: "alpha".into(),
            kind: "fn".into(),
            line: 0,
        }]
    );
}

#[test]
fn record_from_text_handles_a_path_without_an_extension() {
    let rec = record_from_text("Makefile", 0, "all:\n");
    assert_eq!(rec.language, "Makefile");
    assert!(rec.symbols.is_empty());
}

#[test]
fn stats_default_to_zero() {
    assert_eq!(
        IndexStats::default(),
        IndexStats {
            files: 0,
            symbols: 0,
            pending: 0
        }
    );
}

// ── Event vocabulary ─────────────────────────────────────────────────────────

#[test]
fn event_payloads_carry_the_walk_and_record() {
    let indexed = Event::ProjectIndexed {
        project_name: "demo".into(),
        files: vec!["src/main.rs".into()],
        ignored_files: vec![],
    };
    match indexed.clone() {
        Event::ProjectIndexed {
            project_name,
            files,
            ignored_files: _,
        } => {
            assert_eq!(project_name, "demo");
            assert_eq!(files, vec!["src/main.rs".to_string()]);
        }
        _ => panic!("wrong variant"),
    }

    let rec = record_from_text("src/main.rs", 7, "fn main() {}\n");
    match Event::RecordArrived(rec) {
        Event::RecordArrived(rec) => {
            assert_eq!(rec.path, "src/main.rs");
            assert_eq!(rec.symbols.len(), 1);
        }
        _ => panic!("wrong variant"),
    }
}
