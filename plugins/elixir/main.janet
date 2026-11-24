# Elixir plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "elixir" "treesitter")
(semantic/register-provider "elixir" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .ex/.exs buffers by the core extension map on
# buffer-created.

(major-mode/define "elixir-mode"
  {:parent "prog"
   :layers [:elixir-mode]
   :setup (fn []
            (def width (option/get "elixir-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "elixir" {:command "mix" :args @["format" "-"]})

