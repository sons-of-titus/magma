# Sql plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "sql" "treesitter")
(semantic/register-provider "sql" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────
# sql-language-server serves single files, so it starts with the first sql buffer
# rather than waiting for a project to open.

(def sql-exts @["sql"])

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
      (when (and ext (index-of ext sql-exts))
        (lsp/start "sql" (option/get "sql-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .sql buffers by the core extension map on
# buffer-created.

(major-mode/define "sql-mode"
  {:parent "prog"
   :layers [:sql-mode]
   :setup (fn []
            (def width (option/get "sql-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
