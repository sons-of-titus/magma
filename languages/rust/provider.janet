# Rust Language Provider
#
# Registers the Rust language with the Semantic Engine using both an LSP
# provider (rust-analyzer) and a tree-sitter provider for offline parsing.
#
# This file is loaded by `language/load "rust"` and expected to call
# `semantic/register-provider` for the "rust" language ID.

(defn- rust-lsp-start
  "Start rust-analyzer for `root` if it is not already running."
  [root]
  (lsp/start "rust-analyzer" root))

# Register the tree-sitter provider for immediate offline parsing.
(semantic/register-provider "rust" "treesitter")

# Wire LSP: when a Rust project is opened, start rust-analyzer.
(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "rust")
              (os/stat (string root "/Cargo.toml")))
      (rust-lsp-start root))))

# Also register the LSP provider in the Semantic Engine so completions
# from rust-analyzer are surfaced alongside tree-sitter completions.
(semantic/register-provider "rust" "lsp")

# File extension → language binding (used by major-mode detection).
(defn rust/extensions [] @[".rs"])

# Debug adapter name exposed to the Debug System.
(defn rust/debug-adapter [] "lldb")

# Format a buffer using rustfmt (synchronous, small files only).
(defn rust/format [text]
  (def result (process/run "rustfmt" @["--edition" "2021"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
