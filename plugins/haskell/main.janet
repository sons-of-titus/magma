# Haskell plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "haskell" "treesitter")
(semantic/register-provider "haskell" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "cabal.project")
              (marker "stack.yaml"))
      (lsp/start "haskell" (option/get "haskell-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .hs buffers by the core extension map on
# buffer-created.

(major-mode/define "haskell-mode"
  {:parent "prog"
   :layers [:haskell-mode]
   :setup (fn []
            (def width (option/get "haskell-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn haskell/format
  "Format source text with fourmolu; returns the input unchanged on failure."
  [text]
  (def result (process/run "fourmolu" @[] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "haskell-mode"))
    (def formatted (haskell/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "haskell-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "haskell-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
