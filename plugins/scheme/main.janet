# Scheme plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "scheme" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .scm buffers by the core extension map on
# buffer-created.

(major-mode/define "scheme-mode"
  {:parent "prog"
   :layers [:scheme-mode]
   :setup (fn []
            (def width (option/get "scheme-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
