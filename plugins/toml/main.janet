# Toml plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "toml" "treesitter")
(semantic/register-provider "toml" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .toml buffers by the core extension map on
# buffer-created.

(major-mode/define "toml-mode"
  {:parent "prog"
   :layers [:toml-mode]
   :setup (fn []
            (def width (option/get "toml-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
