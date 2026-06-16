//! SemanticEngine — the central intelligence coordinator.
//!
//! Holds registered language providers, a symbol index, a project graph, and a
//! cache of typed diagnostics.  All provider methods are synchronous and
//! non-blocking; providers that need async communication (LSP) enqueue work and
//! return empty results immediately.

use std::collections::HashMap;

use super::provider::{CompletionItem, Diagnostic, DiagnosticSeverity, LanguageProvider, Symbol};
use super::symbol_index::{SymbolIndex, SymbolRef};
use super::project_graph::ProjectGraph;

/// Central coordinator for language intelligence features.
#[derive(Default)]
pub struct SemanticEngine {
    /// Registered providers, keyed by language identifier.
    providers: HashMap<String, Box<dyn LanguageProvider>>,
    /// Global symbol index (populated by `index_buffer`).
    pub symbol_index: SymbolIndex,
    /// Project-level file relationship graph.
    pub project_graph: ProjectGraph,
    /// Typed diagnostics keyed by file path.
    pub diagnostics: HashMap<String, Vec<Diagnostic>>,
}

impl SemanticEngine {
    pub fn new() -> Self { SemanticEngine::default() }

    /// Register a language provider.  Replaces any existing provider for the language.
    pub fn register_provider(&mut self, language_id: &str, provider: Box<dyn LanguageProvider>) {
        self.providers.insert(language_id.to_string(), provider);
    }

    /// Look up the provider for a language.
    pub fn provider_for(&self, language_id: &str) -> Option<&dyn LanguageProvider> {
        self.providers.get(language_id).map(|b| b.as_ref())
    }

    /// Returns true if a provider is registered for `language_id`.
    pub fn has_provider(&self, language_id: &str) -> bool {
        self.providers.contains_key(language_id)
    }

    /// Extract symbols from a buffer and update the symbol index + project graph.
    pub fn index_buffer(&mut self, language: &str, path: &str, text: &str) {
        let mut symbols = match self.providers.get(language) {
            Some(p) => p.symbols(text),
            None => vec![],
        };
        for sym in &mut symbols {
            sym.file = path.to_string();
        }
        let sym_names: Vec<String> = symbols.iter().map(|s| s.name.clone()).collect();
        self.symbol_index.add_symbols(path, symbols);
        self.project_graph.add_file(path, language, sym_names);
    }

    /// Return symbols for a buffer, queried live from its provider.
    pub fn symbols_for_buffer(&self, language: &str, text: &str) -> Vec<Symbol> {
        match self.providers.get(language) {
            Some(p) => p.symbols(text),
            None => vec![],
        }
    }

    /// Return all known definitions for a symbol name (from the index).
    pub fn definitions(&self, name: &str) -> Vec<Symbol> {
        self.symbol_index.lookup(name)
    }

    /// Return all known references for a symbol name.
    pub fn references(&self, name: &str) -> Vec<SymbolRef> {
        self.symbol_index.references(name)
    }

    /// Return documentation for a symbol.
    pub fn documentation(&self, name: &str) -> Option<String> {
        self.symbol_index.documentation(name)
    }

    /// Return symbol names from the index that start with `prefix`.
    pub fn completions_for_prefix(&self, prefix: &str) -> Vec<String> {
        self.symbol_index.completions_for_prefix(prefix)
    }

    /// Return completions from a provider (synchronous, for tree-sitter).
    pub fn completions_for_buffer(
        &self,
        language: &str,
        text: &str,
        offset: usize,
    ) -> Vec<CompletionItem> {
        match self.providers.get(language) {
            Some(p) => p.completion(text, offset),
            None => vec![],
        }
    }

    /// Store typed diagnostics for a file path, replacing any existing entry.
    pub fn update_diagnostics(&mut self, path: &str, diagnostics: Vec<Diagnostic>) {
        if diagnostics.is_empty() {
            self.diagnostics.remove(path);
        } else {
            self.diagnostics.insert(path.to_string(), diagnostics);
        }
    }

    /// Parse the formatted diagnostic strings stored in `buffer.diagnostics` and
    /// update the typed diagnostic cache for `path`.
    pub fn update_diagnostics_from_strings(&mut self, path: &str, strings: &[String]) {
        let typed: Vec<Diagnostic> = strings.iter()
            .filter_map(|s| Diagnostic::from_formatted(s))
            .collect();
        self.update_diagnostics(path, typed);
    }

    /// Return the typed diagnostics for a file.
    pub fn diagnostics_for_path(&self, path: &str) -> Vec<Diagnostic> {
        self.diagnostics.get(path).cloned().unwrap_or_default()
    }

    /// Error count for a path.
    pub fn error_count(&self, path: &str) -> usize {
        self.diagnostics.get(path)
            .map(|v| v.iter().filter(|d| d.severity == DiagnosticSeverity::Error).count())
            .unwrap_or(0)
    }

    /// Warning count for a path.
    pub fn warning_count(&self, path: &str) -> usize {
        self.diagnostics.get(path)
            .map(|v| v.iter().filter(|d| d.severity == DiagnosticSeverity::Warning).count())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::semantic::lsp_provider::LspLanguageProvider;
    use crate::kernel::semantic::provider::DiagnosticSeverity;

    #[test]
    fn register_and_query_provider() {
        let mut engine = SemanticEngine::new();
        assert!(!engine.has_provider("rust"));
        engine.register_provider("rust", Box::new(LspLanguageProvider::new("rust")));
        assert!(engine.has_provider("rust"));
    }

    #[test]
    fn index_buffer_lsp_provider_empty_symbols() {
        let mut engine = SemanticEngine::new();
        engine.register_provider("rust", Box::new(LspLanguageProvider::new("rust")));
        engine.index_buffer("rust", "/src/main.rs", "fn main() {}");
        // LSP provider returns no symbols synchronously.
        assert_eq!(engine.symbol_index.symbol_count(), 0);
        assert_eq!(engine.project_graph.file_count(), 1);
    }

    #[test]
    fn update_and_query_diagnostics() {
        let mut engine = SemanticEngine::new();
        let diags = vec![
            Diagnostic { message: "err".into(), severity: DiagnosticSeverity::Error, line: 0, column: 0 },
            Diagnostic { message: "warn".into(), severity: DiagnosticSeverity::Warning, line: 1, column: 0 },
        ];
        engine.update_diagnostics("/a.rs", diags);
        assert_eq!(engine.error_count("/a.rs"), 1);
        assert_eq!(engine.warning_count("/a.rs"), 1);
        assert_eq!(engine.diagnostics_for_path("/a.rs").len(), 2);
    }

    #[test]
    fn update_diagnostics_from_strings() {
        let mut engine = SemanticEngine::new();
        let strings = vec![
            "[E] line 1:1 missing semicolon".to_string(),
            "[W] line 2:5 unused variable".to_string(),
        ];
        engine.update_diagnostics_from_strings("/b.rs", &strings);
        assert_eq!(engine.error_count("/b.rs"), 1);
        assert_eq!(engine.warning_count("/b.rs"), 1);
    }

    #[test]
    fn update_diagnostics_empty_removes_entry() {
        let mut engine = SemanticEngine::new();
        engine.update_diagnostics_from_strings("/c.rs", &["[E] line 1:1 err".to_string()]);
        assert_eq!(engine.diagnostics.len(), 1);
        engine.update_diagnostics("/c.rs", vec![]);
        assert_eq!(engine.diagnostics.len(), 0);
    }

    #[test]
    fn completions_for_prefix_from_index() {
        let mut engine = SemanticEngine::new();
        use crate::kernel::semantic::provider::{Symbol, SymbolKind};
        engine.symbol_index.add_symbols("/a.rs", vec![
            Symbol { name: "format_str".into(), kind: SymbolKind::Function, file: String::new(), line: 0, column: 0, documentation: None },
            Symbol { name: "format_int".into(), kind: SymbolKind::Function, file: String::new(), line: 1, column: 0, documentation: None },
            Symbol { name: "parse_str".into(),  kind: SymbolKind::Function, file: String::new(), line: 2, column: 0, documentation: None },
        ]);
        let comps = engine.completions_for_prefix("format");
        assert_eq!(comps, vec!["format_int", "format_str"]);
    }
}
