# Swift plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "swift" "treesitter")
(semantic/register-provider "swift" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .swift buffers by the core extension map on
# buffer-created.

(major-mode/define "swift-mode"
  {:parent "prog"
   :layers [:swift-mode]
   :setup (fn []
            (def width (option/get "swift-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "swift" {:command "swift-format" :args @["--mode" "stdin"]})

