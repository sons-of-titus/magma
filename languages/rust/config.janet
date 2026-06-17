# Rust language configuration.
#
# Loaded after provider.janet to set language-specific options.

# Indentation
(option/set "rust-indent-width" "4")
(option/set "rust-indent-style" "space")

# Comment prefix for line comments
(option/set "rust-comment-prefix" "// ")

# LSP server executable
(option/set "rust-lsp-command" "rust-analyzer")

# Maximum line length used by format hints
(option/set "rust-max-line-length" "100")

# Debug adapter
(option/set "rust-debug-adapter" "lldb")
