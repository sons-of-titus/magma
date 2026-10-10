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

(defn- severity-from-tag [tag]
  (cond
    (string/find "E" tag) 1
    (string/find "W" tag) 2
    3))

# Diagnostics arrive as one string per diagnostic, formatted by the core:
# "[E] line 12:4 message…".  Returns {:line n :sev n :msg "…"} or nil.
(defn- parse-diag-item [item]
  (def parts (string/split " " item))
  (when (>= (length parts) 4)
    (def tag (parts 0))
    (def pos (scan-number (first (string/split ":" (parts 2)))))
    (when pos
      {:line (- pos 1)  # the core prints 1-based lines
       :sev (severity-from-tag tag)
       :msg (string/join (slice parts 3) " ")})))

(defn- apply-diag-eol [buf items]
  (buffer/decor-clear-layer buf "lsp-diag-eol")
  # Group by line, keep highest severity (lowest number = highest severity)
  (var line-best @{})
  (each item (string/split "\n" items)
    (when-let [d (parse-diag-item item)]
      (def line (get d :line))
      (def cur (get line-best line))
      (when (or (nil? cur) (< (get d :sev 99) (get cur :sev 99)))
        (put line-best line d))))
  (eachp [line entry] line-best
    (let [face (severity-face (get entry :sev 3))
          text (string " " (get entry :msg ""))]
      (buffer/decor-set-eol buf "lsp-diag-eol" line text face))))

(event/on "lsp-diagnostics"
  (fn [data]
    (let [buf   (scan-number (get data :buffer-id))
          items (get data :items "")]
      (when (and buf (diag-eol-enabled? buf) (not (empty? items)))
        (apply-diag-eol buf items)))))

# Colon verb: :lsp-diag-eol on|off