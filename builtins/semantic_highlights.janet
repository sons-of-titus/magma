# Semantic highlighting presentation — dead code and deprecations.
#
# The mechanism lives in the kernel: an LSP semanticTokens response is
# decoded into (start end scope) ranges in the "semantic" highlight layer,
# where scopes without a registered presentation are dropped.  This file
# owns that presentation: which scopes map to which faces.
#
# Two sources mark dead code:
#   - semantic-token modifiers (rust-analyzer's `unreferenced`), and
#   - hint-severity diagnostics whose message says the code is unused
#     (servers without semanticTokens support).
# Both paint the same faces, so a server upgrading from one to the other
# changes nothing the user sees.

# ── Token types → faces ──────────────────────────────────────────────────────
#
# The standard LSP token types.  Registered onto the existing theme faces so
# a semantic layer agrees with the tree-sitter syntax layer underneath.

(each [scope face]
      [["comment" "comment-face"]
       ["constant" "constant-face"]
       ["function" "function-face"]
       ["keyword" "keyword-face"]
       ["method" "function-face"]
       ["namespace" "type-face"]
       ["operator" "operator-face"]
       ["parameter" "variable-face"]
       ["property" "variable-face"]
       ["string" "string-face"]
       ["type" "type-face"]
       ["variable" "variable-face"]]
  (face/scope-face scope face))

# ── Modifiers → faces ────────────────────────────────────────────────────────
#
# Modifier ranges arrive after their token's type range in the layer, so
# these win on the cells they cover: dead code greys out, deprecations
# strike through.

(face/scope-face "unreferenced" "unused-face")
(face/scope-face "deprecated" "deprecated-face")

# ── Hint-diagnostics fallback ────────────────────────────────────────────────

# Messages that mean "this code is dead" — matched against the server's
# own wording.  Kept deliberately narrow: hint diagnostics also carry
# style suggestions and doc prompts, and dimming those would be wrong.
(def- *dead-code-phrases*
  ["unused" "never used" "not accessed" "unreferenced" "never read"])

(defn- dead-code-message? [message]
  (def lower (string/ascii-lower message))
  (some |(string/find $ lower) *dead-code-phrases*))

# "[H] line 3:5 …" — the one display rendering diagnostics carry.
(defn- parse-hint-position
  [display]
  (when (string/has-prefix? "[H] " display)
    (when-let [line-marker (string/find "line " display)]
      (def rest (string/slice display (+ line-marker 5)))
      (when-let [colon (string/find ":" rest)]
        (var i (inc colon))
        (while (and (< i (length rest))
                    (let [b (rest i)] (and (>= b 48) (<= b 57))))
          (++ i))
        (when-let [line (scan-number (string/slice rest 0 colon))]
          (when-let [col (scan-number (string/slice rest (inc colon) i))]
            # The display rendering is 1-based in both coordinates.
            [(dec line) (dec col)]))))))

(defn- ident-byte? [b]
  (or (and (>= b 97) (<= b 122))
      (and (>= b 65) (<= b 90))
      (and (>= b 48) (<= b 57))
      (= b 95)))

# The identifier starting at (line, col), as a byte range over the buffer.
(defn- word-range-at
  [buf-id line col]
  (when-let [line-start (buffer/line-start-offset buf-id line)]
    (def line-end (or (buffer/line-start-offset buf-id (inc line))
                      (buffer/len buf-id)))
    (def text (buffer/slice buf-id line-start line-end))
    (var end col)
    (while (and (< end (length text)) (ident-byte? (text end)))
      (++ end))
    (when (> end col)
      [(+ line-start col) (+ line-start end)])))

# Recompute the dead-code layer from the buffer's current diagnostics.
(defn dead-code/refresh
  [buf-id]
  (def ranges @[])
  (each display (buffer/diagnostics buf-id)
    (when-let [[line col] (parse-hint-position display)]
      (when (dead-code-message? display)
        (when-let [range (word-range-at buf-id line col)]
          (array/push ranges [(first range) (last range) "unused"])))))
  (buffer/set-highlights-layer buf-id "dead-code" ranges))

(event/on "lsp-diagnostics"
  (fn [data]
    (when-let [buf-id (scan-number (string (get data :buffer-id)))]
      (dead-code/refresh buf-id))))

(print "semantic-highlights loaded")
