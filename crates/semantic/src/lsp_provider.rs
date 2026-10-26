//! LspLanguageProvider — wraps the LSP client in the LanguageProvider trait.
//!
//! Because LSP communication is async, all methods that require a server
//! response return empty results immediately.  The actual data arrives via
//! BackgroundEvents that are emitted on the event bus.

use super::provider::{
    CompletionItem, Diagnostic, LanguageProvider, ParseResult, Symbol,
};

/// A language provider backed by an LSP server.
pub struct LspLanguageProvider {
    pub language_id: String,
}

impl LspLanguageProvider {
    pub fn new(language_id: impl Into<String>) -> Self {
        LspLanguageProvider { language_id: language_id.into() }
    }
}

impl LanguageProvider for LspLanguageProvider {
    fn parse(&self, _text: &str) -> ParseResult {
        ParseResult { success: true, errors: vec![] }
    }

    fn symbols(&self, _text: &str) -> Vec<Symbol> {
        vec![]
    }

    fn diagnostics(&self, _buffer_id: usize, _path: &str) -> Vec<Diagnostic> {
        vec![]
    }

    fn completion(&self, _text: &str, _offset: usize) -> Vec<CompletionItem> {
        vec![]
    }

    fn format(&self, _text: &str) -> Option<String> {
        None
    }

    fn debug_adapter(&self) -> Option<String> {
        Some(format!("lsp:{}", self.language_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_provider_parse_always_succeeds() {
        let p = LspLanguageProvider::new("rust");
        let result = p.parse("fn main() {}");
        assert!(result.success);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn lsp_provider_symbols_empty() {
        let p = LspLanguageProvider::new("rust");
        assert!(p.symbols("fn foo() {}").is_empty());
    }

    #[test]
    fn lsp_provider_debug_adapter_contains_language() {
        let p = LspLanguageProvider::new("python");
        assert_eq!(p.debug_adapter(), Some("lsp:python".to_string()));
    }

    #[test]
    fn lsp_provider_completion_empty() {
        let p = LspLanguageProvider::new("rust");
        assert!(p.completion("let x = ", 8).is_empty());
    }
}
