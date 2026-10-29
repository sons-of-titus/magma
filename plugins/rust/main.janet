# Rust plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

# ── Completion + highlighting ────────────────────────────────────────────────
# tree-sitter serves offline parsing immediately; rust-analyzer is layered
# on for the semantic surface once a project opens.

(semantic/register-provider "rust" "treesitter")
(semantic/register-provider "rust" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (def name (get data :name ""))
    (def cargo-project (try (os/stat (string root "/Cargo.toml")) ([_] nil)))
    (when (or (= name "rust") cargo-project)
      (lsp/start "rust" (option/get "rust-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .rs buffers by the core extension map on buffer-created.

(major-mode/define "rust-mode"
  {:parent "prog"
   :layers [:rust-mode]
   :setup (fn []
            (def width (option/get "rust-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Debugging ────────────────────────────────────────────────────────────────
# The `run` configuration below is debuggable: launching it under the debug
# toolbar starts a debug session against the built binary.

# ── Formatting ───────────────────────────────────────────────────────────────

(defn rust/format
  "Format source text with rustfmt; returns the input unchanged on failure."
  [text]
  (def result (process/run "rustfmt" @["--edition" "2021"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "rust-mode"))
    (def formatted (rust/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "rust-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "rust-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))

# ── Run configurations ───────────────────────────────────────────────────────

(run-config/register "build" "cargo" @["build"])
(run-config/register "test" "cargo" @["test"])
(run-config/register "run" "cargo" @["run"] nil true)
