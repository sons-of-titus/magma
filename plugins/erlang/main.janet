# Erlang plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "erlang" "treesitter")
(semantic/register-provider "erlang" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .erl buffers by the core extension map on
# buffer-created.

(major-mode/define "erlang-mode"
  {:parent "prog"
   :layers [:erlang-mode]
   :setup (fn []
            (def width (option/get "erlang-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
