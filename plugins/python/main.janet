# Python plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "python" "treesitter")
(semantic/register-provider "python" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .python buffers by the core extension map on
# buffer-created.

(major-mode/define "python-mode"
  {:parent "prog"
   :layers [:python-mode]
   :setup (fn []
            (def width (option/get "python-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "python" {:command "black" :args @["-q" "-"]})

