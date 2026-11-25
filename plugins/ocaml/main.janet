# Ocaml plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "ocaml" "treesitter")
(semantic/register-provider "ocaml" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "ocaml"
  ["let" "in" "rec" "if" "then" "else" "match" "with" "when" "begin"
   "end" "type" "of" "and" "module" "sig" "struct" "functor" "open"
   "include" "try" "exception" "function" "fun" "mutable" "ref" "true"
   "false" "not" "ignore" "as" "private" "virtual" "inherit"
   "constraint" "lazy" "assert"]
  "(*")

(syntax/map-extension "ml" "ocaml")
(syntax/map-extension "mli" "ocaml")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "ocaml-mode"
  {:parent "prog"
   :layers [:ocaml-mode]
   :setup (fn []
            (def width (option/get "ocaml-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
