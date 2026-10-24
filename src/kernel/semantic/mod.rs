//! Semantic Engine — language intelligence for Magma.
//!
//! Exposes the LanguageProvider abstraction, symbol indexing, project graph,
//! and the SemanticEngine coordinator.  LSP and tree-sitter are the two
//! built-in provider implementations.

pub mod provider;
pub mod lsp_provider;
pub mod ts_registry;
pub mod ts_schedule;
pub mod treesitter_provider;
pub mod symbol_index;
pub mod project_graph;
pub mod engine;
pub mod language_registry;

/// LSP client — real server lifecycle, revision-tagged requests.
pub mod lsp;
/// LSP wire protocol — framing and JSON-RPC shaping.
pub mod lsp_proto;

pub use lsp::LspRegistry;
pub use provider::{
    CompletionItem, CompletionKind, Diagnostic, DiagnosticSeverity,
    LanguageProvider, ParseResult, Symbol, SymbolKind,
};
pub mod edit_apply;
pub use edit_apply::{apply_workspace_edit, offset_of_lsp_position};
pub use lsp_provider::LspLanguageProvider;
pub use treesitter_provider::TreesitterLanguageProvider;
pub use symbol_index::{SymbolIndex, SymbolRef};
pub use project_graph::{FileNode, ProjectGraph};
pub use engine::SemanticEngine;
pub use language_registry::{LanguageInfo, LanguageRegistry};
