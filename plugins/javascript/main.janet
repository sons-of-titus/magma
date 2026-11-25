# Javascript plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "javascript" "treesitter")
(semantic/register-provider "javascript" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .js/.jsx buffers by the core extension map on
# buffer-created.

(major-mode/define "javascript-mode"
  {:parent "prog"
   :layers [:javascript-mode]
   :setup (fn []
            (def width (option/get "javascript-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "javascript" {:command "npx" :args @["prettier" "--stdin-filepath" "dummy.js"]})

