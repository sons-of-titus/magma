# Terraform plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "terraform" "treesitter")
(semantic/register-provider "terraform" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .tf buffers by the core extension map on
# buffer-created.

(major-mode/define "terraform-mode"
  {:parent "prog"
   :layers [:terraform-mode]
   :setup (fn []
            (def width (option/get "terraform-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "terraform" {:command "terraform" :args @["fmt" "-"]})

