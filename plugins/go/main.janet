# Go plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "go" "treesitter")
(semantic/register-provider "go" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .go buffers by the core extension map on
# buffer-created.

(major-mode/define "go-mode"
  {:parent "prog"
   :layers [:go-mode]
   :setup (fn []
            (def width (option/get "go-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "go" {:command "gofmt" :args @[]})

