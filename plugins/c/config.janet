# C plugin defaults.  Users override these in their init.janet.

# Indentation
(option/set "c-indent-width" "4")
(option/set "c-indent-style" "space")

# Comment prefix for line comments
(option/set "c-comment-prefix" "// ")

# LSP server executable
(option/set "c-lsp-command" "clangd")

# Debug adapter suggested when launching a debug session

# Directory the CMake run configurations configure, build, and test in
(option/set "c-build-dir" "build")

# Format with clang-format before every save of a C buffer
(option/set "c-format-on-save" "false")
