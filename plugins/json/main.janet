# Json plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "json" "treesitter")
(semantic/register-provider "json" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .json buffers by the core extension map on
# buffer-created.

(major-mode/define "json-mode"
  {:parent "prog"
   :layers [:json-mode]
   :setup (fn []
            (def width (option/get "json-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
