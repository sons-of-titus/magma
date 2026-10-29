# LSP diagnostic EOL decorations.
#
# The kernel command `diagnostics-apply-eol` selects the highest-severity
# diagnostic per line from the typed diagnostics it already stores; this
# file owns the per-buffer toggle and the event subscription.

(def- *diag-eol-enabled* @{})  # buf-id → bool

(defn- diag-eol-enabled? [buf]
  (get *diag-eol-enabled* buf true))

(defn lsp-diag-eol/enable [buf]
  "Enable LSP diagnostic EOL annotations for a buffer."
  (put *diag-eol-enabled* buf true))

(defn lsp-diag-eol/disable [buf]
  "Disable LSP diagnostic EOL annotations and clear existing decorations."
  (put *diag-eol-enabled* buf false)
  (buffer/decor-clear-layer buf "lsp-diag-eol"))

(event/on "lsp-diagnostics"
  (fn [data]
    (when-let [buf (scan-number (string (get data :buffer-id)))]
      (when (diag-eol-enabled? buf)
        (editor/run-command "diagnostics-apply-eol" buf)))))
