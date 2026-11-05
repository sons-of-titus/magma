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

# Shared-library file?  Mirrors the plugin system's own test: os/stat
# returns nil for absent paths and follows symlinks to their target.
(defn- ts-grammar-file?
  [path]
  (def m (get (try (os/stat path) ([_] nil)) :mode :missing))
  (or (= :file m) (= :link m)))

# Conventional grammar pickup: a plugin ships its grammar for the running
# platform at <plugin-dir>/grammars/tree-sitter-<lang>.<ext> — .dylib on
# macOS, .dll on Windows, .so elsewhere.  Returns the absolute path, or
# nil when the plugin has none for this platform (fallback highlighting
# applies).
(defn ts/grammar-lib-path
  [root lang]
  (def path
    (string root "/grammars/tree-sitter-" lang
            "." (case (os/which)
                  :macos "dylib"
                  :windows "dll"
                  :mingw "dll"
                  "so")))
  (when (ts-grammar-file? path) path))

# Resolve a mode (or language id) to a grammar loaded in the registry,
# loading it from *ts-grammars* on first use.  Returns nil when no grammar
# backs the name.  A query without a grammar — a plugin that ships
# syntax.scm but no loadable shared library — must leave the native fallback
# highlighter in charge, or the mode gets neither highlighter.
(defn- ts-resolve-language [name]
  (def lang-id
    (if (string/has-suffix? "-mode" name)
      (string/slice name 0 (- (length name) 5))
      name))
  (cond
    (ts/has-language lang-id) lang-id
    (ts/has-language name) name
    (let [path (or (get *ts-grammars* lang-id) (get *ts-grammars* name))]
      (when path
        (try (ts/load-grammar path lang-id) ([e] nil))
        (when (ts/has-language lang-id) lang-id)))))

# Register `query` as the tree-sitter highlight query for a major mode,
# replacing any earlier registration.  The syntax layer is handed to the
# tree-sitter lane only when a grammar backs the mode *and* it accepts the
# query; otherwise the native fallback highlighter keeps (or takes back) the
# layer.  A query that compiles nowhere would own the layer and paint
# nothing, leaving the file with no colors at all.
#
# Call ts/register-grammar (or language/define with :grammar) before this.
(defn ts/set-mode-query
  [mode query]
  # Store as string: file/read yields a buffer, and ts/highlight reads the
  # query through a string-only C boundary — a stored buffer would make
  # every highlight run silently return nil.
  (def src (string query))
  (put *ts-mode-queries* mode src)
  (def lang (ts-resolve-language mode))
  (if (and lang (try (ts/validate-query lang src) ([e] false)))
    (syntax/set-ts-mode mode true)
    (syntax/set-ts-mode mode false)))

# Register a major mode's highlight query from a query file on disk
# (conventionally the plugin's syntax.scm).  A missing or unreadable file
# is ignored — the mode simply gets no tree-sitter highlighting.
(defn ts/load-mode-query
  [mode path]
  # file/open parses its mode keyword per-byte as open flags: :read would
  # flag-error on the 'e'.  :r is the read mode.
  (def f (try (file/open path :r) ([_] nil)))
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
    # `lang` arrives as the major-mode name; plugin grammars load under
    # the language id (language/define) and manual registrations may use
    # either key.  The conventional `<id>-mode` naming bridges the two:
    # the language id is the key providers and the semantic engine use,
    # so it wins.  Only tag the buffer with a language that really has a
    # grammar — a bogus entry silently empties the symbol index.
    (def lang-id
      (if (string/has-suffix? "-mode" lang)
        (string/slice lang 0 (- (length lang) 5))
        lang))
    (def registry-name
      (cond
        (ts/has-language lang-id) lang-id
        (ts/has-language lang) lang
        (let [path (or (get *ts-grammars* lang-id) (get *ts-grammars* lang))]
          (when path
            (try
              (ts/load-grammar path lang-id)
              ([e] (print "ts: failed to load grammar: " e)))
            (when (ts/has-language lang-id) lang-id)))
        nil))
    (when registry-name
      (ts/set-language buf-id registry-name)
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
  (unless result
    # `ts/highlight` returns nil (not an error) when there is no usable
    # tree — no grammar, no language, or a query that does not compile
    # against it.  The fallback highlighter stands down for this mode, so
    # ranges left from the last successful run would only drift through
    # edit transforms: clear them, same as the error path.
    (buffer/set-highlights-layer buf-id "syntax" @[])
    (break))
  (def ranges @[])
  (def text (buffer/slice buf-id 0 (buffer/len buf-id)))
  # The grammar parses None/Some/Ok as plain identifiers — promote them to
  # the constant scope so they read as literals, not variables.
  (each r result
    (var [s e scope] r)
    (when (and (= scope "variable")
               (< s (length text))
               (<= e (length text))
               (or (= (string/slice text s e) "None")
                   (= (string/slice text s e) "Some")
                   (= (string/slice text s e) "Ok")))
      (set scope "constant"))
    (array/push ranges [s e scope]))
  (buffer/set-highlights-layer buf-id "syntax" ranges))

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
    (when-let [raw (get data :buffer-id)]
      (when-let [buf-id (scan-number raw)]
        (ts/highlight-buffer buf-id)))))

(print "tree-sitter loaded")
