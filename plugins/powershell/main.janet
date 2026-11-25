# Powershell plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "powershell" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "powershell"
  ["if" "elseif" "else" "switch" "foreach" "for" "while" "do" "until"
   "break" "continue" "return" "throw" "try" "catch" "finally" "trap"
   "function" "filter" "param" "begin" "process" "end" "in" "exit"
   "class" "enum" "using" "workflow" "configuration" "namespace"
   "module" "data" "dynamicparam" "default" "hidden" "static"
   "true" "false" "null"]
  "#")

(syntax/map-extension "ps1" "powershell")
(syntax/map-extension "psm1" "powershell")
(syntax/map-extension "psd1" "powershell")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "powershell-mode"
  {:parent "prog"
   :layers [:powershell-mode]
   :setup (fn []
            (def width (option/get "powershell-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
