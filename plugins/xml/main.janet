# Xml plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "xml" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .xml buffers by the core extension map on
# buffer-created.

(major-mode/define "xml-mode"
  {:parent "prog"
   :layers [:xml-mode]
   :setup (fn []
            (def width (option/get "xml-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
