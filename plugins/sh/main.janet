# Shell plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "sh" "treesitter")
(semantic/register-provider "sh" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────
# bash-language-server serves single files, so it starts with the first sh buffer
# rather than waiting for a project to open.

(def sh-exts @["sh" "bash" "zsh"])

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
      (when (and ext (index-of ext sh-exts))
        (lsp/start "sh" (option/get "sh-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .sh/.bash/.zsh buffers by the core extension map on
# buffer-created.

(major-mode/define "sh-mode"
  {:parent "prog"
   :layers [:sh-mode]
   :setup (fn []
            (def width (option/get "sh-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
