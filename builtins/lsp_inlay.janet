# Sprint 11b — LSP Inlay Hints
# Subscribes to `lsp-response` for textDocument/inlayHint; maps response items
# to `buffer/decor-set-inline` in the "lsp-inlay" layer.

(def- *inlay-enabled* @{})   # buf-id → bool

(defn- inlay-enabled? [buf]
  (get *inlay-enabled* buf true))

(defn lsp-inlay/enable [buf]
  "Enable LSP inlay hints for a buffer."
  (put *inlay-enabled* buf true))

(defn lsp-inlay/disable [buf]
  "Disable LSP inlay hints and clear existing decorations for a buffer."
  (put *inlay-enabled* buf false)
  (buffer/decor-clear-layer buf "lsp-inlay"))

(defn- apply-inlay-hints [buf hints]
  (buffer/decor-clear-layer buf "lsp-inlay")
  (each hint hints
    (let [pos  (get hint :position {})
          line (get pos :line 0)
          col  (get pos :character 0)
          lbl  (get hint :label "")]
      (buffer/decor-set-inline buf "lsp-inlay" line col lbl "type-face"))))

(event/on "lsp-response"
  (fn [data]
    (let [method (get data :method "")
          buf    (buffer/current)]
      (when (and (= method "textDocument/inlayHint")
                 (not (nil? buf))
                 (inlay-enabled? buf))
        (let [result (get data :result [])]
          (when (indexed? result)
            (apply-inlay-hints buf result)))))))

# Clear inlay hints before each save to avoid stale positions.
(event/on "buffer-before-save"
  (fn [data]
    (let [buf (get data :buffer-id nil)]
      (when (not (nil? buf))
        (buffer/decor-clear-layer buf "lsp-inlay")))))

# Colon verb: :lsp-inlay on|off
(when (and (table? *colon-plugins*) (not (nil? *colon-plugins*)))
  (put *colon-plugins* "lsp-inlay"
    (fn [args]
      (let [sub (get args 0 "")
            buf (buffer/current)]
        (cond
          (= sub "on")  (lsp-inlay/enable buf)
          (= sub "off") (lsp-inlay/disable buf)
          (editor/log-message "Usage: :lsp-inlay on|off"))))))
