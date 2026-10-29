# Lisp plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "lisp" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .lisp buffers by the core extension map on
# buffer-created.

(major-mode/define "lisp-mode"
  {:parent "prog"
   :layers [:lisp-mode]
   :setup (fn []
            (def width (option/get "lisp-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
