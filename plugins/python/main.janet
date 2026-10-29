# Python plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "python" "treesitter")
(semantic/register-provider "python" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "pyproject.toml")
              (marker "setup.py")
              (marker "requirements.txt"))
      (lsp/start "python" (option/get "python-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .python buffers by the core extension map on
# buffer-created.

(major-mode/define "python-mode"
  {:parent "prog"
   :layers [:python-mode]
   :setup (fn []
            (def width (option/get "python-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn python/format
  "Format source text with black; returns the input unchanged on failure."
  [text]
  (def result (process/run "black" @["-q" "-"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "python-mode"))
    (def formatted (python/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "python-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "python-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
