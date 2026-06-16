//! Janet API tests for the Semantic Engine.

use crate::kernel::scripting;
use crate::kernel::semantic::{Symbol, SymbolKind};

// ── C function registration ───────────────────────────────────────────────────

#[test]
fn semantic_symbols_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(semantic/symbols 0)");
        assert_eq!(result, "ok", "semantic/symbols should return ok");
    });
}

#[test]
fn semantic_definitions_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(semantic/definitions "some-symbol")"#);
        assert_eq!(result, "ok");
    });
}

#[test]
fn semantic_references_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(semantic/references "some-symbol")"#);
        assert_eq!(result, "ok");
    });
}

#[test]
fn semantic_documentation_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(semantic/documentation "some-symbol")"#);
        assert_eq!(result, "ok");
    });
}

#[test]
fn semantic_index_buffer_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(semantic/index-buffer 0)");
        assert_eq!(result, "ok");
    });
}

#[test]
fn semantic_register_provider_lsp() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(semantic/register-provider "rust" "lsp")"#);
        assert_eq!(result, "ok");
        assert!(ed.semantic.has_provider("rust"), "rust lsp provider should be registered");
    });
}

#[test]
fn semantic_register_provider_treesitter() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(semantic/register-provider "python" "treesitter")"#);
        assert_eq!(result, "ok");
        assert!(ed.semantic.has_provider("python"), "python ts provider should be registered");
    });
}

#[test]
fn semantic_diagnostics_c_function_registered() {
    janet_test!(ed, {
        let result = scripting::eval("(semantic/diagnostics 0)");
        assert_eq!(result, "ok");
    });
}

// ── Semantic engine state after Janet calls ───────────────────────────────────

#[test]
fn semantic_symbols_returns_empty_array_no_language() {
    janet_test!(ed, {
        // Buffer 0 exists but has no ts_language set → empty array.
        let result = scripting::eval_result("(length (semantic/symbols 0))");
        assert_eq!(result, Ok("0".to_string()), "no language set → empty symbols");
    });
}

#[test]
fn semantic_documentation_returns_nil_for_unknown() {
    janet_test!(ed, {
        let result = scripting::eval_result(r#"(semantic/documentation "nonexistent-sym")"#);
        assert_eq!(result, Ok("nil".to_string()), "unknown symbol → nil documentation");
    });
}

#[test]
fn semantic_references_returns_empty_array_for_unknown() {
    janet_test!(ed, {
        let result = scripting::eval_result(r#"(length (semantic/references "nonexistent"))"#);
        assert_eq!(result, Ok("0".to_string()));
    });
}

#[test]
fn semantic_diagnostics_empty_for_no_path() {
    janet_test!(ed, {
        let result = scripting::eval_result("(length (semantic/diagnostics 0))");
        assert_eq!(result, Ok("0".to_string()), "buffer with no path → empty diagnostics");
    });
}

#[test]
fn semantic_index_buffer_indexes_when_language_set() {
    janet_test!(ed, {
        // Set a ts language on the buffer so index-buffer actually calls a provider.
        scripting::eval(r#"(semantic/register-provider "rust" "lsp")"#);
        scripting::eval("(ts/set-language 0 \"rust\")");
        let result = scripting::eval("(semantic/index-buffer 0)");
        assert_eq!(result, "ok");
        // LSP provider returns no symbols synchronously, but the project graph should have the file.
        // (buffer 0 has no path so graph entry will have empty path)
        assert_eq!(ed.semantic.project_graph.file_count(), 1,
            "project graph should have one entry after indexing");
    });
}

// ── Rust-side state updated from Janet ───────────────────────────────────────

#[test]
fn register_provider_reflects_in_rust_state() {
    janet_test!(ed, {
        assert!(!ed.semantic.has_provider("elixir"));
        scripting::eval(r#"(semantic/register-provider "elixir" "lsp")"#);
        assert!(ed.semantic.has_provider("elixir"));
    });
}

#[test]
fn semantic_definitions_returns_indexed_symbols() {
    janet_test!(ed, {
        // Manually inject a symbol into the index.
        ed.semantic.symbol_index.add_symbols("/src/lib.rs", vec![
            Symbol {
                name: "my_special_fn".to_string(),
                kind: SymbolKind::Function,
                file: "/src/lib.rs".to_string(),
                line: 10,
                column: 0,
                documentation: None,
            },
        ]);
        let result = scripting::eval_result(r#"(length (semantic/definitions "my_special_fn"))"#);
        assert_eq!(result, Ok("1".to_string()), "indexed symbol should be found");
    });
}

#[test]
fn semantic_documentation_returns_indexed_doc() {
    janet_test!(ed, {
        let sym = Symbol {
            name: "documented_fn".to_string(),
            kind: SymbolKind::Function,
            file: "/src/lib.rs".to_string(),
            line: 5,
            column: 0,
            documentation: Some("Does the thing.".to_string()),
        };
        ed.semantic.symbol_index.add_symbols("/src/lib.rs", vec![sym]);
        let result = scripting::eval_result(r#"(semantic/documentation "documented_fn")"#);
        assert_eq!(result, Ok("\"Does the thing.\"".to_string()));
    });
}
