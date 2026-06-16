//! Pure-Rust tests for the Semantic Engine subsystem (no Janet VM).

use crate::kernel::semantic::{
    Diagnostic, DiagnosticSeverity, LspLanguageProvider, SemanticEngine,
    Symbol, SymbolIndex, SymbolKind, ProjectGraph,
};
use crate::kernel::semantic::provider::LanguageProvider;
use crate::tests::helpers;

// ── LanguageProvider trait ────────────────────────────────────────────────────

#[test]
fn lsp_provider_always_succeeds_parse() {
    let p = LspLanguageProvider::new("rust");
    let r = p.parse("fn main() {}");
    assert!(r.success);
    assert!(r.errors.is_empty());
}

#[test]
fn lsp_provider_debug_adapter_contains_lang() {
    let p = LspLanguageProvider::new("go");
    assert_eq!(p.debug_adapter(), Some("lsp:go".to_string()));
}

// ── Diagnostic parsing ────────────────────────────────────────────────────────

#[test]
fn diagnostic_parse_error_severity() {
    let d = Diagnostic::from_formatted("[E] line 1:1 missing semicolon").unwrap();
    assert_eq!(d.severity, DiagnosticSeverity::Error);
    assert_eq!(d.line, 0);
    assert_eq!(d.column, 0);
    assert!(d.message.contains("missing semicolon"));
}

#[test]
fn diagnostic_parse_warning_severity() {
    let d = Diagnostic::from_formatted("[W] line 5:10 unused import").unwrap();
    assert_eq!(d.severity, DiagnosticSeverity::Warning);
    assert_eq!(d.line, 4);
    assert_eq!(d.column, 9);
}

#[test]
fn diagnostic_parse_hint() {
    let d = Diagnostic::from_formatted("[H] line 2:3 consider renaming").unwrap();
    assert_eq!(d.severity, DiagnosticSeverity::Hint);
}

#[test]
fn diagnostic_parse_malformed_returns_none() {
    assert!(Diagnostic::from_formatted("").is_none());
    assert!(Diagnostic::from_formatted("no brackets").is_none());
    assert!(Diagnostic::from_formatted("[E] no line prefix").is_none());
}

// ── SymbolIndex ───────────────────────────────────────────────────────────────

fn make_sym(name: &str, kind: SymbolKind) -> Symbol {
    Symbol { name: name.to_string(), kind, file: String::new(), line: 0, column: 0, documentation: None }
}

#[test]
fn symbol_index_add_and_lookup() {
    let mut idx = SymbolIndex::new();
    idx.add_symbols("lib.rs", vec![make_sym("parse", SymbolKind::Function)]);
    let syms = idx.lookup("parse");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].file, "lib.rs");
}

#[test]
fn symbol_index_file_replace() {
    let mut idx = SymbolIndex::new();
    idx.add_symbols("a.rs", vec![make_sym("old_fn", SymbolKind::Function)]);
    idx.add_symbols("a.rs", vec![make_sym("new_fn", SymbolKind::Function)]);
    assert!(idx.lookup("old_fn").is_empty(), "stale symbols should be replaced");
    assert_eq!(idx.lookup("new_fn").len(), 1);
}

#[test]
fn symbol_index_multi_file() {
    let mut idx = SymbolIndex::new();
    idx.add_symbols("a.rs", vec![make_sym("shared", SymbolKind::Function)]);
    idx.add_symbols("b.rs", vec![make_sym("shared", SymbolKind::Function)]);
    let defs = idx.lookup("shared");
    assert_eq!(defs.len(), 2);
}

#[test]
fn symbol_index_completions_prefix() {
    let mut idx = SymbolIndex::new();
    idx.add_symbols("x.rs", vec![
        make_sym("render_frame",    SymbolKind::Function),
        make_sym("render_surface",  SymbolKind::Function),
        make_sym("dispatch_key",    SymbolKind::Function),
    ]);
    let c = idx.completions_for_prefix("render");
    assert_eq!(c, vec!["render_frame", "render_surface"]);
    assert!(idx.completions_for_prefix("zzz").is_empty());
}

#[test]
fn symbol_index_references() {
    let mut idx = SymbolIndex::new();
    idx.add_reference("foo", "a.rs", 10, 5);
    idx.add_reference("foo", "b.rs", 20, 0);
    let refs = idx.references("foo");
    assert_eq!(refs.len(), 2);
    assert!(refs.iter().any(|r| r.file == "a.rs"));
}

#[test]
fn symbol_index_documentation() {
    let mut idx = SymbolIndex::new();
    let mut sym = make_sym("documented_fn", SymbolKind::Function);
    sym.documentation = Some("The best function.".to_string());
    idx.add_symbols("docs.rs", vec![sym]);
    assert_eq!(idx.documentation("documented_fn"), Some("The best function.".to_string()));
    assert!(idx.documentation("missing").is_none());
}

#[test]
fn symbol_index_remove_file() {
    let mut idx = SymbolIndex::new();
    idx.add_symbols("a.rs", vec![make_sym("alpha", SymbolKind::Function)]);
    idx.add_symbols("b.rs", vec![make_sym("beta", SymbolKind::Function)]);
    idx.remove_file("a.rs");
    assert!(idx.lookup("alpha").is_empty());
    assert_eq!(idx.lookup("beta").len(), 1);
}

// ── ProjectGraph ──────────────────────────────────────────────────────────────

#[test]
fn project_graph_add_file() {
    let mut g = ProjectGraph::new();
    g.add_file("main.rs", "rust", vec!["main".to_string()]);
    assert_eq!(g.file_count(), 1);
    let node = g.files.get("main.rs").unwrap();
    assert_eq!(node.language, "rust");
}

#[test]
fn project_graph_dependencies() {
    let mut g = ProjectGraph::new();
    g.add_dependency("main.rs", "lib.rs");
    assert!(g.dependencies_of("main.rs").contains(&"lib.rs".to_string()));
    assert!(g.dependents_of("lib.rs").contains(&"main.rs".to_string()));
}

#[test]
fn project_graph_transitive_deps() {
    let mut g = ProjectGraph::new();
    g.add_dependency("main.rs", "lib.rs");
    g.add_dependency("lib.rs", "util.rs");
    let trans = g.transitive_dependencies("main.rs");
    assert!(trans.contains(&"lib.rs".to_string()));
    assert!(trans.contains(&"util.rs".to_string()));
    assert!(!trans.contains(&"main.rs".to_string()));
}

// ── SemanticEngine ────────────────────────────────────────────────────────────

#[test]
fn engine_register_and_query_provider() {
    let mut engine = SemanticEngine::new();
    assert!(!engine.has_provider("rust"));
    engine.register_provider("rust", Box::new(LspLanguageProvider::new("rust")));
    assert!(engine.has_provider("rust"));
    assert!(!engine.has_provider("python"));
}

#[test]
fn engine_index_buffer_lsp_no_symbols() {
    let mut engine = SemanticEngine::new();
    engine.register_provider("rust", Box::new(LspLanguageProvider::new("rust")));
    engine.index_buffer("rust", "/src/main.rs", "fn main() {}");
    assert_eq!(engine.symbol_index.symbol_count(), 0, "LSP provider has no sync symbols");
    assert_eq!(engine.project_graph.file_count(), 1);
}

#[test]
fn engine_update_diagnostics_from_strings() {
    let mut engine = SemanticEngine::new();
    engine.update_diagnostics_from_strings("/a.rs", &[
        "[E] line 1:1 error message".to_string(),
        "[W] line 2:5 warning message".to_string(),
    ]);
    assert_eq!(engine.error_count("/a.rs"), 1);
    assert_eq!(engine.warning_count("/a.rs"), 1);
}

#[test]
fn engine_diagnostics_empty_removes_path() {
    let mut engine = SemanticEngine::new();
    engine.update_diagnostics_from_strings("/b.rs", &["[E] line 1:1 err".to_string()]);
    assert_eq!(engine.diagnostics.len(), 1);
    engine.update_diagnostics("/b.rs", vec![]);
    assert_eq!(engine.diagnostics.len(), 0);
}

#[test]
fn engine_completions_for_prefix_from_index() {
    let mut engine = SemanticEngine::new();
    engine.symbol_index.add_symbols("/src.rs", vec![
        make_sym("buffer_insert", SymbolKind::Function),
        make_sym("buffer_delete", SymbolKind::Function),
        make_sym("event_emit",    SymbolKind::Function),
    ]);
    let comps = engine.completions_for_prefix("buffer");
    assert_eq!(comps, vec!["buffer_delete", "buffer_insert"]);
}

#[test]
fn engine_symbols_for_buffer_no_provider_empty() {
    let engine = SemanticEngine::new();
    let syms = engine.symbols_for_buffer("unknown", "fn main() {}");
    assert!(syms.is_empty());
}

// ── Integration: completion_trigger uses semantic symbols ─────────────────────

#[test]
fn completion_trigger_uses_semantic_index() {
    let mut ed = helpers::make_editor_with_buffer("fn main() {\n    buf\n}");
    // Manually index a symbol so the semantic engine has "buffer_write".
    ed.semantic.symbol_index.add_symbols("/test", vec![
        make_sym("buffer_write", SymbolKind::Function),
        make_sym("buffer_read",  SymbolKind::Function),
    ]);
    // Focus the buffer and place cursor after "buf" on line 2.
    let key = helpers::focused_key(&ed);
    if let Some(view) = ed.views.get_mut(&key) {
        // "fn main() {" (11) + "\n" (1) + "    " (4) = 16, then "buf" (3) → cursor at 19
        view.set_cursor(19);
    }
    crate::kernel::command::execute_command(&mut ed, "completion-trigger", &Default::default()).unwrap();
    assert!(ed.completion.visible, "completion should be visible");
    assert!(
        ed.completion.items.iter().any(|i| i.starts_with("buffer")),
        "semantic symbols should appear in completions: {:?}", ed.completion.items
    );
}
