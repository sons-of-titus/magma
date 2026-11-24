# Lua plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "lua" "treesitter")
(semantic/register-provider "lua" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .lua buffers by the core extension map on
# buffer-created.

(major-mode/define "lua-mode"
  {:parent "prog"
   :layers [:lua-mode]
   :setup (fn []
            (def width (option/get "lua-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
