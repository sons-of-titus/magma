# Go plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "go" "treesitter")
(semantic/register-provider "go" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "go.mod"))
      (lsp/start "go" (option/get "go-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .go buffers by the core extension map on
# buffer-created.

(major-mode/define "go-mode"
  {:parent "prog"
   :layers [:go-mode]
   :setup (fn []
            (def width (option/get "go-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn go/format
  "Format source text with gofmt; returns the input unchanged on failure."
  [text]
  (def result (process/run "gofmt" @[] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "go-mode"))
    (def formatted (go/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "go-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "go-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
