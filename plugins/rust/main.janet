# Rust plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

# ── Completion + highlighting ────────────────────────────────────────────────
# tree-sitter serves offline parsing immediately; rust-analyzer is layered
# on for the semantic surface once a project opens.

(semantic/register-provider "rust" "treesitter")
(semantic/register-provider "rust" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .rs buffers by the core extension map on buffer-created.

(major-mode/define "rust-mode"
  {:parent "prog"
   :layers [:rust-mode]
   :setup (fn []
            (def width (option/get "rust-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Debugging ────────────────────────────────────────────────────────────────
# The `run` configuration below is debuggable: launching it under the debug
# toolbar starts a debug session against the built binary.

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "rust" {:command "rustfmt" :args @["--edition" "2021"]})

# ── Run configurations ───────────────────────────────────────────────────────
# Seeded by the rust.cargo toolchain declaration in manifest.janet: marker
# detection on project-opened installs build/test/run/clean/format/lint,
# and the plugin's templates live under project-templates/.
