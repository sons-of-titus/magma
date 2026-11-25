# Yaml plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "yaml" "treesitter")
(semantic/register-provider "yaml" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .yaml/.yml buffers by the core extension map on
# buffer-created.

(major-mode/define "yaml-mode"
  {:parent "prog"
   :layers [:yaml-mode]
   :setup (fn []
            (def width (option/get "yaml-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
