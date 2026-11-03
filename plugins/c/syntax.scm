; Tree-sitter highlight query for C buffers.
;
; Registered automatically by plugin/define as c-mode's highlight query;
; capture names resolve to faces through the scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "auto" "break" "case" "const" "continue" "default" "do"
  "else" "enum" "extern" "for" "goto" "if" "inline"
  "register" "restrict" "return" "sizeof" "static" "struct"
  "switch" "typedef" "union" "unsigned" "signed" "volatile" "while"
] @keyword

; ── Types ─────────────────────────────────────────────────────────────────

(type_identifier) @type
(primitive_type) @type

; ── Functions: definitions, calls, macro definitions ──────────────────────

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function))
(call_expression
  function: (identifier) @function)
(preproc_function_def
  name: (identifier) @function)

; ── Preprocessor ──────────────────────────────────────────────────────────

(preproc_def name: (identifier) @constant)
(preproc_include) @keyword
["#define" "#ifdef" "#ifndef" "#if" "#else" "#elif" "#endif" "#pragma"] @keyword

; ── Constants ─────────────────────────────────────────────────────────────

(enumerator name: (identifier) @constant)
(number_literal) @constant
(char_literal) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_literal) @string
(system_lib_string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
