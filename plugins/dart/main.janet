# Dart plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "dart" "treesitter")
(semantic/register-provider "dart" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "dart"
  ["class" "extends" "implements" "with" "mixin" "abstract" "interface"
   "enum" "extension" "typedef" "library" "import" "export" "part" "of"
   "void" "int" "double" "num" "bool" "String" "var" "final" "const"
   "late" "static" "get" "set" "operator" "async" "await" "yield"
   "try" "catch" "finally" "throw" "rethrow" "if" "else" "for" "while"
   "do" "switch" "case" "default" "break" "continue" "return" "new"
   "this" "super" "is" "as" "in" "show" "hide" "external" "factory"
   "null" "true" "false"]
  "//")

(syntax/map-extension "dart" "dart")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "dart-mode"
  {:parent "prog"
   :layers [:dart-mode]
   :setup (fn []
            (def width (option/get "dart-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
