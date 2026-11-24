# Kotlin plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "kotlin" "treesitter")
(semantic/register-provider "kotlin" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .kt buffers by the core extension map on
# buffer-created.

(major-mode/define "kotlin-mode"
  {:parent "prog"
   :layers [:kotlin-mode]
   :setup (fn []
            (def width (option/get "kotlin-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "kotlin" {:command "ktlint" :args @["--stdin"]})

