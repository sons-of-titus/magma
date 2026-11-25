# Markdown plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/hover.janet"))

(semantic/register-provider "markdown" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .md/.markdown buffers by the core extension map on
# buffer-created.

(major-mode/define "markdown-mode"
  {:parent "text"
   :layers [:markdown-mode]
   :setup (fn []
            (def width (option/get "markdown-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
