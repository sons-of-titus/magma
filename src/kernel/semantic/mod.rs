//! Semantic Engine — language intelligence for Magma.
//!
//! The provider/engine core (language providers, symbol index, project graph,
//! tree-sitter registry, LSP wire protocol) lives in the `semantic` workspace
//! crate.  This module owns the editor-coupled remainder:
//!
//!   lsp.rs         — live language-server lifecycle and background events
//!   edit_apply.rs  — applying an LSP `WorkspaceEdit` to `Editor` buffers
//!   ts_schedule.rs — scheduling incremental parses onto the background pool
//!
//! Those stay here until the background-event vocabulary is split out of
//! `kernel::runtime`.  The re-exports below keep every existing
//! `kernel::semantic::<module>::…` path resolving.

pub use semantic::{
    engine, language_registry, lsp_proto, lsp_provider, project_graph, provider,
    symbol_index, treesitter_provider, ts_registry,
};

pub use semantic::{
    CompletionItem, CompletionKind, Diagnostic, DiagnosticSeverity, FileNode,
    LanguageInfo, LanguageProvider, LanguageRegistry, LspLanguageProvider,
    ParseResult, ProjectGraph, SemanticEngine, Symbol, SymbolIndex, SymbolKind,
    SymbolRef, TreesitterLanguageProvider,
};

pub mod edit_apply;
pub mod lsp;
pub mod ts_schedule;

pub use edit_apply::{apply_workspace_edit, offset_of_lsp_position};
pub use lsp::LspRegistry;
