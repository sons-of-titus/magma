# Org plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "org" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .org buffers by the core extension map on
# buffer-created.

(major-mode/define "org-mode"
  {:parent "text"
   :layers [:org-mode]
   :setup (fn []
            (def width (option/get "org-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
