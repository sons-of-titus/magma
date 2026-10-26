//! ProjectGraph — tracks relationships between files, symbols, and modules.

use std::collections::{HashMap, HashSet};

/// A node in the project graph representing one file.
#[derive(Debug, Clone, Default)]
pub struct FileNode {
    /// Absolute file path.
    pub path: String,
    /// Language identifier (e.g. "rust", "python").
    pub language: String,
    /// Names of symbols defined in this file.
    pub symbols: Vec<String>,
}

/// Directed graph of file relationships.
///
/// An edge from file A to file B means A imports/uses B.
#[derive(Debug, Default)]
pub struct ProjectGraph {
    pub files: HashMap<String, FileNode>,
    /// `dependencies[a]` = files that `a` imports.
    pub dependencies: HashMap<String, Vec<String>>,
    /// `dependents[b]` = files that import `b`.
    pub dependents: HashMap<String, Vec<String>>,
}

impl ProjectGraph {
    pub fn new() -> Self { ProjectGraph::default() }

    /// Add or update a file node.
    pub fn add_file(&mut self, path: &str, language: &str, symbols: Vec<String>) {
        let node = self.files.entry(path.to_string()).or_default();
        node.path     = path.to_string();
        node.language = language.to_string();
        node.symbols  = symbols;
    }

    /// Record that `from` depends on `to`.
    pub fn add_dependency(&mut self, from: &str, to: &str) {
        let deps = self.dependencies.entry(from.to_string()).or_default();
        if !deps.contains(&to.to_string()) {
            deps.push(to.to_string());
        }
        let revdeps = self.dependents.entry(to.to_string()).or_default();
        if !revdeps.contains(&from.to_string()) {
            revdeps.push(from.to_string());
        }
    }

    /// Files that `path` directly imports.
    pub fn dependencies_of(&self, path: &str) -> Vec<String> {
        self.dependencies.get(path).cloned().unwrap_or_default()
    }

    /// Files that directly import `path`.
    pub fn dependents_of(&self, path: &str) -> Vec<String> {
        self.dependents.get(path).cloned().unwrap_or_default()
    }

    /// Remove a file and all its edges.
    pub fn remove_file(&mut self, path: &str) {
        self.files.remove(path);
        if let Some(deps) = self.dependencies.remove(path) {
            for dep in &deps {
                if let Some(revs) = self.dependents.get_mut(dep) {
                    revs.retain(|r| r != path);
                }
            }
        }
        if let Some(revs) = self.dependents.remove(path) {
            for rev in &revs {
                if let Some(deps) = self.dependencies.get_mut(rev) {
                    deps.retain(|d| d != path);
                }
            }
        }
    }

    /// Transitive closure of files that `path` depends on (depth-first).
    pub fn transitive_dependencies(&self, path: &str) -> Vec<String> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut stack   = vec![path.to_string()];
        while let Some(cur) = stack.pop() {
            if visited.insert(cur.clone()) {
                for dep in self.dependencies_of(&cur) {
                    if !visited.contains(&dep) {
                        stack.push(dep);
                    }
                }
            }
        }
        visited.remove(path);
        let mut out: Vec<String> = visited.into_iter().collect();
        out.sort();
        out
    }

    /// Total number of tracked files.
    pub fn file_count(&self) -> usize { self.files.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_file_and_query() {
        let mut g = ProjectGraph::new();
        g.add_file("a.rs", "rust", vec!["foo".to_string()]);
        assert_eq!(g.file_count(), 1);
        let node = g.files.get("a.rs").unwrap();
        assert_eq!(node.language, "rust");
        assert_eq!(node.symbols, vec!["foo"]);
    }

    #[test]
    fn dependencies_roundtrip() {
        let mut g = ProjectGraph::new();
        g.add_dependency("main.rs", "lib.rs");
        g.add_dependency("main.rs", "util.rs");
        let deps = g.dependencies_of("main.rs");
        assert!(deps.contains(&"lib.rs".to_string()));
        assert!(deps.contains(&"util.rs".to_string()));
        let revdeps = g.dependents_of("lib.rs");
        assert!(revdeps.contains(&"main.rs".to_string()));
    }

    #[test]
    fn no_duplicate_edges() {
        let mut g = ProjectGraph::new();
        g.add_dependency("a.rs", "b.rs");
        g.add_dependency("a.rs", "b.rs");
        assert_eq!(g.dependencies_of("a.rs").len(), 1);
    }

    #[test]
    fn remove_file_clears_edges() {
        let mut g = ProjectGraph::new();
        g.add_file("a.rs", "rust", vec![]);
        g.add_dependency("b.rs", "a.rs");
        g.remove_file("a.rs");
        assert_eq!(g.file_count(), 0);
        assert!(g.dependents_of("a.rs").is_empty());
    }

    #[test]
    fn transitive_dependencies() {
        let mut g = ProjectGraph::new();
        g.add_dependency("main.rs", "lib.rs");
        g.add_dependency("lib.rs",  "util.rs");
        let trans = g.transitive_dependencies("main.rs");
        assert!(trans.contains(&"lib.rs".to_string()));
        assert!(trans.contains(&"util.rs".to_string()));
        assert!(!trans.contains(&"main.rs".to_string()));
    }
}
