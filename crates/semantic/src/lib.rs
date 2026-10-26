//! Semantic Engine — language intelligence for Magma.
//!
//! Exposes the LanguageProvider abstraction, symbol indexing, project graph,
//! and the SemanticEngine coordinator.  LSP and tree-sitter are the two
//! built-in provider implementations.
//!
//! The editor-coupled pieces stay in the kernel: the live LSP client
//! (`kernel::semantic::lsp`) owns processes and talks to the background event
//! channel, and `edit_apply`/`ts_schedule` mutate `Editor` directly.

pub mod provider;
pub mod lsp_provider;
pub mod ts_registry;
pub mod treesitter_provider;
pub mod symbol_index;
pub mod project_graph;
pub mod engine;
pub mod language_registry;

/// LSP wire protocol — framing and JSON-RPC shaping.
pub mod lsp_proto;

pub use provider::{
    CompletionItem, CompletionKind, Diagnostic, DiagnosticSeverity,
    LanguageProvider, ParseResult, Symbol, SymbolKind,
};
pub use lsp_provider::LspLanguageProvider;
pub use treesitter_provider::TreesitterLanguageProvider;
pub use symbol_index::{SymbolIndex, SymbolRef};
pub use project_graph::{FileNode, ProjectGraph};
pub use engine::SemanticEngine;
pub use language_registry::{LanguageInfo, LanguageRegistry};
