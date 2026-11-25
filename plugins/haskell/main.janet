# Haskell plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "haskell" "treesitter")
(semantic/register-provider "haskell" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .hs buffers by the core extension map on
# buffer-created.

(major-mode/define "haskell-mode"
  {:parent "prog"
   :layers [:haskell-mode]
   :setup (fn []
            (def width (option/get "haskell-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "haskell" {:command "fourmolu" :args @[]})

