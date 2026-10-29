# Ruby plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "ruby" "treesitter")
(semantic/register-provider "ruby" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "Gemfile"))
      (lsp/start "ruby" (option/get "ruby-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .rb buffers by the core extension map on
# buffer-created.

(major-mode/define "ruby-mode"
  {:parent "prog"
   :layers [:ruby-mode]
   :setup (fn []
            (def width (option/get "ruby-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn ruby/format
  "Format source text with rubocop; returns the input unchanged on failure."
  [text]
  (def result (process/run "rubocop" @["--auto-correct" "--stdin" "dummy.rb"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "ruby-mode"))
    (def formatted (ruby/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "ruby-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "ruby-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
