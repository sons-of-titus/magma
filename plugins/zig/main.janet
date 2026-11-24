# Zig plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "zig" "treesitter")
(semantic/register-provider "zig" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .zig buffers by the core extension map on
# buffer-created.

(major-mode/define "zig-mode"
  {:parent "prog"
   :layers [:zig-mode]
   :setup (fn []
            (def width (option/get "zig-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "zig" {:command "zig" :args @["fmt" "--stdin"]})

