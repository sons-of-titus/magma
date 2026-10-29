# Emacs-lisp plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "emacs-lisp" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .el buffers by the core extension map on
# buffer-created.

(major-mode/define "emacs-lisp-mode"
  {:parent "prog"
   :layers [:emacs-lisp-mode]
   :setup (fn []
            (def width (option/get "emacs-lisp-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
