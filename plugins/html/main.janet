# Html plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "html" "treesitter")
(semantic/register-provider "html" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────
# vscode-html-languageserver serves single files, so it starts with the first html buffer
# rather than waiting for a project to open.

(def html-exts @["html" "htm"])

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
      (when (and ext (index-of ext html-exts))
        (lsp/start "html" (option/get "html-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .html/.htm buffers by the core extension map on
# buffer-created.

(major-mode/define "html-mode"
  {:parent "prog"
   :layers [:html-mode]
   :setup (fn []
            (def width (option/get "html-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
