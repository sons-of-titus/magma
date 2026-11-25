# Nix plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "nix" "treesitter")
(semantic/register-provider "nix" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .nix buffers by the core extension map on
# buffer-created.

(major-mode/define "nix-mode"
  {:parent "prog"
   :layers [:nix-mode]
   :setup (fn []
            (def width (option/get "nix-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "nix" {:command "nixpkgs-fmt" :args @[]})

