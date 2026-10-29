//! Grammar shared-library loading, against a real compiled library.
//!
//! The stub C source below mirrors the `TSLanguage` header layout
//! (tree-sitter src/parser.h) far enough for `ts_parser_set_language`:
//! the ABI version at offset 0 and a non-NULL `lex_fn`.  Nothing parses
//! against it — the stub exists to exercise symbol probing and ABI
//! validation on a genuine shared library.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const STUB_C: &str = r#"
#include <stdint.h>
#include <stdbool.h>

#if defined(_WIN32)
#define STUB_EXPORT __declspec(dllexport)
#else
#define STUB_EXPORT __attribute__((visibility("default")))
#endif

typedef struct {
    uint32_t abi_version;
    uint32_t symbol_count;
    uint32_t alias_count;
    uint32_t token_count;
    uint32_t external_token_count;
    uint32_t state_count;
    uint32_t large_state_count;
    uint32_t production_id_count;
    uint32_t field_count;
    uint16_t max_alias_sequence_length;
    const void *parse_table;
    const void *small_parse_table;
    const void *small_parse_table_map;
    const void *parse_actions;
    const void *symbol_names;
    const void *field_names;
    const void *field_map_slices;
    const void *field_map_entries;
    const void *symbol_metadata;
    const void *public_symbol_map;
    const void *alias_map;
    const void *alias_sequences;
    const void *lex_modes;
    bool (*lex_fn)(void *lexer, uint32_t state);
    bool (*keyword_lex_fn)(void *lexer, uint32_t state);
} StubLanguage;

static bool stub_lex(void *lexer, uint32_t state) {
    (void)lexer; (void)state;
    return false;
}

#define STUB_HEADER 14, 1, 0, 1, 0, 1, 0, 0, 0, 0
#define STUB_TABLES 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0

static const StubLanguage good_language = {
    STUB_HEADER, STUB_TABLES, stub_lex, 0,
};
static const StubLanguage future_language = {
    99, 1, 0, 1, 0, 1, 0, 0, 0, 0, STUB_TABLES, stub_lex, 0,
};

STUB_EXPORT const void *tree_sitter_stublang(void) { return &good_language; }
STUB_EXPORT const void *tree_sitter_aliased(void) { return &good_language; }
STUB_EXPORT const void *tree_sitter_broken(void) { return &future_language; }
"#;

fn lib_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(windows) {
        "dll"
    } else {
        "so"
    }
}

/// Compile the stub once per test process; every test dlopens the result.
fn stub_lib() -> &'static PathBuf {
    static LIB: OnceLock<PathBuf> = OnceLock::new();
    LIB.get_or_init(|| {
        let dir = std::env::temp_dir().join("magma_stub_grammar_tests");
        std::fs::create_dir_all(&dir).expect("stub dir");
        let source = dir.join("stub.c");
        std::fs::write(&source, STUB_C).expect("write stub.c");
        let out = dir.join(format!("tree-sitter-stub.{}", lib_extension()));
        let _ = std::fs::remove_file(&out);
        compile(&source, &out);
        assert!(out.exists(), "stub library was not produced");
        out
    })
}

fn compile(source: &Path, out: &Path) {
    let status;
    #[cfg(unix)]
    {
        let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
        status = std::process::Command::new(cc)
            .args(["-O2", "-fPIC", "-shared"])
            .arg(source)
            .arg("-o")
            .arg(out)
            .status()
            .expect("run cc");
    }
    #[cfg(windows)]
    {
        let msvc_target = format!("{}-pc-windows-msvc", std::env::consts::ARCH);
        if let Some(tool) = cc::windows_registry::find_tool(&msvc_target, "cl") {
            let dir = out.parent().expect("out parent");
            let def = dir.join("stub.def");
            std::fs::write(&def, "EXPORTS\n    tree_sitter_stublang\n    tree_sitter_aliased\n    tree_sitter_broken\n")
                .expect("write def");
            let mut cmd = std::process::Command::new(tool.path());
            cmd.envs(tool.get_envs().map(|(k, v)| (k.to_os_string(), v.to_os_string())));
            cmd.current_dir(dir);
            cmd.args(["/nologo", "/O2", "/LD"]);
            cmd.arg(source);
            cmd.arg(format!("/Fe:{}", out.display()));
            cmd.arg(format!("/DEF:{}", def.display()));
            status = cmd.status().expect("run cl");
        } else {
            let gcc = std::env::var("CC").unwrap_or_else(|_| "gcc".into());
            status = std::process::Command::new(gcc)
                .args(["-O2", "-fPIC", "-shared"])
                .arg(source)
                .arg("-o")
                .arg(out)
                .status()
                .expect("run gcc");
        }
    }
    assert!(status.success(), "stub compilation failed");
}

#[test]
fn loads_grammar_by_conventional_symbol_name() {
    // "stublang" is deliberately absent from the legacy fallback list, so
    // success here proves the loader probes tree_sitter_<name>.
    let loaded = semantic::ts_registry::load_grammar_from_lib(
        stub_lib().to_str().expect("utf8 path"),
        "stublang",
        None,
    );
    assert!(loaded.is_ok(), "{:?}", loaded.err());
}

#[test]
fn explicit_symbol_overrides_the_convention() {
    let loaded = semantic::ts_registry::load_grammar_from_lib(
        stub_lib().to_str().expect("utf8 path"),
        "unrelated-registry-name",
        Some("tree_sitter_aliased"),
    );
    assert!(loaded.is_ok(), "{:?}", loaded.err());
}

#[test]
fn abi_outside_the_runtime_window_is_refused() {
    let loaded = semantic::ts_registry::load_grammar_from_lib(
        stub_lib().to_str().expect("utf8 path"),
        "broken",
        Some("tree_sitter_broken"),
    );
    let err = loaded.expect_err("an ABI-99 grammar must not load");
    assert!(err.contains("ABI"), "unexpected error: {err}");
}
