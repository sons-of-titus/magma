# Toml plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "toml" "treesitter")
(semantic/register-provider "toml" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────
# taplo serves single files, so it starts with the first toml buffer
# rather than waiting for a project to open.

(def toml-exts @["toml"])

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
      (when (and ext (index-of ext toml-exts))
        (lsp/start "toml" (option/get "toml-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .toml buffers by the core extension map on
# buffer-created.

(major-mode/define "toml-mode"
  {:parent "prog"
   :layers [:toml-mode]
   :setup (fn []
            (def width (option/get "toml-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
