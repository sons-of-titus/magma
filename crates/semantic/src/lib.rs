//! Semantic Engine — language intelligence for Magma.
//!
//! Exposes the LanguageProvider abstraction, symbol indexing, project graph,
//! and the SemanticEngine coordinator.  LSP and tree-sitter are the two
//! built-in provider implementations.
//!
//! The live LSP client ([`lsp`]) owns its child processes, reader/writer
//! threads, and request correlation.  It reports results through
//! [`lsp::EventSink`] instead of naming the application's background-event
//! enum, so the client lives below the kernel.
//!
//! The editor-coupled pieces stay in the kernel: `edit_apply` mutates buffers
//! and `ts_schedule` submits parses to the background lane.

pub mod engine;
pub mod language_registry;
pub mod lsp;
pub mod lsp_provider;
pub mod project_graph;
pub mod provider;
pub mod symbol_index;
pub mod treesitter_provider;
pub mod ts_registry;

/// LSP wire protocol — framing and JSON-RPC shaping.
pub mod lsp_proto;

pub use engine::SemanticEngine;
pub use language_registry::{LanguageInfo, LanguageRegistry};
pub use lsp::{Event, EventSink, LspHandle, LspRegistry};
pub use lsp_provider::LspLanguageProvider;
pub use project_graph::{FileNode, ProjectGraph};
pub use provider::{
    CompletionItem, CompletionKind, Diagnostic, DiagnosticSeverity, LanguageProvider, ParseResult,
    Symbol, SymbolKind,
};
pub use symbol_index::{SymbolIndex, SymbolRef};
pub use treesitter_provider::TreesitterLanguageProvider;
