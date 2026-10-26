//! Project detection — a marker-file table, first match wins.  Detection is
//! cheap (existence checks only, no traversal) and drives run-config seeding
//! and debug adapter suggestions.

use std::path::Path;

/// `(language, marker files)` in priority order: a repo carrying both
/// `Cargo.toml` and `Makefile` is a Rust project.
const DETECTORS: &[(&str, &[&str])] = &[
    ("rust",   &["Cargo.toml"]),
    ("go",     &["go.mod"]),
    ("node",   &["package.json"]),
    ("make",   &["Makefile"]),
    ("python", &["pyproject.toml", "setup.py"]),
    ("cmake",  &["CMakeLists.txt"]),
    ("maven",  &["pom.xml"]),
    ("gradle", &["build.gradle", "build.gradle.kts"]),
];

pub fn detect_language(root: &Path) -> Option<&'static str> {
    DETECTORS.iter()
        .find(|(_, markers)| markers.iter().any(|m| root.join(m).exists()))
        .map(|(lang, _)| *lang)
}

/// Debug adapter candidates per detected language, in preference order —
/// the first one installed wins.
pub fn adapter_candidates(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["codelldb", "lldb-dap", "lldb-vscode"],
        "go" => &["dlv-dap", "dlv"],
        "python" => &["debugpy"],
        "node" => &["js-debug-adapter", "node"],
        "cmake" => &["codelldb", "lldb-dap"],
        _ => &[],
    }
}

/// The resolved path of the first candidate adapter actually installed for
/// `language`, through `toolchain::find_extended` — so packaging that hides the
/// binary from `$PATH` (Xcode's `lldb-dap`, keg-only Homebrew LLVM, CodeLLDB)
/// still counts.  The path is returned, not the bare name: a binary resolved
/// outside `$PATH` cannot be spawned by name.
pub fn find_adapter(language: &str) -> Option<std::path::PathBuf> {
    adapter_candidates(language).iter()
        .find_map(|candidate| crate::toolchain::find_extended(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_prefers_earlier_detectors() {
        let tmp = std::env::temp_dir().join("detect_priority");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "").unwrap();
        std::fs::write(tmp.join("Makefile"), "").unwrap();
        assert_eq!(detect_language(&tmp), Some("rust"));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn unknown_language_has_no_adapter() {
        assert!(adapter_candidates("cobol").is_empty());
        assert_eq!(find_adapter("cobol"), None);
    }

    #[test]
    fn rust_candidates_prefer_codelldb_then_lldb_dap() {
        assert_eq!(adapter_candidates("rust"), &["codelldb", "lldb-dap", "lldb-vscode"]);
    }

    #[test]
    fn every_detector_language_has_defaults() {
        for (lang, _) in DETECTORS {
            assert!(!run_config::defaults_for(lang).is_empty(),
                "{lang} is detected but has no default run configs");
        }
    }
}
