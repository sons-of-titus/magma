# Ini plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "ini" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "ini"
  ["true" "false" "yes" "no" "on" "off" "null"]
  ";")

(each ext ["ini" "cfg" "conf" "properties" "env"]
  (syntax/map-extension ext "ini"))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "ini-mode"
  {:parent "text"
   :layers [:ini-mode]
   :setup (fn []
            (def width (option/get "ini-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
