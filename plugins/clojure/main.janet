# Clojure plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "clojure" "treesitter")
(semantic/register-provider "clojure" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "deps.edn")
              (marker "project.clj"))
      (lsp/start "clojure" (option/get "clojure-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .clj buffers by the core extension map on
# buffer-created.

(major-mode/define "clojure-mode"
  {:parent "prog"
   :layers [:clojure-mode]
   :setup (fn []
            (def width (option/get "clojure-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
