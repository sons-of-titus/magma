# Php plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "php" "treesitter")
(semantic/register-provider "php" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "php"
  ["abstract" "and" "array" "as" "break" "callable" "case" "catch"
   "class" "clone" "const" "continue" "declare" "default" "do" "echo"
   "else" "elseif" "empty" "enum" "extends" "final" "finally" "fn"
   "for" "foreach" "function" "global" "goto" "if" "implements"
   "include" "include_once" "instanceof" "interface" "isset" "list"
   "match" "namespace" "new" "or" "print" "private" "protected"
   "public" "readonly" "require" "require_once" "return" "static"
   "switch" "throw" "trait" "try" "unset" "use" "var" "while" "xor"
   "yield" "true" "false" "null" "this"]
  "//")

(syntax/map-extension "php" "php")
(syntax/map-extension "phtml" "php")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "php-mode"
  {:parent "prog"
   :layers [:php-mode]
   :setup (fn []
            (def width (option/get "php-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
