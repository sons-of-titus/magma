# Java plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "java" "treesitter")
(semantic/register-provider "java" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "pom.xml")
              (marker "build.gradle"))
      (lsp/start "java" (option/get "java-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .java buffers by the core extension map on
# buffer-created.

(major-mode/define "java-mode"
  {:parent "prog"
   :layers [:java-mode]
   :setup (fn []
            (def width (option/get "java-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
