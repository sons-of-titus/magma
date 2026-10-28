//! LanguageProvider trait — the abstraction that the Semantic Engine uses to
//! talk to any intelligence backend (LSP, tree-sitter, etc.).

/// The result of synchronously parsing a buffer.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub success: bool,
    pub errors: Vec<String>,
}

/// A symbol kind as understood by the Semantic Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Variable,
    Constant,
    Module,
    Interface,
    Unknown,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Module => "module",
            SymbolKind::Interface => "interface",
            SymbolKind::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A symbol definition extracted from a buffer.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub documentation: Option<String>,
}

/// Diagnostic data model, shared with the text engine that stores it.
pub use text::diagnostic::{Diagnostic, DiagnosticSeverity};

/// Completion candidate returned by a language provider.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub insert_text: String,
    pub kind: CompletionKind,
}

/// Completion candidate kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    Text,
    Function,
    Variable,
    Keyword,
    Snippet,
    Unknown,
}

impl CompletionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompletionKind::Text => "text",
            CompletionKind::Function => "function",
            CompletionKind::Variable => "variable",
            CompletionKind::Keyword => "keyword",
            CompletionKind::Snippet => "snippet",
            CompletionKind::Unknown => "unknown",
        }
    }
}

/// Abstraction over any intelligence backend.
///
/// All methods are synchronous and infallible.  Backends that need async work
/// (LSP, remote APIs) enqueue the work and return empty results immediately;
/// actual results arrive later via the event bus.
///
/// Design constraint: never block the UI thread.
pub trait LanguageProvider: Send + Sync {
    /// Parse `text` and return a structural result.
    ///
    /// Tree-sitter providers return real errors; LSP providers return a trivial
    /// success because LSP parsing is async.
    fn parse(&self, text: &str) -> ParseResult;

    /// Extract symbol definitions from `text`.
    fn symbols(&self, text: &str) -> Vec<Symbol>;

    /// Return any locally-known diagnostics for a file.
    ///
    /// For LSP providers this always returns empty — diagnostics arrive later via
    /// the `lsp-diagnostics` event.
    fn diagnostics(&self, buffer_id: usize, path: &str) -> Vec<Diagnostic>;

    /// Return completion candidates at `offset` within `text`.
    ///
    /// For LSP providers this always returns empty — completions arrive later via
    /// the `lsp-completion-items` event.
    fn completion(&self, text: &str, offset: usize) -> Vec<CompletionItem>;

    /// Format `text` according to the language's conventions.
    ///
    /// Returns `None` if the provider does not support formatting.
    fn format(&self, text: &str) -> Option<String>;

    /// Name of the debug adapter for this language, if any.
    fn debug_adapter(&self) -> Option<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_kind_display() {
        assert_eq!(SymbolKind::Function.as_str(), "function");
        assert_eq!(SymbolKind::Struct.as_str(), "struct");
        assert_eq!(format!("{}", SymbolKind::Module), "module");
    }
}
