# Janet Language Provider
#
# Registers the Janet language with the Semantic Engine.  Janet does not have
# an LSP server, so only the tree-sitter provider is registered for offline
# symbol extraction.

# Register tree-sitter provider for Janet source files.
(semantic/register-provider "janet" "treesitter")

# File extension → language binding.
(defn janet/extensions [] @[".janet" ".jdn"])

# Janet does not have a debug adapter.
(defn janet/debug-adapter [] nil)

# Format Janet source by re-indenting (no external formatter available).
(defn janet/format [text] text)

# Keyword completions specific to Janet core.
(def janet/core-keywords
  @["defn" "defn-" "def" "var" "let" "fn" "do" "if" "when" "unless"
    "cond" "case" "each" "eachp" "eachk" "map" "filter" "reduce"
    "array" "table" "string" "tuple" "struct" "buffer" "fiber"
    "try" "protect" "error" "assert" "print" "pp" "string/format"
    "os/stat" "os/dir" "os/getcwd" "math/floor" "math/ceil"])
