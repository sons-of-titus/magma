# Graphql plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "graphql" "treesitter")
(semantic/register-provider "graphql" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "graphql"
  ["query" "mutation" "subscription" "fragment" "on" "type" "input"
   "interface" "union" "enum" "scalar" "schema" "extend" "implements"
   "directive" "true" "false" "null"]
  "#")

(syntax/map-extension "graphql" "graphql")
(syntax/map-extension "gql" "graphql")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "graphql-mode"
  {:parent "prog"
   :layers [:graphql-mode]
   :setup (fn []
            (def width (option/get "graphql-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
