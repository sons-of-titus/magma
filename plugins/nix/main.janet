# Nix plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "nix" "treesitter")
(semantic/register-provider "nix" "lsp")

# ── LSP ──────────────────────────────────────────────────────────────────────

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (when (or (marker "flake.nix"))
      (lsp/start "nix" (option/get "nix-lsp-command")))))

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .nix buffers by the core extension map on
# buffer-created.

(major-mode/define "nix-mode"
  {:parent "prog"
   :layers [:nix-mode]
   :setup (fn []
            (def width (option/get "nix-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(defn nix/format
  "Format source text with nixpkgs-fmt; returns the input unchanged on failure."
  [text]
  (def result (process/run "nixpkgs-fmt" @[] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))

(defn- format-current-buffer []
  (def buf (buffer/current))
  (when (and buf (= (buffer/major-mode buf) "nix-mode"))
    (def formatted (nix/format (buffer/slice buf 0 (buffer/len buf))))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 formatted)))

(command/define "nix-format-buffer" (fn [&] (format-current-buffer)))

(event/on "buffer-before-save"
  (fn [_]
    (when (= (option/get "nix-format-on-save") "true")
      # Direct call: entering the command registry from a handler would
      # create a nested fiber, which Janet silently drops.
      (format-current-buffer))))
