# Cpp plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "cpp" "treesitter")
(semantic/register-provider "cpp" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "compile_commands.json")
              (marker "compile_flags.txt"))
      (lsp/start "cpp" (option/get "cpp-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .cpp/.cc/.cxx/.hpp buffers by the core extension map on
# buffer-created.

(major-mode/define "cpp-mode"
  {:parent "prog"
   :layers [:cpp-mode]
   :setup (fn []
            (def width (option/get "cpp-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn cpp/format
  "Format source text with clang-format; returns the input unchanged on failure."
  [text]
  (def result (process/run "clang-format" @["-style=file"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "cpp-mode"))
    (def formatted (cpp/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "cpp-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "cpp-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
