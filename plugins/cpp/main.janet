# Cpp plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "cpp" "treesitter")
(semantic/register-provider "cpp" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .cpp/.cc/.cxx/.hpp buffers by the core extension map on
# buffer-created.

(major-mode/define "cpp-mode"
  {:parent "prog"
   :layers [:cpp-mode]
   :setup (fn []
            (def width (option/get "cpp-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "cpp" {:command "clang-format" :args @["-style=file"]})

