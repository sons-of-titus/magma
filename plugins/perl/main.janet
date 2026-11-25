# Perl plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "perl" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "perl"
  ["sub" "my" "our" "local" "use" "no" "require" "package" "if" "elsif"
   "else" "unless" "while" "until" "for" "foreach" "do" "last" "next"
   "redo" "goto" "return" "wantarray" "eval" "die" "warn" "print"
   "printf" "sprintf" "exists" "delete" "defined" "bless" "ref" "scalar"
   "keys" "values" "each" "sort" "map" "grep" "push" "pop" "shift"
   "unshift" "chomp" "chop" "split" "join" "reverse" "lc" "uc" "length"
   "substr" "index" "int" "abs" "srand" "undef"]
  "#")

(syntax/map-extension "pl" "perl")
(syntax/map-extension "pm" "perl")
(syntax/map-extension "t" "perl")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "perl-mode"
  {:parent "prog"
   :layers [:perl-mode]
   :setup (fn []
            (def width (option/get "perl-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
