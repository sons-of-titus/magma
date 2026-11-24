# Ruby plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "ruby" "treesitter")
(semantic/register-provider "ruby" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .rb buffers by the core extension map on
# buffer-created.

(major-mode/define "ruby-mode"
  {:parent "prog"
   :layers [:ruby-mode]
   :setup (fn []
            (def width (option/get "ruby-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "ruby" {:command "rubocop" :args @["--auto-correct" "--stdin" "dummy.rb"]})

