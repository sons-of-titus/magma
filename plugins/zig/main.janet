# Zig plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "zig" "treesitter")
(semantic/register-provider "zig" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "build.zig"))
      (lsp/start "zig" (option/get "zig-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .zig buffers by the core extension map on
# buffer-created.

(major-mode/define "zig-mode"
  {:parent "prog"
   :layers [:zig-mode]
   :setup (fn []
            (def width (option/get "zig-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn zig/format
  "Format source text with zig fmt; returns the input unchanged on failure."
  [text]
  (def result (process/run "zig" @["fmt" "--stdin"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "zig-mode"))
    (def formatted (zig/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "zig-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "zig-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
