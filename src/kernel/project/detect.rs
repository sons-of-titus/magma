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
/// the first one found on PATH is used.
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
    fn every_detector_language_has_defaults() {
        for (lang, _) in DETECTORS {
            assert!(!crate::kernel::run_config::defaults_for(lang).is_empty(),
                "{lang} is detected but has no default run configs");
        }
    }
}
