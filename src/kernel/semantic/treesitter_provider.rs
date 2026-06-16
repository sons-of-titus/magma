//! TreesitterLanguageProvider — wraps tree-sitter grammars in the LanguageProvider trait.
//!
//! Symbols are extracted by running capture queries.  Diagnostics always return
//! empty — tree-sitter reports syntax errors through `parse()`, not `diagnostics()`.
//! Completion returns empty — symbol-based completions are served by the SemanticEngine
//! via its SymbolIndex.

use super::provider::{
    CompletionItem, Diagnostic, LanguageProvider, ParseResult, Symbol, SymbolKind,
};
use super::ts_registry;

/// A language provider backed by an offline tree-sitter grammar.
pub struct TreesitterLanguageProvider {
    pub language_name: String,
}

impl TreesitterLanguageProvider {
    pub fn new(language_name: impl Into<String>) -> Self {
        TreesitterLanguageProvider { language_name: language_name.into() }
    }

    /// Generic tree-sitter capture query for common top-level symbols.
    ///
    /// This tries multiple node type patterns.  If the grammar does not define a
    /// node type used in a pattern, `ts_registry::run_query` returns `None` for
    /// that specific query; the caller handles that gracefully.
    fn symbol_queries() -> &'static [(&'static str, &'static str)] {
        &[
            ("(function_item name: (identifier) @function)", "function"),
            ("(function_declaration name: (identifier) @function)", "function"),
            ("(method_declaration name: (field_identifier) @method)", "method"),
            ("(method_definition name: (property_identifier) @method)", "method"),
            ("(struct_item name: (type_identifier) @struct)", "struct"),
            ("(enum_item name: (type_identifier) @enum)", "enum"),
            ("(impl_item type: (type_identifier) @class)", "class"),
            ("(class_declaration name: (identifier) @class)", "class"),
            ("(module_item name: (identifier) @module)", "module"),
            ("(const_item name: (identifier) @constant)", "constant"),
            ("(static_item name: (identifier) @constant)", "constant"),
        ]
    }
}

impl LanguageProvider for TreesitterLanguageProvider {
    fn parse(&self, text: &str) -> ParseResult {
        match ts_registry::parse_text(&self.language_name, text) {
            Some(errors) => ParseResult { success: errors.is_empty(), errors },
            None => ParseResult { success: true, errors: vec![] },
        }
    }

    fn symbols(&self, text: &str) -> Vec<Symbol> {
        let text_bytes = text.as_bytes();
        let mut out: Vec<Symbol> = Vec::new();
        let mut seen_spans: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for (query_src, default_kind) in Self::symbol_queries() {
            let Some(captures) = ts_registry::run_query(&self.language_name, text, query_src)
            else { continue; };

            for (start, end, capture_name) in captures {
                if seen_spans.contains(&(start, end)) { continue; }
                seen_spans.insert((start, end));

                let name = match std::str::from_utf8(&text_bytes[start..end]) {
                    Ok(s) => s.to_string(),
                    Err(_) => continue,
                };
                let kind = match capture_name.as_str() {
                    "function" => SymbolKind::Function,
                    "method"   => SymbolKind::Method,
                    "struct"   => SymbolKind::Struct,
                    "enum"     => SymbolKind::Enum,
                    "class"    => SymbolKind::Class,
                    "module"   => SymbolKind::Module,
                    "constant" => SymbolKind::Constant,
                    "variable" => SymbolKind::Variable,
                    _          => match *default_kind {
                        "function" => SymbolKind::Function,
                        "method"   => SymbolKind::Method,
                        "struct"   => SymbolKind::Struct,
                        "enum"     => SymbolKind::Enum,
                        "class"    => SymbolKind::Class,
                        "module"   => SymbolKind::Module,
                        "constant" => SymbolKind::Constant,
                        _          => SymbolKind::Unknown,
                    },
                };
                let before = &text[..start];
                let line   = before.chars().filter(|&c| c == '\n').count();
                let last_nl = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
                let column  = before[last_nl..].chars().count();
                out.push(Symbol { name, kind, file: String::new(), line, column, documentation: None });
            }
        }
        out
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
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treesitter_provider_parse_no_grammar_succeeds() {
        let p = TreesitterLanguageProvider::new("nonexistent-lang");
        let result = p.parse("some text");
        // No grammar registered → trivial success.
        assert!(result.success);
    }

    #[test]
    fn treesitter_provider_symbols_no_grammar_empty() {
        let p = TreesitterLanguageProvider::new("nonexistent-lang");
        assert!(p.symbols("fn foo() {}").is_empty());
    }

    #[test]
    fn treesitter_provider_debug_adapter_none() {
        let p = TreesitterLanguageProvider::new("rust");
        assert_eq!(p.debug_adapter(), None);
    }
}
