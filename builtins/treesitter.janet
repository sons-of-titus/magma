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

# Language-specific tree-sitter highlighting queries.
(def *ts-mode-queries*
  {"rust-mode"  "(match
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
   "janet-mode" "(match
                    (symbol) @function
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (keyword) @keyword)"
   "python-mode" "(match
                    (function_definition name: (identifier) @function)
                    (class_definition name: (identifier) @type)
                    (call function: (identifier) @function)
                    (string) @string
                    (comment) @comment
                    (integer) @constant
                    (float) @constant
                    (boolean) @constant)"
   "javascript-mode" "(match
                    (function_declaration name: (identifier) @function)
                    (arrow_function) @function
                    (method_definition name: (property_identifier) @function)
                    (class_declaration name: (identifier) @type)
                    (call_expression function: (identifier) @function)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (this) @variable.builtin
                    (identifier) @variable)"
   "typescript-mode" "(match
                    (function_declaration name: (identifier) @function)
                    (method_definition name: (property_identifier) @function)
                    (class_declaration name: (type_identifier) @type)
                    (interface_declaration name: (type_identifier) @type)
                    (type_alias_declaration name: (type_identifier) @type)
                    (call_expression function: (identifier) @function)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (identifier) @variable)"
   "go-mode"    "(match
                    (function_declaration name: (identifier) @function)
                    (method_declaration name: (field_identifier) @function)
                    (type_declaration (type_spec name: (type_identifier) @type))
                    (call_expression function: (identifier) @function)
                    (string_literal) @string
                    (comment) @comment
                    (int_literal) @constant
                    (float_literal) @constant
                    (true) @constant (false) @constant
                    (identifier) @variable)"
   "c-mode"     "(match
                    (function_definition name: (identifier) @function)
                    (call_expression function: (identifier) @function)
                    (string_literal) @string
                    (comment) @comment
                    (number_literal) @constant
                    (identifier) @variable)"
   "cpp-mode"   "(match
                    (function_definition name: (identifier) @function)
                    (call_expression function: (identifier) @function)
                    (type_identifier) @type
                    (string_literal) @string
                    (comment) @comment
                    (number_literal) @constant
                    (identifier) @variable)"
   "ruby-mode"  "(match
                    (method name: (identifier) @function)
                    (singleton_method name: (identifier) @function)
                    (call method: (identifier) @function)
                    (class name: (constant) @type)
                    (module name: (constant) @module)
                    (string) @string
                    (comment) @comment
                    (integer) @constant
                    (float) @constant
                    (true) @constant (false) @constant (nil) @constant
                    (identifier) @variable)"
   "sh-mode"    "(match
                    (function_definition name: (word) @function)
                    (command name: (command_name) @function)
                    (string) @string
                    (comment) @comment
                    (variable_expansion) @variable)"
   "lua-mode"   "(match
                    (function_declaration name: (identifier) @function)
                    (function_call name: (identifier) @function)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (true) @constant (false) @constant (nil) @constant)"
   "java-mode"  "(match
                    (method_declaration name: (identifier) @function)
                    (class_declaration name: (identifier) @type)
                    (interface_declaration name: (identifier) @type)
                    (method_invocation name: (identifier) @function)
                    (string_literal) @string
                    (line_comment) @comment
                    (decimal_integer_literal) @constant
                    (true) @constant (false) @constant (null_literal) @constant)"
   "kotlin-mode" "(match
                    (function_declaration name: (simple_identifier) @function)
                    (class_declaration name: (simple_identifier) @type)
                    (call_expression function: (simple_identifier) @function)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (true) @constant (false) @constant (null) @constant)"
   "swift-mode" "(match
                    (function_declaration name: (identifier) @function)
                    (class_declaration name: (identifier) @type)
                    (struct_declaration name: (identifier) @type)
                    (call_expression function: (identifier) @function)
                    (string_literal) @string
                    (comment) @comment
                    (integer_literal) @constant
                    (true) @constant (false) @constant (nil) @constant
                    (identifier) @variable)"
   "haskell-mode" "(match
                    (function name: (variable) @function)
                    (class name: (type) @type)
                    (constructor name: (constructor) @type)
                    (string) @string
                    (comment) @comment
                    (integer) @constant
                    (float) @constant)"
   "elixir-mode" "(match
                    (call target: (identifier) @function)
                    (def name: (identifier) @function)
                    (defmodule name: (alias) @type)
                    (defstruct name: (alias) @type)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (true) @constant (false) @constant (nil) @constant)"
   "erlang-mode" "(match
                    (function name: (atom) @function)
                    (module_attribute) @module
                    (string) @string
                    (comment) @comment
                    (integer) @constant
                    (atom) @constant)"
   "nim-mode"   "(match
                    (proc_declaration name: (identifier) @function)
                    (func_declaration name: (identifier) @function)
                    (call_expression name: (identifier) @function)
                    (type_declaration name: (identifier) @type)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (true) @constant (false) @constant (nil) @constant
                    (identifier) @variable)"
   "zig-mode"   "(match
                    (function_declaration name: (identifier) @function)
                    (call_expression function: (identifier) @function)
                    (struct_declaration name: (identifier) @type)
                    (enum_declaration name: (identifier) @type)
                    (string) @string
                    (line_comment) @comment
                    (number) @constant
                    (true) @constant (false) @constant)
                    (identifier) @variable)"
   "clojure-mode" "(match
                    (list_lit (sym) @function)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (keyword) @keyword
                    (true) @constant (false) @constant (nil) @constant)"
   "json-mode"  "(match
                    (string) @string
                    (number) @constant
                    (true) @constant (false) @constant (null) @constant
                    (pair key: (string) @variable))"
   "toml-mode"  "(match
                    (table_name) @module
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (boolean) @constant)"
   "yaml-mode"  "(match
                    (scalar) @string
                    (comment) @comment
                    (boolean) @constant
                    (integer) @constant
                    (float) @constant
                    (null) @constant)"
   "html-mode"  "(match
                    (tag_name) @type
                    (attribute_name) @variable
                    (string) @string
                    (comment) @comment)"
   "css-mode"   "(match
                    (class_selector (class_name) @function)
                    (id_selector (id_name) @function)
                    (tag_name) @type
                    (string_value) @string
                    (integer_value) @constant
                    (float_value) @constant
                    (comment) @comment)"
   "nix-mode"   "(match
                    (identifier) @function
                    (string) @string
                    (comment) @comment
                    (integer) @constant
                    (true) @constant (false) @constant (null) @constant)"
   "terraform-mode" "(match
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (true) @constant (false) @constant (null) @constant)"
   "sql-mode"   "(match
                    (identifier) @function
                    (string) @string
                    (comment) @comment
                    (number) @constant)"
   "emacs-lisp-mode" "(match
                    (function name: (symbol) @function)
                    (defun name: (symbol) @function)
                    (defmacro name: (symbol) @function)
                    (string) @string
                    (comment) @comment
                    (number) @constant
                    (keyword) @constant
                    (t) @constant (nil) @constant)"
   "lisp-mode"  "(match
                    (defun name: (symbol) @function)
                    (defmacro name: (symbol) @function)
                    (string) @string
                    (comment) @comment
                    (number) @constant)"
   "scheme-mode" "(match
                    (define (function_name: (symbol) @function))
                    (string) @string
                    (comment) @comment
                    (number) @constant)"
   "fish-mode"  "(match
                    (function_definition name: (word) @function)
                    (command name: (command_name) @function)
                    (string) @string
                    (comment) @comment
                    (variable_expansion) @variable)"
   "org-mode"   "(match
                    (headline (stars) @function)
                    (keyword) @keyword
                    (string) @string
                    (comment) @comment)"})

# Auto-highlight a buffer using tree-sitter.
# (ts/highlight-buffer buf-id) — runs the language-specific query and sets
# the "syntax" highlight layer.
(defn ts/highlight-buffer
  [buf-id]
  (def lang (try (buffer/major-mode buf-id) ([_] nil)))
  (when (nil? lang) (break))
  (ts/ensure-language buf-id lang)
  (def query (get *ts-mode-queries* lang))
  (when (nil? query) (break))
  (def result (try
    (ts/query buf-id query)
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
