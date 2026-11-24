# Typescript plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "typescript" "treesitter")
(semantic/register-provider "typescript" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .ts/.tsx buffers by the core extension map on
# buffer-created.

(major-mode/define "typescript-mode"
  {:parent "prog"
   :layers [:typescript-mode]
   :setup (fn []
            (def width (option/get "typescript-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "typescript" {:command "npx" :args @["prettier" "--stdin-filepath" "dummy.ts"]})

