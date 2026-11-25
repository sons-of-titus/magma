# Groovy plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "groovy" "treesitter")
(semantic/register-provider "groovy" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "groovy"
  ["def" "if" "else" "for" "while" "in" "return" "switch" "case"
   "break" "continue" "import" "package" "new" "try" "catch" "finally"
   "throw" "class" "interface" "trait" "extends" "implements" "public"
   "private" "protected" "static" "final" "abstract" "void" "boolean"
   "int" "long" "double" "float" "String" "true" "false" "null" "it"
   "as" "this" "super" "assert" "synchronized" "volatile" "transient"
   "native" "enum"]
  "//")

(syntax/map-extension "groovy" "groovy")
(syntax/map-extension "gradle" "groovy")
(syntax/map-extension "jenkinsfile" "groovy")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "groovy-mode"
  {:parent "prog"
   :layers [:groovy-mode]
   :setup (fn []
            (def width (option/get "groovy-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
