# Sprint 11b — LSP Diagnostic EOL Decorations
# Subscribes to `lsp-diagnostics` events; for each line with at least one
# diagnostic, sets one EndOfLine decoration showing the highest-severity message.

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

(defn- severity-face [sev]
  (cond
    (<= sev 1) "error-face"
    (= sev 2)  "warning-face"
    "comment-face"))

(defn- apply-diag-eol [buf diags]
  (buffer/decor-clear-layer buf "lsp-diag-eol")
  # Group by line, keep highest severity (lowest number = highest severity)
  (var line-best @{})  # line → {:msg "..." :sev n}
  (each d diags
    (let [rng  (get d :range {})
          pos  (get rng :start {})
          line (get pos :line 0)
          sev  (get d :severity 3)
          msg  (get d :message "")]
      (let [cur (get line-best line nil)]
        (when (or (nil? cur) (< sev (get cur :sev 99)))
          (put line-best line {:msg msg :sev sev})))))
  (eachp [line entry] line-best
    (let [face (severity-face (get entry :sev 3))
          text (string " " (get entry :msg ""))]
      (buffer/decor-set-eol buf "lsp-diag-eol" line text face))))

(event/on "lsp-diagnostics"
  (fn [data]
    (let [buf   (get data :buffer-id nil)
          diags (get data :diagnostics [])]
      (when (and buf (diag-eol-enabled? buf) (indexed? diags))
        (apply-diag-eol buf diags)))))

# Colon verb: :lsp-diag-eol on|off
(when (and (table? *colon-plugins*) (not (nil? *colon-plugins*)))
  (put *colon-plugins* "lsp-diag-eol"
    (fn [args]
      (let [sub (get args 0 "")
            buf (buffer/current)]
        (cond
          (= sub "on")  (lsp-diag-eol/enable buf)
          (= sub "off") (lsp-diag-eol/disable buf)
          (editor/log-message "Usage: :lsp-diag-eol on|off"))))))
