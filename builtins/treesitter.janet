# Tree-sitter integration for Magma
# This script provides the Janet-side glue for tree-sitter parsing and
# highlighting.  Language-specific grammar loading and queries are defined
# in Janet extensions (one per language).

# Registered tree-sitter languages: lang-name → grammar-path
(def *ts-grammars* @{})

# Register a tree-sitter grammar for a language.
# (ts/register-grammar lang-name grammar-path)
(defn ts/register-grammar
  [lang path]
  (put *ts-grammars* lang path))

# Ensure a buffer has a tree-sitter language set.
# (ts/ensure-language buf-id lang)
(defn ts/ensure-language
  [buf-id lang]
  (def cur (try (ts/has-tree? buf-id) ([e] false)))
  (unless cur
    (def grammar-path (get *ts-grammars* lang))
    (when grammar-path
      (try
        (ts/load-grammar grammar-path lang)
        ([e] (print "ts: failed to load grammar: " e))))
    (ts/set-language buf-id lang)
    (ts/parse buf-id)))

# Auto-highlight a buffer using tree-sitter.
# (ts/highlight-buffer buf-id) — runs the language-specific query and sets
# the "syntax" highlight layer.
(defn ts/highlight-buffer
  [buf-id]
  (def lang (try (buffer/major-mode buf-id) ([_] nil)))
  (when (nil? lang) (break))
  (ts/ensure-language buf-id lang)
  (def result (try
    (ts/query buf-id
      (case lang
        "rust-mode"       "(match
                              (call_expression) @function
                              (function_item name: (identifier) @function)
                              (generic_function name: (identifier) @function)
                              (type_identifier) @type
                              (primitive_type) @type.builtin
                              (string_literal) @string
                              (line_comment) @comment
                              (int_literal) @constant
                              (float_literal) @constant
                              (boolean_literal) @constant
                              (self) @variable.builtin
                              (let_declaration pattern: (identifier) @variable)
                              (parameter (identifier) @variable))"
        "janet-mode"      "(match
                              (symbol) @function
                              (string) @string
                              (comment) @comment
                              (number) @constant
                              (keyword) @keyword)"
        "python-mode"     "(match
                              (function_definition name: (identifier) @function)
                              (class_definition name: (identifier) @type)
                              (call function: (identifier) @function)
                              (string) @string
                              (comment) @comment
                              (integer) @constant
                              (float) @constant
                              (boolean) @constant)"
        nil))
    ([e] (break))))
  (when result
    (def ranges @[])
    (each r result
      (array/push ranges [(r 0) (r 1) (r 2)]))
    (buffer/set-highlights-layer buf-id "syntax" ranges)))

# Event handler — auto-parse on buffer change (debounced via the Janet
# event bus).
(event/on "buffer-changed"
  (fn [data]
    (def raw (get data :buffer-id))
    (when raw
      (def buf-id (scan-number raw))
      (when buf-id
        (ts/highlight-buffer buf-id)))))

(print "tree-sitter loaded")
