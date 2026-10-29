# Json plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "json" "treesitter")
(semantic/register-provider "json" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────
# vscode-json-languageserver serves single files, so it starts with the first json buffer
# rather than waiting for a project to open.

(def json-exts @["json"])

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
      (when (and ext (index-of ext json-exts))
        (lsp/start "json" (option/get "json-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .json buffers by the core extension map on
# buffer-created.

(major-mode/define "json-mode"
  {:parent "prog"
   :layers [:json-mode]
   :setup (fn []
            (def width (option/get "json-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
