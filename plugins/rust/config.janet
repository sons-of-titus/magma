# Rust plugin defaults.  Users override these in their init.janet.

# Indentation
(option/set "rust-indent-width" "4")
(option/set "rust-indent-style" "space")

# Comment prefix for line comments
(option/set "rust-comment-prefix" "// ")

# LSP server executable
(option/set "rust-lsp-command" "rust-analyzer")

# Maximum line length used by format hints
(option/set "rust-max-line-length" "100")

# Debug adapter used when launching a debuggable run configuration
(option/set "rust-debug-adapter" "lldb")

# Format with rustfmt before every save of a Rust buffer
(option/set "rust-format-on-save" "false")
