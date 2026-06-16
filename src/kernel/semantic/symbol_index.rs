//! SymbolIndex — stores symbol definitions, references, and documentation.

use std::collections::HashMap;
use super::provider::Symbol;

/// A reference to a symbol (a use site, not a definition).
#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
}

/// Global symbol index.  Indexed by symbol name for fast lookup.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// All known symbol definitions, keyed by name.
    pub symbols: HashMap<String, Vec<Symbol>>,
    /// All known symbol references, keyed by name.
    pub references: HashMap<String, Vec<SymbolRef>>,
}

impl SymbolIndex {
    pub fn new() -> Self { SymbolIndex::default() }

    /// Index all symbols extracted from a file.  Replaces any previously
    /// indexed symbols for that file.
    pub fn add_symbols(&mut self, file: &str, mut symbols: Vec<Symbol>) {
        // Remove stale entries for this file.
        for entries in self.symbols.values_mut() {
            entries.retain(|s| s.file != file);
        }
        self.symbols.retain(|_, v| !v.is_empty());

        // Stamp every symbol with the file path and insert.
        for sym in &mut symbols {
            sym.file = file.to_string();
        }
        for sym in symbols {
            self.symbols.entry(sym.name.clone()).or_default().push(sym);
        }
    }

    /// Look up all definitions for a symbol name.
    pub fn lookup(&self, name: &str) -> Vec<Symbol> {
        self.symbols.get(name).cloned().unwrap_or_default()
    }

    /// Return all known symbols as a flat list.
    pub fn all_symbols(&self) -> Vec<Symbol> {
        self.symbols.values().flatten().cloned().collect()
    }

    /// Return symbol names that start with `prefix` (for completion).
    pub fn completions_for_prefix(&self, prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = self.symbols.keys()
            .filter(|n| n.starts_with(prefix))
            .cloned()
            .collect();
        out.sort();
        out
    }

    /// Record a reference to a symbol.
    pub fn add_reference(&mut self, name: &str, file: &str, line: usize, column: usize) {
        self.references.entry(name.to_string()).or_default().push(SymbolRef {
            name: name.to_string(),
            file: file.to_string(),
            line,
            column,
        });
    }

    /// Look up all known references for a symbol name.
    pub fn references(&self, name: &str) -> Vec<SymbolRef> {
        self.references.get(name).cloned().unwrap_or_default()
    }

    /// Return documentation for a symbol (from its first definition that has doc).
    pub fn documentation(&self, name: &str) -> Option<String> {
        self.symbols.get(name)?
            .iter()
            .find_map(|s| s.documentation.clone())
    }

    /// Remove all symbols and references for a file.
    pub fn remove_file(&mut self, file: &str) {
        for entries in self.symbols.values_mut() {
            entries.retain(|s| s.file != file);
        }
        self.symbols.retain(|_, v| !v.is_empty());
        for refs in self.references.values_mut() {
            refs.retain(|r| r.file != file);
        }
        self.references.retain(|_, v| !v.is_empty());
    }

    /// Total number of indexed symbol definitions.
    pub fn symbol_count(&self) -> usize {
        self.symbols.values().map(|v| v.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::semantic::provider::SymbolKind;

    fn make_sym(name: &str, kind: SymbolKind) -> Symbol {
        Symbol { name: name.to_string(), kind, file: String::new(), line: 0, column: 0, documentation: None }
    }

    #[test]
    fn add_and_lookup() {
        let mut idx = SymbolIndex::new();
        idx.add_symbols("a.rs", vec![make_sym("foo", SymbolKind::Function)]);
        let syms = idx.lookup("foo");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "foo");
        assert_eq!(syms[0].file, "a.rs");
    }

    #[test]
    fn replace_on_re_index() {
        let mut idx = SymbolIndex::new();
        idx.add_symbols("a.rs", vec![make_sym("foo", SymbolKind::Function)]);
        idx.add_symbols("a.rs", vec![make_sym("bar", SymbolKind::Function)]);
        assert!(idx.lookup("foo").is_empty(), "foo should be replaced");
        assert_eq!(idx.lookup("bar").len(), 1);
    }

    #[test]
    fn completions_for_prefix() {
        let mut idx = SymbolIndex::new();
        idx.add_symbols("a.rs", vec![
            make_sym("fmt_write", SymbolKind::Function),
            make_sym("fmt_read",  SymbolKind::Function),
            make_sym("parse",     SymbolKind::Function),
        ]);
        let comps = idx.completions_for_prefix("fmt");
        assert_eq!(comps, vec!["fmt_read", "fmt_write"]);
        assert!(idx.completions_for_prefix("xyz").is_empty());
    }

    #[test]
    fn references_roundtrip() {
        let mut idx = SymbolIndex::new();
        idx.add_reference("foo", "b.rs", 10, 5);
        idx.add_reference("foo", "c.rs", 20, 0);
        let refs = idx.references("foo");
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn remove_file_cleans_up() {
        let mut idx = SymbolIndex::new();
        idx.add_symbols("a.rs", vec![make_sym("foo", SymbolKind::Function)]);
        idx.add_symbols("b.rs", vec![make_sym("bar", SymbolKind::Function)]);
        idx.remove_file("a.rs");
        assert!(idx.lookup("foo").is_empty());
        assert_eq!(idx.lookup("bar").len(), 1);
    }

    #[test]
    fn documentation_from_first_definition() {
        let mut idx = SymbolIndex::new();
        let mut sym = make_sym("documented", SymbolKind::Function);
        sym.documentation = Some("Does things.".to_string());
        idx.add_symbols("a.rs", vec![sym]);
        assert_eq!(idx.documentation("documented"), Some("Does things.".to_string()));
        assert!(idx.documentation("undocumented").is_none());
    }

    #[test]
    fn symbol_count() {
        let mut idx = SymbolIndex::new();
        idx.add_symbols("a.rs", vec![
            make_sym("a", SymbolKind::Function),
            make_sym("b", SymbolKind::Function),
        ]);
        assert_eq!(idx.symbol_count(), 2);
    }
}
