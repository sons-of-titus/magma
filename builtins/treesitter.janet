# Tree-sitter integration for Magma
#
# Core owns the mechanism: grammar loading, the per-mode query registry,
# and buffer highlighting.  Language knowledge — the queries themselves —
# belongs to plugins: each plugin's syntax.scm is registered as its major
# mode's highlight query by plugin/define (see builtins/plugin_system.janet).

# Registered tree-sitter languages: lang-name → grammar-path
(def *ts-grammars* @{})

# Per-mode tree-sitter highlight queries: mode-name → query source.
(def *ts-mode-queries* @{})

# Register a tree-sitter grammar for a language.
# (ts/register-grammar lang-name grammar-path)
(defn ts/register-grammar
  [lang path]
  (put *ts-grammars* lang path))

# Register `query` as the tree-sitter highlight query for a major mode,
# replacing any earlier registration.  The mode's syntax layer belongs to
# the tree-sitter lane from here on: the native fallback highlighter
# stands down for it and stale fallback ranges are cleared.
(defn ts/set-mode-query
  [mode query]
  (put *ts-mode-queries* mode query)
  (syntax/set-ts-mode mode true))

# Register a major mode's highlight query from a query file on disk
# (conventionally the plugin's syntax.scm).  A missing or unreadable file
# is ignored — the mode simply gets no tree-sitter highlighting.
(defn ts/load-mode-query
  [mode path]
  (def f (try (file/open path :read) ([_] nil)))
  (when f
    (def query (file/read f :all))
    (file/close f)
    (when (and query (> (length query) 0))
      (ts/set-mode-query mode query))))

# Ensure a buffer has a tree-sitter language set.
# (ts/ensure-language buf-id lang)
(defn ts/ensure-language
  [buf-id lang]
  (def cur (try (ts/has-tree? buf-id) ([e] false)))
  (unless cur
    (def grammar-path (get *ts-grammars* lang))
    # Only tag the buffer with a language that has a registered grammar —
    # the semantic engine keys symbol indexing off this language, so a
    # bogus entry (e.g. "fundamental") silently empties the symbol index.
    (when grammar-path
      (try
        (ts/load-grammar grammar-path lang)
        ([e] (print "ts: failed to load grammar: " e)))
      (ts/set-language buf-id lang)
      (ts/parse buf-id))))

# Auto-highlight a buffer using tree-sitter.
# (ts/highlight-buffer buf-id) — runs the mode's query and sets the
# "syntax" highlight layer; no-ops when the mode has no query.
(defn ts/highlight-buffer
  [buf-id]
  (def lang (try (buffer/major-mode buf-id) ([_] nil)))
  (def query (get *ts-mode-queries* lang))
  (when (nil? query) (break))
  (ts/ensure-language buf-id lang)
  (def result (try
    (ts/highlight buf-id query)
    ([e]
      # A failed query must not leave stale ranges behind: the layer is
      # owned here, so an unusable tree means an explicitly empty layer.
      (buffer/set-highlights-layer buf-id "syntax" @[])
      (break))))
  (when result
    (def ranges @[])
    (each r result
      (array/push ranges [(r 0) (r 1) (r 2)]))
    (buffer/set-highlights-layer buf-id "syntax" ranges)))

# ── Scope → face mapping ─────────────────────────────────────────────────────
#
# Query capture names become highlight scopes; these registrations resolve
# the conventional capture names onto the theme's faces so plugin queries
# color without per-plugin wiring.  face/scope-face matches by prefix,
# longest first — dotted captures like `variable.builtin` resolve through
# their parent.

(each [scope face]
      [["comment" "comment-face"]
       ["constant" "constant-face"]
       ["error" "error-face"]
       ["function" "function-face"]
       ["method" "function-face"]
       ["keyword" "keyword-face"]
       ["module" "type-face"]
       ["operator" "operator-face"]
       ["property" "variable-face"]
       ["punctuation" "punctuation-face"]
       ["string" "string-face"]
       ["type" "type-face"]
       ["variable" "variable-face"]]
  (face/scope-face scope face))

# Event handlers.
#
# buffer-changed must NOT re-highlight synchronously: ts/highlight runs a
# full-tree query, so per keystroke that is O(document) on the interactive
# path — typing degraded and tab blocked completely on large files.  The
# Rust lane parses off-thread, coalesced per buffer, and emits
# ts-tree-updated when a fresh tree lands; highlighting follows that.
(event/on "buffer-changed"
  (fn [_] nil))

# Highlighting follows the off-thread parse lane: when the Rust side lands
# a fresh tree for a buffer, re-run the query against it.
(event/on "ts-tree-updated"
  (fn [data]
    (when-let [raw (get data "buffer-id")]
      (when-let [buf-id (scan-number raw)]
        (ts/highlight-buffer buf-id)))))

(print "tree-sitter loaded")
