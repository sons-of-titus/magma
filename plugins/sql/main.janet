# Sql plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "sql" "treesitter")
(semantic/register-provider "sql" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .sql buffers by the core extension map on
# buffer-created.

(major-mode/define "sql-mode"
  {:parent "prog"
   :layers [:sql-mode]
   :setup (fn []
            (def width (option/get "sql-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
