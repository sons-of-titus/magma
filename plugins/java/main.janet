# Java plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "java" "treesitter")
(semantic/register-provider "java" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .java buffers by the core extension map on
# buffer-created.

(major-mode/define "java-mode"
  {:parent "prog"
   :layers [:java-mode]
   :setup (fn []
            (def width (option/get "java-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
