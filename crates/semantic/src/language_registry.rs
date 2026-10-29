//! LanguageRegistry — tracks languages registered by plugins.
//!
//! Each language entry carries the plugin's asset paths:
//!   - `syntax.scm`     — tree-sitter query file
//!   - `config.janet`   — format, completion, and adapter settings

use std::collections::HashMap;

/// Metadata for a registered language.
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    pub name: String,
    /// Absolute path to the language's `syntax.scm` file, if provided.
    pub syntax_path: Option<String>,
    /// Absolute path to the language's `config.janet` file, if provided.
    pub config_path: Option<String>,
    /// Whether the owning plugin has been loaded.
    pub loaded: bool,
}

impl LanguageInfo {
    pub fn new(name: impl Into<String>) -> Self {
        LanguageInfo {
            name: name.into(),
            syntax_path: None,
            config_path: None,
            loaded: false,
        }
    }

    pub fn with_paths(mut self, syntax_path: Option<String>, config_path: Option<String>) -> Self {
        self.syntax_path = syntax_path;
        self.config_path = config_path;
        self
    }
}

/// Registry of languages registered by plugins.
#[derive(Default)]
pub struct LanguageRegistry {
    languages: HashMap<String, LanguageInfo>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register or replace a language entry.
    pub fn register(&mut self, info: LanguageInfo) {
        self.languages.insert(info.name.clone(), info);
    }

    /// Remove a language from the registry.  Returns true if it existed.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.languages.remove(name).is_some()
    }

    /// Look up a language by name.
    pub fn get(&self, name: &str) -> Option<&LanguageInfo> {
        self.languages.get(name)
    }

    /// Sorted list of all registered languages.
    pub fn list(&self) -> Vec<&LanguageInfo> {
        let mut v: Vec<&LanguageInfo> = self.languages.values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Mark a language's owning plugin as loaded.
    pub fn mark_loaded(&mut self, name: &str) {
        if let Some(info) = self.languages.get_mut(name) {
            info.loaded = true;
        }
    }

    /// True if the named language is registered and its plugin is loaded.
    pub fn is_loaded(&self, name: &str) -> bool {
        self.languages.get(name).map(|i| i.loaded).unwrap_or(false)
    }

    /// Total number of registered languages.
    pub fn count(&self) -> usize {
        self.languages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> LanguageRegistry {
        let mut r = LanguageRegistry::new();
        r.register(LanguageInfo::new("rust").with_paths(
            Some("/langs/rust/syntax.scm".into()),
            Some("/langs/rust/config.janet".into()),
        ));
        r.register(LanguageInfo::new("janet").with_paths(Some("/langs/janet/syntax.scm".into()), None));
        r
    }

    #[test]
    fn new_registry_is_empty() {
        assert_eq!(LanguageRegistry::new().count(), 0);
    }

    #[test]
    fn register_and_count() {
        let r = make_registry();
        assert_eq!(r.count(), 2);
    }

    #[test]
    fn list_is_sorted() {
        let r = make_registry();
        let names: Vec<&str> = r.list().iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["janet", "rust"]);
    }

    #[test]
    fn get_returns_info() {
        let r = make_registry();
        let info = r.get("rust").unwrap();
        assert_eq!(info.syntax_path.as_deref(), Some("/langs/rust/syntax.scm"));
        assert_eq!(info.config_path.as_deref(), Some("/langs/rust/config.janet"));
    }

    #[test]
    fn get_missing_returns_none() {
        let r = make_registry();
        assert!(r.get("python").is_none());
    }

    #[test]
    fn unregister_removes_entry() {
        let mut r = make_registry();
        assert!(r.unregister("rust"));
        assert!(!r.unregister("rust"));
        assert_eq!(r.count(), 1);
    }

    #[test]
    fn mark_loaded_changes_flag() {
        let mut r = make_registry();
        assert!(!r.is_loaded("rust"));
        r.mark_loaded("rust");
        assert!(r.is_loaded("rust"));
    }

    #[test]
    fn mark_loaded_unknown_is_noop() {
        let mut r = LanguageRegistry::new();
        r.mark_loaded("nolang"); // should not panic
    }

    #[test]
    fn is_loaded_unknown_returns_false() {
        let r = make_registry();
        assert!(!r.is_loaded("unknown"));
    }

    #[test]
    fn info_with_paths_partial() {
        let r = make_registry();
        let info = r.get("janet").unwrap();
        assert!(info.syntax_path.is_some());
        assert!(info.config_path.is_none());
    }

    #[test]
    fn register_replaces_existing() {
        let mut r = LanguageRegistry::new();
        r.register(LanguageInfo::new("go").with_paths(Some("/old/syntax.scm".into()), None));
        r.register(LanguageInfo::new("go").with_paths(Some("/new/syntax.scm".into()), None));
        assert_eq!(r.count(), 1);
        assert_eq!(
            r.get("go").unwrap().syntax_path.as_deref(),
            Some("/new/syntax.scm")
        );
    }
}
