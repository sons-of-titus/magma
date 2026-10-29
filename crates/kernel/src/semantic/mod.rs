//! Semantic Engine — language intelligence for Magma.
//!
//! The provider/engine core (language providers, symbol index, project graph,
//! tree-sitter registry, LSP wire protocol, and the live LSP client) lives in
//! the `semantic` workspace crate.  The client reports through
//! `semantic::lsp::EventSink`; this module only adapts those events to the
//! kernel's background channel.
//!
//! The editor-coupled remainder stays here:
//!
//!   edit_apply.rs  — applying an LSP `WorkspaceEdit` to `Editor` buffers
//!   ts_schedule.rs — scheduling incremental parses onto the background pool
//!
//! The re-exports below keep every existing `kernel::semantic::<module>::…`
//! path resolving.

pub use semantic::{
    engine, language_registry, lsp, lsp_proto, lsp_provider, project_graph, provider, symbol_index,
    treesitter_provider, ts_registry,
};

pub use semantic::{
    CompletionItem, CompletionKind, Diagnostic, DiagnosticSeverity, FileNode, LanguageInfo,
    LanguageProvider, LanguageRegistry, LspLanguageProvider, ParseResult, ProjectGraph,
    SemanticEngine, Symbol, SymbolIndex, SymbolKind, SymbolRef, TreesitterLanguageProvider,
};

pub use semantic::{Event, LspRegistry};

pub mod edit_apply;
pub mod fallback_syntax;
pub mod ts_schedule;

pub use edit_apply::{apply_workspace_edit, offset_of_lsp_position};
