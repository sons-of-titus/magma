# Nim plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "nim" "treesitter")
(semantic/register-provider "nim" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .nim buffers by the core extension map on
# buffer-created.

(major-mode/define "nim-mode"
  {:parent "prog"
   :layers [:nim-mode]
   :setup (fn []
            (def width (option/get "nim-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
