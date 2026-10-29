# Protobuf plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "protobuf" "treesitter")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .proto buffers by the core extension map on
# buffer-created.

(major-mode/define "protobuf-mode"
  {:parent "prog"
   :layers [:protobuf-mode]
   :setup (fn []
            (def width (option/get "protobuf-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
