//! Semantic Engine — language intelligence for Magma.
//!
//! Exposes the LanguageProvider abstraction, symbol indexing, project graph,
//! and the SemanticEngine coordinator.  LSP and tree-sitter are the two
//! built-in provider implementations.

pub mod provider;
pub mod lsp_provider;
pub mod ts_registry;
pub mod treesitter_provider;
pub mod symbol_index;
pub mod project_graph;
pub mod engine;

/// The original LSP JSON-RPC client — spawns and manages server processes.
pub mod client;
/// JSON-RPC response parsers.
pub mod parse;

pub use client::{LspManager, pending_map};
pub use provider::{
    CompletionItem, CompletionKind, Diagnostic, DiagnosticSeverity,
    LanguageProvider, ParseResult, Symbol, SymbolKind,
};
pub use lsp_provider::LspLanguageProvider;
pub use treesitter_provider::TreesitterLanguageProvider;
pub use symbol_index::{SymbolIndex, SymbolRef};
pub use project_graph::{FileNode, ProjectGraph};
pub use engine::SemanticEngine;
