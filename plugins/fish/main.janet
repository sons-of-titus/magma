# Fish plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "fish" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .fish buffers by the core extension map on
# buffer-created.

(major-mode/define "fish-mode"
  {:parent "prog"
   :layers [:fish-mode]
   :setup (fn []
            (def width (option/get "fish-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
