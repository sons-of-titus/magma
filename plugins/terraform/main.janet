# Terraform plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "terraform" "treesitter")
(semantic/register-provider "terraform" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker ".terraform"))
      (lsp/start "terraform" (option/get "terraform-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .tf buffers by the core extension map on
# buffer-created.

(major-mode/define "terraform-mode"
  {:parent "prog"
   :layers [:terraform-mode]
   :setup (fn []
            (def width (option/get "terraform-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn terraform/format
  "Format source text with terraform fmt; returns the input unchanged on failure."
  [text]
  (def result (process/run "terraform" @["fmt" "-"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "terraform-mode"))
    (def formatted (terraform/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "terraform-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "terraform-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
