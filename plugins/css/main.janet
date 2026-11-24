# Css plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "css" "treesitter")
(semantic/register-provider "css" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .css/.scss buffers by the core extension map on
# buffer-created.

(major-mode/define "css-mode"
  {:parent "prog"
   :layers [:css-mode]
   :setup (fn []
            (def width (option/get "css-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
