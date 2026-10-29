# Tree-sitter grammars

Language plugins ship their tree-sitter grammar as a shared library, built
from the pinned upstream sources in `grammars.toml`.  This document covers
the layout, the build workflow per platform, and the compatibility rules.

## Layout

Each bundled language plugin carries a `grammars/` directory holding its
compiled grammar for the running platform:

    plugins/rust/grammars/tree-sitter-rust.dylib   (macOS)
    plugins/sh/grammars/tree-sitter-sh.so          (Linux)
    plugins/python/grammars/tree-sitter-python.dll (Windows)

Built libraries are gitignored; `grammars.toml` — repo, pinned commit,
parser sources — is the checked-in source of truth.  `plugin/define` picks
the library up by convention (`ts/grammar-lib-path`): a plugin whose
`grammars/tree-sitter-<lang>.<ext>` exists for the running platform loads
it at registration and gets tree-sitter highlighting; one without falls
back to the native keyword highlighter.  Manifests never reference the
grammar path explicitly.

## Building

    cargo xtask grammars --all            # every language in grammars.toml
    cargo xtask grammars --lang rust go   # a subset
    cargo xtask grammars --all --verify   # + dlopen every result and check
                                          #   export symbol and grammar ABI
    cargo xtask grammars --list           # show the pinned sources

The tool clones each repository into `target/grammars/` (cached across
runs), checks out the pinned revision, and compiles `parser.c` plus any
`scanner.c` / `scanner.cc` in the parser directory.  Repositories that no
longer vendor the generated parser are regenerated with the `tree-sitter`
CLI, which must be on `PATH` for those languages
(`cargo install tree-sitter-cli`).

### Platform notes

- **macOS / Linux** — uses `$CC` (default `cc`); C++ scanners compile with
  `$CXX` (default `c++`) and the C++ driver links when one is present.
- **Windows (MSVC)** — the toolchain is located through the `cc` crate's
  registry lookup (Visual Studio or Build Tools with the C++ workload
  installed).  A generated `.def` file exports the grammar's language
  function; Windows DLLs export nothing without it.
- **Windows (MinGW)** — falls back to `gcc` from `PATH`; `-shared` exports
  all public symbols by default.

## Compatibility rules

- The editor's tree-sitter runtime (workspace `tree-sitter` crate, 0.27)
  accepts grammar ABI versions 13–15.  `--verify` reads the ABI version
  from each built library and refuses anything outside that window; the
  editor's loader re-validates and refuses with a clean error at load.
- Grammars whose C entry point deviates from `tree_sitter_<language>`
  (dashes→underscores) declare their real export under `symbol` in
  `grammars.toml`; the same override is available to third-party plugins
  via the language spec's `:grammar-symbol` key.
- A grammar that cannot meet the ABI window is skipped deliberately —
  e.g. clojure (upstream still emits ABI 9), noted in `grammars.toml` —
  and its plugin keeps fallback highlighting.  Never ship a grammar that
  has not passed `--verify`.
