# Scala plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "scala" "treesitter")
(semantic/register-provider "scala" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "scala"
  ["def" "val" "var" "lazy" "object" "class" "trait" "case" "extends"
   "with" "for" "yield" "if" "then" "else" "while" "do" "match"
   "return" "new" "import" "export" "package" "sealed" "implicit"
   "override" "abstract" "final" "private" "protected" "type" "this"
   "super" "null" "true" "false" "require" "given" "using" "enum"
   "end" "throw" "try" "catch" "finally"]
  "//")

(syntax/map-extension "scala" "scala")
(syntax/map-extension "sc" "scala")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "scala-mode"
  {:parent "prog"
   :layers [:scala-mode]
   :setup (fn []
            (def width (option/get "scala-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
