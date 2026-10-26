//! LanguageRegistry — tracks languages available as external Janet modules.
//!
//! Each language can provide:
//!   - `provider.janet` — Janet-side Language Provider implementation
//!   - `syntax.scm`     — tree-sitter query file
//!   - `config.janet`   — format, completion, and adapter settings

use std::collections::HashMap;

/// Metadata for a registered language.
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    pub name: String,
    /// Absolute path to the language's `provider.janet` file, if provided.
    pub provider_path: Option<String>,
    /// Absolute path to the language's `syntax.scm` file, if provided.
    pub syntax_path: Option<String>,
    /// Absolute path to the language's `config.janet` file, if provided.
    pub config_path: Option<String>,
    /// Whether the provider.janet has been successfully loaded into the Janet VM.
    pub loaded: bool,
}

impl LanguageInfo {
    pub fn new(name: impl Into<String>) -> Self {
        LanguageInfo {
            name: name.into(),
            provider_path: None,
            syntax_path: None,
            config_path: None,
            loaded: false,
        }
    }

    pub fn with_paths(
        mut self,
        provider_path: Option<String>,
        syntax_path: Option<String>,
        config_path: Option<String>,
    ) -> Self {
        self.provider_path = provider_path;
        self.syntax_path = syntax_path;
        self.config_path = config_path;
        self
    }
}

/// Registry of available languages loaded from the `languages/` directory.
#[derive(Default)]
pub struct LanguageRegistry {
    languages: HashMap<String, LanguageInfo>,
}

impl LanguageRegistry {
    pub fn new() -> Self { Self::default() }

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

    /// Mark a language's provider as successfully loaded.
    pub fn mark_loaded(&mut self, name: &str) {
        if let Some(info) = self.languages.get_mut(name) {
            info.loaded = true;
        }
    }

    /// True if the named language is registered and its provider is loaded.
    pub fn is_loaded(&self, name: &str) -> bool {
        self.languages.get(name).map(|i| i.loaded).unwrap_or(false)
    }

    /// Total number of registered languages.
    pub fn count(&self) -> usize {
        self.languages.len()
    }

    /// Discover languages from a `languages/` root directory.
    ///
    /// For each sub-directory `<root>/<lang>/`, registers a `LanguageInfo`
    /// pointing to whichever of `provider.janet`, `syntax.scm`, and
    /// `config.janet` actually exist on disk.
    pub fn discover(&mut self, languages_root: &str) {
        let root = std::path::Path::new(languages_root);
        let Ok(entries) = std::fs::read_dir(root) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let Some(lang) = path.file_name().and_then(|n| n.to_str()) else { continue };
            let lang = lang.to_string();

            let provider = path.join("provider.janet");
            let syntax   = path.join("syntax.scm");
            let config   = path.join("config.janet");

            let info = LanguageInfo {
                name: lang.clone(),
                provider_path: provider.exists().then(|| provider.to_string_lossy().into_owned()),
                syntax_path:   syntax.exists().then(|| syntax.to_string_lossy().into_owned()),
                config_path:   config.exists().then(|| config.to_string_lossy().into_owned()),
                loaded: false,
            };
            self.languages.insert(lang, info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> LanguageRegistry {
        let mut r = LanguageRegistry::new();
        r.register(LanguageInfo::new("rust").with_paths(
            Some("/langs/rust/provider.janet".into()),
            Some("/langs/rust/syntax.scm".into()),
            Some("/langs/rust/config.janet".into()),
        ));
        r.register(LanguageInfo::new("janet").with_paths(
            Some("/langs/janet/provider.janet".into()),
            None, None,
        ));
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
        assert_eq!(info.provider_path.as_deref(), Some("/langs/rust/provider.janet"));
        assert_eq!(info.syntax_path.as_deref(), Some("/langs/rust/syntax.scm"));
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
        assert!(info.provider_path.is_some());
        assert!(info.syntax_path.is_none());
        assert!(info.config_path.is_none());
    }

    #[test]
    fn discover_nonexistent_dir_is_noop() {
        let mut r = LanguageRegistry::new();
        r.discover("/nonexistent/path/that/does/not/exist");
        assert_eq!(r.count(), 0);
    }

    #[test]
    fn discover_creates_entries_for_existing_dirs() {
        use std::io::Write;
        let tmp = std::env::temp_dir().join(format!("magma_lang_test_{}", std::process::id()));
        let lang_dir = tmp.join("testlang");
        std::fs::create_dir_all(&lang_dir).unwrap();
        let provider_path = lang_dir.join("provider.janet");
        let _ = std::fs::File::create(&provider_path).and_then(|mut f| f.write_all(b"# provider"));
        let _ = std::fs::File::create(lang_dir.join("config.janet")).and_then(|mut f| f.write_all(b"# cfg"));

        let mut r = LanguageRegistry::new();
        r.discover(tmp.to_str().unwrap());

        let _ = std::fs::remove_dir_all(&tmp);

        assert_eq!(r.count(), 1);
        let info = r.get("testlang").unwrap();
        assert!(info.provider_path.is_some());
        assert!(info.config_path.is_some());
        assert!(info.syntax_path.is_none());
    }

    #[test]
    fn register_replaces_existing() {
        let mut r = LanguageRegistry::new();
        r.register(LanguageInfo::new("go").with_paths(Some("/old/provider.janet".into()), None, None));
        r.register(LanguageInfo::new("go").with_paths(Some("/new/provider.janet".into()), None, None));
        assert_eq!(r.count(), 1);
        assert_eq!(r.get("go").unwrap().provider_path.as_deref(), Some("/new/provider.janet"));
    }
}
