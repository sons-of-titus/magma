# Yaml plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "yaml" "treesitter")
(semantic/register-provider "yaml" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────
# yaml-language-server serves single files, so it starts with the first yaml buffer
# rather than waiting for a project to open.

(def yaml-exts @["yaml" "yml"])

(defn- path-extension
  "File extension of `path` (after the last dot), or nil."
  [path]
  (when-let [i (last (string/find-all "." path))]
    (string/slice path (+ i 1))))

(event/on "buffer-created"
  (fn [data]
    (def path (get data :path ""))
    (when (> (length path) 0)
      (def ext (path-extension path))
      (when (and ext (index-of ext yaml-exts))
        (lsp/start "yaml" (option/get "yaml-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .yaml/.yml buffers by the core extension map on
# buffer-created.

(major-mode/define "yaml-mode"
  {:parent "prog"
   :layers [:yaml-mode]
   :setup (fn []
            (def width (option/get "yaml-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
