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
    /// File extensions mapped to this language (plugin declarations only;
    /// the built-in extension map is unaffected).
    pub extensions: Vec<String>,
    /// Absolute path to a plugin-provided grammar shared library, if the
    /// plugin ships one.  Loaded at registration, never reloaded.
    pub grammar_path: Option<String>,
    /// Whether the grammar library loaded and passed the ABI check.
    pub grammar_loaded: bool,
    /// Line comment marker (`"//"`), from the registration's `:comments`.
    pub comment_line: Option<String>,
    /// Block comment open/close markers (`["/*", "*/"]`).
    pub comment_block: Option<(String, String)>,
    /// Indentation width from the registration's `:indentation`.
    pub indent_size: Option<u32>,
    /// `"spaces"` or `"tabs"`.
    pub indent_style: Option<String>,
    /// Whether the plugin also provides the `config.janet` via the
    /// registration table (metadata; the file loads as before).
    pub loaded: bool,
    /// The plugin that registered this language; registry-level plugin
    /// unload removes its languages.
    pub owner: Option<String>,
}

impl LanguageInfo {
    pub fn new(name: impl Into<String>) -> Self {
        LanguageInfo {
            name: name.into(),
            syntax_path: None,
            config_path: None,
            extensions: Vec::new(),
            grammar_path: None,
            grammar_loaded: false,
            comment_line: None,
            comment_block: None,
            indent_size: None,
            indent_style: None,
            loaded: false,
            owner: None,
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

    /// Remove every language registered by `owner`; returns how many.
    pub fn remove_owner(&mut self, owner: &str) -> usize {
        let before = self.languages.len();
        self.languages
            .retain(|_, info| info.owner.as_deref() != Some(owner));
        before - self.languages.len()
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
        r.register(
            LanguageInfo::new("janet").with_paths(Some("/langs/janet/syntax.scm".into()), None),
        );
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
        assert_eq!(
            info.config_path.as_deref(),
            Some("/langs/rust/config.janet")
        );
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
