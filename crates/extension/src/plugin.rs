//! Plugin records — a plugin is an installed extension with an on-disk
//! root, an entry point, declared dependencies, and the languages it
//! serves.  The registry tracks what is installed and what is loaded;
//! dependency resolution lives on the Janet side (behavior), here we only
//! keep observable state.

use std::collections::HashMap;

/// Metadata for one registered plugin.
#[derive(Debug, Clone)]
pub struct PluginRecord {
    pub name: String,
    pub version: String,
    /// Absolute path to the plugin's directory on disk.
    pub root: String,
    /// Path of the entry script, relative to `root`.
    pub entry_point: String,
    /// Names of plugins that must be loaded before this one.
    pub dependencies: Vec<String>,
    /// Language IDs this plugin provides for (also registered as languages).
    pub languages: Vec<String>,
    /// Whether the entry script has been evaluated successfully.
    pub loaded: bool,
    /// One-line what-this-is for the Plugins page and `:plugin-info`.
    pub description: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    /// Free-form discovery keywords.
    pub tags: Vec<String>,
}

/// Registry of installed plugins, keyed by name.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: HashMap<String, PluginRecord>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a record.  Re-registering resets the loaded flag.
    pub fn register(&mut self, record: PluginRecord) {
        let mut record = record;
        record.loaded = false;
        self.plugins.insert(record.name.clone(), record);
    }

    pub fn get(&self, name: &str) -> Option<&PluginRecord> {
        self.plugins.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut PluginRecord> {
        self.plugins.get_mut(name)
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        self.plugins.remove(name).is_some()
    }

    /// All records sorted by name.
    pub fn list(&self) -> Vec<&PluginRecord> {
        let mut v: Vec<_> = self.plugins.values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    pub fn is_loaded(&self, name: &str) -> bool {
        self.plugins.get(name).is_some_and(|p| p.loaded)
    }

    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str, deps: &[&str]) -> PluginRecord {
        PluginRecord {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            root: format!("/plugins/{name}"),
            entry_point: "main.janet".to_string(),
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            languages: Vec::new(),
            loaded: false,
            description: None,
            author: None,
            homepage: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn new_registry_is_empty() {
        assert_eq!(PluginRegistry::new().count(), 0);
    }

    #[test]
    fn register_and_get_round_trips_fields() {
        let mut reg = PluginRegistry::new();
        reg.register(record("rust", &[]));
        let p = reg.get("rust").unwrap();
        assert_eq!(p.root, "/plugins/rust");
        assert_eq!(p.entry_point, "main.janet");
        assert!(p.dependencies.is_empty());
        assert!(!p.loaded);
    }

    #[test]
    fn re_register_resets_loaded_flag() {
        let mut reg = PluginRegistry::new();
        reg.register(record("rust", &[]));
        reg.get_mut("rust").unwrap().loaded = true;
        assert!(reg.is_loaded("rust"));
        reg.register(record("rust", &[]));
        assert!(!reg.is_loaded("rust"));
    }

    #[test]
    fn list_is_sorted_by_name() {
        let mut reg = PluginRegistry::new();
        reg.register(record("zig", &[]));
        reg.register(record("rust", &[]));
        let names: Vec<&str> = reg.list().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["rust", "zig"]);
    }

    #[test]
    fn unregister_removes_and_reports() {
        let mut reg = PluginRegistry::new();
        reg.register(record("rust", &[]));
        assert!(reg.unregister("rust"));
        assert!(!reg.unregister("rust"));
        assert!(reg.get("rust").is_none());
    }

    #[test]
    fn is_loaded_false_for_unknown() {
        let reg = PluginRegistry::new();
        assert!(!reg.is_loaded("nope"));
    }
}
